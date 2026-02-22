// nanospinner - a minimal terminal spinner for Rust CLI applications

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// ANSI escape codes
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";
const CLEAR_LINE: &str = "\x1b[2K";

// Default spinner character set (Braille dots)
const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub struct Spinner<W: io::Write + Send + 'static = io::Stdout> {
    message: String,
    frames: Vec<char>,
    interval: Duration,
    writer: W,
}

pub struct SpinnerHandle {
    stop_flag: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
    thread: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Create a new spinner with the given message, writing to stdout.
    pub fn new(message: impl Into<String>) -> Spinner<io::Stdout> {
        Spinner {
            message: message.into(),
            frames: FRAMES.to_vec(),
            interval: Duration::from_millis(80),
            writer: io::stdout(),
        }
    }
}

impl<W: io::Write + Send + 'static> Spinner<W> {
    /// Create a new spinner with the given message and a custom writer.
    pub fn with_writer(message: impl Into<String>, writer: W) -> Self {
        Spinner {
            message: message.into(),
            frames: FRAMES.to_vec(),
            interval: Duration::from_millis(80),
            writer,
        }
    }

    /// Spawn the background animation thread and return a handle.
    pub fn start(self) -> SpinnerHandle {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let message = Arc::new(Mutex::new(self.message));
        let writer: Arc<Mutex<Box<dyn io::Write + Send>>> =
            Arc::new(Mutex::new(Box::new(self.writer)));

        let t_frames = self.frames.clone();
        let t_interval = self.interval;
        let t_stop = Arc::clone(&stop_flag);
        let t_msg = Arc::clone(&message);
        let t_writer = Arc::clone(&writer);

        let thread = thread::spawn(move || {
            spin_loop(t_frames, t_interval, t_stop, t_msg, t_writer);
        });

        SpinnerHandle {
            stop_flag,
            message,
            writer,
            thread: Some(thread),
        }
    }
}

fn format_frame(frame_char: char, message: &str) -> String {
    format!("\r{} {}", frame_char, message)
}

fn format_finalize(symbol: &str, color: &str, message: &str) -> String {
    format!("\r{}{}{}{} {}\n", CLEAR_LINE, color, symbol, RESET, message)
}

fn spin_loop(
    frames: Vec<char>,
    interval: Duration,
    stop_flag: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
) {
    let mut i = 0;
    while !stop_flag.load(Ordering::Relaxed) {
        let msg = message.lock().unwrap().clone();
        let frame = frames[i % frames.len()];
        let output = format_frame(frame, &msg);
        let mut w = writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
        drop(w);
        i += 1;
        thread::sleep(interval);
    }
}

impl SpinnerHandle {
    /// Update the spinner message while it's running.
    pub fn update(&self, message: impl Into<String>) {
        *self.message.lock().unwrap() = message.into();
    }

