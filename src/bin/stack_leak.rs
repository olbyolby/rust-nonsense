use std::{io::Read, thread};

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
        ProtectionType::Loud => std::process::abort(),
        ProtectionType::Quiet => loop { std::thread::park() }
    }
}

fn main() {
    let mut some_data = [0u8; 16];
    let offset = std::io::stdin().read(&mut some_data).unwrap();
    
    magical_stack_leak(ProtectionType::Loud, some_data, |data| {
        let data: &'static str  = str::from_utf8(&data[..offset]).unwrap().trim();

        let data2 = data; // So the reference can be moved
        thread::spawn(move || {
            loop {
                println!("The data is: {}", data2);
                std::thread::sleep(std::time::Duration::from_secs(1))
            }
        });

        println!("Hello! The data is: {data}", );
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();

        let line = line.trim();
        println!("You just sent: {line}!");

        std::thread::park();
    });
}
