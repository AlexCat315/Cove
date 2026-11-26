//! Locate and apply exact text revisions.

use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeSpan {
    pub before: Range<usize>,
    pub after: Range<usize>,
}

/// Apply replacements against the original text as one operation.
/// Each anchor must occur exactly once, and anchors must not overlap.
pub fn apply_replacements<'a>(
    text: &str,
    edits: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Option<String> {
    let mut located = Vec::new();
    for (before, after) in edits {
        if before.is_empty() || before == after {
            return None;
        }
        let start = text.find(before)?;
        // A repeated anchor can overlap its first occurrence.
        if text[start + before.chars().next()?.len_utf8()..].contains(before) {
            return None;
        }
        located.push((start..start + before.len(), after));
    }
    if located.is_empty() {
        return None;
    }
    located.sort_unstable_by_key(|(range, _)| range.start);
    let mut revised = String::with_capacity(text.len());
    let mut end = 0;
    for (range, replacement) in located {
        revised.push_str(text.get(end..range.start)?);
        revised.push_str(replacement);
        end = range.end;
    }
    revised.push_str(&text[end..]);
    Some(revised)
}

/// Find the smallest contiguous replacement between two UTF-8 strings.
/// Ranges include every changed scalar and never cut a UTF-8 sequence.
/// An insertion or deletion has an empty range on one side.
pub fn change_span(before: &str, after: &str) -> Option<ChangeSpan> {
    if before == after {
        return None;
    }
    let prefix = common_bytes(before.chars(), after.chars());
    let suffix = common_bytes(
        before[prefix..].chars().rev(),
        after[prefix..].chars().rev(),
    );
    Some(ChangeSpan {
        before: prefix..before.len() - suffix,
        after: prefix..after.len() - suffix,
    })
}

fn common_bytes(left: impl Iterator<Item = char>, right: impl Iterator<Item = char>) -> usize {
    left.zip(right)
        .take_while(|(a, b)| a == b)
        .map(|(ch, _)| ch.len_utf8())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_exact_edits_without_touching_the_intervening_text() {
        let text = "他推门。\n\n她转头。\n门合上了。";
        assert_eq!(
            apply_replacements(text, [("门合上了。", "门还开着。"), ("推门", "敲门")]).unwrap(),
            "他敲门。\n\n她转头。\n门还开着。"
        );
        assert!(apply_replacements("哈哈哈", [("哈哈", "好")]).is_none());
        assert!(apply_replacements(text, [("他推门", ""), ("门。", "门")]).is_none());
        assert!(apply_replacements(text, [("她转身", "她转头")]).is_none());
    }

    #[test]
    fn separates_the_edited_text_from_the_unchanged_context() {
        let before = "她把画卖了钱，没说话。";
        let after = "她合上平板，没说话。";
        let span = change_span(before, after).unwrap();
        assert_eq!(&before[span.before], "把画卖了钱");
        assert_eq!(&after[span.after], "合上平板");
        assert!(change_span(before, before).is_none());
        assert_eq!(change_span("夜色", "夜深色").unwrap().before, 3..3);
    }
}
