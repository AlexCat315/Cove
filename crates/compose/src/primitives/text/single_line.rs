//! Chooses source text before shaping the visible line, preserving ligatures,
//! fallback fonts and extended grapheme clusters instead of slicing glyphs.

use cove_core::line_breaks::is_newline;
use unicode_segmentation::UnicodeSegmentation;

pub struct SingleLineText;

impl SingleLineText {
    pub fn fitting(source: &str, width: f32, measure: impl Fn(&str) -> f32) -> String {
        let text = normalized_single_line(source);
        if !width.is_finite() {
            return text;
        }
        if width <= 0.0 {
            return String::new();
        }
        if measure(&text) <= width {
            return text;
        }
        if measure("\u{2026}") > width {
            return String::new();
        }
        let characters: Vec<&str> = text.graphemes(true).collect();
        let mut lower = 0usize;
        let mut upper = characters.len();
        while lower < upper {
            let count = (lower + upper).div_ceil(2);
            let mut candidate: String = characters[..count].concat();
            candidate.push('\u{2026}');
            if measure(&candidate) <= width {
                lower = count;
            } else {
                upper = count - 1;
            }
        }
        let mut result: String = characters[..lower].concat();
        result.push('\u{2026}');
        result
    }
}

/// Newline-separated source collapses to one line. Swift's `split` omits empty
/// subsequences, so consecutive separators contribute a single space.
pub fn normalized_single_line(source: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    for grapheme in source.graphemes(true) {
        if is_newline(grapheme) {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
        } else {
            current.push_str(grapheme);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts.join(" ")
}
