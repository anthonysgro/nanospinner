use crate::shared::{format_finalize_plain, CLEAR_LINE, FRAMES, GREEN, RED, RESET};

use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

fn multi_spin_loop(
    frames: &[char],
    interval: Duration,
    stop_flag: &Arc<AtomicBool>,
    lines: &Arc<Mutex<Vec<SpinnerLine>>>,
    writer: &Arc<Mutex<Box<dyn io::Write + Send>>>,
    last_visible_count: &Arc<AtomicUsize>,
) {
    let mut frame_idx: usize = 0;
    let mut prev_line_count: usize = 0;

    while !stop_flag.load(Ordering::Acquire) {
        // 1-2-3: Lock, clone state, release.
        let snapshot = lines.lock().unwrap().clone();

        if !snapshot.is_empty() {
            let mut w = writer.lock().unwrap();

            // 4: Move cursor up to overwrite previous frame (skip on first frame).
            if prev_line_count > 0 {
                write!(w, "\x1b[{prev_line_count}A").unwrap();
            }

            // 5: Redraw each visible line.
            let frame_char = frames[frame_idx % frames.len()];
            let mut visible_count: usize = 0;
            for line in &snapshot {
                match &line.status {
                    LineStatus::Active => {
                        write!(w, "\r{}{} {}\n", CLEAR_LINE, frame_char, line.message).unwrap();
                        visible_count += 1;
                    }
                    LineStatus::Succeeded => {
                        write!(w, "\r{}{}✔{} {}\n", CLEAR_LINE, GREEN, RESET, line.message)
                            .unwrap();
                        visible_count += 1;
                    }
                    LineStatus::SucceededWith(msg) => {
                        write!(w, "\r{CLEAR_LINE}{GREEN}✔{RESET} {msg}\n").unwrap();
                        visible_count += 1;
                    }
                    LineStatus::Failed => {
                        write!(w, "\r{}{}✖{} {}\n", CLEAR_LINE, RED, RESET, line.message).unwrap();
                        visible_count += 1;
                    }
                    LineStatus::FailedWith(msg) => {
                        write!(w, "\r{CLEAR_LINE}{RED}✖{RESET} {msg}\n").unwrap();
                        visible_count += 1;
                    }
                    LineStatus::Cleared => { /* skip — no output */ }
                }
            }

            // 6: Erase vacated rows left by cleared lines.
            let vacated = prev_line_count.saturating_sub(visible_count);
            for _ in 0..vacated {
                write!(w, "\r{CLEAR_LINE}\n").unwrap();
            }
            // Move cursor back up past the vacated rows so it sits right
            // after the visible lines — keeps prev_line_count correct.
            if vacated > 0 {
                write!(w, "\x1b[{vacated}A").unwrap();
            }

            // 7: Flush the writer.
            w.flush().unwrap();
            prev_line_count = visible_count;
            last_visible_count.store(visible_count, Ordering::Relaxed);
        }

        // 6: Advance the global frame counter.
        frame_idx = frame_idx.wrapping_add(1);
        thread::sleep(interval);
    }
}

// ---------------------------------------------------------------------------
// Multi-spinner types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LineStatus {
    /// Still animating.
    Active,
    /// Finalized with success (green ✔).
    Succeeded,
    /// Finalized with a replacement success message.
    SucceededWith(String),
    /// Finalized with failure (red ✖).
    Failed,
    /// Finalized with a replacement failure message.
    FailedWith(String),
    /// Silently dismissed — produces no output.
    Cleared,
}

#[derive(Clone)]
pub(crate) struct SpinnerLine {
    pub(crate) message: String,
    pub(crate) status: LineStatus,
}

/// A builder for a multi-spinner group that manages multiple concurrent
/// spinners on separate terminal lines.
///
/// Mirrors the [`crate::Spinner`] construction pattern: call [`MultiSpinner::new`]
/// for stdout, or [`MultiSpinner::with_writer`] / [`MultiSpinner::with_writer_tty`]
/// for custom writers.
pub struct MultiSpinner<W: io::Write + Send + 'static = io::Stdout> {
    writer: W,
    is_tty: bool,
}

impl Default for MultiSpinner<io::Stdout> {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiSpinner {
    /// Create a new multi-spinner writing to stdout with automatic TTY detection.
    #[must_use]
    pub fn new() -> MultiSpinner<io::Stdout> {
        MultiSpinner {
            writer: io::stdout(),
            is_tty: io::stdout().is_terminal(),
        }
    }
}

impl<W: io::Write + Send + 'static> MultiSpinner<W> {
    /// Create a new multi-spinner with a custom writer. `is_tty` defaults to `false`.
    pub fn with_writer(writer: W) -> Self {
        MultiSpinner {
            writer,
            is_tty: false,
        }
    }

    /// Create a new multi-spinner with a custom writer and an explicit TTY flag.
    pub fn with_writer_tty(writer: W, is_tty: bool) -> Self {
        MultiSpinner { writer, is_tty }
    }

    /// Start the multi-spinner group and return a handle for managing spinners.
    ///
    /// Consumes the `MultiSpinner` builder. In plain mode (`is_tty` is false),
    /// no background thread is spawned. In TTY mode, a render-loop thread will
    /// be started (added in a later task).
    #[must_use]
    pub fn start(self) -> MultiSpinnerHandle {
        let writer: Arc<Mutex<Box<dyn io::Write + Send>>> =
            Arc::new(Mutex::new(Box::new(self.writer)));
        let lines: Arc<Mutex<Vec<SpinnerLine>>> = Arc::new(Mutex::new(Vec::new()));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let last_visible_count = Arc::new(AtomicUsize::new(0));
        let is_tty = self.is_tty;

        let thread = if is_tty {
            let t_stop = Arc::clone(&stop_flag);
            let t_lines = Arc::clone(&lines);
            let t_writer = Arc::clone(&writer);
            let t_visible = Arc::clone(&last_visible_count);

            Some(thread::spawn(move || {
                multi_spin_loop(
                    FRAMES,
                    Duration::from_millis(80),
                    &t_stop,
                    &t_lines,
                    &t_writer,
                    &t_visible,
                );
            }))
        } else {
            stop_flag.store(true, Ordering::Release);
            None
        };

        MultiSpinnerHandle {
            lines,
            writer,
            stop_flag,
            thread: Mutex::new(thread),
            is_tty,
            last_visible_count,
        }
    }
}

