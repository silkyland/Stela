//! Inspect a saved boot-log report without opening or writing any disk.
use stela_runtime::pc::device_starts::{self, Decision, Needed};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: device-start-record BOOTLOG.txt")?;
    let stream = std::fs::read(path)?;
    let skips = device_starts::read(&stream);
    println!(
        "Whole marked boot found: {}",
        device_starts::settled(&stream)
    );
    for function in skips.iter() {
        let decision = device_starts::decide(&skips, &Needed::EMPTY, None, function);
        assert_eq!(decision, Decision::Skip);
        println!(
            "{}: {:?} if not required by the boot",
            function.line(device_starts::START),
            decision
        );
    }
    println!("Recorded functions: {}", skips.iter().count());
    Ok(())
}
