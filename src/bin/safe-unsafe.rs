// Extremely "safe" code that causes memory corruption with neither `unsafe` nor a soundness hole.
use std::io::{Seek, Write};
use std::fs::OpenOptions;

fn main() {
    let some_data = "Memory safety is important to ensure your Rust code does not exhibit undefined behavior or memory corruption.";

    // Rust is "safe", they said. Safe rust can "Never cause memory corruption", they said.
    let mut file = OpenOptions::new().write(true).open("/proc/self/mem").unwrap();

    // Not a line of `unsafe {}` in sight
    file.seek(std::io::SeekFrom::Start(some_data.as_ptr().addr() as u64)).unwrap();
    file.write(b"WHERE'S YOUR MEMORY-SAFETY GOD NOW, RUST?").unwrap(); // Undefined behavior

    // In theory, Rust can optimize some_data directly into this println!, since Rust doesn't change it and it's "immutable", but hopefully not.
    println!("{}", some_data)
}

