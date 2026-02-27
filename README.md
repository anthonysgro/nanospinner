# ⠋ nanospinner [![Build Status](https://github.com/anthonysgro/nanospinner/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/anthonysgro/nanospinner/actions) [![Crates.io](https://img.shields.io/crates/v/nanospinner)](https://crates.io/crates/nanospinner) [![Docs.rs](https://docs.rs/nanospinner/badge.svg)](https://docs.rs/nanospinner/latest/nanospinner/) [![License](https://img.shields.io/crates/l/nanospinner)](https://crates.io/crates/nanospinner) [![Coverage Status](https://coveralls.io/repos/github/anthonysgro/nanospinner/badge.svg?branch=main)](https://coveralls.io/github/anthonysgro/nanospinner?branch=main)

A minimal, zero-dependency terminal spinner for Rust applications. Supports single and multi-spinner modes.

![demo](demo.gif)

Inspired by the [nanospinner](https://github.com/usmanyunusov/nanospinner) npm package, `nanospinner` gives you a lightweight animated spinner using only the Rust standard library — no heavy crates, no transitive dependencies, under 700 lines of code.

Part of the [nano](https://github.com/anthonysgro/nano) crate family — zero-dependency building blocks for Rust.

## Motivation

Most Rust spinner crates (like `indicatif` or `spinoff`) are feature-rich but pull in multiple dependencies, increasing compile times and binary size. If all you need is a simple spinner with a message, a success state, and a failure state, those crates are overkill.

`nanospinner` solves this by providing the essentials and nothing more:

- Zero external dependencies (only `std`)
- Simple, ergonomic API
- Thread-safe with clean shutdown

## Comparison

| | `nanospinner` | `spinoff` | `indicatif` |
|---|---|---|---|
| Dependencies | 0 | 4 | 6 |
| Clean Build Time | ~0.2s | ~1.2s | ~1.4s |
| Customizable Frames | Default Braille set | Yes (80+ sets) | Yes |
| Multiple Spinners | Yes | No | Yes |
| Auto TTY Detection | Yes | No | Yes |
| Custom Writer | Yes (io::Write) | Stderr only | Yes (custom trait) |
| Progress Bars | No | No | Yes |
| Async Support | No | No | Optional (`tokio` feature) |

Build times measured from a clean `cargo build --release` on macOS aarch64 (Apple Silicon). Your numbers may vary by platform.

`nanospinner` is for when you want a spinner and nothing else.

## Features

- Animated Braille dot spinner (`⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏`)
- Colored finalization: green `✔` for success, red `✖` for failure
- Update the message while the spinner is running
- Custom writer support (stdout, stderr, or any `io::Write + Send`)
- Automatic cleanup via `Drop` — no thread leaks if you forget to stop
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

#### Create and start a spinner

```rust
let handle = Spinner::new("Downloading files...").start();
```

#### Finalize with success or failure

```rust
handle.success();           // ✔ Downloading files...
handle.fail();              // ✖ Downloading files...
```

#### Finalize with a replacement message

```rust
handle.success_with("Done!");              // ✔ Done!
handle.fail_with("Connection timed out");  // ✖ Connection timed out
```

#### Update the message mid-spin

```rust
let handle = Spinner::new("Step 1...").start();
thread::sleep(Duration::from_secs(1));
handle.update("Step 2...");
thread::sleep(Duration::from_secs(1));
handle.success_with("All steps complete");
```

#### Write to a custom destination

```rust
use std::io;

let handle = Spinner::with_writer("Processing...", io::stderr()).start();
thread::sleep(Duration::from_secs(1));
handle.success();
```

#### Stop without a symbol

```rust
let mut handle = Spinner::new("Working...").start();
thread::sleep(Duration::from_secs(1));
handle.stop(); // clears the line, no symbol printed
```

#### Piped / non-TTY output

When stdout isn't a terminal (e.g. piped to a file or another program), `nanospinner` automatically skips the animation and ANSI color codes. The final result is printed as plain text:

```bash
$ my_tool | cat
✔ Done!
```

No configuration needed — `Spinner::new()` detects this automatically. If you're using a custom writer and want to force TTY behavior, use `with_writer_tty`:

```rust
let handle = Spinner::with_writer_tty("Building...", my_writer, true).start();
```

### Multi-Spinner

For concurrent tasks, `MultiSpinner` manages multiple spinners on separate terminal lines with a single background render thread.

#### Basic usage

```rust
use nanospinner::MultiSpinner;
use std::thread;
use std::time::Duration;

let mut handle = MultiSpinner::new().start();

let line1 = handle.add("Downloading...");
let line2 = handle.add("Compiling...");

thread::sleep(Duration::from_secs(2));
line1.success();
line2.success_with("Compiled successfully!");

handle.stop();
```

#### Update and finalize individual spinners

```rust
let line = handle.add("Processing...");
line.update("Processing (50%)...");

// Finalize with success or failure
line.success();              // ✔ Processing (50%)...
line.success_with("Done!");  // ✔ Done!
line.fail();                 // ✖ Processing (50%)...
line.fail_with("Error");     // ✖ Error

// Or silently dismiss the line
line.clear();                // (line disappears, no output)
```

#### Dismiss a line with clear

Use `clear()` to silently remove a spinner line without printing any symbol or message. Remaining lines collapse together with no gap.

```rust
use nanospinner::MultiSpinner;
use std::thread;
use std::time::Duration;

let mut handle = MultiSpinner::new().start();

let line1 = handle.add("Checking cache...");
let line2 = handle.add("Downloading...");
let line3 = handle.add("Compiling...");

thread::sleep(Duration::from_secs(1));
line1.clear(); // cache check done — dismiss silently

thread::sleep(Duration::from_secs(1));
line2.success_with("Downloaded!");
line3.success();

handle.stop();
// Only the downloaded/compiled lines appear in the final output
```

#### Thread-based usage

`SpinnerLineHandle` is `Send`, so you can move it to worker threads:

```rust
use nanospinner::MultiSpinner;
use std::thread;
use std::time::Duration;

let mut handle = MultiSpinner::new().start();

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

#### Piped / non-TTY output

When stdout isn't a terminal, `MultiSpinner` skips animation and the render thread entirely. Each spinner prints a single plain-text result line when finalized:

```bash
$ my_tool | cat
✔ Task 1 complete
✔ Task 2 complete
✖ Task 3 failed
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
