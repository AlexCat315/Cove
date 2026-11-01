//! Translation of `System/Sources/CoveProcess/WindowsLaunch.swift`.
//!
//! Windows builds pass a single command line and environment block to
//! `CreateProcessW`; batch files are routed through `cmd.exe` only after
//! rejecting shell-syntax characters.
#![cfg(windows)]

use super::command::{ProcessCommand, ProcessError};
use super::tool_locator::ProcessEnvironment;
use std::collections::BTreeMap;
use std::path::Path;

pub struct WindowsLaunch {
    pub executable: String,
    pub command: String,
    pub environment: String,
    pub directory: String,
}

impl WindowsLaunch {
    pub fn new(request: &ProcessCommand) -> Result<Self, ProcessError> {
        let values: BTreeMap<String, String> = request
            .environment
            .iter()
            .map(|(key, value)| (key.to_uppercase(), value.clone()))
            .collect();
        let environment = values
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("\0")
            + "\0\0";
        let directory = native_path(&request.directory);
        let path = native_path(&request.executable);
        let extension = request
            .executable
            .extension()
            .map(|value| value.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if extension == "bat" || extension == "cmd" {
            // Batch files have shell syntax, unlike native argv. Reject
            // syntax-changing input.
            let unsafe_characters = ['"', '\r', '\n', '%', '!', '?', '^', '&', '|', '<', '>'];
            if std::iter::once(&path)
                .chain(request.arguments.iter())
                .any(|value| value.contains(unsafe_characters))
            {
                return Err(ProcessError::InvalidCommand);
            }
            let executable = ProcessEnvironment::value("ComSpec", &request.environment)
                .unwrap_or_else(|| {
                    format!(
                        "{}\\System32\\cmd.exe",
                        ProcessEnvironment::value("SystemRoot", &request.environment)
                            .unwrap_or_else(|| "C:\\Windows".into())
                    )
                });
            let command = format!(
                "{} /d /s /v:off /c \"{}\"",
                quote(&executable),
                std::iter::once(&path)
                    .chain(request.arguments.iter())
                    .map(|value| format!("\"{value}\""))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            Ok(Self {
                executable,
                command,
                environment,
                directory,
            })
        } else {
            let command = std::iter::once(&path)
                .chain(request.arguments.iter())
                .map(|value| quote(value))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(Self {
                executable: path,
                command,
                environment,
                directory,
            })
        }
    }
}

/// Backslash-separated, as programs read their own `argv[0]`: `cmd.exe` scans
/// the whole command line for switches and takes `C:/…/cmd.exe` as `/c`.
fn native_path(path: &Path) -> String {
    let mut value = path.to_string_lossy().into_owned();
    if value.starts_with('/') && value.chars().nth(2) == Some(':') {
        value.remove(0);
    }
    value.replace('/', "\\")
}

/// Microsoft CRT `argv` quoting; native arguments never pass through a shell.
/// Plain arguments stay bare: programs with their own parsers, such as
/// `cmd.exe`, do not recognise a quoted `/c`.
fn quote(argument: &str) -> String {
    if !argument.is_empty()
        && !argument
            .chars()
            .any(|character| " \t\n\u{0B}\"".contains(character))
    {
        return argument.to_owned();
    }
    let mut result = String::from("\"");
    let mut slashes = 0;
    for character in argument.chars() {
        if character == '\\' {
            slashes += 1;
            continue;
        }
        if character == '"' {
            result.push_str(&"\\".repeat(slashes * 2 + 1));
            result.push('"');
        } else {
            result.push_str(&"\\".repeat(slashes));
            result.push(character);
        }
        slashes = 0;
    }
    result.push_str(&"\\".repeat(slashes * 2));
    result.push('"');
    result
}
