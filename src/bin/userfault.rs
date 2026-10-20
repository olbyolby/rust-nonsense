use std::{ffi::c_void, io::Read, ops::Deref, os::fd::AsRawFd, pin::Pin, sync::{Mutex, OnceLock}, thread::JoinHandle};

use thiserror::Error;
use userfaultfd::{Event, FaultKind, FeatureFlags, IoctlFlags, ReadWrite, Uffd};

struct MagicalLazyStdin {
    uffd: Uffd,
    page_size: usize,
    mappings: Mutex<Vec<Mapping>>,
    thread: OnceLock<JoinHandle<()>>
}

struct MagicalLazyStdinThread {
    this: *const MagicalLazyStdin
}
impl MagicalLazyStdinThread {
    fn execute(self) {
        let this = &*self;

        while let Ok(Some(Event::Pagefault { kind: FaultKind::Missing, rw, addr, .. })) = this.uffd.read_event() {
            let mut mappings = this.mappings.lock().unwrap();
            let (i, map) = match mappings.iter().enumerate().filter(|(_, map)| map.contains(addr.addr())).next() {
                None => continue,
                Some((i, map)) => (i, *map)
            };

            mappings.remove(i);

            // Make a big working space to copy to the faulted page
            let mut space = unsafe { 
                let mut space = Box::<[u8]>::new_uninit_slice(map.legnth.next_multiple_of(self.page_size));
                for place in space.iter_mut() {
                    place.write(b' ');
                }

                space.assume_init()
            };

            // Upon access to the missing page, the page becomes initialized, as if it always had been initialized.
            // Or at least that's what the Ministry of Truth tells me. 
            match rw {
                ReadWrite::Read => unsafe {
                    let end = read_line_or_end(&mut space);

                    // To safely make &str, we have to ensure they're UTF8 when we initalize them. That's what we do here.
                    if map.utf8 && let Err(_) = str::from_utf8(&mut space) {
                        // Invalid encoding ):
                        const MESSAGE: &'static str = "Invalid UTF8 input! ):";
                        for (i, place) in space[..end.max(MESSAGE.len())].iter_mut().enumerate() {
                            *place = *MESSAGE.as_bytes().get(i).unwrap_or(&b' ');
                        }
                    }

                    self.uffd.copy(space.as_ptr() as *const c_void, map.start as *mut c_void, map.legnth.next_multiple_of(self.page_size), true).unwrap();
                },
                ReadWrite::Write => unsafe {
                    self.uffd.copy(space.as_ptr() as *const c_void, map.start as *mut c_void, map.legnth.next_multiple_of(self.page_size), true).unwrap();
                }
            }
        }
    }
}
impl Deref for MagicalLazyStdinThread {
    type Target = MagicalLazyStdin;
    fn deref(&self) -> &Self::Target {
        // Safety: MagicalLazyStdinThread is ONLY constructed with a reference to a pinned MagicalLazyStdin,
        // and that pinned object is this responsible for joining the thread upon it's drop(). As such, this is always valid.
        unsafe {
            self.this.as_ref_unchecked()
        }
    }
}
unsafe impl Send for MagicalLazyStdinThread {}

#[derive(Debug, Error)]
enum MapError {
    #[error("IO Error acquiring memory: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Error registering with userfaultfd: {0}")]
    UserfaultFd(#[from] userfaultfd::Error)
}
impl MagicalLazyStdin {
    fn new() -> Result<Self, userfaultfd::Error> {
        Ok(MagicalLazyStdin {
            uffd: userfaultfd::UffdBuilder::new()
                .close_on_exec(true)
                .require_features(FeatureFlags::PAGEFAULT_FLAG_WP)
                .require_ioctls(IoctlFlags::REGISTER | IoctlFlags::API | IoctlFlags::UNREGISTER)
                .user_mode_only(false)
                .create()?,
            page_size: unsafe { libc::sysconf(libc::_SC_PAGE_SIZE).try_into().unwrap() },
            mappings: Mutex::new(Vec::new()),
            thread: OnceLock::new()
        })
    }
    fn start(self: Pin<&Self>) {
        // TODO: Implement canceling system.
        let container = MagicalLazyStdinThread { this: &*self };
        self.thread.set(std::thread::spawn(move || container.execute())).unwrap();
    }

