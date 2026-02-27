//! A minimal, zero-dependency terminal spinner for Rust CLI applications.
//!
//! `nanospinner` provides lightweight animated spinners for giving users
//! feedback during long-running CLI operations. Run a single spinner or
//! multiple concurrent spinners — each on a background thread so your main
//! logic stays unblocked.
//!
//! Built with only the Rust standard library — no transitive dependencies,
//! fast compile times, and a tiny binary footprint.
//!
//! # Quick start
//!
//! ```no_run
//! use nanospinner::Spinner;
//! use std::thread;
//! use std::time::Duration;
//!
//! let handle = Spinner::new("Loading...").start();
//! thread::sleep(Duration::from_secs(2));
//! handle.success();
//! ```
//!
//! # Usage
//!
//! ## Single spinner
//!
//! ### Finishing with success or failure
//!
//! Use [`SpinnerHandle::success`] for a green ✔ or [`SpinnerHandle::fail`]
//! for a red ✖. Both consume the handle and stop the animation.
//!
//! ```no_run
//! # use nanospinner::Spinner;
//! # use std::thread;
//! # use std::time::Duration;
//! let handle = Spinner::new("Deploying...").start();
//! thread::sleep(Duration::from_secs(1));
//! handle.fail(); // ✖ Deploying...
//! ```
//!
//! You can also replace the message at finalization:
//!
//! ```no_run
//! # use nanospinner::Spinner;
//! # use std::thread;
//! # use std::time::Duration;
//! let handle = Spinner::new("Compiling...").start();
//! thread::sleep(Duration::from_secs(2));
//! handle.success_with("Compiled in 2.1s"); // ✔ Compiled in 2.1s
//! ```
//!
//! ### Updating the message mid-spin
//!
//! ```no_run
//! # use nanospinner::Spinner;
//! # use std::thread;
//! # use std::time::Duration;
//! let handle = Spinner::new("Step 1...").start();
//! thread::sleep(Duration::from_secs(1));
//! handle.update("Step 2...");
//! thread::sleep(Duration::from_secs(1));
//! handle.success_with("All steps complete");
//! ```
//!
//! ### Custom writers
//!
//! Write to stderr or any [`std::io::Write`] + [`Send`] target:
//!
//! ```no_run
//! # use nanospinner::Spinner;
//! # use std::thread;
//! # use std::time::Duration;
//! let handle = Spinner::with_writer("Processing...", std::io::stderr()).start();
//! thread::sleep(Duration::from_secs(1));
//! handle.success();
//! ```
//!
//! ### TTY detection
//!
//! When stdout is not a terminal (e.g. piped to a file), `nanospinner`
//! automatically skips the animation and ANSI escape codes. The final
//! result is printed as plain text:
//!
//! ```text
//! $ my_tool | cat
//! ✔ Done!
//! ```
//!
//! For custom writers you can force TTY behavior with
//! [`Spinner::with_writer_tty`].
//!
//! ## Multiple spinners
//!
//! [`MultiSpinner`] renders several spinner lines at once, each
//! independently updatable and finalizable. Use
//! [`MultiSpinnerHandle::add`] to dynamically append new lines — even
//! after the animation has started. Lines can be finished with
//! [`SpinnerLineHandle::success`] / [`SpinnerLineHandle::fail`], or
//! silently dismissed with [`SpinnerLineHandle::clear`] — cleared lines
//! disappear and the remaining lines collapse together with no gap.
//!
//! ```no_run
//! use nanospinner::MultiSpinner;
//! use std::thread;
//! use std::time::Duration;
//!
//! let mut handle = MultiSpinner::new().start();
//!
//! let line1 = handle.add("Compiling crate A...");
//! let line2 = handle.add("Compiling crate B...");
//! let line3 = handle.add("Checking crate C...");
//!
//! thread::sleep(Duration::from_secs(2));
//! line1.success();
//! line2.fail_with("Crate B had errors.");
//! line3.clear(); // silently dismissed — no output
//!
//! handle.stop();
//! ```
//!
//! Each [`SpinnerLineHandle`] is `Send`, so you can move it to another
//! thread and finalize or clear it from there. For custom output targets
//! or explicit TTY control, see [`MultiSpinner::with_writer`] and
//! [`MultiSpinner::with_writer_tty`].
//!
//! # Features
//!
//! - Zero dependencies — only `std`
//! - Braille-dot animation (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`) on a single line
//! - Multiple concurrent spinners via [`MultiSpinner`] — each line
//!   independently updatable and finalizable, or silently dismissible
//!   via [`SpinnerLineHandle::clear`]
//! - Update the message while spinning via [`SpinnerHandle::update`]
//! - Finish with [`SpinnerHandle::success`] (✔) or [`SpinnerHandle::fail`] (✖)
//! - Replacement messages via [`SpinnerHandle::success_with`] / [`SpinnerHandle::fail_with`]
//! - Pluggable writer for testing or custom output targets
//! - Automatic TTY detection — ANSI codes and animation are skipped when
//!   output is piped or redirected
//! - Clean shutdown via [`Drop`] — no thread leaks if you forget to stop

mod multi;
mod shared;
mod spinner;

pub use multi::{MultiSpinner, MultiSpinnerHandle, SpinnerLineHandle};
pub use spinner::{Spinner, SpinnerHandle};
