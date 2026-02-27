//! Non-TTY behavior demo.
//!
//! When output isn't a terminal, nanospinner skips animation and ANSI
//! codes entirely. Only the final result lines are emitted as plain text.
//! Cleared lines produce no output at all.
//!
//! This example captures output in a buffer (forcing the non-TTY path)
//! and prints what was produced so you can see exactly what downstream
//! consumers receive when piping.
//!
//! Run with: cargo run --example non_tty
//!
//! You can also pipe the real stdout examples to see the same behavior:
//!   cargo run --example demo | cat

use nanospinner::{MultiSpinner, Spinner};
use std::io;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct BufWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for BufWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

fn drain(buf: &Arc<Mutex<Vec<u8>>>) -> String {
    let mut v = buf.lock().unwrap();
    let s = String::from_utf8(v.clone()).expect("valid UTF-8");
    v.clear();
    s
}

fn main() {
    let buf = Arc::new(Mutex::new(Vec::<u8>::new()));

    // ── Single spinner ──────────────────────────────────────────────────
    println!("Single spinner:\n");

    let h = Spinner::with_writer("Compiling...", BufWriter(Arc::clone(&buf))).start();
    h.success();

    let h = Spinner::with_writer("Deploying...", BufWriter(Arc::clone(&buf))).start();
    h.fail_with("Connection refused");

    let h = Spinner::with_writer("Step 1...", BufWriter(Arc::clone(&buf))).start();
    h.update("Step 2...");
    h.success();

    print!("{}", drain(&buf));

    // ── Multi-spinner with clears ───────────────────────────────────────
    println!("\nMulti-spinner with clears:\n");

    let handle = MultiSpinner::with_writer(BufWriter(Arc::clone(&buf))).start();

    let build = handle.add("Building project...");
    let lint = handle.add("Running linter...");
    let test = handle.add("Running tests...");
    let docs = handle.add("Generating docs...");

    lint.clear(); // silently dismissed — no output
    build.success();
    docs.fail_with("Missing doc comments.");
    test.success_with("42 tests passed.");

    print!("{}", drain(&buf));
}
