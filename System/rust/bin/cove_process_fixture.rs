//! Test fixture for `System/rust/tests/contracts.rs`.
//!
//! Mirrors `System/Tests/CoveProcessTests/cove_process_fixture`, which the Swift
//! suite spawns as a real child process so the tests exercise the OS
//! implementation rather than a mock. Each mode prints exactly what the
//! corresponding test expects and nothing more.

use std::io::{Read, Write};

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        // One line on each stream, then a clean exit.
        "output" => {
            print!("stdout");
            eprint!("stderr");
            std::io::stdout().flush().ok();
            std::io::stderr().flush().ok();
        }
        // Print `ready` without closing, then echo whatever arrives. The test
        // relies on `ready` reaching it *before* any input, which is why the
        // flush above and here both matter.
        "interactive" => {
            print!("ready");
            std::io::stdout().flush().ok();
            let mut buffer = [0u8; 64];
            loop {
                match std::io::stdin().read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        print!("{}", String::from_utf8_lossy(&buffer[..count]));
                        std::io::stdout().flush().ok();
                    }
                    Err(_) => break,
                }
            }
        }
        // Report one variable the parent injected. An absent value prints
        // nothing, so the test fails loudly rather than comparing empty strings.
        "environment" => {
            print!("{}", std::env::var("COVE_FIXTURE_VALUE").unwrap_or_default());
            std::io::stdout().flush().ok();
        }
        // Stay alive until cancelled.
        "sleep" => {
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        // Echo the received arguments as a JSON array of strings. The test looks
        // for each argument in its quoted form, so an empty argument, an embedded
        // quote and a trailing backslash all have to survive both the native
        // quoting layer and this encoding.
        "arguments" => {
            let encoded: Vec<String> = std::env::args()
                .skip(2)
                .map(|argument| {
                    let mut escaped = String::with_capacity(argument.len() + 2);
                    escaped.push('"');
                    for character in argument.chars() {
                        match character {
                            '"' => escaped.push_str("\\\""),
                            '\\' => escaped.push_str("\\\\"),
                            _ => escaped.push(character),
                        }
                    }
                    escaped.push('"');
                    escaped
                })
                .collect();
            println!("[{}]", encoded.join(","));
            std::io::stdout().flush().ok();
        }
        // An unknown mode is a programming error in the test, not a runtime case.
        other => {
            eprintln!("unknown fixture mode: {other}");
            std::process::exit(2);
        }
    }
}
