use nanospinner::Spinner;
use std::thread;
use std::time::Duration;

fn main() {
    // 1. Success Demo
    let handle = Spinner::new("Downloading the internet...").start();
    thread::sleep(Duration::from_secs(2));
    handle.success();

    // 2. Update Demo
    let handle = Spinner::new("Initializing...").start();
    thread::sleep(Duration::from_secs(1));
    handle.update("Still initializing, but faster...");
    thread::sleep(Duration::from_secs(1));
    handle.success_with("Ready!");

    // 3. Fail Demo
    let handle = Spinner::new("Attempting to bypass physics...").start();
    thread::sleep(Duration::from_secs(2));
    handle.fail_with("Laws of thermodynamics won.");
}
