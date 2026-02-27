use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(test)]
use crate::shared::RESET;
use crate::shared::{
    format_finalize, format_finalize_plain, format_frame, CLEAR_LINE, FRAMES, GREEN, RED,
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
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
        if self.is_tty {
            let mut w = self.writer.lock().unwrap();
            write!(w, "\r{CLEAR_LINE}").unwrap();
            w.flush().unwrap();
        }
    }

    /// Stop the spinner and print a green ✔ with the current message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn success(self) {
        let msg = self.message.lock().unwrap().clone();
        self.stop();
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
        self.stop();
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
        self.stop();
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
        self.stop();
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
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        if let Some(thread) = self.thread.get_mut().unwrap().take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let spinner = Spinner::with_writer_tty(msg.clone(), writer, true);
            let handle = spinner.start();
            thread::sleep(Duration::from_millis(100));
            handle.fail();

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
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
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let spinner = Spinner::with_writer_tty(original, writer, true);
            let handle = spinner.start();
            thread::sleep(Duration::from_millis(100));
            handle.fail_with(replacement.clone());

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            prop_assert!(output.contains(RED), "TTY fail_with output must contain RED ANSI code");
            prop_assert!(output.contains("✖"), "TTY fail_with output must contain ✖ symbol");
            prop_assert!(output.contains(&replacement), "TTY fail_with output must contain the replacement message");
            prop_assert!(output.contains(RESET), "TTY fail_with output must contain RESET ANSI code");
        }

        #[test]
        fn property_with_writer_tty_false_produces_plain_output(
            msg in "[^\x00]{1,50}"
        ) {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let spinner = Spinner::with_writer_tty(msg.clone(), writer, false);
            let handle = spinner.start();
            handle.success();

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            prop_assert!(!output.contains("\x1b["), "with_writer_tty(false) output must not contain ANSI codes");
            let expected = format!("✔ {}\n", msg);
            prop_assert_eq!(output, expected, "with_writer_tty(false) must produce plain text output");
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
        let handle = spinner.start();
        thread::sleep(Duration::from_millis(100));
        handle.stop();
    }

    #[test]
    fn test_drop_without_stop_joins_thread() {
        let spinner = Spinner::with_writer("test", Vec::<u8>::new());
        let handle = spinner.start();
        thread::sleep(Duration::from_millis(100));
        drop(handle);
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
}
