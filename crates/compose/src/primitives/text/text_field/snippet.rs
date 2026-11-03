//! Translation of Primitives/Text/TextField/Snippet.swift.
//!
//! LSP snippet expansion and its tab-stop session. Snippet syntax: `$1`,
//! `${1:placeholder}`, `${1|one,two|}` (the first choice is inserted) and `$0` for
//! the final caret. `\$`, `\}` and `\\` escape. Variables such as `$TM_FILENAME`
//! insert nothing.
use std::cell::RefCell;
use std::rc::Rc;

use crate::primitives::text::text_buffer::TextBuffer;
use cove_core::scene::{Node, NodeResource};
use cove_platform::KeyEvent;

pub use crate::primitives::text::text_completion_rebase::TextInsertFormat;

/// One visited snippet position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSnippetStop {
    pub number: usize,
    /// Character offsets inside the expanded text.
    pub start: usize,
    pub end: usize,
}

/// Expanded snippet text and its tab stops, in visiting order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TextSnippet {
    pub text: String,
    pub stops: Vec<TextSnippetStop>,
}

impl TextSnippet {
    /// Expand `template`, recording the first occurrence of each numbered stop.
    pub fn new(template: &str) -> Self {
        let characters: Vec<char> = template.chars().collect();
        let mut output = String::new();
        let mut length = 0usize;
        let mut found: std::collections::HashMap<usize, (usize, usize)> =
            std::collections::HashMap::new();
        let mut index = 0usize;

        fn read_number(characters: &[char], index: &mut usize) -> Option<usize> {
            let mut digits = String::new();
            while *index < characters.len() && characters[*index].is_ascii_digit() {
                digits.push(characters[*index]);
                *index += 1;
            }
            digits.parse().ok()
        }

        // Nested parsing needs to append into a scratch buffer; the parser keeps
        // one shared `output`/`length` pair and saves/restores around nested
        // stops, exactly as the Swift does.
        fn read_placeholder(
            characters: &[char],
            index: &mut usize,
            output: &mut String,
            length: &mut usize,
            found: &mut std::collections::HashMap<usize, (usize, usize)>,
        ) -> String {
            let mut text = String::new();
            while *index < characters.len() {
                let character = characters[*index];
                if character == '\\' && *index + 1 < characters.len() {
                    text.push(characters[*index + 1]);
                    *index += 2;
                    continue;
                }
                if character == '}' {
                    *index += 1;
                    return text;
                }
                if character == '$' {
                    let start = *length + text.len();
                    let saved = std::mem::take(output);
                    let saved_length = *length;
                    *length = start;
                    parse_dollar(characters, index, output, length, found);
                    text.push_str(output);
                    *output = saved;
                    *length = saved_length;
                    continue;
                }
                text.push(character);
                *index += 1;
            }
            text
        }

        // Mirrors Swift's `parseDollar()` exactly: bare `$N` records an empty
        // stop; `${N:placeholder}` and `${N|a,b|}` insert text; `${VAR:default}`
        // keeps only the default; anything else emits a literal `$`.
        fn parse_dollar(
            characters: &[char],
            index: &mut usize,
            output: &mut String,
            length: &mut usize,
            found: &mut std::collections::HashMap<usize, (usize, usize)>,
        ) {
            *index += 1;
            if *index < characters.len() && characters[*index].is_ascii_digit() {
                if let Some(number) = read_number(characters, index) {
                    let start = *length;
                    found.entry(number).or_insert((start, start));
                }
                return;
            }
            if *index >= characters.len() || characters[*index] != '{' {
                if *index < characters.len()
                    && (characters[*index].is_alphabetic() || characters[*index] == '_')
                {
                    while *index < characters.len()
                        && (characters[*index].is_alphabetic() || characters[*index] == '_')
                    {
                        *index += 1;
                    }
                    return;
                }
                output.push('$');
                *length += 1;
                return;
            }
            *index += 1;
            if *index < characters.len() && characters[*index].is_ascii_digit() {
                let Some(number) = read_number(characters, index) else {
                    return;
                };
                let start = *length;
                if *index < characters.len() && characters[*index] == ':' {
                    *index += 1;
                    let placeholder = read_placeholder(characters, index, output, length, found);
                    output.push_str(&placeholder);
                    *length += placeholder.len();
                    found.entry(number).or_insert((start, *length));
                } else if *index < characters.len() && characters[*index] == '|' {
                    *index += 1;
                    // Scan the whole choice list, remembering the first choice;
                    // escapes are honoured. Then skip the closing `|}`.
                    let mut choice = String::new();
                    let mut first: Option<String> = None;
                    while *index < characters.len() && characters[*index] != '|' {
                        if characters[*index] == ',' && first.is_none() {
                            first = Some(choice.clone());
                        }
                        if characters[*index] == '\\' && *index + 1 < characters.len() {
                            choice.push(characters[*index + 1]);
                            *index += 2;
                            continue;
                        }
                        choice.push(characters[*index]);
                        *index += 1;
                    }
                    *index = (*index + 2).min(characters.len());
                    let inserted = first.unwrap_or(choice);
                    output.push_str(&inserted);
                    *length += inserted.len();
                    found.entry(number).or_insert((start, *length));
                } else {
                    if *index < characters.len() && characters[*index] == '}' {
                        *index += 1;
                    }
                    found.entry(number).or_insert((start, start));
                }
                return;
            }
            // `${VARIABLE}` or `${VARIABLE:default}`: keep the default only.
            while *index < characters.len()
                && (characters[*index].is_alphabetic() || characters[*index] == '_')
            {
                *index += 1;
            }
            if *index < characters.len() && characters[*index] == ':' {
                *index += 1;
                let placeholder = read_placeholder(characters, index, output, length, found);
                output.push_str(&placeholder);
                *length += placeholder.len();
            } else if *index < characters.len() && characters[*index] == '}' {
                *index += 1;
            }
        }

        while index < characters.len() {
            let character = characters[index];
            if character == '\\'
                && index + 1 < characters.len()
                && matches!(characters[index + 1], '$' | '}' | '\\')
            {
                output.push(characters[index + 1]);
                length += characters[index + 1].len_utf8();
                index += 2;
                continue;
            }
            if character == '$' {
                parse_dollar(
                    &characters,
                    &mut index,
                    &mut output,
                    &mut length,
                    &mut found,
                );
                continue;
            }
            output.push(character);
            length += character.len_utf8();
            index += 1;
        }

        let mut numbered: Vec<usize> = found.keys().copied().filter(|number| *number > 0).collect();
        // Parse syntax in scalars, retain positions in bytes, then map the
        // completed expansion once. Graphemes can join across placeholders.
        let buffer = TextBuffer::new(&output);
        let range = |start, end| {
            let mut lower = buffer.character_index_for_utf8_offset(start);
            if start != end && buffer.utf8_offset_for_character_index(lower) > start {
                lower = lower.saturating_sub(1);
            }
            (lower, buffer.character_index_for_utf8_offset(end))
        };
        numbered.sort_unstable();
        let mut stops: Vec<TextSnippetStop> = numbered
            .into_iter()
            .map(|number| {
                let (start, end) = range(found[&number].0, found[&number].1);
                TextSnippetStop { number, start, end }
            })
            .collect();
        let (start, end) = found.get(&0).copied().unwrap_or((length, length));
        let (start, end) = range(start, end);
        stops.push(TextSnippetStop {
            number: 0,
            start,
            end,
        });
        Self {
            text: output,
            stops,
        }
    }
}

