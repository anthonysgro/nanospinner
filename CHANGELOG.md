# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.5]

### Changed

- `SpinnerHandle::stop()` and `MultiSpinnerHandle::stop()` now consume `self` instead of borrowing `&self`, consistent with `success()` and `fail()` — the compiler now prevents using a handle after stopping it
- Internal: extracted shared shutdown logic into private `shutdown(&self)` method, used by both `stop()` and `Drop`
- Restructured README: consolidated API docs, removed duplicated TTY/writer sections

## [0.2.4]

### Changed

- Trimmed README: removed and motivation section fluff

## [0.2.3]

### Changed

- `SpinnerHandle::stop()` now takes `&self` instead of `&mut self`, matching the same change made to `MultiSpinnerHandle::stop()` in v0.2.2

## [0.2.2]

### Changed

- `MultiSpinnerHandle::stop()` now takes `&self` instead of `&mut self` for consistent interior mutability API — callers no longer need `mut` bindings just to call `stop()`

## [0.2.1]

### Changed

- Updated README: revised motivation section and expanded comparison table with async support and thread-safety rows

## [0.2.0]

### Added

- `MultiSpinner` for managing multiple concurrent spinners on separate terminal lines
- `MultiSpinnerHandle` with `add()` and `stop()` methods
- `SpinnerLineHandle` for controlling individual spinner lines (`update`, `success`, `success_with`, `fail`, `fail_with`)
- `SpinnerLineHandle::clear()` to silently dismiss a spinner line — remaining lines collapse with no gap
- `MultiSpinner::with_writer()` and `MultiSpinner::with_writer_tty()` for custom output targets
- `SpinnerLineHandle` is `Send` — can be moved to worker threads
- Plain-mode (non-TTY) support for multi-spinner — skips animation, prints plain text on finalization
- New examples: `examples/multi.rs`, `examples/stress.rs`

### Fixed

- Ghost text when spinner message is updated to a shorter string (added `CLEAR_LINE` to `format_frame`)
- Ghost lines left on terminal when `clear()` reduces visible line count in TTY mode

### Changed

- Internal: added property-based tests with `proptest` across all modules

## [0.1.2]

### Changed

- Internal module split: refactored monolithic `lib.rs` into `shared.rs`, `spinner.rs`, `multi.rs` submodules
- No public API changes

## [0.1.1]

### Added

- Automatic TTY detection — ANSI codes and animation skipped when output is piped
- `Spinner::with_writer_tty()` for explicit TTY control

## [0.1.0]

### Added

- Initial release of nanospinner
- Single spinner with `Spinner::new()` and `Spinner::with_writer()`
- Background thread animation with braille dot frames (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`)
- `success()`, `fail()`, `success_with()`, `fail_with()` finalization methods
- `update()` for changing the message mid-spin
- `stop()` for clearing without a symbol
- `Drop` implementation for clean shutdown
- Zero dependencies (only `std`)

[Unreleased]: https://github.com/anthonysgro/nanospinner/compare/v0.2.5...HEAD
[0.2.5]: https://github.com/anthonysgro/nanospinner/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/anthonysgro/nanospinner/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/anthonysgro/nanospinner/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/anthonysgro/nanospinner/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/anthonysgro/nanospinner/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/anthonysgro/nanospinner/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/anthonysgro/nanospinner/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/anthonysgro/nanospinner/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/anthonysgro/nanospinner/releases/tag/v0.1.0
