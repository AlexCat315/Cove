//! Translation of `Tests/CoveUIComposeTests/BracketMatchingTests.swift`.
//!
//! Behaviour checklist:
//! - ordinary typing and caret movement do not scan the document
//! - nested pairs skip brackets inside strings and comments
//! - redraws reuse a cached pair and edits invalidate it, including for a
//!   document that has no match at all

mod support;

use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::bracket_matching::{
    BracketPair, TextBracketMatching,
};

#[test]
fn ordinary_text_does_not_scan() {
    let buffer = TextBuffer::from("a".repeat(200_000));
    let matcher = TextBracketMatching::default();
    for caret in [0, 10, 100_000, 199_999, 200_000] {
        assert_eq!(
            matcher.pair(&buffer, caret),
            None,
            "caret {caret} sits in a document with no brackets"
        );
    }
    assert_eq!(
        matcher.scanned_utf8_count(),
        0,
        "plain text never triggers a lexical scan"
    );
}

#[test]
fn nested_pairs_skip_strings_and_comments() {
    let text = "你好({\"}\" /* ) */ [1]})";
    let buffer = TextBuffer::from(text);
    let matcher = TextBracketMatching::default();
    let opening = "你好".len();
    let closing = text.len() - 1;

    assert_eq!(
        matcher.pair(&buffer, 2),
        Some(BracketPair { open: opening, close: closing }),
        "the outer parenthesis matches across the skipped string and comment"
    );
    assert_eq!(
        matcher.pair(&buffer, buffer.character_count()),
        Some(BracketPair { open: closing, close: opening }),
        "the closing caret finds the same pair reversed"
    );
    assert_eq!(
        matcher.pair(&TextBuffer::from("// ()"), 3),
        None,
        "brackets inside a comment match nothing"
    );
    assert_eq!(
        matcher.pair(&TextBuffer::from("\"()\""), 1),
        None,
        "brackets inside a string match nothing"
    );
}

#[test]
fn revision_cache_reuses_and_invalidates() {
    let matcher = TextBracketMatching::default();
    let buffer = TextBuffer::from("(hello)");
    assert_eq!(
        matcher.pair(&buffer, 0),
        Some(BracketPair { open: 0, close: 6 })
    );

    // Redraws reuse the cached pair without rescanning.
    let scanned = matcher.scanned_utf8_count();
    for _ in 0..60 {
        assert_eq!(
            matcher.pair(&buffer, 0),
            Some(BracketPair { open: 0, close: 6 })
        );
    }
    assert_eq!(
        matcher.scanned_utf8_count(),
        scanned,
        "repeated redraws do not rescan the buffer"
    );

    // Deleting the closing parenthesis invalidates the cached pair.
    let edited = buffer.delete(6..7);
    assert_eq!(
        matcher.pair(&edited, 0),
        None,
        "the edit invalidates the old pair"
    );

    // A document with no match also caches its "no match" answer.
    let missing = matcher.scanned_utf8_count();
    assert_eq!(matcher.pair(&edited, 0), None);
    assert_eq!(
        matcher.scanned_utf8_count(),
        missing,
        "a missing match is cached too"
    );

    assert_eq!(
        matcher.pair(&TextBuffer::empty(), 99),
        None,
        "a caret past the end of an empty buffer matches nothing"
    );
}