/// Handle returned by [`MultiSpinner::start`] for managing a running
/// multi-spinner group.
pub struct MultiSpinnerHandle {
    lines: Arc<Mutex<Vec<SpinnerLine>>>,
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
    stop_flag: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
    is_tty: bool,
    last_visible_count: Arc<AtomicUsize>,
}

/// Handle for controlling a single spinner line within a multi-spinner group.
///
/// `SpinnerLineHandle` is [`Send`] so it can be moved to worker threads.
pub struct SpinnerLineHandle {
    index: usize,
    lines: Arc<Mutex<Vec<SpinnerLine>>>,
    writer: Arc<Mutex<Box<dyn io::Write + Send>>>,
    is_tty: bool,
}

impl MultiSpinnerHandle {
    /// Add a new spinner line with the given message and return a handle to
    /// control it.
    ///
    /// In plain mode no output is produced until the returned
    /// [`SpinnerLineHandle`] is finalized.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn add(&self, message: impl Into<String>) -> SpinnerLineHandle {
        let mut lines = self.lines.lock().unwrap();
        lines.push(SpinnerLine {
            message: message.into(),
            status: LineStatus::Active,
        });
        let index = lines.len() - 1;
        SpinnerLineHandle {
            index,
            lines: Arc::clone(&self.lines),
            writer: Arc::clone(&self.writer),
            is_tty: self.is_tty,
        }
    }

    /// Stop the multi-spinner group: signal the render loop to stop, join the
    /// background thread, and finalize any still-active lines.
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
            self.render_final();
        }
    }

    fn render_final(&self) {
        if !self.is_tty {
            return;
        }
        let Ok(snapshot) = self.lines.lock().map(|g| g.clone()) else {
            return;
        };
        let visible = self.last_visible_count.load(Ordering::Relaxed);
        if visible == 0 {
            return;
        }
        let Ok(mut w) = self.writer.lock() else {
            return;
        };
        let _ = write!(w, "\x1b[{visible}A");
        let mut final_visible: usize = 0;
        for line in &snapshot {
            match &line.status {
                LineStatus::Active => {
                    let _ = write!(w, "\r{CLEAR_LINE}\n");
                    final_visible += 1;
                }
                LineStatus::Succeeded => {
                    let _ = write!(w, "\r{}{}✔{} {}\n", CLEAR_LINE, GREEN, RESET, line.message);
                    final_visible += 1;
                }
                LineStatus::SucceededWith(msg) => {
                    let _ = write!(w, "\r{CLEAR_LINE}{GREEN}✔{RESET} {msg}\n");
                    final_visible += 1;
                }
                LineStatus::Failed => {
                    let _ = write!(w, "\r{}{}✖{} {}\n", CLEAR_LINE, RED, RESET, line.message);
                    final_visible += 1;
                }
                LineStatus::FailedWith(msg) => {
                    let _ = write!(w, "\r{CLEAR_LINE}{RED}✖{RESET} {msg}\n");
                    final_visible += 1;
                }
                LineStatus::Cleared => { /* skip — no output */ }
            }
        }
        for _ in 0..visible.saturating_sub(final_visible) {
            let _ = write!(w, "\r{CLEAR_LINE}\n");
        }
        let _ = w.flush();
    }
}

