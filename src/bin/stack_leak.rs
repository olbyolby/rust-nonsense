use std::{io::{Read, Write}, thread};

enum ProtectionType {
    Quiet,
    Loud
}
fn magical_stack_leak<T: 'static, F: FnOnce(&'static T) -> R, R>(protection_type: ProtectionType, data: T, callback: F) -> ! {
    // Safety: Safe becasue `data` will never be dropped or invalidated
    let data: &'static T = unsafe { std::mem::transmute(&data) };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(data)));
    
    // Prevent data being dropped/invalidated
    match protection_type {
        ProtectionType::Quiet => loop { std::thread::park() }
        ProtectionType::Loud => {
            eprintln!("Unexpected return of stack-leaked data. Exploding to preven dangling &'static"); 
            std::process::abort()
        }
    }
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
            for i in 0..30 {
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
