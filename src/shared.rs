// ANSI escape codes
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const RED: &str = "\x1b[31m";
pub(crate) const YELLOW: &str = "\x1b[33m";
pub(crate) const BLUE: &str = "\x1b[34m";
pub(crate) const RESET: &str = "\x1b[0m";
pub(crate) const CLEAR_LINE: &str = "\x1b[2K";

// Default spinner character set (Braille dots)
pub(crate) const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub(crate) fn format_frame(frame_char: char, message: &str) -> String {
    format!("\r{CLEAR_LINE}{frame_char} {message}")
}

pub(crate) fn format_finalize(symbol: &str, color: &str, message: &str) -> String {
    format!("\r{CLEAR_LINE}{color}{symbol}{RESET} {message}\n")
}

pub(crate) fn format_finalize_plain(symbol: &str, message: &str) -> String {
    format!("{symbol} {message}\n")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io;
    use std::sync::{Arc, Mutex};

    /// Shared test helper — a cloneable Write target backed by an Arc<Mutex<Vec<u8>>>.
    /// Defined once, used by all test modules.
    #[derive(Clone)]
    pub(crate) struct TestWriter(pub(crate) Arc<Mutex<Vec<u8>>>);

    impl TestWriter {
        pub(crate) fn new() -> (Self, Arc<Mutex<Vec<u8>>>) {
            let buf = Arc::new(Mutex::new(Vec::<u8>::new()));
            (TestWriter(Arc::clone(&buf)), buf)
        }

        pub(crate) fn output(&self) -> String {
            String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
        }
    }

    impl io::Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.lock().unwrap().flush()
        }
    }

    #[test]
    fn test_yellow_constant() {
        assert_eq!(YELLOW, "\x1b[33m");
    }

    #[test]
    fn test_blue_constant() {
        assert_eq!(BLUE, "\x1b[34m");
    }

    proptest! {
        #[test]
        fn property_format_frame_exact(msg in ".*", idx in 0usize..1000) {
            let frame_char = FRAMES[idx % FRAMES.len()];
            let result = format_frame(frame_char, &msg);
            let expected = format!("\r{CLEAR_LINE}{frame_char} {msg}");
            prop_assert_eq!(result, expected);
        }

        #[test]
        fn property_format_finalize_all_variants(msg in ".*") {
            // TTY mode — all 4 symbols with exact string equality
            let success_tty = format_finalize("✔", GREEN, &msg);
            prop_assert_eq!(success_tty, format!("\r{CLEAR_LINE}{GREEN}✔{RESET} {msg}\n"));

            let fail_tty = format_finalize("✖", RED, &msg);
            prop_assert_eq!(fail_tty, format!("\r{CLEAR_LINE}{RED}✖{RESET} {msg}\n"));

            let warn_tty = format_finalize("⚠", YELLOW, &msg);
            prop_assert_eq!(warn_tty, format!("\r{CLEAR_LINE}{YELLOW}⚠{RESET} {msg}\n"));

            let info_tty = format_finalize("ℹ", BLUE, &msg);
            prop_assert_eq!(info_tty, format!("\r{CLEAR_LINE}{BLUE}ℹ{RESET} {msg}\n"));

            // Plain mode — all 4 symbols with exact string equality
            prop_assert_eq!(format_finalize_plain("✔", &msg), format!("✔ {msg}\n"));
            prop_assert_eq!(format_finalize_plain("✖", &msg), format!("✖ {msg}\n"));
            prop_assert_eq!(format_finalize_plain("⚠", &msg), format!("⚠ {msg}\n"));
            prop_assert_eq!(format_finalize_plain("ℹ", &msg), format!("ℹ {msg}\n"));
        }

        #[test]
        fn property_plain_finalization_warn_info_format(msg in "[^\x1b]*") {
            // Warn plain finalization
            let warn_output = format_finalize_plain("⚠", &msg);
            let expected_warn = format!("⚠ {}\n", msg);
            prop_assert!(!warn_output.contains('\x1b'),
                "warn plain output must not contain ANSI escape sequences");
            prop_assert_eq!(warn_output, expected_warn,
                "warn plain output must match expected format");

            // Info plain finalization
            let info_output = format_finalize_plain("ℹ", &msg);
            let expected_info = format!("ℹ {}\n", msg);
            prop_assert!(!info_output.contains('\x1b'),
                "info plain output must not contain ANSI escape sequences");
            prop_assert_eq!(info_output, expected_info,
                "info plain output must match expected format");
        }
    }
}
