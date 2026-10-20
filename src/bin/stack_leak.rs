use std::{io::{Read, Write}, thread};

enum ProtectionType {
    Quiet,
    Loud
}
fn magical_stack_leak<T: 'static, F: FnOnce(&'static T) -> R, R>(protection_type: ProtectionType, data: T, callback: F) -> ! {
    // Safety: Safe becasue `data` will never be dropped or invalidated
    let data: &'static T = unsafe { std::mem::transmute(&data) };
    std::mem::forget(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(data)))); // See emergency_print
    
    // Prevent data being dropped/invalidated. park(), abort(), and emergency_print() are all guarnteeds to never panic.
    match protection_type {
        ProtectionType::Quiet => loop { std::thread::park() }
        ProtectionType::Loud => {
            emergency_print("Unexpected return of stack-leaked data. Exploding to prevent a  dangling &'static.\nGoodbye.");
            std::process::abort()
        }
    }
}

// Print a static error message to stderr.
// Safety: GUARANTEED to never panic, unsafe code may rely on this.
fn emergency_print(message: &'static str) {
    // If *logging the error* fails, \0/
    // Safety: catch_unwind seems to indicate droping the panic payload could, itself panic. So don't drop the payload.
    std::mem::forget(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| eprintln!("{}", message)))); 
}

fn main() {
    let mut some_data = [0u8; 32];

    print!("Give me some data! Up to {} characters :3\n> ", some_data.len());
    std::io::stdout().flush().unwrap();
    let offset = std::io::stdin().read(&mut some_data).unwrap();
    
    magical_stack_leak(ProtectionType::Loud, some_data, |data| {
        let data: &'static str  = str::from_utf8(&data[..offset]).expect("I wanted UTF8 ):").trim();

        let data2 = data; // So the reference can be moved
        thread::spawn(move || {
            for _ in 0..30 {
                println!("The data is: {}", data2);
                std::thread::sleep(std::time::Duration::from_secs(1))
            }

            println!("I shall die now :/");
            std::process::exit(1);
        });

        println!("Hello! The data is: {data}\nYou should type a hex number! <3", );
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();

        let line = line.trim();
        let line = line.strip_prefix("0x").unwrap_or(line);
        println!("You just sent: {line}!");

        let num: u128 = u128::from_str_radix(line,16).expect("I wanted hexedecimal TWT"); // So you can trigger a panic
        println!("As a number, that's {num}!");

        println!("I shall sleep until the other thread does it's thing! Guten nacht, user!");
        std::thread::park();
    });
}
