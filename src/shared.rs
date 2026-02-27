// ANSI escape codes
pub(crate) const GREEN: &str = "\x1b[32m";
pub(crate) const RED: &str = "\x1b[31m";
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
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_format_finalize_plain() {
        assert_eq!(format_finalize_plain("✔", "hello"), "✔ hello\n");
        assert_eq!(format_finalize_plain("✖", "oops"), "✖ oops\n");
    }

    proptest! {
        #[test]
        fn property_frame_format_correctness(msg in ".*", idx in 0usize..1000) {
            let frame_char = FRAMES[idx % FRAMES.len()];
            let result = format_frame(frame_char, &msg);
            let expected = format!("\r{}{} {}", CLEAR_LINE, frame_char, msg);
            prop_assert_eq!(result, expected);
        }

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

        #[test]
        fn property_frame_visible_content_preserved(idx in 0usize..1000, msg in ".*") {
            let frame_char = FRAMES[idx % FRAMES.len()];
            let result = format_frame(frame_char, &msg);

            // Strip only complete ANSI escape sequences: \x1b[ followed by [0-9;]* then a letter
            let mut stripped = String::new();
            let bytes = result.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                    // Tentatively try to match a complete ANSI escape sequence
                    let start = i;
                    let mut j = i + 2; // skip \x1b[
                    // Consume parameter bytes: digits and semicolons
                    while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b';') {
                        j += 1;
                    }
                    // Check if we have a terminating letter
                    if j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                        // Complete ANSI escape — skip it entirely
                        i = j + 1;
                    } else {
                        // Incomplete/invalid sequence — keep the \x1b as literal
                        stripped.push('\x1b');
                        i = start + 1;
                    }
                } else {
                    // Handle multi-byte UTF-8 chars properly
                    if let Some(ch) = result[i..].chars().next() {
                        stripped.push(ch);
                        i += ch.len_utf8();
                    } else {
                        i += 1;
                    }
                }
            }

            // Assert the stripped visible content equals the expected format
            let expected_visible = format!("\r{frame_char} {msg}");
            prop_assert_eq!(&stripped, &expected_visible,
                "Stripped visible content must equal \\r{{frame_char}} {{msg}}");

            // Assert result contains frame_char
            prop_assert!(result.contains(frame_char),
                "format_frame output must contain frame_char '{}' but got: {:?}", frame_char, result);

            // Assert result contains the message
            prop_assert!(result.contains(msg.as_str()),
                "format_frame output must contain msg {:?} but got: {:?}", msg, result);
        }

        #[test]
        fn property_frame_contains_clear_line(idx in 0usize..1000, msg in ".*") {
            let frame_char = FRAMES[idx % FRAMES.len()];
            let result = format_frame(frame_char, &msg);

            // Assert result contains CLEAR_LINE
            prop_assert!(result.contains(CLEAR_LINE),
                "format_frame output must contain CLEAR_LINE but got: {:?}", result);

            // Assert result starts with "\r\x1b[2K"
            prop_assert!(result.starts_with(&format!("\r{CLEAR_LINE}")),
                "format_frame output must start with \\r\\x1b[2K but got: {:?}", result);

            // Assert result contains frame_char and msg
            prop_assert!(result.contains(frame_char),
                "format_frame output must contain frame_char '{}' but got: {:?}", frame_char, result);
            prop_assert!(result.contains(msg.as_str()),
                "format_frame output must contain msg {:?} but got: {:?}", msg, result);

            // Assert result equals the expected format
            let expected = format!("\r{CLEAR_LINE}{frame_char} {msg}");
            prop_assert_eq!(result, expected,
                "format_frame output must equal expected format");
        }
    }
}
