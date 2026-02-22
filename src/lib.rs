//! A minimal, zero-dependency terminal spinner for Rust CLI applications.
//!
//! `nanospinner` provides a lightweight animated spinner for giving users
//! feedback during long-running CLI operations. It runs the animation on a
//! background thread so your main logic stays unblocked.
//!
//! # Quick start
//!
//! ```no_run
//! use nanospinner::Spinner;
//! use std::thread;
//! use std::time::Duration;
//!
//! let mut handle = Spinner::new("Loading...").start();
//! thread::sleep(Duration::from_secs(2));
//! handle.success();
//! ```
//!
//! # Features
//!
//! - Zero dependencies (only `std`)
//! - Braille-dot animation that stays on a single line
//! - Update the message while spinning via [`SpinnerHandle::update`]
//! - Finish with [`SpinnerHandle::success`] (✔ green) or [`SpinnerHandle::fail`] (✖ red)
//! - Pluggable writer for testing or custom output targets
//! - Automatic TTY detection: ANSI codes and animation are skipped when
//!   output is piped or redirected

use std::io::{self, IsTerminal};
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
    is_tty: bool,
}

pub struct SpinnerHandle {
    stop_flag: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
    thread: Option<JoinHandle<()>>,
    is_tty: bool,
}

impl Spinner {
    /// Create a new spinner with the given message, writing to stdout.
    ///
    /// Automatically detects whether stdout is a terminal. When it isn't
    /// (e.g. output is piped or redirected), the spinner skips animation
    /// and ANSI codes, printing plain text instead.
    pub fn new(message: impl Into<String>) -> Spinner<io::Stdout> {
        Spinner {
            message: message.into(),
            frames: FRAMES.to_vec(),
            interval: Duration::from_millis(80),
            is_tty: io::stdout().is_terminal(),
            writer: io::stdout(),
        }
    }
}

impl<W: io::Write + Send + 'static> Spinner<W> {
    /// Create a new spinner with the given message and a custom writer.
    ///
    /// `is_tty` defaults to `false` for custom writers. Use
    /// [`Spinner::with_writer_tty`] if you need to override this.
    pub fn with_writer(message: impl Into<String>, writer: W) -> Self {
        Spinner {
            message: message.into(),
            frames: FRAMES.to_vec(),
            interval: Duration::from_millis(80),
            is_tty: false,
            writer,
        }
    }

    /// Create a new spinner with the given message, a custom writer, and
    /// an explicit TTY flag controlling whether ANSI codes are emitted.
    pub fn with_writer_tty(message: impl Into<String>, writer: W, is_tty: bool) -> Self {
        Spinner {
            message: message.into(),
            frames: FRAMES.to_vec(),
            interval: Duration::from_millis(80),
            is_tty,
            writer,
        }
    }

    /// Spawn the background animation thread and return a handle.
    ///
    /// When the output is not a TTY, no background thread is spawned and
    /// the animation is skipped entirely.
    pub fn start(self) -> SpinnerHandle {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let message = Arc::new(Mutex::new(self.message));
        let writer: Arc<Mutex<Box<dyn io::Write + Send>>> =
            Arc::new(Mutex::new(Box::new(self.writer)));
        let is_tty = self.is_tty;

        let thread = if is_tty {
            let t_frames = self.frames.clone();
            let t_interval = self.interval;
            let t_stop = Arc::clone(&stop_flag);
            let t_msg = Arc::clone(&message);
            let t_writer = Arc::clone(&writer);

            Some(thread::spawn(move || {
                spin_loop(t_frames, t_interval, t_stop, t_msg, t_writer);
            }))
        } else {
            // Mark as already stopped so drop() is a no-op.
            stop_flag.store(true, Ordering::Relaxed);
            None
        };

        SpinnerHandle {
            stop_flag,
            message,
            writer,
            thread,
            is_tty,
        }
    }
}

fn format_frame(frame_char: char, message: &str) -> String {
    format!("\r{} {}", frame_char, message)
}

fn format_finalize(symbol: &str, color: &str, message: &str) -> String {
    format!("\r{}{}{}{} {}\n", CLEAR_LINE, color, symbol, RESET, message)
}

