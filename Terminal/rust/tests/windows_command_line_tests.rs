//! Translation of `Terminal/Tests/CoveUITerminalTests/WindowsCommandLineTests.swift`.
//!
//! Behaviour checklist:
//! - arguments round-trip through `CommandLineToArgvW` quoting rules
//! - the environment block is sorted case-insensitively and double-terminated
//!
//! The ConPTY backend only builds on Windows, but both encodings are pure string
//! work kept outside it, so — like the Swift suite — these run on every
//! platform. No `#[cfg]` gate here: nothing below needs a Windows runtime.

use std::collections::BTreeMap;

use cove_terminal::process::{command_line_string, environment_block_utf16};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn arguments_round_trip_through_command_line_to_argv_w_quoting_rules() {
    assert_eq!(
        command_line_string(&args(&["C:\\Program Files\\pwsh.exe", "-NoLogo"])),
        "\"C:\\Program Files\\pwsh.exe\" -NoLogo",
        "a path with a space is quoted, a plain flag is not"
    );
    assert_eq!(
        command_line_string(&args(&["echo", ""])),
        "echo \"\"",
        "an empty argument still gets a pair of quotes"
    );
    assert_eq!(
        command_line_string(&args(&["say \"hi\""])),
        "\"say \\\"hi\\\"\"",
        "embedded quotes are escaped"
    );
    assert_eq!(
        command_line_string(&args(&["a\\\"b"])),
        "\"a\\\\\\\"b\"",
        "the backslash before a quote is doubled, then the quote is escaped"
    );
    assert_eq!(
        command_line_string(&args(&["dir with space\\"])),
        "\"dir with space\\\\\"",
        "trailing backslashes are doubled so the closing quote is not escaped"
    );
    assert_eq!(
        command_line_string(&args(&["C:\\no\\spaces\\"])),
        "C:\\no\\spaces\\",
        "an argument needing no quoting is passed through untouched, trailing backslash included"
    );
}

#[test]
fn the_environment_block_is_sorted_case_insensitively_and_double_terminated() {
    let environment = BTreeMap::from([
        ("path".to_owned(), "C:\\".to_owned()),
        ("Apple".to_owned(), "1".to_owned()),
        ("TERM".to_owned(), "xterm".to_owned()),
    ]);

    let text = String::from_utf16(&environment_block_utf16(&environment))
        .expect("the block is valid UTF-16");

    assert_eq!(
        text, "Apple=1\0path=C:\\\0TERM=xterm\0\0",
        "entries are ordered by upper-cased name and the block ends with two NULs"
    );
}
