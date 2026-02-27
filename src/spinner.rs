use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(test)]
use crate::shared::RESET;
use crate::shared::{
    format_finalize, format_finalize_plain, format_frame, BLUE, CLEAR_LINE, FRAMES, GREEN, RED,
    YELLOW,
};

/// A builder for configuring and starting a terminal spinner.
///
/// Use [`Spinner::new`] for stdout, or [`Spinner::with_writer`] /
/// [`Spinner::with_writer_tty`] for custom output targets. Call
/// [`Spinner::start`] to begin the animation and get a [`SpinnerHandle`].
pub struct Spinner<W: io::Write + Send + 'static = io::Stdout> {
    message: String,
    frames: Vec<char>,
    interval: Duration,
    writer: W,
    is_tty: bool,
}

/// Handle for controlling a running spinner.
///
/// Returned by [`Spinner::start`]. Use [`SpinnerHandle::update`] to change
/// the message mid-spin, and finalize with [`SpinnerHandle::success`] or
/// [`SpinnerHandle::fail`]. Dropping the handle will automatically stop
/// the background thread.
pub struct SpinnerHandle {
    stop_flag: Arc<AtomicBool>,
    message: Arc<Mutex<String>>,
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
    is_tty: bool,
}

impl Spinner {
    /// Create a new spinner with the given message, writing to stdout.
    ///
    /// Automatically detects whether stdout is a terminal. When it isn't
    /// (e.g. output is piped or redirected), the spinner skips animation
    /// and ANSI codes, printing plain text instead.
    #[must_use]
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
    #[must_use]
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
                spin_loop(&t_frames, t_interval, &t_stop, &t_msg, &t_writer);
            }))
        } else {
            // Mark as already stopped so drop() is a no-op.
            stop_flag.store(true, Ordering::Release);
            None
        };

        SpinnerHandle {
            stop_flag,
            message,
            writer,
            thread: Mutex::new(thread),
            is_tty,
        }
    }
}

fn spin_loop(
    frames: &[char],
    interval: Duration,
    stop_flag: &Arc<AtomicBool>,
    message: &Arc<Mutex<String>>,
    writer: &Arc<Mutex<Box<dyn io::Write + Send>>>,
) {
    let mut i = 0;
    while !stop_flag.load(Ordering::Acquire) {
        let msg = message.lock().unwrap().clone();
        let frame = frames[i];
        let output = format_frame(frame, &msg);
        let mut w = writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
        drop(w);
        i = (i + 1) % frames.len();
        thread::sleep(interval);
    }
}

