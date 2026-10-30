//! Translation of `Primitives/Controls/Fields/SearchField.swift` (the model
//! half: matching, highlighting, visibility and key handling).
//!
//! Search input built on the regular `TextField` chrome: the field's clear
//! affordance, submit and Escape handling, and an optional suggestion list that
//! supports arrow keys, Return and pointer.
//!
//! Behaviour checklist:
//! - matching is case-insensitive over the trimmed query; an empty query shows
//!   every suggestion
//! - suggestions are hidden while suppressed, when disabled, and when the list
//!   is empty
//! - Arrow Down/Up move the highlight and clamp at both ends
//! - Escape clears the text when `clears_on_escape`
//! - Return chooses the highlighted suggestion, else submits
//! - choosing suppresses the list so it does not immediately reopen

/// Which key a `handle` call consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SearchKeyOutcome {
    /// The key was not ours; the field should process it normally.
    Passthrough,
    /// Redraw with the same text (the highlight moved or the list closed).
    Redraw,
    /// The text was replaced; callers holding a buffer must write it.
    ReplaceText(&'static str),
}

/// Result of handling one key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchAction {
    None,
    /// Move the highlight and stay open.
    MoveHighlight,
    /// Clear the text (Escape with `clears_on_escape`).
    Clear,
    /// Choose `suggestions[index]`.
    Choose(usize),
    /// Submit the current text.
    Submit,
}

/// Observable state of one search field.
#[derive(Clone, Debug, Default)]
pub struct SearchFieldState {
    /// Index of the highlighted suggestion.
    pub highlighted: Option<usize>,
    pub is_focused: bool,
    /// Set after choosing, so the list does not immediately reopen.
    pub suppress_suggestions: bool,
}

impl SearchFieldState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Suggestions matching `query`, case-insensitively over the trimmed text.
    ///
    /// An empty query shows every suggestion — the list is a browse surface when
    /// nothing has been typed yet.
    pub fn matches(query: &str, suggestions: &[String]) -> Vec<String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return suggestions.to_vec();
        }
        let needle = trimmed.to_lowercase();
        suggestions
            .iter()
            .filter(|suggestion| suggestion.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    /// Whether the suggestion list is shown at all.
    pub fn shows_suggestions(&self, text: &str, suggestions: &[String], is_enabled: bool) -> bool {
        if self.suppress_suggestions || !is_enabled {
            return false;
        }
        if !self.is_focused && text.is_empty() {
            return false;
        }
        !Self::matches(text, suggestions).is_empty()
    }

    /// Called when the text changes: reopen the list and highlight the first
    /// row, or clear the highlight when nothing matches.
    pub fn on_change(&mut self, text: &str, suggestions: &[String]) {
        self.suppress_suggestions = false;
        self.highlighted = if Self::matches(text, suggestions).is_empty() { None } else { Some(0) };
    }

    pub fn on_focus(&mut self, text: &str, suggestions: &[String]) {
        self.is_focused = true;
        self.suppress_suggestions = false;
        if self.highlighted.is_none() && !Self::matches(text, suggestions).is_empty() {
            self.highlighted = Some(0);
        }
    }

    pub fn on_blur(&mut self) {
        self.is_focused = false;
        self.highlighted = None;
    }

    pub fn on_clear(&mut self) {
        self.highlighted = None;
    }

    /// Moves the highlight by `delta`, clamping at both ends.
    ///
    /// Starting from no highlight, a downward move lands on the first row and an
    /// upward move on the last.
    pub fn move_highlight(&mut self, delta: i64, count: usize) {
        if count == 0 {
            return;
        }
        let last = count as i64 - 1;
        let current = match self.highlighted {
            Some(index) => index as i64,
            None if delta > 0 => -1,
            None => count as i64,
        };
        self.highlighted = Some((current + delta).clamp(0, last) as usize);
    }

    /// Choosing writes the suggestion, closes the list and drops focus.
    pub fn choose(&mut self) {
        self.highlighted = None;
        self.is_focused = false;
        self.suppress_suggestions = true;
    }

    /// Decides what a key press should do.
    pub fn handle_key(
        &mut self,
        key: SearchKey,
        text: &str,
        suggestions: &[String],
        clears_on_escape: bool,
    ) -> SearchAction {
        match key {
            SearchKey::ArrowDown | SearchKey::ArrowUp => {
                let visible = Self::matches(text, suggestions);
                if visible.is_empty() {
                    return SearchAction::None;
                }
                self.move_highlight(if key == SearchKey::ArrowDown { 1 } else { -1 }, visible.len());
                SearchAction::MoveHighlight
            }
            SearchKey::Escape => {
                if !clears_on_escape {
                    return SearchAction::None;
                }
                self.highlighted = None;
                SearchAction::Clear
            }
            SearchKey::Return | SearchKey::KeypadEnter => {
                let visible = Self::matches(text, suggestions);
                match self.highlighted.filter(|index| *index < visible.len()) {
                    Some(index) => {
                        self.choose();
                        SearchAction::Choose(index)
                    }
                    None => SearchAction::Submit,
                }
            }
        }
    }
}

/// The keys a search field intercepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchKey {
    ArrowDown,
    ArrowUp,
    Escape,
    Return,
    KeypadEnter,
}
