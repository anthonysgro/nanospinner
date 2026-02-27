//! Comprehensive single-spinner examples.
//!
//! Run with: cargo run --example single

use nanospinner::Spinner;
use std::thread;
use std::time::Duration;

fn main() {
    // Success
    let handle = Spinner::new("Downloading the internet...").start();
    thread::sleep(Duration::from_secs(2));
    handle.success();

    thread::sleep(Duration::from_millis(500));

    // Success with replacement message
    let handle = Spinner::new("Compiling...").start();
    thread::sleep(Duration::from_secs(2));
    handle.success_with("Compiled in 2.1s");

    thread::sleep(Duration::from_millis(500));

    // Fail
    let handle = Spinner::new("Connecting to server...").start();
    thread::sleep(Duration::from_secs(2));
    handle.fail();

    thread::sleep(Duration::from_millis(500));

    // Fail with replacement message
    let handle = Spinner::new("Deploying to production...").start();
    thread::sleep(Duration::from_secs(2));
    handle.fail_with("Connection timed out.");

    thread::sleep(Duration::from_millis(500));

    // Warn
    let handle = Spinner::new("Checking disk space...").start();
    thread::sleep(Duration::from_secs(2));
    handle.warn_with("Disk usage above 80%.");

    thread::sleep(Duration::from_millis(500));

    // Info
    let handle = Spinner::new("Scanning environment...").start();
    thread::sleep(Duration::from_secs(2));
    handle.info_with("Using cached config.");

    thread::sleep(Duration::from_millis(500));

    // Update mid-spin
    let handle = Spinner::new("Step 1 of 3...").start();
    thread::sleep(Duration::from_secs(1));
    handle.update("Step 2 of 3...");
    thread::sleep(Duration::from_secs(1));
    handle.update("Step 3 of 3...");
    thread::sleep(Duration::from_secs(1));
    handle.success_with("All steps complete.");

    thread::sleep(Duration::from_millis(500));

    // Stop without symbol
    let handle = Spinner::new("Temporary task...").start();
    thread::sleep(Duration::from_secs(2));
    handle.stop();
}
