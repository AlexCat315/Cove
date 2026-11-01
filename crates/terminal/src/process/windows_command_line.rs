//! Windows process-launch encodings.
//!
//! Pure string work, kept outside the ConPTY backend so the quoting rules are
//! exercised on every platform rather than only on Windows.

/// Joins arguments the way `CommandLineToArgvW` splits them.
pub fn command_line_string(arguments: &[String]) -> String {
    let mut line = String::new();
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            line.push(' ');
        }
        if argument.is_empty()
            || argument
                .chars()
                .any(|c| matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '"'))
        {
            line.push('"');
            let mut backslashes = 0usize;
            for character in argument.chars() {
                match character {
                    '\\' => backslashes += 1,
                    '"' => {
                        // Backslashes before a quote are doubled, then the quote
                        // is escaped.
                        for _ in 0..backslashes * 2 + 1 {
                            line.push('\\');
                        }
                        line.push('"');
                        backslashes = 0;
                    }
                    _ => {
                        for _ in 0..backslashes {
                            line.push('\\');
                        }
                        line.push(character);
                        backslashes = 0;
                    }
                }
            }
            // Trailing backslashes are doubled so the closing quote is not
            // treated as escaped.
            for _ in 0..backslashes * 2 {
                line.push('\\');
            }
            line.push('"');
        } else {
            line.push_str(argument);
        }
    }
    line
}

/// `NAME=value\0...\0\0`, sorted case-insensitively as Windows expects.
pub fn environment_block_utf16(
    environment: &std::collections::BTreeMap<String, String>,
) -> Vec<u16> {
    let mut entries: Vec<(&String, &String)> = environment.iter().collect();
    entries.sort_by_key(|(key, _)| key.to_uppercase());
    let mut block = Vec::new();
    for (key, value) in entries {
        block.extend(format!("{key}={value}").encode_utf16());
        block.push(0);
    }
    block.push(0);
    block
}
