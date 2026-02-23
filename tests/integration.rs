use nanospinner::Spinner;
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// A shared buffer wrapper that implements `io::Write + Send + 'static`,
/// allowing us to read the output after the spinner finishes.
#[derive(Clone)]
struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl io::Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

#[test]
fn test_spinner_success_output_contains_checkmark_and_message() {
    let inner = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedBuffer(Arc::clone(&inner));

    let spinner = Spinner::with_writer("Loading...", writer);
    let handle = spinner.start();

    // Let a few frames render
    thread::sleep(Duration::from_millis(200));

    handle.success();

    let output =
        String::from_utf8(inner.lock().unwrap().clone()).expect("output should be valid UTF-8");
    assert!(output.contains("✔"), "output should contain the ✔ symbol");
    assert!(
        output.contains("Loading..."),
        "output should contain the spinner message"
    );
}

#[test]
fn test_non_tty_output_is_plain_text() {
    let inner = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedBuffer(Arc::clone(&inner));

    // with_writer defaults is_tty to false
    let spinner = Spinner::with_writer("Deploying...", writer);
    let handle = spinner.start();
    thread::sleep(Duration::from_millis(200));
    handle.success();

    let output = String::from_utf8(inner.lock().unwrap().clone()).unwrap();

    // No ANSI escape codes anywhere in the output
    assert!(
        !output.contains("\x1b["),
        "non-TTY output must not contain ANSI escape codes"
    );
    // No carriage returns (spinner frame overwrites)
    assert!(
        !output.contains('\r'),
        "non-TTY output must not contain carriage returns"
    );
    // No spinner frame characters
    assert!(
        !output.contains('⠋'),
        "non-TTY output must not contain spinner frames"
    );
    // Just the clean final line
    assert_eq!(output, "✔ Deploying...\n");
}

#[test]
fn test_non_tty_fail_output_is_plain_text() {
    let inner = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedBuffer(Arc::clone(&inner));

    let spinner = Spinner::with_writer("Building...", writer);
    let handle = spinner.start();
    thread::sleep(Duration::from_millis(200));
    handle.fail_with("Build failed");

    let output = String::from_utf8(inner.lock().unwrap().clone()).unwrap();
    assert!(
        !output.contains("\x1b["),
        "non-TTY output must not contain ANSI escape codes"
    );
    assert_eq!(output, "✖ Build failed\n");
}

#[test]
fn test_tty_mode_produces_ansi_and_animation() {
    let inner = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedBuffer(Arc::clone(&inner));

    // Explicitly enable TTY mode
    let spinner = Spinner::with_writer_tty("Compiling...", writer, true);
    let handle = spinner.start();
    thread::sleep(Duration::from_millis(200));
    handle.success();

    let output = String::from_utf8(inner.lock().unwrap().clone()).unwrap();

    // Should contain ANSI color codes
    assert!(
        output.contains("\x1b[32m"),
        "TTY output should contain green ANSI code"
    );
    assert!(
        output.contains("\x1b[0m"),
        "TTY output should contain reset ANSI code"
    );
    // Should contain spinner animation frames
    assert!(
        output.contains('⠋'),
        "TTY output should contain spinner frames"
    );
    assert!(output.contains("✔"), "TTY output should contain ✔");
}

#[test]
fn test_tty_mode_fail_emits_ansi_codes() {
    let inner = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedBuffer(Arc::clone(&inner));

    let spinner = Spinner::with_writer_tty("message", writer, true);
    let handle = spinner.start();
    thread::sleep(Duration::from_millis(200));
    handle.fail();

    let output = String::from_utf8(inner.lock().unwrap().clone()).unwrap();

    // Should contain red ANSI color code
    assert!(
        output.contains("\x1b[31m"),
        "TTY fail output should contain red ANSI code"
    );
    // Should contain the ✖ symbol
    assert!(output.contains("✖"), "TTY fail output should contain ✖");
    // Should contain the original message
    assert!(
        output.contains("message"),
        "TTY fail output should contain the message"
    );
    // Should contain ANSI reset code
    assert!(
        output.contains("\x1b[0m"),
        "TTY fail output should contain reset ANSI code"
    );
    // Should contain spinner animation frames (confirming TTY mode ran)
    assert!(
        output.contains('⠋'),
        "TTY fail output should contain spinner frames"
    );
}

#[test]
fn test_tty_mode_fail_with_emits_ansi_codes() {
    let inner = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedBuffer(Arc::clone(&inner));

    let spinner = Spinner::with_writer_tty("original", writer, true);
    let handle = spinner.start();
    thread::sleep(Duration::from_millis(200));
    handle.fail_with("replacement message");

    let output = String::from_utf8(inner.lock().unwrap().clone()).unwrap();

    // Should contain red ANSI color code
    assert!(
        output.contains("\x1b[31m"),
        "TTY fail_with output should contain red ANSI code"
    );
    // Should contain the ✖ symbol
    assert!(
        output.contains("✖"),
        "TTY fail_with output should contain ✖"
    );
    // Should contain the replacement message
    assert!(
        output.contains("replacement message"),
        "TTY fail_with output should contain the replacement message"
    );
    // Should contain ANSI reset code
    assert!(
        output.contains("\x1b[0m"),
        "TTY fail_with output should contain reset ANSI code"
    );
    // Should contain spinner animation frames (confirming TTY mode ran)
    assert!(
        output.contains('⠋'),
        "TTY fail_with output should contain spinner frames"
    );
}

#[test]
fn test_with_writer_tty_false_behaves_as_non_tty() {
    let inner = Arc::new(Mutex::new(Vec::<u8>::new()));
    let writer = SharedBuffer(Arc::clone(&inner));

    let spinner = Spinner::with_writer_tty("message", writer, false);
    let handle = spinner.start();
    thread::sleep(Duration::from_millis(200));
    handle.success();

    let output = String::from_utf8(inner.lock().unwrap().clone()).unwrap();

    // No ANSI escape codes
    assert!(
        !output.contains("\x1b["),
        "with_writer_tty(false) output must not contain ANSI escape codes"
    );
    // No carriage returns
    assert!(
        !output.contains('\r'),
        "with_writer_tty(false) output must not contain carriage returns"
    );
    // No spinner frames
    assert!(
        !output.contains('⠋'),
        "with_writer_tty(false) output must not contain spinner frames"
    );
    // Exact plain text output
    assert_eq!(output, "✔ message\n");
}
