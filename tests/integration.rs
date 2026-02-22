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

    let output = String::from_utf8(inner.lock().unwrap().clone()).expect("output should be valid UTF-8");
    assert!(output.contains("✔"), "output should contain the ✔ symbol");
    assert!(output.contains("Loading..."), "output should contain the spinner message");
}
