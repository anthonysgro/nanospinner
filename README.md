# ⠋ nanospinner

[![Crates.io](https://img.shields.io/crates/v/nanospinner)](https://crates.io/crates/nanospinner)
[![Docs.rs](https://docs.rs/nanospinner/badge.svg)](https://docs.rs/nanospinner/latest/nanospinner/)

A minimal, zero-dependency terminal spinner for Rust CLI applications.

![demo](demo.gif)

Inspired by the [nanospinner](https://github.com/usmanyunusov/nanospinner) npm package, `nanospinner` gives you a lightweight animated spinner using only the Rust standard library — no heavy crates, no transitive dependencies, under 200 lines of code.

## Motivation

Most Rust spinner crates (like `indicatif` or `spinoff`) are feature-rich but pull in multiple dependencies, increasing compile times and binary size. If all you need is a simple spinner with a message, a success state, and a failure state, those crates are overkill.

`nanospinner` solves this by providing the essentials and nothing more:

- Zero external dependencies (only `std`)
- Tiny footprint (< 200 LOC)
- Simple, ergonomic API
- Thread-safe with clean shutdown

## Comparison

| Crate | Dependencies | Lines of Code | Clean Build Time | Customizable Frames | Progress Bars |
|-------|-------------|---------------|------------------|---------------------|---------------|
| `nanospinner` | 0 | ~200 | ~0.1s | Default Braille set | No |
| `spinoff` | 3+ | ~1,000+ | ~1.2s | Yes (80+ sets) | No |
| `indicatif` | 5+ | ~5,000+ | ~1.4s | Yes | Yes |

Build times measured from a clean `cargo build --release` on macOS aarch64 (Apple Silicon). Your numbers may vary by platform.

`nanospinner` is for when you want a spinner and nothing else.

## Features

- Animated Braille dot spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`)
- Colored finalization: green `✔` for success, red `✖` for failure
- Update the message while the spinner is running
- Custom writer support (stdout, stderr, or any `io::Write + Send`)
- Automatic cleanup via `Drop` — no thread leaks if you forget to stop
- Automatic TTY detection — ANSI codes and animation are skipped when output is piped or redirected

## Quick Start

Add `nanospinner` to your `Cargo.toml`:

```toml
[dependencies]
nanospinner = "0.1.0"
```

```rust
use nanospinner::Spinner;
use std::thread;
use std::time::Duration;

fn main() {
    let handle = Spinner::new("Loading...").start();
    thread::sleep(Duration::from_secs(2));
    handle.success();
}
```

## Usage

### Create and start a spinner

```rust
let handle = Spinner::new("Downloading files...").start();
```

### Finalize with success or failure

```rust
handle.success();           // ✔ Downloading files...
handle.fail();              // ✖ Downloading files...
```

### Finalize with a replacement message

```rust
handle.success_with("Done!");              // ✔ Done!
handle.fail_with("Connection timed out");  // ✖ Connection timed out
```

### Update the message mid-spin

```rust
let handle = Spinner::new("Step 1...").start();
thread::sleep(Duration::from_secs(1));
handle.update("Step 2...");
thread::sleep(Duration::from_secs(1));
handle.success_with("All steps complete");
```

### Write to a custom destination

```rust
use std::io;

let handle = Spinner::with_writer("Processing...", io::stderr()).start();
thread::sleep(Duration::from_secs(1));
handle.success();
```

### Stop without a symbol

```rust
let mut handle = Spinner::new("Working...").start();
thread::sleep(Duration::from_secs(1));
handle.stop(); // clears the line, no symbol printed
```

### Piped / non-TTY output

When stdout isn't a terminal (e.g. piped to a file or another program), `nanospinner` automatically skips the animation and ANSI color codes. The final result is printed as plain text:

```bash
$ my_tool | cat
✔ Done!
```

No configuration needed — `Spinner::new()` detects this automatically. If you're using a custom writer and want to force TTY behavior, use `with_writer_tty`:

```rust
let handle = Spinner::with_writer_tty("Building...", my_writer, true).start();
```

## Contributing

Contributions are welcome. To get started:

1. Fork the repository
2. Create a feature branch (`git checkout -b my-feature`)
3. Make your changes
4. Run the tests: `cargo test`
5. Submit a pull request

Please keep changes minimal and focused. This crate's goal is to stay small and as dependency-free as possible.

## License

This project is licensed under the [MIT License](LICENSE).
