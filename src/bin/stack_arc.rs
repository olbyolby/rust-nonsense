// A some-what over engineered (The whole CollatzCycle::Stand thing has no reason to exist) demonstration of a "StackArc",
// A stack allocated, safe, reference counted, data type.
use std::{collections::HashMap, ops::Deref, pin::Pin, ptr::NonNull, sync::{Mutex, atomic::{AtomicUsize, Ordering}}, thread::Thread};

use indexmap::IndexSet;


fn main() {
    let data: StackArcInner<Mutex<HashMap<u128, Option<CollatzCycle>>>> = StackArcInner::new(Mutex::new(HashMap::new()));

    let data = std::pin::pin!(data);
    let data = data.as_ref().acquire();

    let threads = 16;
    let block_size = 1024;
    let block_count = 1024*1024;

    let remainder = block_count % threads;
    if remainder != 0 {
        println!("Invalid configuration(block count should be a multiple of thread amount to properly partition.");
        println!("Add or subtract {} or {} blocks, respectively", threads - remainder, remainder);
    }
    for offset in 0..threads as usize {
        let data = data.clone();
        std::thread::spawn(move || CollatzCycle::thread(block_size, offset as u128, block_count/threads, threads, data.clone()));
    }

    println!("Executing...");
    loop {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
    
        match line.trim() {
            "cancel" => break,
            "show" => {
                let data = data.lock().unwrap();
                for (number, conjecture) in data.iter() {
                    print!("{}: ", number);
                    match conjecture {
                        None => print!("May not terminate(overflowed)"),
                        Some(CollatzCycle::Standard) => {
                            print!("Standard; 1+2+3")
                        },
                        Some(CollatzCycle::Unique(conjecture)) => {
                            print!("A new cycle!! ; ");

                            let mut hailstones = conjecture.iter();
                            print!("{}", hailstones.next().unwrap());
                            for hailstone in conjecture {
                                print!("+{}", hailstone)
                            }
                        }
                    }
                    println!("");
                }
            },
            _ => continue
        }

        
    }


    println!("Aborting.\nWaiting for threads.");
}


impl CollatzCycle {
    fn calculate(number: u128) -> Option<CollatzCycle> {
        let mut seen = IndexSet::new();

        let mut hailstone = number;
        while !seen.contains(&hailstone) {
            seen.insert(hailstone);
            hailstone = match hailstone % 2 {
                0 => hailstone.checked_div(2)?, // hailstone / 2
                _ => hailstone.checked_mul(3)?.checked_add(1)? // 3*hailstone + 1
            }
        }

        // Find start of loop
        let start = seen.get_index_of(&hailstone).unwrap();

        // Save space for the expected result
        let cycle = &seen[start..];
        if cycle.len() == 3 && cycle.iter().all(|n| [1,2,3].contains(n)) {
            return Some(Self::Standard)
        };

        // Store the loop for a newly discovered loop
        let mut space = Box::new_uninit_slice(cycle.len());
        for index in 0..space.len() {
            space[index].write(cycle[index]);
        };
        
        // Safety: Safe as the above for loop will ALWAYS initalize at least space.len() values, or panic.
        unsafe {
            Some(CollatzCycle::Unique(space.assume_init()))
        }

    }

    fn thread(block_size: usize, block_start: u128, block_count: usize, count_by: usize, output: StackArc<Mutex<HashMap<u128, Option<Self>>>>) {
        for block in (block_start..block_start as u128 + block_count as u128).step_by(count_by) {
            for number in (block*block_size as u128)..(block+1)*block_size as u128 {
                let result = Self::calculate(number as u128);

                output.lock().unwrap().insert(number, result);         
            }
        }
    }
}

enum CollatzCycle {
    Standard, // 3-2-1 loop
    Unique(Box<[u128]>)
}
impl Deref for CollatzCycle {
    type Target = [u128];
    fn deref(&self) -> &[u128] {
        match self {
            Self::Standard => &[3,2,1],
            Self::Unique(data) => &*data
        }
    }
}





pub struct StackArcInner<T> {
    ref_count: AtomicUsize,
    creator: Thread,
    data: T,   
}
impl<T> StackArcInner<T> {
    pub fn new(data: T) -> StackArcInner<T> {
        Self {
            ref_count: AtomicUsize::new(0),
            creator: std::thread::current(),
            data
        }
    }

    // Create a new 
    pub fn acquire(self: Pin<&Self>) -> StackArc<T> {
        self.ref_count.fetch_add(1, Ordering::Acquire);

        StackArc {
            data: NonNull::from_ref(&*self)
        }
    }

}
impl<T> Drop for StackArcInner<T> {
    fn drop(&mut self) {
        // Wait for ref count to drop to zero
        while self.ref_count.load(Ordering::Acquire) != 0 {
            std::thread::park()
        }
    }
}

pub struct StackArc<T> {
    data: NonNull<StackArcInner<T>>
}
impl<T> StackArc<T> {
    fn get_inner(&self) -> &StackArcInner<T> {
        unsafe {
            self.data.as_ref()
        }
    }
}
impl<T> From<Pin<&StackArcInner<T>>> for StackArc<T> {
    fn from(value: Pin<&StackArcInner<T>>) -> Self {
        value.acquire()
    }
}

impl<T> Clone for StackArc<T> {
    fn clone(&self) -> Self {
        self.get_inner().ref_count.fetch_add(1, Ordering::Relaxed);
        
        StackArc {
            data: self.data
        }
    }
}
impl<T> Deref for StackArc<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.get_inner().data
    }
}
impl<T> Drop for StackArc<T> {
    fn drop(&mut self) {
        let previous = self.get_inner().ref_count.fetch_sub(1, Ordering::AcqRel);
        if previous == 1 {
            self.get_inner().creator.unpark();
        }
    }
}

// Safety: Arc<T> can only be accessed if T: Sync. T need not impl Send as destruction can only happen on the thread that made the StackArcInner<T>
unsafe impl<T: Sync> Send for StackArc<T> {}
// Safety: You can only access the StackArc if you can access it's data
unsafe impl<T: Sync> Sync for StackArc<T> {}