impl Drop for MultiSpinnerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl SpinnerLineHandle {
    /// Update the message for this spinner line.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn update(&self, message: impl Into<String>) {
        let mut lines = self.lines.lock().unwrap();
        lines[self.index].message = message.into();
    }

    /// Finalize this spinner line with a green ✔ and the current message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn success(self) {
        let mut lines = self.lines.lock().unwrap();
        let message = lines[self.index].message.clone();
        lines[self.index].status = LineStatus::Succeeded;
        drop(lines);
        if !self.is_tty {
            let mut w = self.writer.lock().unwrap();
            write!(w, "{}", format_finalize_plain("✔", &message)).unwrap();
            w.flush().unwrap();
        }
    }

    /// Finalize this spinner line with a green ✔ and a replacement message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn success_with(self, message: impl Into<String>) {
        let msg = message.into();
        let mut lines = self.lines.lock().unwrap();
        lines[self.index].status = LineStatus::SucceededWith(msg.clone());
        drop(lines);
        if !self.is_tty {
            let mut w = self.writer.lock().unwrap();
            write!(w, "{}", format_finalize_plain("✔", &msg)).unwrap();
            w.flush().unwrap();
        }
    }

    /// Finalize this spinner line with a red ✖ and the current message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn fail(self) {
        let mut lines = self.lines.lock().unwrap();
        let message = lines[self.index].message.clone();
        lines[self.index].status = LineStatus::Failed;
        drop(lines);
        if !self.is_tty {
            let mut w = self.writer.lock().unwrap();
            write!(w, "{}", format_finalize_plain("✖", &message)).unwrap();
            w.flush().unwrap();
        }
    }

    /// Finalize this spinner line with a red ✖ and a replacement message.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn fail_with(self, message: impl Into<String>) {
        let msg = message.into();
        let mut lines = self.lines.lock().unwrap();
        lines[self.index].status = LineStatus::FailedWith(msg.clone());
        drop(lines);
        if !self.is_tty {
            let mut w = self.writer.lock().unwrap();
            write!(w, "{}", format_finalize_plain("✖", &msg)).unwrap();
            w.flush().unwrap();
        }
    }

    /// Silently dismiss this spinner line.
    ///
    /// The line disappears from the terminal on the next render frame
    /// (TTY mode) or produces no output at all (plain mode). Remaining
    /// lines collapse together with no gap.
    ///
    /// This consumes the handle, preventing further updates.
    ///
    /// # Panics
    /// Panics if the internal mutex is poisoned.
    pub fn clear(self) {
        let mut lines = self.lines.lock().unwrap();
        lines[self.index].status = LineStatus::Cleared;
        // No writer interaction — the line is silently dismissed.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn _assert_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SpinnerLineHandle>();
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

    #[test]
    fn test_multi_spinner_tty_single_spinner_renders() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer_tty(writer, true).start();
        let line = handle.add("Compiling crate");
        thread::sleep(Duration::from_millis(200));
        line.success();
        thread::sleep(Duration::from_millis(100));
        handle.stop();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();

        // Verify braille animation frames were rendered
        let braille_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        assert!(
            braille_frames.iter().any(|&c| output.contains(c)),
            "TTY output must contain braille animation frames"
        );
        // Verify green ✔ for success
        assert!(
            output.contains(GREEN),
            "TTY output must contain GREEN ANSI code"
        );
        assert!(output.contains("✔"), "TTY output must contain ✔");
        // Verify the message is present
        assert!(
            output.contains("Compiling crate"),
            "TTY output must contain the spinner message"
        );
        // Verify ANSI escape codes are present
        assert!(
            output.contains("\x1b["),
            "TTY output must contain ANSI escape codes"
        );
    }

    #[test]
    fn test_multi_spinner_tty_add_after_finalize() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer_tty(writer, true).start();

        // Add spinner A and finalize it
        let line_a = handle.add("Task A");
        thread::sleep(Duration::from_millis(200));
        line_a.success();

        // Add spinner B after A is finalized
        let line_b = handle.add("Task B");
        thread::sleep(Duration::from_millis(200));
        line_b.fail();

        thread::sleep(Duration::from_millis(100));
        handle.stop();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();

        // Both messages should appear in the output
        assert!(
            output.contains("Task A"),
            "output must contain Task A message"
        );
        assert!(
            output.contains("Task B"),
            "output must contain Task B message"
        );
        // ✔ for A (success) and ✖ for B (fail)
        assert!(output.contains("✔"), "output must contain ✔ for Task A");
        assert!(output.contains("✖"), "output must contain ✖ for Task B");
    }

    #[test]
    fn test_multi_spinner_tty_stop_joins_thread() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer_tty(writer, true).start();
        let _line = handle.add("Working");
        thread::sleep(Duration::from_millis(100));
        handle.stop();
        // The fact that we reach this point means stop() joined the thread without hanging.
    }

    #[test]
    fn test_multi_spinner_tty_drop_without_stop() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer_tty(writer, true).start();
        let _line = handle.add("Working");
        thread::sleep(Duration::from_millis(100));
        drop(handle);
        // The fact that we reach this point means drop() joined the thread without hanging.
    }

    #[test]
    fn test_multi_spinner_drop_renders_same_as_stop() {
        // Run with stop()
        let buf_stop = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf_stop));
        let handle = MultiSpinner::with_writer_tty(writer, true).start();
        let a = handle.add("Alpha");
        let b = handle.add("Beta");
        thread::sleep(Duration::from_millis(150));
        a.success_with("Alpha done.");
        b.fail_with("Beta failed.");
        thread::sleep(Duration::from_millis(100));
        handle.stop();
        let len_stop = buf_stop.lock().unwrap().len();

        // Run with drop (no stop)
        let buf_drop = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf_drop));
        let handle = MultiSpinner::with_writer_tty(writer, true).start();
        let a = handle.add("Alpha");
        let b = handle.add("Beta");
        thread::sleep(Duration::from_millis(150));
        a.success_with("Alpha done.");
        b.fail_with("Beta failed.");
        thread::sleep(Duration::from_millis(100));
        drop(handle);
        let len_drop = buf_drop.lock().unwrap().len();

        // Both should have produced final render output (not just animation).
        // Exact byte equality is fragile due to timing, but both should contain
        // the final status symbols.
        let out_stop = String::from_utf8(buf_stop.lock().unwrap().clone()).unwrap();
        let out_drop = String::from_utf8(buf_drop.lock().unwrap().clone()).unwrap();
        assert!(out_stop.contains("✔"), "stop output must contain ✔");
        assert!(out_stop.contains("✖"), "stop output must contain ✖");
        assert!(out_drop.contains("✔"), "drop output must contain ✔");
        assert!(out_drop.contains("✖"), "drop output must contain ✖");
        // Both should have written more than just animation frames
        assert!(len_stop > 0, "stop must produce output");
        assert!(len_drop > 0, "drop must produce output");
    }

    #[test]
    fn test_spinner_line_handle_send_to_thread() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer(writer).start();
        let line_handle = handle.add("Task from another thread");

        // Move the SpinnerLineHandle to another thread and finalize it there
        let t = thread::spawn(move || {
            line_handle.success();
        });
        t.join().expect("thread must not panic");

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(output, "✔ Task from another thread\n");
    }

    #[test]
    fn test_multiple_handles_finalized_from_different_threads() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer(writer).start();
        let h1 = handle.add("alpha");
        let h2 = handle.add("beta");
        let h3 = handle.add("gamma");

        // Move each handle to a different thread and finalize concurrently
        let threads: Vec<thread::JoinHandle<()>> = vec![
            thread::spawn(move || {
                h1.success();
            }),
            thread::spawn(move || {
                h2.fail();
            }),
            thread::spawn(move || {
                h3.success_with("gamma done");
            }),
        ];

        for t in threads {
            t.join()
                .expect("thread must not panic during concurrent finalization");
        }

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        let output_lines: Vec<&str> = output.split('\n').filter(|l| !l.is_empty()).collect();

        // All three lines must appear exactly once
        assert_eq!(output_lines.len(), 3, "must have exactly 3 output lines");
        assert!(
            output_lines.contains(&"✔ alpha"),
            "output must contain '✔ alpha'"
        );
        assert!(
            output_lines.contains(&"✖ beta"),
            "output must contain '✖ beta'"
        );
        assert!(
            output_lines.contains(&"✔ gamma done"),
            "output must contain '✔ gamma done'"
        );
    }

    #[test]
    fn test_stop_finalization_clear_one_among_others() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer_tty(writer, true).start();

        let line1 = handle.add("first-line");
        let line2 = handle.add("second-line");
        let line3 = handle.add("third-line");

        // Let the render loop run a few frames
        thread::sleep(Duration::from_millis(200));

        line1.success();
        line2.clear();
        line3.fail();

        // Let the render loop pick up finalized statuses
        thread::sleep(Duration::from_millis(100));

        handle.stop();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();

        // Extract the final frame: everything from the last cursor-up sequence onward
        let last_cursor_up_pos = {
            let bytes = output.as_bytes();
            let mut last_pos = None;
            for i in 0..bytes.len().saturating_sub(3) {
                if bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
                    let mut j = i + 2;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > i + 2 && j < bytes.len() && bytes[j] == b'A' {
                        last_pos = Some(i);
                    }
                }
            }
            last_pos
        };

        let final_frame = last_cursor_up_pos
            .map(|pos| &output[pos..])
            .expect("TTY output must contain at least one cursor-up sequence");

        // Cleared line must NOT appear in the final frame
        assert!(
            !final_frame.contains("second-line"),
            "cleared line 'second-line' must NOT appear in the final frame"
        );
        // Succeeded line must appear
        assert!(
            final_frame.contains("first-line"),
            "succeeded line 'first-line' must appear in the final frame"
        );
        // Failed line must appear
        assert!(
            final_frame.contains("third-line"),
            "failed line 'third-line' must appear in the final frame"
        );
        // Final frame must contain ✔ and ✖
        assert!(final_frame.contains("✔"), "final frame must contain ✔");
        assert!(final_frame.contains("✖"), "final frame must contain ✖");
    }

    #[test]
    fn test_stop_finalization_all_cleared() {
        let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
        let writer = TestWriter(Arc::clone(&buf));

        let handle = MultiSpinner::with_writer_tty(writer, true).start();

        let line1 = handle.add("alpha");
        let line2 = handle.add("beta");
        let line3 = handle.add("gamma");

        // Let the render loop run a few frames
        thread::sleep(Duration::from_millis(200));

        line1.clear();
        line2.clear();
        line3.clear();

        // Let the render loop pick up the cleared statuses
        thread::sleep(Duration::from_millis(100));

        // Capture buffer length before stop
        let len_before_stop = buf.lock().unwrap().len();

        handle.stop();

        let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        let output_after_stop = &output[len_before_stop..];

        // When all lines are cleared, last_visible_count is 0, so stop()
        // should NOT write any cursor-up escape or final redraw.
        let has_cursor_up_after_stop = {
            let bytes = output_after_stop.as_bytes();
            let mut found = false;
            for i in 0..bytes.len().saturating_sub(3) {
                if bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
                    let mut j = i + 2;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > i + 2 && j < bytes.len() && bytes[j] == b'A' {
                        found = true;
                        break;
                    }
                }
            }
            found
        };

        assert!(
            !has_cursor_up_after_stop,
            "stop() must NOT write a cursor-up escape when all lines are cleared"
        );

        // The cleared messages must not appear in any final redraw
        assert!(
            !output_after_stop.contains("alpha"),
            "cleared message 'alpha' must not appear in stop output"
        );
        assert!(
            !output_after_stop.contains("beta"),
            "cleared message 'beta' must not appear in stop output"
        );
        assert!(
            !output_after_stop.contains("gamma"),
            "cleared message 'gamma' must not appear in stop output"
        );
    }

    proptest! {
        #[test]
        fn property_add_grows_line_list(msg in ".*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();
            let line_handle = handle.add(msg.clone());

            let lines = handle.lines.lock().unwrap();
            prop_assert_eq!(lines.len(), 1, "line count must be 1 after a single add()");
            prop_assert_eq!(lines[0].message.clone(), msg, "stored message must match the input");
            prop_assert_eq!(lines[0].status.clone(), LineStatus::Active, "new line must be Active");

            drop(line_handle);
        }

        #[test]
        fn property_plain_mode_defers_output(msg in ".*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            // with_writer defaults is_tty to false, so this is plain mode
            let handle = MultiSpinner::with_writer(writer).start();
            let _line_handle = handle.add(msg);

            let output = buf.lock().unwrap();
            prop_assert_eq!(output.len(), 0, "add() in plain mode must produce zero bytes of output");
        }

        #[test]
        fn property_update_changes_message(initial in ".*", updated in ".*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();
            let line_handle = handle.add(initial);
            line_handle.update(updated.clone());

            let lines = handle.lines.lock().unwrap();
            prop_assert_eq!(lines[0].message.clone(), updated, "message must match the updated value after update()");
        }

        #[test]
        fn property_plain_mode_success_output(msg in "\\PC*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();
            let line_handle = handle.add(msg.clone());
            line_handle.success();

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let expected = format!("✔ {}\n", msg);
            prop_assert_eq!(output.clone(), expected, "success() output must be '✔ {{message}}\\n'");
            prop_assert!(!output.contains("\x1b["), "output must contain no ANSI escape codes");
            prop_assert!(!output.contains('\r'), "output must contain no carriage returns");
        }

        #[test]
        fn property_plain_mode_success_with_output(original in "\\PC*", replacement in "\\PC*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();
            let line_handle = handle.add(original);
            line_handle.success_with(replacement.clone());

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let expected = format!("✔ {}\n", replacement);
            prop_assert_eq!(output.clone(), expected, "success_with() output must be '✔ {{replacement}}\\n'");
            prop_assert!(!output.contains("\x1b["), "output must contain no ANSI escape codes");
            prop_assert!(!output.contains('\r'), "output must contain no carriage returns");
        }

        #[test]
        fn property_plain_mode_fail_output(msg in "\\PC*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();
            let line_handle = handle.add(msg.clone());
            line_handle.fail();

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let expected = format!("✖ {}\n", msg);
            prop_assert_eq!(output.clone(), expected, "fail() output must be '✖ {{message}}\\n'");
            prop_assert!(!output.contains("\x1b["), "output must contain no ANSI escape codes");
            prop_assert!(!output.contains('\r'), "output must contain no carriage returns");
        }

        #[test]
        fn property_plain_mode_fail_with_output(original in "\\PC*", replacement in "\\PC*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();
            let line_handle = handle.add(original);
            line_handle.fail_with(replacement.clone());

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let expected = format!("✖ {}\n", replacement);
            prop_assert_eq!(output.clone(), expected, "fail_with() output must be '✖ {{replacement}}\\n'");
            prop_assert!(!output.contains("\x1b["), "output must contain no ANSI escape codes");
            prop_assert!(!output.contains('\r'), "output must contain no carriage returns");
        }

        #[test]
        fn property_plain_mode_finalization_order(messages in prop::collection::vec("\\PC+", 2..8)) {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();

            // Add all spinners, collecting their handles
            let handles: Vec<SpinnerLineHandle> = messages
                .iter()
                .map(|msg| handle.add(msg.clone()))
                .collect();

            // Finalize in reverse order to prove output follows finalization order, not add order
            let reversed_messages: Vec<String> = messages.iter().rev().cloned().collect();
            for line_handle in handles.into_iter().rev() {
                line_handle.success();
            }

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let output_lines: Vec<&str> = output.split('\n').filter(|l| !l.is_empty()).collect();

            // Build expected lines in finalization (reverse) order
            let expected: Vec<String> = reversed_messages
                .iter()
                .map(|msg| format!("✔ {}", msg))
                .collect();

            prop_assert_eq!(
                output_lines.len(),
                expected.len(),
                "number of output lines must match number of finalized spinners"
            );

            for (i, (actual, exp)) in output_lines.iter().zip(expected.iter()).enumerate() {
                prop_assert_eq!(
                    *actual,
                    exp.as_str(),
                    "output line {} must match finalization order (reversed add order)",
                    i
                );
            }
        }

        #[test]
        fn property_concurrent_finalization_safety(messages in prop::collection::vec("\\PC+", 2..8)) {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();

            // Add N spinners, collecting their handles
            let line_handles: Vec<SpinnerLineHandle> = messages
                .iter()
                .map(|msg| handle.add(msg.clone()))
                .collect();

            // Move each handle to a separate thread and finalize concurrently
            let threads: Vec<thread::JoinHandle<()>> = line_handles
                .into_iter()
                .map(|lh| {
                    thread::spawn(move || {
                        lh.success();
                    })
                })
                .collect();

            // Join all threads — verify no panics occurred
            for t in threads {
                t.join().expect("thread must not panic during concurrent finalization");
            }

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let output_lines: Vec<&str> = output.split('\n').filter(|l| !l.is_empty()).collect();

            // Every finalized line must appear in output exactly once
            prop_assert_eq!(
                output_lines.len(),
                messages.len(),
                "number of output lines must equal number of finalized spinners"
            );

            for msg in &messages {
                let expected = format!("✔ {}", msg);
                let count = output_lines.iter().filter(|&&l| l == expected.as_str()).count();
                prop_assert_eq!(
                    count,
                    1,
                    "message '{}' must appear exactly once in output, found {}",
                    msg,
                    count
                );
            }
        }

        #[test]
        fn property_clear_transitions_status_to_cleared(msg in ".*") {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();
            let line_handle = handle.add(msg);
            line_handle.clear();

            let lines = handle.lines.lock().unwrap();
            prop_assert_eq!(
                lines[0].status.clone(),
                LineStatus::Cleared,
                "clear() must set status to Cleared"
            );
        }

        #[test]
        fn property_clear_produces_no_output_plain_mode(
            messages in prop::collection::vec("\\PC+", 1..=10),
            clear_flags in prop::collection::vec(any::<bool>(), 1..=10),
        ) {
            // Align lengths: use the shorter of the two vecs
            let count = messages.len().min(clear_flags.len());
            let messages = &messages[..count];
            let clear_flags = &clear_flags[..count];

            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer(writer).start();

            let handles: Vec<SpinnerLineHandle> = messages
                .iter()
                .map(|msg| handle.add(msg.clone()))
                .collect();

            // Finalize each line: clear if flag is true, success otherwise
            for (lh, &should_clear) in handles.into_iter().zip(clear_flags.iter()) {
                if should_clear {
                    lh.clear();
                } else {
                    lh.success();
                }
            }

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let output_lines: Vec<&str> = output.split('\n').filter(|l| !l.is_empty()).collect();

            // Count expected success lines (non-cleared)
            let expected_count = clear_flags.iter().filter(|&&f| !f).count();
            prop_assert_eq!(
                output_lines.len(),
                expected_count,
                "output line count must equal number of non-cleared lines"
            );

            // Every output line must be a success-formatted line for a non-cleared message
            for line in &output_lines {
                prop_assert!(
                    line.starts_with("✔ "),
                    "every output line must be a success line, got: '{}'",
                    line
                );
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn property_tty_render_loop_output(
            success_msg in "[a-zA-Z0-9 ]{1,30}",
            fail_msg in "[a-zA-Z0-9 ]{1,30}"
        ) {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add two spinner lines
            let line1 = handle.add(success_msg.clone());
            let line2 = handle.add(fail_msg.clone());

            // Sleep to allow a few render cycles (80ms interval)
            thread::sleep(Duration::from_millis(200));

            // Finalize: one success, one fail
            line1.success();
            line2.fail();

            // Sleep briefly to let the render loop pick up the finalized state
            thread::sleep(Duration::from_millis(100));

            handle.stop();

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();

            // Verify ANSI cursor-up sequences for repositioning
            let has_cursor_up = output.contains("\x1b[") && {
                // Look for \x1b[{n}A pattern
                let bytes = output.as_bytes();
                let mut found = false;
                for i in 0..bytes.len().saturating_sub(3) {
                    if bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
                        // Check if followed by digit(s) and 'A'
                        let mut j = i + 2;
                        while j < bytes.len() && bytes[j].is_ascii_digit() {
                            j += 1;
                        }
                        if j > i + 2 && j < bytes.len() && bytes[j] == b'A' {
                            found = true;
                            break;
                        }
                    }
                }
                found
            };
            prop_assert!(has_cursor_up, "TTY multi-spinner output must contain ANSI cursor-up sequences (\\x1b[{{n}}A)");

            // Verify braille animation frame characters are present
            let braille_frames = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let has_braille = braille_frames.iter().any(|&c| output.contains(c));
            prop_assert!(has_braille, "TTY multi-spinner output must contain braille animation frame characters");

            // Verify green ✔ with ANSI color codes for success-finalized lines
            prop_assert!(output.contains(GREEN), "TTY multi-spinner output must contain GREEN ANSI code for success");
            prop_assert!(output.contains("✔"), "TTY multi-spinner output must contain ✔ for success");

            // Verify red ✖ with ANSI color codes for fail-finalized lines
            prop_assert!(output.contains(RED), "TTY multi-spinner output must contain RED ANSI code for failure");
            prop_assert!(output.contains("✖"), "TTY multi-spinner output must contain ✖ for failure");

            // Verify messages are present in the output
            prop_assert!(output.contains(&success_msg), "TTY multi-spinner output must contain the success message");
            prop_assert!(output.contains(&fail_msg), "TTY multi-spinner output must contain the fail message");
        }

        #[test]
        fn property_cleared_lines_produce_no_rendered_output(
            messages in prop::collection::vec("[a-zA-Z0-9]{3,15}", 2..=5),
            clear_flags in prop::collection::vec(any::<bool>(), 2..=5),
        ) {
            // Align lengths: use the shorter of the two vecs
            let count = messages.len().min(clear_flags.len());
            let messages = &messages[..count];
            let clear_flags = &clear_flags[..count];

            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add all lines
            let handles: Vec<SpinnerLineHandle> = messages
                .iter()
                .map(|msg| handle.add(msg.clone()))
                .collect();

            // Sleep briefly to let the render loop run a few frames
            thread::sleep(Duration::from_millis(200));

            // Finalize each line: clear if flag is true, success otherwise
            for (lh, &should_clear) in handles.into_iter().zip(clear_flags.iter()) {
                if should_clear {
                    lh.clear();
                } else {
                    lh.success();
                }
            }

            // Sleep briefly to let the render loop pick up finalized statuses
            thread::sleep(Duration::from_millis(100));

            handle.stop();

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();

            // Cleared messages must NOT appear in the final stop output.
            // The stop finalization is the last redraw — we check the output
            // after the last cursor-up sequence for the final frame.
            // Find the last cursor-up sequence to isolate the final redraw.
            let last_cursor_up_pos = {
                let bytes = output.as_bytes();
                let mut last_pos = None;
                for i in 0..bytes.len().saturating_sub(3) {
                    if bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
                        let mut j = i + 2;
                        while j < bytes.len() && bytes[j].is_ascii_digit() {
                            j += 1;
                        }
                        if j > i + 2 && j < bytes.len() && bytes[j] == b'A' {
                            last_pos = Some(i);
                        }
                    }
                }
                last_pos
            };

            if let Some(pos) = last_cursor_up_pos {
                let final_frame = &output[pos..];

                // Cleared messages must not appear in the final frame
                for (i, msg) in messages.iter().enumerate() {
                    if clear_flags[i] {
                        prop_assert!(
                            !final_frame.contains(msg.as_str()),
                            "cleared message '{}' must NOT appear in the final rendered frame",
                            msg
                        );
                    }
                }

                // Non-cleared (succeeded) messages must appear in the final frame
                for (i, msg) in messages.iter().enumerate() {
                    if !clear_flags[i] {
                        prop_assert!(
                            final_frame.contains(msg.as_str()),
                            "non-cleared message '{}' must appear in the final rendered frame",
                            msg
                        );
                    }
                }
            }
        }

        #[test]
        fn property_visible_line_count_equals_total_minus_cleared(
            messages in prop::collection::vec("[a-zA-Z0-9]{3,15}", 2..=5),
            clear_flags in prop::collection::vec(any::<bool>(), 2..=5),
        ) {
            // Align lengths: use the shorter of the two vecs
            let count = messages.len().min(clear_flags.len());
            let messages = &messages[..count];
            let clear_flags = &clear_flags[..count];

            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add all lines
            let handles: Vec<SpinnerLineHandle> = messages
                .iter()
                .map(|msg| handle.add(msg.clone()))
                .collect();

            // Sleep briefly to let the render loop run a few frames
            thread::sleep(Duration::from_millis(200));

            // Finalize each line: clear if flag is true, success otherwise
            let cleared_count = clear_flags.iter().filter(|&&f| f).count();
            let expected_visible = count - cleared_count;

            for (lh, &should_clear) in handles.into_iter().zip(clear_flags.iter()) {
                if should_clear {
                    lh.clear();
                } else {
                    lh.success();
                }
            }

            // Sleep briefly to let the render loop pick up finalized statuses
            thread::sleep(Duration::from_millis(200));

            // Read last_visible_count from the handle (accessible since we're in the same module)
            let visible = handle.last_visible_count.load(Ordering::Relaxed);

            handle.stop();

            prop_assert_eq!(
                visible,
                expected_visible,
                "last_visible_count ({}) must equal total ({}) minus cleared ({})",
                visible,
                count,
                cleared_count
            );
        }
    }

    /// Helper: count non-overlapping occurrences of `needle` in `haystack`.
    fn count_occurrences(haystack: &str, needle: &str) -> usize {
        haystack.matches(needle).count()
    }

    /// Helper: find the byte position of the last cursor-up sequence (\x1b[{n}A)
    /// in the output, returning the position and the cursor-up value.
    fn find_last_cursor_up(output: &str) -> Option<(usize, usize)> {
        let bytes = output.as_bytes();
        let mut last: Option<(usize, usize)> = None;
        for i in 0..bytes.len().saturating_sub(3) {
            if bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
                let mut j = i + 2;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 2 && j < bytes.len() && bytes[j] == b'A' {
                    let n: usize = std::str::from_utf8(&bytes[i + 2..j])
                        .unwrap()
                        .parse()
                        .unwrap();
                    last = Some((i, n));
                }
            }
        }
        last
    }

    /// Helper: split output into frames by finding cursor-up sequences.
    /// Each frame starts at a cursor-up sequence and ends before the next one.
    fn find_all_frames(output: &str) -> Vec<&str> {
        let bytes = output.as_bytes();
        let mut positions: Vec<usize> = Vec::new();
        for i in 0..bytes.len().saturating_sub(3) {
            if bytes[i] == b'\x1b' && bytes[i + 1] == b'[' {
                let mut j = i + 2;
                while j < bytes.len() && bytes[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 2 && j < bytes.len() && bytes[j] == b'A' {
                    positions.push(i);
                }
            }
        }
        let mut frames = Vec::new();
        for (idx, &pos) in positions.iter().enumerate() {
            let end = if idx + 1 < positions.len() {
                positions[idx + 1]
            } else {
                output.len()
            };
            frames.push(&output[pos..end]);
        }
        frames
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn property_ghost_lines_render_loop(
            total_lines in 2usize..=8,
            clear_seed in prop::collection::vec(any::<bool>(), 2..=8),
        ) {
            // Ensure at least one line is cleared and at least one remains visible
            // (so we have a bug condition: prev_line_count > visible_count > 0)
            let count = total_lines.min(clear_seed.len());
            let clear_flags: Vec<bool> = clear_seed[..count].to_vec();
            let cleared_count = clear_flags.iter().filter(|&&f| f).count();
            let visible_count = count - cleared_count;

            // Skip cases where nothing is cleared (no bug condition) or all cleared
            // (visible_count == 0, render loop won't produce a frame to check)
            prop_assume!(cleared_count > 0 && visible_count > 0);

            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add all spinner lines
            let handles: Vec<SpinnerLineHandle> = (0..count)
                .map(|i| handle.add(format!("line-{}", i)))
                .collect();

            // Let the render loop establish prev_line_count = count (all lines visible)
            thread::sleep(Duration::from_millis(250));

            // Record buffer position before clearing
            let pos_before_clear = buf.lock().unwrap().len();

            // Clear the chosen subset of lines
            for (lh, &should_clear) in handles.into_iter().zip(clear_flags.iter()) {
                if should_clear {
                    lh.clear();
                } else {
                    lh.success();
                }
            }

            // Let the render loop run at least one frame after the clears
            thread::sleep(Duration::from_millis(250));

            handle.stop();

            let full_output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let post_clear_output = &full_output[pos_before_clear..];

            // Find ALL frames in post-clear output and check if any has enough CLEAR_LINE.
            // The frame that first renders after the clear should have cursor-up = count
            // (the prev_line_count from before the clear) and should emit CLEAR_LINE
            // for ALL count rows: visible_count content lines + vacated rows.
            //
            // On unfixed code, only visible_count CLEAR_LINE sequences appear per frame,
            // so the vacated rows are NOT erased.
            let frames = find_all_frames(post_clear_output);
            let has_frame_with_vacated_erasure = frames.iter().any(|frame| {
                let cl_count = count_occurrences(frame, CLEAR_LINE);
                // The frame must have CLEAR_LINE for visible lines + vacated rows
                cl_count >= count
            });

            let best_frame_cl = frames.iter()
                .map(|frame| count_occurrences(frame, CLEAR_LINE))
                .max()
                .unwrap_or(0);

            prop_assert!(
                has_frame_with_vacated_erasure,
                "After clearing {} of {} lines, at least one render frame must contain \
                 >= {} CLEAR_LINE sequences (visible={} + vacated={}), but best frame had {}. \
                 This confirms ghost lines are NOT erased.",
                cleared_count, count, count, visible_count, cleared_count,
                best_frame_cl
            );
        }

        #[test]
        fn property_ghost_lines_stop_path(
            total_lines in 2usize..=8,
            clear_seed in prop::collection::vec(any::<bool>(), 2..=8),
        ) {
            let count = total_lines.min(clear_seed.len());
            let clear_flags: Vec<bool> = clear_seed[..count].to_vec();
            let cleared_count = clear_flags.iter().filter(|&&f| f).count();
            let visible_count = count - cleared_count;

            // Need at least one cleared and at least one visible for the stop() path test
            prop_assume!(cleared_count > 0 && visible_count > 0);

            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add all spinner lines
            let handles: Vec<SpinnerLineHandle> = (0..count)
                .map(|i| handle.add(format!("stopline-{}", i)))
                .collect();

            // Let the render loop establish prev_line_count = count
            thread::sleep(Duration::from_millis(250));

            // Clear chosen lines right before stop — minimize time for render loop
            // to process the clear, so stop() must handle the vacated rows
            for (lh, &should_clear) in handles.into_iter().zip(clear_flags.iter()) {
                if should_clear {
                    lh.clear();
                } else {
                    lh.success();
                }
            }

            // Record position just before stop
            let pos_before_stop = buf.lock().unwrap().len();

            // Stop immediately — the render loop may or may not have processed the clear
            handle.stop();

            let full_output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let stop_output = &full_output[pos_before_stop..];

            // The stop() output should contain a cursor-up and then render visible lines
            // plus erase vacated rows. Total CLEAR_LINE in stop output should be >= cursor_up_val
            // because stop() must erase all rows it moved up over.
            //
            // On unfixed code, stop() only renders visible_count lines and doesn't erase
            // vacated rows, so CLEAR_LINE count will be < cursor_up_val when
            // last_visible_count > visible_count.

            if let Some((_last_up_pos, cursor_up_val)) = find_last_cursor_up(stop_output) {
                let stop_frame = &stop_output[_last_up_pos..];
                let clear_line_count = count_occurrences(stop_frame, CLEAR_LINE);

                // The stop frame should erase all rows it moved up over.
                prop_assert!(
                    clear_line_count >= cursor_up_val,
                    "stop() frame moved cursor up by {} but only emitted {} CLEAR_LINE sequences. \
                     Expected at least {} to erase all rows (visible={}, vacated={}). \
                     Ghost lines remain in the stop output.",
                    cursor_up_val, clear_line_count, cursor_up_val,
                    visible_count, cleared_count
                );
            }
            // If no cursor-up in stop output, the render loop already processed
            // the clear and set last_visible_count to visible_count. In that case,
            // the render loop should have erased the vacated rows (tested above).
        }
    }

    /// Helper: extract the cursor-up value from a frame string.
    /// Returns the N from \x1b[NA at the start of the frame.
    fn extract_cursor_up_value(frame: &str) -> Option<usize> {
        let bytes = frame.as_bytes();
        if bytes.len() >= 4 && bytes[0] == b'\x1b' && bytes[1] == b'[' {
            let mut j = 2;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > 2 && j < bytes.len() && bytes[j] == b'A' {
                return std::str::from_utf8(&bytes[2..j])
                    .ok()
                    .and_then(|s| s.parse().ok());
            }
        }
        None
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        fn property_preservation_render_no_clears(
            num_spinners in 1usize..=8,
        ) {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add N spinner lines (all remain Active — no clears, no finalization)
            let _handles: Vec<SpinnerLineHandle> = (0..num_spinners)
                .map(|i| handle.add(format!("preserve-{}", i)))
                .collect();

            // Let the render loop run several frames
            thread::sleep(Duration::from_millis(350));

            // Capture output from the render loop BEFORE stop
            let render_output_len = buf.lock().unwrap().len();

            // Drop handles to avoid consuming them (they stay Active)
            drop(_handles);

            handle.stop();

            let full_output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            // Only analyze render loop output (before stop), since stop() clears
            // Active lines with just CLEAR_LINE (no message content).
            let render_output = &full_output[..render_output_len];
            let frames = find_all_frames(render_output);

            // We need at least one frame with cursor-up (i.e., not the first frame)
            prop_assert!(
                frames.len() >= 2,
                "Expected at least 2 render frames for {} spinners, got {}",
                num_spinners, frames.len()
            );

            // Check frames after the first one (which has cursor-up)
            for (idx, frame) in frames.iter().enumerate().skip(1) {
                // Each frame should have cursor-up = num_spinners
                if let Some(up_val) = extract_cursor_up_value(frame) {
                    prop_assert_eq!(
                        up_val, num_spinners,
                        "Frame {} cursor-up should be {} (num_spinners), got {}",
                        idx, num_spinners, up_val
                    );
                }

                // Each frame should have exactly num_spinners CLEAR_LINE sequences
                // (one per visible line, no extras for vacated rows since none are cleared)
                let cl_count = count_occurrences(frame, CLEAR_LINE);
                prop_assert_eq!(
                    cl_count, num_spinners,
                    "Frame {} should have exactly {} CLEAR_LINE sequences (one per line), got {}",
                    idx, num_spinners, cl_count
                );

                // Each frame should contain all spinner messages
                for i in 0..num_spinners {
                    let msg = format!("preserve-{}", i);
                    prop_assert!(
                        frame.contains(&msg),
                        "Frame {} must contain message '{}' (no lines cleared)",
                        idx, msg
                    );
                }
            }
        }

        #[test]
        fn property_preservation_stop_no_clears(
            num_spinners in 1usize..=8,
            finalize_pattern in prop::collection::vec(0u8..4, 1..=8),
        ) {
            let count = num_spinners.min(finalize_pattern.len());

            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add spinner lines
            let handles: Vec<SpinnerLineHandle> = (0..count)
                .map(|i| handle.add(format!("stopkeep-{}", i)))
                .collect();

            // Let the render loop run a few frames
            thread::sleep(Duration::from_millis(250));

            // Finalize all lines with non-clear methods only
            // 0 = success, 1 = fail, 2 = success_with, 3 = fail_with
            for (lh, &pattern) in handles.into_iter().zip(finalize_pattern.iter()) {
                match pattern % 4 {
                    0 => lh.success(),
                    1 => lh.fail(),
                    2 => lh.success_with("custom-success"),
                    3 => lh.fail_with("custom-fail"),
                    _ => unreachable!(),
                }
            }

            // Let the render loop pick up finalized statuses
            thread::sleep(Duration::from_millis(100));

            // Record position before stop
            let pos_before_stop = buf.lock().unwrap().len();

            handle.stop();

            let full_output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
            let stop_output = &full_output[pos_before_stop..];

            // stop() should move cursor up by count (all lines visible, none cleared)
            if let Some((_, cursor_up_val)) = find_last_cursor_up(stop_output) {
                prop_assert_eq!(
                    cursor_up_val, count,
                    "stop() cursor-up should be {} (all lines visible, none cleared), got {}",
                    count, cursor_up_val
                );
            }

            // stop() should emit exactly count CLEAR_LINE sequences (one per visible line)
            // No extra CLEAR_LINE for vacated rows since nothing was cleared
            if let Some((last_up_pos, _)) = find_last_cursor_up(stop_output) {
                let stop_frame = &stop_output[last_up_pos..];
                let cl_count = count_occurrences(stop_frame, CLEAR_LINE);
                prop_assert_eq!(
                    cl_count, count,
                    "stop() frame should have exactly {} CLEAR_LINE sequences, got {}",
                    count, cl_count
                );
            }
        }

        #[test]
        fn property_preservation_finalized_lines_visible(
            num_spinners in 2usize..=8,
            finalize_pattern in prop::collection::vec(0u8..4, 2..=8),
        ) {
            let count = num_spinners.min(finalize_pattern.len());

            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            let writer = TestWriter(Arc::clone(&buf));

            let handle = MultiSpinner::with_writer_tty(writer, true).start();

            // Add spinner lines with unique messages
            let messages: Vec<String> = (0..count)
                .map(|i| format!("finmsg-{}", i))
                .collect();

            let handles: Vec<SpinnerLineHandle> = messages
                .iter()
                .map(|msg| handle.add(msg.clone()))
                .collect();

            // Let the render loop run a few frames
            thread::sleep(Duration::from_millis(250));

            // Finalize all lines with non-clear methods
            let patterns: Vec<u8> = finalize_pattern[..count].to_vec();
            for (lh, &pattern) in handles.into_iter().zip(patterns.iter()) {
                match pattern % 4 {
                    0 => lh.success(),
                    1 => lh.fail(),
                    2 => lh.success_with(format!("custom-{}", "ok")),
                    3 => lh.fail_with(format!("custom-{}", "err")),
                    _ => unreachable!(),
                }
            }

            // Let the render loop pick up finalized statuses
            thread::sleep(Duration::from_millis(100));

            handle.stop();

            let output = String::from_utf8(buf.lock().unwrap().clone()).unwrap();

            // Find the final frame (stop() output)
            if let Some((last_up_pos, _)) = find_last_cursor_up(&output) {
                let final_frame = &output[last_up_pos..];

                // All lines should be visible in the final frame (none cleared)
                // Check that the correct symbol appears for each finalization type
                let mut success_count = 0usize;
                let mut fail_count = 0usize;

                for &pattern in &patterns {
                    match pattern % 4 {
                        0 | 2 => success_count += 1,
                        1 | 3 => fail_count += 1,
                        _ => unreachable!(),
                    }
                }

                // Final frame must contain the right number of success/fail symbols
                let checkmark_count = count_occurrences(final_frame, "✔");
                let cross_count = count_occurrences(final_frame, "✖");

                prop_assert_eq!(
                    checkmark_count, success_count,
                    "Final frame should have {} ✔ symbols, got {}",
                    success_count, checkmark_count
                );
                prop_assert_eq!(
                    cross_count, fail_count,
                    "Final frame should have {} ✖ symbols, got {}",
                    fail_count, cross_count
                );

                // Total visible lines = count (none cleared)
                let total_symbols = checkmark_count + cross_count;
                prop_assert_eq!(
                    total_symbols, count,
                    "Final frame should have {} total finalized lines, got {}",
                    count, total_symbols
                );

                // For success/fail (not _with), original messages should appear
                for (i, &pattern) in patterns.iter().enumerate() {
                    match pattern % 4 {
                        0 | 1 => {
                            prop_assert!(
                                final_frame.contains(&messages[i]),
                                "Final frame must contain original message '{}' for line {}",
                                messages[i], i
                            );
                        }
                        2 => {
                            prop_assert!(
                                final_frame.contains("custom-ok"),
                                "Final frame must contain replacement message 'custom-ok' for success_with line {}",
                                i
                            );
                        }
                        3 => {
                            prop_assert!(
                                final_frame.contains("custom-err"),
                                "Final frame must contain replacement message 'custom-err' for fail_with line {}",
                                i
                            );
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}
