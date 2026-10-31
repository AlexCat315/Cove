//! Translation of CoveUICore/LineBreaks.swift.
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;
pub fn is_newline(grapheme: &str) -> bool {
    matches!(
        grapheme,
        "\n" | "\r" | "\r\n" | "\u{b}" | "\u{c}" | "\u{85}" | "\u{2028}" | "\u{2029}"
    )
}
fn is_space(value: &str) -> bool {
    matches!(value, " " | "\t")
}
fn is_ideographic(value: &str) -> bool {
    value.chars().next().is_some_and(|c| matches!(c as u32, 0x2e80..=0x9fff | 0xac00..=0xd7af | 0xf900..=0xfaff | 0xff00..=0xffef | 0x20000..=0x3ffff))
}
pub fn line_break_opportunities(text: &str) -> HashSet<usize> {
    const CLOSING: &str = "，。、；：？！）」』】〉》〕…·,.;:!?)]}%";
    const OPENING: &str = "（「『【〈《〔([{";
    let mut result = HashSet::new();
    let mut previous = None;
    for (offset, character) in text.grapheme_indices(true) {
        if let Some(before) = previous {
            if !is_space(character) && !is_newline(character) {
                if is_space(before) {
                    result.insert(offset);
                } else if is_ideographic(character) || is_ideographic(before) {
                    if !CLOSING.contains(character) && !OPENING.contains(before) {
                        result.insert(offset);
                    }
                } else if matches!(before, "-" | "/" | "—")
                    && character.chars().any(char::is_alphanumeric)
                {
                    result.insert(offset);
                }
            }
        }
        previous = Some(character);
    }
    result
}
