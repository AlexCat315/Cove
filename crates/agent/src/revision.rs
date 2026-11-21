//! Compute the changed span of an author-approved text revision.

use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeSpan {
    pub before: Range<usize>,
    pub after: Range<usize>,
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
