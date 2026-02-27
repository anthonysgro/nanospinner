//! Custom write destinations — spinners on stderr.
//!
//! Keeps stdout free for structured output (JSON, piped data, etc.)
//! while the user still sees spinner feedback on the terminal.
//!
//! For any `io::Write + Send` target, use `with_writer`. To force
//! ANSI codes on a non-TTY writer, use `with_writer_tty`.
//!
//! Run with: cargo run --example custom_writer

use nanospinner::{MultiSpinner, Spinner};
use std::io;
use std::thread;
use std::time::Duration;

fn main() {
    // Single spinner on stderr.
    let handle = Spinner::with_writer("Installing dependencies...", io::stderr()).start();
    thread::sleep(Duration::from_secs(2));
    handle.success_with("Installed 47 packages.");

    thread::sleep(Duration::from_millis(500));

    // Multi-spinner on stderr.
    let mut handle = MultiSpinner::with_writer(io::stderr()).start();

    let build = handle.add("Building project...");
    let lint = handle.add("Running linter...");
    let test = handle.add("Running tests...");

    thread::sleep(Duration::from_secs(2));
    lint.clear();
    build.success();
    thread::sleep(Duration::from_secs(1));
    test.success_with("42 tests passed.");

    handle.stop();
}
