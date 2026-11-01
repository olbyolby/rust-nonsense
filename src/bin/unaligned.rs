use std::{any::TypeId, io::Write, mem::MaybeUninit, ptr::NonNull, rc::Rc};




pub struct Arbitrary<const S: usize> {
    data: [MaybeUninit<u8>; S],
    dropper: unsafe fn(*mut ()),
    cloner: unsafe fn(&mut Self) -> Self,
    type_id: TypeId
}
impl<const S: usize> Arbitrary<S> {
    pub fn new<T: Clone + 'static>(data: T) -> Self {
        Self::size_check::<T>();


        let mut data_buffer = [MaybeUninit::uninit(); S];

        unsafe {
            (data_buffer.as_mut_ptr() as *mut T).write_unaligned(data);
        }
        

        Self {
            data: data_buffer,
            dropper: Self::dropper::<T>(),
            cloner: Self::cloner::<T>(),
            type_id: TypeId::of::<T>()
        }     
    }

    pub fn read<T: Clone + 'static>(&mut self) -> Option<T> {
        Self::size_check::<T>();

        match self.type_id == TypeId::of::<T>() {
            false => None,
            true => unsafe {
                // Move out of self.data to properly align T
                let data: T = self.get().read_unaligned();

                // Clone the data, catching panic to ensure data is moved back to self.data
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| data.clone()));

                // Move data back to self.data
                self.get().write_unaligned(data);

                // Resume panic or return the new value
                match result {
                    Ok(result) => Some(result),
                    Err(payload) => std::panic::resume_unwind(payload)
                }
                
            }
        }
    }

    // Write a value into the arbitary buffer of memory. Ensures buffer is of sufficent size.
    pub fn write<T: Clone + 'static>(&mut self, data: T) {
        Self::size_check::<T>();

        // Drop the old value
        unsafe {
            (self.dropper)(self.get().as_ptr() as *mut ())
        }

        self.type_id = TypeId::of::<T>();
        self.dropper = Self::dropper::<T>();
        self.cloner = Self::cloner::<T>();
        unsafe {
            self.get().write_unaligned(data);
        }
    }

   fn get<T>(&mut self) -> NonNull<T> {
        // Safety: A reference to a local variable can't be null by definition
        unsafe {
            NonNull::new_unchecked(&raw mut self.data as *mut T)
        }
    }

    const fn dropper<T>() -> unsafe fn(*mut ()) {
        let dropper: fn(*mut ()) = |data: *mut ()| {
            let data = unsafe {
                (data as *mut T).read_unaligned()
            };
            drop(data)
        };

        dropper
    }
    const fn cloner<T: Clone + 'static>() -> unsafe fn(&mut Self) -> Self {
        let cloner: fn(&mut Self) -> Self  = |data| {
            let data = data.read::<T>().unwrap();
            Self::new(data)    
        };

        cloner
    }

    const fn size_check<T>() {
        const {
            assert!(size_of::<T>() <= S, "T does not fit in S bytes");
        }
    }

    // Can't use the Clone trait 'cause we need &mut for cloner(), since it needs to call .get() and that moves out of self temporarily
    fn clone(&mut self) -> Self {
        unsafe {
            (self.cloner)(self)
        }
    }
}
impl<const S: usize> Drop for Arbitrary<S> {
    fn drop(&mut self) {
        unsafe {
            (self.dropper)(self.get().as_ptr() as *mut ())
        }
    }
}

#[derive(Clone, Debug)]
struct DropCheck(&'static str);
impl Drop for DropCheck {
    fn drop(&mut self) {
        println!("{}", self.0);
    }
}

fn read_line(question: &str) -> String {
    print!("{}: ", question);
    std::io::stdout().flush().unwrap();

    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer).unwrap();

    buffer
}

fn main() {
    
    let mut data: Arbitrary<32> = Arbitrary::new(Into::<Rc<str>>::into(read_line("Option(float or number)")));

    println!("You sent: \"{}\"", data.read::<Rc<str>>().unwrap().trim());


    let mut copy = data.clone(); 
    let input = data.read::<Rc<str>>().unwrap();
    match input.trim() {
        "float" => {
            let value: f64 = read_line("Float").trim().parse().unwrap();
            data.write(value);
        },
        "number" => {
            let value: u64 = read_line("Number").trim().parse().unwrap();
            data.write(value);
        },
        _ => {
            println!("Invalid option.");
        }
    } 

    if let Some(float) = data.read::<f64>() {
        println!("You entered: {}, plus 1: {}", float, float + 1.0);
    } else if let Some(number) = data.read::<u64>() {
        println!("You entered: {}, plus 1: {}", number, number + 1);
    } else {
        println!("You put something wrong!");
    }

    println!("You still put: \"{}\"", copy.read::<Rc<str>>().unwrap().trim());

    // Should fail to compile
    //data.write([0; 128]);
    //let buffer = data.read::<[u32; 128]>();

    // Check drop
    const SIZE: usize = size_of::<DropCheck>();

    let mut dropper: Arbitrary<SIZE> = Arbitrary::new(DropCheck("I was dropped upon first swap!"));
    dropper.write(DropCheck("I was dropped upon swap!"));
    dropper.write(DropCheck("I was droppped on scope end!"));


    
}