fn format_finalize_plain(symbol: &str, message: &str) -> String {
    format!("{} {}\n", symbol, message)
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
        if self.is_tty {
            let mut w = self.writer.lock().unwrap();
            write!(w, "\r{}", CLEAR_LINE).unwrap();
            w.flush().unwrap();
        }
    }

    /// Stop the spinner and print a green ✔ with the current message.
    pub fn success(mut self) {
        let msg = self.message.lock().unwrap().clone();
        self.stop();
        let output = if self.is_tty {
            format_finalize("✔", GREEN, &msg)
        } else {
            format_finalize_plain("✔", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a green ✔ with a replacement message.
    pub fn success_with(mut self, message: impl Into<String>) {
        self.stop();
        let msg = message.into();
        let output = if self.is_tty {
            format_finalize("✔", GREEN, &msg)
        } else {
            format_finalize_plain("✔", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a red ✖ with the current message.
    pub fn fail(mut self) {
        let msg = self.message.lock().unwrap().clone();
        self.stop();
        let output = if self.is_tty {
            format_finalize("✖", RED, &msg)
        } else {
            format_finalize_plain("✖", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{}", output).unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a red ✖ with a replacement message.
    pub fn fail_with(mut self, message: impl Into<String>) {
        self.stop();
        let msg = message.into();
        let output = if self.is_tty {
            format_finalize("✖", RED, &msg)
        } else {
            format_finalize_plain("✖", &msg)
        };
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

    #[test]
    fn test_non_tty_success_has_no_ansi_codes() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let spinner = Spinner::with_writer("Compiling...", writer);
        let handle = spinner.start();
        handle.success();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !output.contains("\x1b["),
            "non-TTY output must not contain ANSI escape codes"
        );
        assert!(
            !output.contains(CLEAR_LINE),
            "non-TTY output must not contain CLEAR_LINE"
        );
        assert!(output.contains("✔"), "non-TTY output should contain ✔");
        assert!(
            output.contains("Compiling..."),
            "non-TTY output should contain the message"
        );
        assert_eq!(output, "✔ Compiling...\n");
    }

    #[test]
    fn test_non_tty_fail_has_no_ansi_codes() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let spinner = Spinner::with_writer("Deploying...", writer);
        let handle = spinner.start();
        handle.fail();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !output.contains("\x1b["),
            "non-TTY output must not contain ANSI escape codes"
        );
        assert!(output.contains("✖"), "non-TTY output should contain ✖");
        assert_eq!(output, "✖ Deploying...\n");
    }

    #[test]
    fn test_non_tty_success_with_has_no_ansi_codes() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let spinner = Spinner::with_writer("Working...", writer);
        let handle = spinner.start();
        handle.success_with("Done!");

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !output.contains("\x1b["),
            "non-TTY output must not contain ANSI escape codes"
        );
        assert_eq!(output, "✔ Done!\n");
    }

    #[test]
    fn test_non_tty_skips_animation() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let spinner = Spinner::with_writer("Loading...", writer);
        let handle = spinner.start();
        // Sleep long enough that a TTY spinner would have written frames
        thread::sleep(Duration::from_millis(200));
        handle.success();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        // Should only contain the final line, no spinner frames
        assert!(
            !output.contains('⠋'),
            "non-TTY output must not contain spinner frames"
        );
        assert!(
            !output.contains('\r'),
            "non-TTY output must not contain carriage returns"
        );
        assert_eq!(output, "✔ Loading...\n");
    }

    #[test]
    fn test_tty_mode_emits_ansi_codes() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let spinner = Spinner::with_writer_tty("Building...", writer, true);
        let handle = spinner.start();
        thread::sleep(Duration::from_millis(200));
        handle.success();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            output.contains("\x1b["),
            "TTY output should contain ANSI escape codes"
        );
        assert!(output.contains("✔"), "TTY output should contain ✔");
        assert!(output.contains(GREEN), "TTY output should contain GREEN");
    }

    #[test]
    fn test_format_finalize_plain() {
        assert_eq!(format_finalize_plain("✔", "hello"), "✔ hello\n");
        assert_eq!(format_finalize_plain("✖", "oops"), "✖ oops\n");
    }

    /// A simple Write wrapper around a shared buffer for tests.
    #[derive(Clone)]
    struct TestWriter(Arc<Mutex<Vec<u8>>>);

    impl io::Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }
}
