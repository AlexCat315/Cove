//! Detects long verbatim or punctuation-only copies in generated text.

use std::collections::HashMap;
use std::ops::Range;

const PASSAGE_LENGTH: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatedFrom {
    Draft,
    Reference,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repetition {
    /// UTF-8 byte range into the original draft.
    pub range: Range<usize>,
    pub from: RepeatedFrom,
}

/// Returns a passage of 24 consecutive letters or numbers that recurs within
/// the draft or appears in a reference. Punctuation, spacing and case are
/// ignored. Matches within a draft must be non-overlapping.
pub fn find_repetition<'a>(
    draft: &str,
    references: impl IntoIterator<Item = &'a str>,
) -> Option<Repetition> {
    let (offsets, characters): (Vec<_>, Vec<_>) = draft
        .char_indices()
        .flat_map(|(offset, ch)| normalized(ch).map(move |ch| (offset, ch)))
        .unzip();
    if characters.len() < PASSAGE_LENGTH {
        return None;
    }
    let repetition = |start, from| Repetition {
        range: offsets[start]
            ..offsets
                .get(start + PASSAGE_LENGTH)
                .copied()
                .unwrap_or(draft.len()),
        from,
    };
    let mut passages = HashMap::<&[char], usize>::new();
    for (start, passage) in characters.windows(PASSAGE_LENGTH).enumerate() {
        let first = *passages.entry(passage).or_insert(start);
        if first + PASSAGE_LENGTH <= start {
            return Some(repetition(start, RepeatedFrom::Draft));
        }
    }
    for reference in references {
        let characters: Vec<_> = reference.chars().flat_map(normalized).collect();
        for passage in characters.windows(PASSAGE_LENGTH) {
            if let Some(&start) = passages.get(passage) {
                return Some(repetition(start, RepeatedFrom::Reference));
            }
        }
    }
    None
}

fn normalized(ch: char) -> impl Iterator<Item = char> {
    ch.to_lowercase().filter(|ch| ch.is_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catches_copied_passages_with_different_punctuation_and_spacing() {
        let reference = "他拧亮桌上的煤油灯。把那封没有署名的信重新塞回窗台下的砖缝里。";
        let draft =
            "他拧亮桌上的煤油灯，把那封没有署名的信重新塞回窗台下的砖缝里。窗外传来敲门声。";
        let found = find_repetition(draft, [reference]).unwrap();
        assert_eq!(found.from, RepeatedFrom::Reference);
        assert!(draft[found.range].starts_with("他拧亮桌上的煤油灯，"));
        assert!(find_repetition("煤油灯熄了。他拉开门，接过来人手里的伞。", [reference]).is_none());
    }

    #[test]
    fn catches_repeated_prose_in_the_draft_without_matching_overlaps() {
        let line = "The key was hidden under the floorboard.";
        let draft = format!("{line}\nTHE KEY WAS HIDDEN UNDER THE FLOORBOARD.");
        let found = find_repetition(&draft, []).unwrap();
        assert_eq!(found.from, RepeatedFrom::Draft);
        assert!(found.range.start >= line.len());
        assert!(find_repetition(&"啦".repeat(PASSAGE_LENGTH * 2 - 1), []).is_none());
        assert!(
            find_repetition(
                "晏澄转身看向北门。晏澄握着剑。",
                ["晏澄听见脚步声。晏澄推开门。"]
            )
            .is_none()
        );
    }
}