    // Safety: `T` MUST have the same layout and validity as, depneding on the "utf8" argument, [u8] or str. 
    unsafe fn lazy_read<T>(&self, length: usize, utf8: bool) -> Result<&'static mut [T], MapError> {
        let data = unsafe {
            libc::mmap(std::ptr::null_mut(), length * size_of::<T>(),  libc::PROT_NONE, libc::MAP_ANONYMOUS | libc::MAP_PRIVATE, -1, 0)
        };
        if data == std::ptr::null_mut() {
            return Err(std::io::Error::last_os_error().into());
        };


        self.uffd.register(data, length.next_multiple_of(self.page_size))?;
        self.mappings.lock().unwrap().push(Mapping {
            start: data.addr(),
            legnth: length * size_of::<T>(),
            utf8
        });

        unsafe {
            // Safety: The extremely, absolutely initialized data, contained within &mut T does not *yet*.
            // However, it *will* exist upon being accessed(or it'll kill the process 'cause sigbus), and is, 
            // therefore, entirely safe, as Rust code will never even percieve this fact, since any access trigers
            // a pagefault that will prompty initalize the data, *as-if* it had always existed.
            // As for the life time, we don't ever free it outside poisoning, so this is just straight up a memory leak. Hence &'static
            Ok(std::slice::from_raw_parts_mut(data as *mut T, length))
        }
    }
    #[allow(dead_code)]
    fn lazy_bytes(&self, length: usize) -> Result<&'static mut [u8], MapError> {
        unsafe {
            self.lazy_read(length, false)
        }
    }
    fn lazy_text(&self, length: usize) -> Result<&'static mut str, MapError> {
        unsafe {
            self.lazy_read::<u8>(length, true).map(|text| std::mem::transmute(text))
        }
    }
}
fn read_line_or_end(data: &mut [u8]) -> usize {
    let mut stdin = std::io::stdin().lock();

    let mut amount = 0;
    for place in data.iter_mut() {
        stdin.read(std::slice::from_mut(place)).unwrap();
        amount += 1;

        if *place == b'\n' {
            break;
        }
    }

    return amount
} 

impl Drop for MagicalLazyStdin {
    fn drop(&mut self) {
        // Join the thread
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }

        // Posion all remaining uffd maps
        for mapping in self.mappings.get_mut().unwrap() {
            unsafe {
                let pos = userfaultfd_sys::uffdio_poison {
                    range: userfaultfd_sys::uffdio_range { start: mapping.start as u64, len: mapping.legnth.next_multiple_of(self.page_size) as u64},
                    mode: 0,
                    updated: 0
                };

                let result = libc::ioctl(self.uffd.as_raw_fd(), userfaultfd_sys::_UFFDIO_POISON, &raw const pos);
                if result == -1 {
                    eprintln!("Feck. Failed to posion memory range, can't safely proceed without creating references to uninitalized/invalid data.");
                    std::process::abort();
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Mapping {
    start: usize,
    legnth: usize,
    utf8: bool
}
impl Mapping {
    fn contains(&self, ptr: usize) -> bool {
        self.start <= ptr && ptr <= self.start + self.legnth
    }
}


fn main() {
    let stdin = MagicalLazyStdin::new().unwrap();
    let stdin = std::pin::pin!(stdin);
    let stdin = stdin.as_ref();

    stdin.start();

    let c: &str = stdin.lazy_text(128).unwrap();
    let a: &str = stdin.lazy_text(16).unwrap();
    let b: &str = stdin.lazy_text(16).unwrap();
    

    let x: u32 = b.trim().parse().unwrap();
    let y: u32 = a.trim().parse().unwrap();
    
    println!("{x} + {y} = {}", x+y);

    std::fs::write("./data.txt", c).unwrap();
    std::process::exit(0);
}
