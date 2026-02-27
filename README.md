# ⠋ nanospinner [![Build Status](https://github.com/anthonysgro/nanospinner/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/anthonysgro/nanospinner/actions) [![Crates.io](https://img.shields.io/crates/v/nanospinner)](https://crates.io/crates/nanospinner) [![Docs.rs](https://docs.rs/nanospinner/badge.svg)](https://docs.rs/nanospinner/latest/nanospinner/) [![License](https://img.shields.io/crates/l/nanospinner)](https://crates.io/crates/nanospinner) [![Coverage Status](https://coveralls.io/repos/github/anthonysgro/nanospinner/badge.svg?branch=main)](https://coveralls.io/github/anthonysgro/nanospinner?branch=main)

A minimal, zero-dependency terminal spinner for Rust applications. Supports single and multi-spinner modes.

![demo](demo.gif)

Inspired by the Node.js [nanospinner](https://github.com/usmanyunusov/nanospinner) npm package, `nanospinner` gives you a lightweight animated spinner using only the Rust standard library — no heavy crates, no transitive dependencies, builds in .2 seconds.

Part of the [nano](https://github.com/anthonysgro/nano) crate family — zero-dependency building blocks for Rust.

## Motivation

Most Rust spinner crates sit at two extremes: lightweight but limited (`spinoff`), or feature-rich but heavy (`indicatif`). `nanospinner` sits in the middle: thread-safe handles, multi-spinner support, custom writers, and automatic TTY detection, all with zero dependencies and builds in under .2 seconds. If you need a spinner (not a progress bar), you probably don't need anything else.

## Comparison

| | `nanospinner` | `spinoff` | `indicatif` |
|---|---|---|---|
| Dependencies | 0 | 4 | 6 |
| Clean Build Time | ~0.2s | ~1.2s | ~1.4s |
| Customizable Frames | Default Braille set | Yes (80+ sets) | Yes |
| Multiple Spinners | Yes | No | Yes |
| Auto TTY Detection | Yes | No | Yes |
| Custom Writer | Yes (io::Write) | Stderr only | Yes (custom trait) |
| Thread-Safe Handles | Yes (`Send`) | No | Yes (`Send + Sync`) |
| Progress Bars | No | No | Yes |
| Async Support | No | No | Optional (`tokio` feature) |

Build times measured from a clean `cargo build --release` on macOS aarch64 (Apple Silicon). Your numbers may vary by platform.

`nanospinner` is for when you want a spinner and nothing else.

## Features

- Animated Braille dot spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`)
- Colored finalization: green `✔` for success, red `✖` for failure
- Update the message while the spinner is running
- Custom writer support (stdout, stderr, or any `io::Write + Send`)
- Automatic cleanup via `Drop` — renders final state and joins the background thread, even if you never call `stop()`
- Automatic TTY detection — ANSI codes and animation are skipped when output is piped or redirected
- Multi-spinner support — manage multiple concurrent spinners on separate terminal lines
- Thread-safe SpinnerLineHandle — move individual spinner controls to worker threads

## Quick Start

Add `nanospinner` to your project:

```bash
cargo add nanospinner
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

### Single Spinner

`Spinner::new(msg).start()` spawns a background thread that animates the spinner. It returns a `SpinnerHandle` you use to update or finalize the spinner. Calling `success()` or `fail()` stops the thread and prints the final line — no separate `stop()` needed. If you drop the handle without finalizing, the thread is joined and the line is cleared automatically.

#### `SpinnerHandle` methods

| Method | Effect |
|---|---|
| `update(msg)` | Change the message while spinning |
| `success()` | Stop and print `✔` with the current message |
| `success_with(msg)` | Stop and print `✔` with a replacement message |
| `fail()` | Stop and print `✖` with the current message |
| `fail_with(msg)` | Stop and print `✖` with a replacement message |
| `stop()` | Stop and clear the line (no symbol) |
| *drop* | Same as `stop()` — joins the thread, clears the line |

#### Examples

```rust
use nanospinner::Spinner;
use std::thread;
use std::time::Duration;

// Basic: start, wait, finalize
let handle = Spinner::new("Downloading...").start();
thread::sleep(Duration::from_secs(2));
handle.success(); // ✔ Downloading...

// Update mid-spin, finalize with a replacement message
let handle = Spinner::new("Step 1...").start();
thread::sleep(Duration::from_secs(1));
handle.update("Step 2...");
thread::sleep(Duration::from_secs(1));
handle.success_with("All steps complete"); // ✔ All steps complete
```

### Multi-Spinner

`MultiSpinner` manages multiple spinner lines with a single background render thread. The key difference from a single spinner: finalizing a line (`success`, `fail`, `clear`) only updates that line's status — the render thread keeps running and redraws all lines each frame. You must call `stop()` on the group handle (or let it drop) to shut down the render thread.

#### `MultiSpinnerHandle` methods

| Method | Effect |
|---|---|
| `add(msg)` | Add a spinner line, returns a `SpinnerLineHandle` |
| `stop()` | Stop the render thread and print final state |
| *drop* | Same as `stop()` |

#### `SpinnerLineHandle` methods

Each `SpinnerLineHandle` controls one line in the group. Finalizing consumes the handle, preventing double-finalization. Handles are `Send` so they can be moved to worker threads.

| Method | Effect |
|---|---|
| `update(msg)` | Change this line's message |
| `success()` | Finalize with `✔` and the current message |
| `success_with(msg)` | Finalize with `✔` and a replacement message |
| `fail()` | Finalize with `✖` and the current message |
| `fail_with(msg)` | Finalize with `✖` and a replacement message |
| `clear()` | Silently dismiss — line disappears, no output |

#### Examples

```rust
use nanospinner::MultiSpinner;
use std::thread;
use std::time::Duration;

// Basic: add lines, finalize, stop the group
let handle = MultiSpinner::new().start();

let line1 = handle.add("Downloading...");
let line2 = handle.add("Compiling...");

thread::sleep(Duration::from_secs(2));
line1.success();
line2.fail_with("Compile error");

handle.stop(); // shuts down the render thread
```

```rust
// Clear: silently dismiss lines you no longer need
let handle = MultiSpinner::new().start();

let check = handle.add("Running checks...");
let lint  = handle.add("Linting...");
let build = handle.add("Building...");

thread::sleep(Duration::from_secs(1));
lint.clear(); // line disappears, remaining lines collapse

thread::sleep(Duration::from_secs(1));
check.success();
build.success();

handle.stop();
```

```rust
// Thread-based: move line handles to worker threads
let handle = MultiSpinner::new().start();

let workers: Vec<_> = (1..=3)
    .map(|i| {
        let line = handle.add(format!("Worker {i} processing..."));
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(i));
            line.success_with(format!("Worker {i} done"));
        })
    })
    .collect();

for w in workers {
    w.join().unwrap();
}

handle.stop();
```

### Custom Writers and TTY Detection

Both `Spinner` and `MultiSpinner` auto-detect whether stdout is a terminal. When it isn't (piped, redirected), animation and ANSI codes are skipped — only plain text is printed:

```text
$ my_tool | cat
✔ Done!
```

For custom output targets, both offer `with_writer` and `with_writer_tty` constructors:

```rust
// Custom writer (defaults to non-TTY — no ANSI codes)
let handle = Spinner::with_writer("Processing...", std::io::stderr()).start();
let handle = MultiSpinner::with_writer(my_writer).start();

// Custom writer with explicit TTY control
let handle = Spinner::with_writer_tty("Building...", my_writer, true).start();
let handle = MultiSpinner::with_writer_tty(my_writer, true).start();
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