/// Active snippet tab stops in buffer character offsets. Edits inside the current
/// stop grow or shrink it and shift the later stops; any other edit, Escape or
/// leaving the final stop ends the session.
#[derive(Default)]
pub struct TextSnippetSession {
    stops: RefCell<Vec<(usize, usize)>>,
    current: RefCell<usize>,
    buffer: RefCell<Option<TextBuffer>>,
}

impl TextSnippetSession {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    pub fn is_active(&self) -> bool {
        self.buffer.borrow().is_some()
    }

    /// Start after an insertion; returns the first stop to select.
    pub fn start(
        &self,
        snippet: &TextSnippet,
        offset: usize,
        buffer: &TextBuffer,
    ) -> Option<(usize, usize)> {
        let stops: Vec<(usize, usize)> = snippet
            .stops
            .iter()
            .map(|stop| (stop.start + offset, stop.end + offset))
            .collect();
        *self.current.borrow_mut() = 0;
        // A lone final stop is just a caret placement.
        if stops.len() <= 1 {
            let only = stops.first().copied();
            self.end();
            return only;
        }
        *self.stops.borrow_mut() = stops.clone();
        *self.buffer.borrow_mut() = Some(buffer.clone());
        stops.first().copied()
    }

    pub fn end(&self) {
        self.stops.borrow_mut().clear();
        *self.current.borrow_mut() = 0;
        *self.buffer.borrow_mut() = None;
    }

    /// Tab moves forward, Shift+Tab back, Escape ends. `Some(Some(stop))` selects
    /// a stop, `Some(None)` means the session ended, `None` is not a snippet key.
    pub fn command(&self, event: &KeyEvent, next: &TextBuffer) -> Option<Option<(usize, usize)>> {
        if !self.is_active() {
            return None;
        }
        if event.modifiers.has_gui() || event.modifiers.has_ctrl() || event.modifiers.has_alt() {
            return None;
        }
        if event.scancode != crate::Scancode::TAB && event.scancode != crate::Scancode::ESCAPE {
            return None;
        }
        if !self.synchronize(next) {
            self.end();
            return None;
        }
        if event.scancode == crate::Scancode::ESCAPE {
            self.end();
            return Some(None);
        }
        let mut current = *self.current.borrow();
        if event.modifiers.has_shift() {
            current = current.saturating_sub(1);
        } else {
            current += 1;
            let stops = self.stops.borrow();
            if current >= stops.len().saturating_sub(1) {
                let final_stop = stops.last().copied();
                drop(stops);
                self.end();
                return Some(final_stop);
            }
        }
        *self.current.borrow_mut() = current;
        Some(self.stops.borrow().get(current).copied())
    }

    fn synchronize(&self, next: &TextBuffer) -> bool {
        let previous = { self.buffer.borrow().clone() };
        let Some(previous) = previous else {
            return false;
        };
        *self.buffer.borrow_mut() = Some(next.clone());
        if previous.has_same_contents(next) {
            return true;
        }
        let Some(delta) = next.edit_delta(&previous) else {
            return true;
        };
        let current = *self.current.borrow();
        let stop = {
            let stops = self.stops.borrow();
            match stops.get(current).copied() {
                Some(stop) => stop,
                None => return false,
            }
        };
        if delta.character_range.start < stop.0 || delta.character_range.end > stop.1 {
            return false;
        }
        let shift = next.character_count() as isize - previous.character_count() as isize;
        let mut stops = self.stops.borrow_mut();
        for (index, range) in stops.iter_mut().enumerate() {
            if index == current {
                range.1 = (range.1 as isize + shift).max(0) as usize;
            } else if range.0 >= stop.1 {
                range.0 = (range.0 as isize + shift).max(0) as usize;
                range.1 = (range.1 as isize + shift).max(0) as usize;
            }
        }
        true
    }
}

impl NodeResource for TextSnippetSession {
    fn mount(&self, _node: &Node) {}

    fn unmount(&self, _node: &Node) {
        self.end();
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
