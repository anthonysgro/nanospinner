//! Quick demo of nanospinner — single and multi-spinner in action.
//!
//! Run with: cargo run --example demo

use nanospinner::{MultiSpinner, Spinner};
use std::thread;
use std::time::Duration;

fn main() {
    // Single spinner — start, update, finish.
    let handle = Spinner::new("Installing dependencies...").start();
    thread::sleep(Duration::from_secs(2));
    handle.update("Almost there...");
    thread::sleep(Duration::from_secs(1));
    handle.success_with("Installed 47 packages.");

    thread::sleep(Duration::from_millis(500));

    // Multi-spinner — parallel tasks with mixed outcomes.
    let mut handle = MultiSpinner::new().start();

    let build = handle.add("Building project...");
    let lint = handle.add("Running linter...");
    let test = handle.add("Running tests...");
    let docs = handle.add("Generating docs...");

    thread::sleep(Duration::from_secs(2));
    lint.clear(); // silently dismiss — no issues found
    thread::sleep(Duration::from_secs(1));
    build.success();
    docs.fail_with("Missing doc comments.");
    thread::sleep(Duration::from_secs(2));
    test.success_with("42 tests passed.");

    handle.stop();
}