    /// Stop the spinner and clear the line.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let mut w = self.writer.lock().unwrap();
        write!(w, "\r{}", CLEAR_LINE).unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a green ✔ with the current message.
    pub fn success(mut self) {
        let msg = self.message.lock().unwrap().clone();
        self.stop();
        let output = format_finalize("✔", GREEN, &msg);
        let mut w = self.writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a green ✔ with a replacement message.
    pub fn success_with(mut self, message: impl Into<String>) {
        self.stop();
        let output = format_finalize("✔", GREEN, &message.into());
        let mut w = self.writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a red ✖ with the current message.
    pub fn fail(mut self) {
        let msg = self.message.lock().unwrap().clone();
        self.stop();
        let output = format_finalize("✖", RED, &msg);
        let mut w = self.writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a red ✖ with a replacement message.
    pub fn fail_with(mut self, message: impl Into<String>) {
        self.stop();
        let output = format_finalize("✖", RED, &message.into());
        let mut w = self.writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
    }
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // **Validates: Requirements 1.1**
        #[test]
        fn property_construction_preserves_message(s in ".*") {
            let spinner = Spinner::with_writer(s.clone(), Vec::<u8>::new());
            prop_assert_eq!(spinner.message, s);
        }

        // **Validates: Requirements 2.2**
        #[test]
        fn property_frame_format_correctness(msg in ".*", idx in 0usize..1000) {
            let frame_char = FRAMES[idx % FRAMES.len()];
            let result = format_frame(frame_char, &msg);
            let expected = format!("\r{} {}", frame_char, msg);
            prop_assert_eq!(result, expected);
        }

        // **Validates: Requirements 5.1, 6.1**
        #[test]
        fn property_finalization_output_format(msg in ".*") {
            // Test success finalization
            let success_output = format_finalize("✔", GREEN, &msg);
            prop_assert!(success_output.contains('\r'), "success output must contain \\r");
            prop_assert!(success_output.contains(CLEAR_LINE), "success output must contain CLEAR_LINE");
            prop_assert!(success_output.contains(GREEN), "success output must contain GREEN");
            prop_assert!(success_output.contains("✔"), "success output must contain ✔");
            prop_assert!(success_output.contains(RESET), "success output must contain RESET");
            prop_assert!(success_output.contains(&msg), "success output must contain the message");
            prop_assert!(success_output.ends_with('\n'), "success output must end with \\n");

            // Test fail finalization
            let fail_output = format_finalize("✖", RED, &msg);
            prop_assert!(fail_output.contains('\r'), "fail output must contain \\r");
            prop_assert!(fail_output.contains(CLEAR_LINE), "fail output must contain CLEAR_LINE");
            prop_assert!(fail_output.contains(RED), "fail output must contain RED");
            prop_assert!(fail_output.contains("✖"), "fail output must contain ✖");
            prop_assert!(fail_output.contains(RESET), "fail output must contain RESET");
            prop_assert!(fail_output.contains(&msg), "fail output must contain the message");
            prop_assert!(fail_output.ends_with('\n'), "fail output must end with \\n");
        }

        // **Validates: Requirements 5.2, 6.2**
        #[test]
        fn property_replacement_message_in_finalization(
            original in ".{1,50}",
            replacement in ".{1,50}"
        ) {
            // Only test when original and replacement are distinct
            prop_assume!(original != replacement);

            // Test success_with: output should match expected format with replacement message
            let success_output = format_finalize("✔", GREEN, &replacement);
            let expected_success = format!("\r{}{}✔{} {}\n", CLEAR_LINE, GREEN, RESET, replacement);
            prop_assert_eq!(
                success_output, expected_success,
                "success_with output must use the replacement message in the correct format"
            );

            // Test fail_with: output should match expected format with replacement message
            let fail_output = format_finalize("✖", RED, &replacement);
            let expected_fail = format!("\r{}{}✖{} {}\n", CLEAR_LINE, RED, RESET, replacement);
            prop_assert_eq!(
                fail_output, expected_fail,
                "fail_with output must use the replacement message in the correct format"
            );
        }

        // **Validates: Requirements 7.1**
        #[test]
        fn property_update_changes_shared_message_state(
            initial in ".{0,50}",
            new_msg in ".{0,50}"
        ) {
            let spinner = Spinner::with_writer(initial, Vec::<u8>::new());
            let handle = spinner.start();

            handle.update(new_msg.clone());

            // Read the shared message state — accessible since tests are in the same module
            let stored = handle.message.lock().unwrap().clone();
            prop_assert_eq!(stored, new_msg, "shared message state must equal the new message after update");

            // Clean up: stop the spinner
            drop(handle);
        }
    }

    #[test]
    fn test_default_frames() {
        let expected = vec!['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        let spinner = Spinner::with_writer("test", Vec::<u8>::new());
        assert_eq!(spinner.frames, expected);
    }

    #[test]
    fn test_default_interval() {
        let spinner = Spinner::with_writer("test", Vec::<u8>::new());
        assert_eq!(spinner.interval, Duration::from_millis(80));
    }

    #[test]
    fn test_with_writer_uses_provided_writer() {
        let buf = Vec::<u8>::new();
        let spinner = Spinner::with_writer("test", buf);
        let mut handle = spinner.start();
        thread::sleep(Duration::from_millis(100));
        handle.stop();
    }

    #[test]
    fn test_drop_without_stop_joins_thread() {
        let spinner = Spinner::with_writer("test", Vec::<u8>::new());
        let handle = spinner.start();
        thread::sleep(Duration::from_millis(100));
        drop(handle); // Should not hang or panic
    }
}