impl SpinnerHandle {
    /// Update the spinner message while it's running.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn update(&self, message: impl Into<String>) {
        *self.message.lock().unwrap() = message.into();
    }

    /// Stop the spinner and clear the line.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn stop(self) {
        self.shutdown();
    }

    fn shutdown(&self) {
        self.stop_flag.store(true, Ordering::Release);
        let thread = self.thread.lock().unwrap().take();
        if let Some(thread) = thread {
            let _ = thread.join();
            if self.is_tty {
                if let Ok(mut w) = self.writer.lock() {
                    let _ = write!(w, "\r{CLEAR_LINE}");
                    let _ = w.flush();
                }
            }
        }
    }

    /// Stop the spinner and print a green ✔ with the current message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn success(self) {
        let msg = self.message.lock().unwrap().clone();
        self.shutdown();
        let output = if self.is_tty {
            format_finalize("✔", GREEN, &msg)
        } else {
            format_finalize_plain("✔", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a green ✔ with a replacement message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn success_with(self, message: impl Into<String>) {
        self.shutdown();
        let msg = message.into();
        let output = if self.is_tty {
            format_finalize("✔", GREEN, &msg)
        } else {
            format_finalize_plain("✔", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a red ✖ with the current message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn fail(self) {
        let msg = self.message.lock().unwrap().clone();
        self.shutdown();
        let output = if self.is_tty {
            format_finalize("✖", RED, &msg)
        } else {
            format_finalize_plain("✖", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a red ✖ with a replacement message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn fail_with(self, message: impl Into<String>) {
        self.shutdown();
        let msg = message.into();
        let output = if self.is_tty {
            format_finalize("✖", RED, &msg)
        } else {
            format_finalize_plain("✖", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a yellow ⚠ with the current message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn warn(self) {
        let msg = self.message.lock().unwrap().clone();
        self.shutdown();
        let output = if self.is_tty {
            format_finalize("⚠", YELLOW, &msg)
        } else {
            format_finalize_plain("⚠", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a yellow ⚠ with a replacement message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn warn_with(self, message: impl Into<String>) {
        self.shutdown();
        let msg = message.into();
        let output = if self.is_tty {
            format_finalize("⚠", YELLOW, &msg)
        } else {
            format_finalize_plain("⚠", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a blue ℹ with the current message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn info(self) {
        let msg = self.message.lock().unwrap().clone();
        self.shutdown();
        let output = if self.is_tty {
            format_finalize("ℹ", BLUE, &msg)
        } else {
            format_finalize_plain("ℹ", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }

    /// Stop the spinner and print a blue ℹ with a replacement message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn info_with(self, message: impl Into<String>) {
        self.shutdown();
        let msg = message.into();
        let output = if self.is_tty {
            format_finalize("ℹ", BLUE, &msg)
        } else {
            format_finalize_plain("ℹ", &msg)
        };
        let mut w = self.writer.lock().unwrap();
        write!(w, "{output}").unwrap();
        w.flush().unwrap();
    }
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::tests::TestWriter;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn property_construction_preserves_message(s in ".*") {
            let spinner = Spinner::with_writer(s.clone(), Vec::<u8>::new());
            prop_assert_eq!(spinner.message, s);
        }

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

    // TTY property tests use fewer cases (20) since each spawns a thread + sleeps
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn property_tty_fail_output_contains_ansi_symbol_and_message(
            msg in "[^\x00]{1,50}"
        ) {
            let (writer, _buf) = TestWriter::new();
            let reader = writer.clone();

            let spinner = Spinner::with_writer_tty(msg.clone(), writer, true);
            let handle = spinner.start();
            thread::sleep(Duration::from_millis(100));
            handle.fail();

            let output = reader.output();
            prop_assert!(output.contains(RED), "TTY fail output must contain RED ANSI code");
            prop_assert!(output.contains("✖"), "TTY fail output must contain ✖ symbol");
            prop_assert!(output.contains(&msg), "TTY fail output must contain the message");
            prop_assert!(output.contains(RESET), "TTY fail output must contain RESET ANSI code");
        }

        #[test]
        fn property_tty_fail_with_output_contains_ansi_symbol_and_replacement(
            original in "[^\x00]{1,50}",
            replacement in "[^\x00]{1,50}"
        ) {
            let (writer, _buf) = TestWriter::new();
            let reader = writer.clone();

            let spinner = Spinner::with_writer_tty(original, writer, true);
            let handle = spinner.start();
            thread::sleep(Duration::from_millis(100));
            handle.fail_with(replacement.clone());

            let output = reader.output();
            prop_assert!(output.contains(RED), "TTY fail_with output must contain RED ANSI code");
            prop_assert!(output.contains("✖"), "TTY fail_with output must contain ✖ symbol");
            prop_assert!(output.contains(&replacement), "TTY fail_with output must contain the replacement message");
            prop_assert!(output.contains(RESET), "TTY fail_with output must contain RESET ANSI code");
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
    fn test_lifecycle_no_panic() {
        // start → sleep → stop
        let spinner = Spinner::with_writer("test", Vec::<u8>::new());
        let handle = spinner.start();
        thread::sleep(Duration::from_millis(100));
        handle.stop();

        // start → sleep → drop
        let spinner = Spinner::with_writer("test", Vec::<u8>::new());
        let handle = spinner.start();
        thread::sleep(Duration::from_millis(100));
        drop(handle);
    }

    #[test]
    fn test_single_spinner_drop_clears_line_like_stop() {
        // With stop()
        let (writer, _buf_stop) = TestWriter::new();
        let reader_stop = writer.clone();
        let handle = Spinner::with_writer_tty("Working...", writer, true).start();
        thread::sleep(Duration::from_millis(100));
        handle.stop();
        let out_stop = reader_stop.output();

        // With drop (no stop)
        let (writer, _buf_drop) = TestWriter::new();
        let reader_drop = writer.clone();
        let handle = Spinner::with_writer_tty("Working...", writer, true).start();
        thread::sleep(Duration::from_millis(100));
        drop(handle);
        let out_drop = reader_drop.output();

        // Both must contain the clear-line sequence
        assert!(
            out_stop.contains(CLEAR_LINE),
            "stop output must contain CLEAR_LINE"
        );
        assert!(
            out_drop.contains(CLEAR_LINE),
            "drop output must contain CLEAR_LINE"
        );
    }

    #[test]
    fn test_non_tty_finalization_all_variants() {
        // (symbol, original_message, replacement_message, finalize_fn)
        // Each variant is tested with original message and with a replacement message.
        let cases: Vec<(&str, &str, &str, Box<dyn Fn(SpinnerHandle)>)> = vec![
            // success — original message
            ("✔", "msg1", "msg1", Box::new(|h| h.success())),
            // success — replacement message
            (
                "✔",
                "ignored",
                "replacement1",
                Box::new(|h| h.success_with("replacement1".to_string())),
            ),
            // fail — original message
            ("✖", "msg2", "msg2", Box::new(|h| h.fail())),
            // fail — replacement message
            (
                "✖",
                "ignored",
                "replacement2",
                Box::new(|h| h.fail_with("replacement2".to_string())),
            ),
            // warn — original message
            ("⚠", "msg3", "msg3", Box::new(|h| h.warn())),
            // warn — replacement message
            (
                "⚠",
                "ignored",
                "replacement3",
                Box::new(|h| h.warn_with("replacement3".to_string())),
            ),
            // info — original message
            ("ℹ", "msg4", "msg4", Box::new(|h| h.info())),
            // info — replacement message
            (
                "ℹ",
                "ignored",
                "replacement4",
                Box::new(|h| h.info_with("replacement4".to_string())),
            ),
        ];

        for (symbol, initial_msg, expected_msg, finalize) in cases {
            let (writer, _buf) = TestWriter::new();
            let reader = writer.clone();

            let spinner = Spinner::with_writer(initial_msg, writer);
            let handle = spinner.start();
            thread::sleep(Duration::from_millis(50));
            finalize(handle);

            let output = reader.output();
            let expected = format!("{symbol} {expected_msg}\n");
            assert_eq!(
                output, expected,
                "non-TTY {symbol} output must be \"{symbol} {expected_msg}\\n\", got: {output:?}"
            );
            assert!(
                !output.contains("\x1b["),
                "non-TTY {symbol} output must not contain ANSI escape codes"
            );
            assert!(
                !output.contains('\r'),
                "non-TTY {symbol} output must not contain carriage returns"
            );
            assert!(
                !output.contains('⠋'),
                "non-TTY {symbol} output must not contain spinner frames"
            );
        }
    }

    #[test]
    fn test_tty_mode_emits_ansi_codes() {
        let (writer, _buf) = TestWriter::new();
        let reader = writer.clone();

        let spinner = Spinner::with_writer_tty("Building...", writer, true);
        let handle = spinner.start();
        thread::sleep(Duration::from_millis(200));
        handle.success();

        let output = reader.output();
        assert!(
            output.contains("\x1b["),
            "TTY output should contain ANSI escape codes"
        );
        assert!(output.contains("✔"), "TTY output should contain ✔");
        assert!(output.contains(GREEN), "TTY output should contain GREEN");
    }
}
