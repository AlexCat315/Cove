//! Finds plain-text URLs on a screen row, for programs that print links without
//! OSC 8 (compilers, `git`, `ls`, logs...).

use crate::engine::{CellWidth, TerminalFrame};
use crate::view::link_opener;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UrlMatch {
    pub row: usize,
    /// Inclusive first and last columns the URL covers.
    pub columns: (usize, usize),
    pub url: String,
}

/// The URL covering `column` on `row`, if any.
pub fn find(frame: &TerminalFrame, row: usize, column: usize) -> Option<UrlMatch> {
    if row >= frame.rows || column >= frame.columns {
        return None;
    }
    // Rebuild the row as text, remembering which column each char came from, so a
    // wide glyph maps back to its own column.
    let mut text = String::with_capacity(frame.columns);
    let mut column_of_char: Vec<usize> = Vec::with_capacity(frame.columns);
    for (index, cell) in frame.row(row).iter().enumerate() {
        if cell.width == CellWidth::Spacer {
            continue;
        }
        let piece = if cell.text.is_empty() {
            " "
        } else {
            cell.text.as_str()
        };
        for _ in piece.chars() {
            column_of_char.push(index);
        }
        text.push_str(piece);
    }

    for (start, end) in url_spans(&text) {
        let first = *column_of_char.get(start)?;
        let last_char = end.saturating_sub(1);
        let last = *column_of_char.get(last_char)?;
        let last_cell = frame.cell(row, last);
        let last_column = if last_cell.width == CellWidth::Wide {
            (last + 1).min(frame.columns - 1)
        } else {
            last
        };
        if !(first..=last_column).contains(&column) {
            continue;
        }
        let link = &text[start..end];
        let candidate = if link.to_ascii_lowercase().starts_with("www.") {
            format!("https://{link}")
        } else {
            link.to_owned()
        };
        if let Some(url) = link_opener::url(&candidate) {
            return Some(UrlMatch {
                row,
                columns: (first, last_column),
                url,
            });
        }
    }
    None
}

/// Byte ranges of URL-shaped runs in `text`, with sentence punctuation trimmed.
///
/// This mirrors the Swift regex `(?:(?:https?|ftp|file)://|mailto:|www\\.)[^\\s<>"'`{}|\\\\^]+`
/// without pulling a regex engine into the crate hot path.
fn url_spans(text: &str) -> Vec<(usize, usize)> {
    const PREFIXES: [&str; 6] = [
        "https://", "http://", "ftp://", "file://", "mailto:", "www.",
    ];
    let lower = text.to_ascii_lowercase();
    let mut spans = Vec::new();
    let mut index = 0usize;
    while index < text.len() {
        // A prefix must start on a char boundary, which every ASCII prefix does.
        let Some((offset, prefix_len)) = PREFIXES
            .iter()
            .filter_map(|prefix| {
                lower[index..]
                    .find(prefix)
                    .map(|offset| (offset, prefix.len()))
            })
            .min_by_key(|(offset, _)| *offset)
        else {
            break;
        };
        let start = index + offset;
        let mut end = start + prefix_len;
        for (offset, character) in text[end..].char_indices() {
            if !is_url_char(character) {
                break;
            }
            end = start + prefix_len + offset + character.len_utf8();
        }
        let end = trim_trailing(text, start, end);
        if end > start + prefix_len {
            spans.push((start, end));
            index = end;
        } else {
            index = start + prefix_len;
        }
    }
    spans
}

fn is_url_char(character: char) -> bool {
    !matches!(
        character,
        ' ' | '\t' | '\n' | '\r' | '<' | '>' | '"' | '\'' | '`' | '{' | '}' | '|' | '\\' | '^'
    ) && !character.is_control()
}

/// Drops sentence punctuation after a URL, keeping a `)` that closes a `(` inside
/// it (`https://en.wikipedia.org/wiki/Foo_(bar)`).
fn trim_trailing(text: &str, start: usize, end: usize) -> usize {
    let candidate = &text[start..end];
    let trailing = ['.', ',', ';', ':', '!', '?', '\'', '"', ')', ']', '}', '>'];
    let mut keep = candidate;
    loop {
        let Some(last) = keep.chars().next_back() else {
            break;
        };
        if !trailing.contains(&last) {
            break;
        }
        if last == ')' {
            let opens = keep.matches('(').count();
            let closes = keep.matches(')').count();
            if opens >= closes {
                break;
            }
        }
        keep = &keep[..keep.len() - last.len_utf8()];
    }
    start + keep.len()
}
