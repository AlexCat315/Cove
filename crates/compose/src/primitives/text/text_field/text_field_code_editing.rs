//! Translation of Primitives/Text/TextField/TextFieldCodeEditing.swift.
//!
//! Newline and block-indent editing for code-shaped fields. Both operate on the
//! bound buffer and the live selection, so they share one edit transaction.
//! Character indexes are extended grapheme clusters, as in Swift.

use cove_core::line_breaks::is_newline;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextEditKind {
    #[default]
    Insertion,
    /// One undo step for the whole batch.
    Atomic,
}

/// Returns the newline to insert at `caret`, carrying the current line's indent
/// and one extra level after an open brace or bracket.
pub fn indented_newline(
    buffer: &str,
    caret: usize,
    indentation_width: Option<usize>,
    vertical: bool,
) -> String {
    let Some(width) = indentation_width else {
        return "\n".to_string();
    };
    if !vertical {
        return "\n".to_string();
    }
    let characters: Vec<&str> = buffer.graphemes(true).collect();
    let caret = caret.min(characters.len());
    let line_start = characters[..caret]
        .iter()
        .rposition(|character| is_newline(character))
        .map_or(0, |index| index + 1);
    let line: String = characters[line_start..caret].concat();
    let indent: String = line
        .graphemes(true)
        .take_while(|character| *character == " " || *character == "\t")
        .collect();
    let last = line
        .graphemes(true)
        .rev()
        .find(|character| !character.chars().all(char::is_whitespace));
    let extra = if matches!(last, Some("{") | Some("[")) {
        " ".repeat(width)
    } else {
        String::new()
    };
    format!("\n{indent}{extra}")
}

/// Result of an indent/outdent batch.
#[derive(Clone, Debug, PartialEq)]
pub struct IndentResult {
    pub text: String,
    pub caret: usize,
    pub anchor: Option<usize>,
    pub changed: bool,
}

/// Adds or removes one indentation level on every line touched by the
/// selection. `selection` is the half-open `[low, high)` range, if any.
pub fn indent_lines(
    buffer: &str,
    caret: usize,
    anchor: Option<usize>,
    selection: Option<(usize, usize)>,
    width: usize,
    removing: bool,
    max_length: Option<usize>,
) -> IndentResult {
    let mut characters: Vec<String> = buffer.graphemes(true).map(str::to_owned).collect();
    let mut caret = caret.min(characters.len());
    let mut anchor = anchor.map(|value| value.min(characters.len()));
    let selection =
        selection.map(|(low, high)| (low.min(characters.len()), high.min(characters.len())));
    let lower = selection.map_or(caret, |(low, _)| low);
    let upper = selection.map_or(lower, |(low, high)| low.max(high.saturating_sub(1)));
    let line_index = |characters: &[String], index: usize| -> usize {
        characters[..index.min(characters.len())]
            .iter()
            .filter(|character| is_newline(character))
            .count()
    };
    let line_start = |characters: &[String], line: usize| -> usize {
        if line == 0 {
            return 0;
        }
        let mut seen = 0;
        for (index, character) in characters.iter().enumerate() {
            if is_newline(character) {
                seen += 1;
                if seen == line {
                    return index + 1;
                }
            }
        }
        characters.len()
    };
    let first = line_index(&characters, lower);
    let last = line_index(&characters, upper);
    for line in (first..=last).rev() {
        let start = line_start(&characters, line);
        let (removed, insertion): (usize, String) = if removing {
            let mut count = 0;
            while count < width
                && characters
                    .get(start + count)
                    .is_some_and(|value| value == " ")
            {
                count += 1;
            }
            if count == 0 && characters.get(start).is_some_and(|value| value == "\t") {
                count = 1;
            }
            (count, String::new())
        } else {
            let spaces = match selection {
                None => width - ((caret.saturating_sub(start)) % width),
                Some(_) => width,
            };
            (0, " ".repeat(spaces))
        };
        let offset = if !removing && selection.is_none() {
            caret
        } else {
            start
        };
        let removed_end = (offset + removed).min(characters.len());
        characters.splice(
            offset..removed_end,
            insertion.graphemes(true).map(str::to_owned),
        );
        let shift = insertion.graphemes(true).count() as isize - removed as isize;
        let move_index = |index: usize| -> usize {
            if index < offset {
                index
            } else {
                ((index as isize + shift).max(offset as isize)) as usize
            }
        };
        caret = move_index(caret);
        if let Some(value) = anchor {
            anchor = Some(move_index(value));
        }
    }
    let text: String = characters.join("");
    if let Some(max_length) = max_length
        && characters.len() > max_length
    {
        return IndentResult {
            text: buffer.to_string(),
            caret,
            anchor,
            changed: false,
        };
    }
    let changed = text != buffer;
    IndentResult {
        text,
        caret,
        anchor,
        changed,
    }
}
