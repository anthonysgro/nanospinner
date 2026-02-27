//! Comprehensive multi-spinner examples.
//!
//! Run with: cargo run --example multi

use nanospinner::MultiSpinner;
use std::thread;
use std::time::Duration;

fn main() {
    // Basic: add lines, update, finalize with mixed outcomes.
    println!("Basic multi-spinner");
    {
        let handle = MultiSpinner::new().start();

        let a = handle.add("Compiling crate A...");
        let b = handle.add("Compiling crate B...");
        let c = handle.add("Compiling crate C...");

        thread::sleep(Duration::from_secs(2));
        b.update("Compiling crate B (linking...)");

        thread::sleep(Duration::from_secs(2));
        a.success();
        b.warn_with("Crate B compiled with warnings.");
        c.fail_with("Crate C had errors.");

        handle.stop();
    }

    thread::sleep(Duration::from_millis(500));

    // Clear: silently dismiss lines.
    println!("\nClear demo");
    {
        let handle = MultiSpinner::new().start();

        let check = handle.add("Running checks...");
        let lint = handle.add("Linting...");
        let fmt = handle.add("Formatting...");
        let docs = handle.add("Building docs...");

        thread::sleep(Duration::from_secs(2));
        lint.clear(); // dismiss — line disappears

        thread::sleep(Duration::from_secs(1));
        fmt.clear(); // dismiss another

        thread::sleep(Duration::from_secs(1));
        check.success();
        docs.success_with("Docs generated.");

        handle.stop();
    }

    thread::sleep(Duration::from_millis(500));

    // Dynamic: add new lines after others finish.
    println!("\nDynamic add");
    {
        let handle = MultiSpinner::new().start();

        let a = handle.add("Phase 1: Init...");
        let b = handle.add("Phase 1: Validate...");

        thread::sleep(Duration::from_secs(2));
        a.success();
        b.success();

        let c = handle.add("Phase 2: Build...");
        let d = handle.add("Phase 2: Test...");

        thread::sleep(Duration::from_secs(2));
        c.success();
        d.fail_with("3 tests failed.");

        handle.stop();
    }

    thread::sleep(Duration::from_millis(500));

    // Thread-based: move handles to worker threads.
    println!("\nThread-based workers");
    {
        let handle = MultiSpinner::new().start();

        let workers: Vec<_> = (1..=4)
            .map(|i| {
                let line = handle.add(format!("Worker {i} processing..."));
                thread::spawn(move || {
                    thread::sleep(Duration::from_secs(i));
                    line.success_with(format!("Worker {i} done in {i}s"));
                })
            })
            .collect();

        for w in workers {
            w.join().unwrap();
        }

        handle.stop();
    }
}
