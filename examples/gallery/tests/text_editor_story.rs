//! Behaviour tests for the text-editor story.
//!
//! Translation of `GalleryContent/Stories/Controls/TextEditorStory.swift`. The
//! story carries its own byte-level Swift lexer; byte offsets and the fold scan's
//! string/comment exemption are exactly what drifts silently, so they are pinned
//! directly.

use cove_examples::gallery_content::catalog::GalleryOptions;
use cove_examples::gallery_content::stories::text_editor_story::{
    FoldRegion, KEYWORDS, SAMPLE_SOURCE, STRESS_LINES, SYNTAX_LIMIT, TextEditorStory, TokenKind,
    folds, is_keyword, token_at, token_color, tokens,
};

fn kinds(source: &str) -> Vec<(String, TokenKind)> {
    tokens(source)
        .into_iter()
        .map(|token| (source[token.range()].to_owned(), token.kind))
        .collect()
}

#[test]
fn the_keyword_list_is_byte_sorted_so_binary_search_is_valid() {
    // The Swift original listed `self` before `Self`; the Rust list is re-sorted.
    let mut sorted = KEYWORDS;
    sorted.sort_unstable();
    assert_eq!(KEYWORDS, sorted, "KEYWORDS must be in byte order");
}

#[test]
fn keywords_are_recognised_both_casings_of_self() {
    assert!(is_keyword("let"));
    assert!(is_keyword("struct"));
    assert!(is_keyword("self"));
    assert!(is_keyword("Self"));
    assert!(!is_keyword("Greeting"));
    assert!(!is_keyword("lt"));
}

#[test]
fn the_sample_document_tokenises_its_keywords_and_types() {
    let found = kinds(SAMPLE_SOURCE);
    assert!(found.contains(&("struct".to_owned(), TokenKind::Keyword)));
    assert!(found.contains(&("let".to_owned(), TokenKind::Keyword)));
    assert!(found.contains(&("func".to_owned(), TokenKind::Keyword)));
    assert!(found.contains(&("return".to_owned(), TokenKind::Keyword)));
    // Capitalised identifiers are types; a lowercase non-keyword is not tokenised.
    assert!(found.contains(&("String".to_owned(), TokenKind::Type)));
    assert!(found.contains(&("Greeting".to_owned(), TokenKind::Type)));
    assert!(
        !found.iter().any(|(text, _)| text == "name"),
        "plain identifiers are uncoloured"
    );
}

#[test]
fn a_line_comment_stops_before_the_newline() {
    let found = kinds("let a = 1 // note\nlet b = 2");
    let comment = found
        .iter()
        .find(|(_, kind)| *kind == TokenKind::Comment)
        .unwrap();
    assert_eq!(
        comment.0, "// note",
        "the newline is not part of the comment"
    );
}

#[test]
fn a_string_honours_backslash_escapes() {
    // The escaped quote must not close the string early.
    let source = r#"let s = "a\"b""#;
    let string = tokens(source)
        .into_iter()
        .find(|token| token.kind == TokenKind::String)
        .expect("a string token");
    assert_eq!(&source[string.range()], r#""a\"b""#);
}

#[test]
fn an_unterminated_string_runs_to_the_end() {
    let source = "let s = \"never closed";
    let string = tokens(source)
        .into_iter()
        .find(|t| t.kind == TokenKind::String)
        .unwrap();
    assert_eq!(string.end, source.len());
}

#[test]
fn a_trailing_backslash_does_not_run_past_the_end() {
    // `min` keeps the escape skip inside the buffer.
    let source = "let s = \"tail\\";
    let string = tokens(source)
        .into_iter()
        .find(|t| t.kind == TokenKind::String)
        .unwrap();
    assert_eq!(string.end, source.len());
}

#[test]
fn numbers_are_tokenised() {
    let found = kinds("let x = 42");
    assert!(found.contains(&("42".to_owned(), TokenKind::Number)));
}

#[test]
fn braces_inside_strings_and_comments_do_not_fold() {
    // A brace in a string must not open a region that a later real brace closes.
    let source = "let a = 1\nlet s = \"{\"\nlet b = 2\n}\n";
    let token_list = tokens(source);
    let regions = folds(source, &token_list);
    assert!(
        regions.iter().all(|region| region.first_line != 1),
        "the string brace on line 1 must not start a fold, got {regions:?}"
    );
}

#[test]
fn a_comment_brace_does_not_fold() {
    let source = "// }\nlet a = 1\n";
    let token_list = tokens(source);
    assert!(
        folds(source, &token_list).is_empty(),
        "the closing brace is inside a comment"
    );
}

#[test]
fn a_real_block_folds_across_its_lines() {
    let source = "struct A {\n  let x = 1\n}\n";
    let regions = folds(source, &tokens(source));
    assert_eq!(
        regions,
        vec![FoldRegion {
            first_line: 0,
            last_line: 2
        }]
    );
}

#[test]
fn a_single_line_block_is_not_foldable() {
    let source = "func f() { }\n";
    assert!(
        folds(source, &tokens(source)).is_empty(),
        "a brace pair on one line has nothing to hide"
    );
}

#[test]
fn nested_blocks_fold_inside_out_but_are_sorted_by_start() {
    let source = "a {\nb {\nc\n}\n}\n";
    let regions = folds(source, &tokens(source));
    assert_eq!(
        regions,
        vec![
            FoldRegion {
                first_line: 0,
                last_line: 4
            },
            FoldRegion {
                first_line: 1,
                last_line: 3
            },
        ],
        "sorted by first line, so the outer block comes first"
    );
}

#[test]
fn the_sample_document_produces_the_expected_two_regions() {
    let story = TextEditorStory::new(GalleryOptions::default());
    let computed = story.computed_folds();
    // `struct Greeting {` on line 0 closes on line 6; `func message() -> String {`
    // on line 3 closes on line 5.
    assert_eq!(
        computed,
        vec![
            FoldRegion {
                first_line: 0,
                last_line: 6
            },
            FoldRegion {
                first_line: 3,
                last_line: 5
            },
        ]
    );
}

#[test]
fn documents_over_the_limit_are_not_tokenised_or_folded() {
    let big = "a".repeat(SYNTAX_LIMIT + 1);
    assert!(tokens(&big).is_empty(), "over the size guard");
    assert!(folds(&big, &[]).is_empty());
    // Exactly at the limit is still processed.
    let at_limit = "a".repeat(SYNTAX_LIMIT);
    assert!(folds(&at_limit, &tokens(&at_limit)).is_empty());
}

#[test]
fn token_at_finds_the_covering_token_by_binary_search() {
    let source = "let x = 1";
    let list = tokens(source);
    // Offset 0 is inside `let`.
    assert_eq!(token_at(0, &list).map(|t| t.kind), Some(TokenKind::Keyword));
    // `let` covers bytes 0..3, so offset 2 is inside it and 3 is the gap.
    assert_eq!(token_at(2, &list).map(|t| t.kind), Some(TokenKind::Keyword));
    assert_eq!(
        token_at(3, &list),
        None,
        "the gap between tokens is uncoloured"
    );
    assert_eq!(
        token_at(4, &list),
        None,
        "the space after `let` is uncoloured"
    );
    // The final byte is the digit, at index `len - 1`.
    let digit = source.len() - 1;
    assert_eq!(
        token_at(digit, &list).map(|t| t.kind),
        Some(TokenKind::Number)
    );
    assert_eq!(
        token_at(source.len(), &list),
        None,
        "one past the end has no token"
    );
}

#[test]
fn light_and_dark_palettes_both_cover_every_kind() {
    for kind in [
        TokenKind::Keyword,
        TokenKind::String,
        TokenKind::Comment,
        TokenKind::Number,
        TokenKind::Type,
    ] {
        let light = token_color(kind, true);
        let dark = token_color(kind, false);
        assert_ne!(light, dark, "{kind:?} must differ between appearances");
    }
}

#[test]
fn the_story_starts_in_code_mode_on_the_sample() {
    let story = TextEditorStory::new(GalleryOptions::default());
    assert!(story.is_code_mode());
    assert_eq!(story.source(), SAMPLE_SOURCE);
    assert_eq!(
        story.line_count(),
        8,
        "the trailing newline starts an empty eighth line"
    );
    assert!(story.is_bookmarked());
    assert_eq!(story.breakpoints(), vec![3]);
    assert_eq!(story.fold_regions().len(), 2);
}

#[test]
fn the_diagnostic_range_tracks_the_first_line() {
    let story = TextEditorStory::new(GalleryOptions::default());
    let (range, message) = story.diagnostic();
    assert_eq!(&story.source()[range], "struct Greeting {");
    assert!(message.starts_with("Sample warning"));

    story.set_source("x\ny");
    let (range, _) = story.diagnostic();
    assert_eq!(&story.source()[range], "x", "the range follows an edit");
}

#[test]
fn loading_the_stress_sample_clears_breakpoints_and_folds() {
    let story = TextEditorStory::new(GalleryOptions::default());
    story.load_lines(STRESS_LINES);
    assert_eq!(story.line_count(), STRESS_LINES);
    assert!(
        story.breakpoints().is_empty(),
        "old line numbers are meaningless"
    );
    assert!(story.fold_regions().is_empty());
    assert!(
        story.status().contains("100001"),
        "the caret reports line 100,001"
    );
    // And the document is past the syntax limit, so no tokens were built.
    assert!(
        story.tokens().is_empty(),
        "200k lines exceed the 300 KB guard"
    );
}

#[test]
fn toggling_a_bookmark_flips_it() {
    let story = TextEditorStory::new(GalleryOptions::default());
    story.toggle_bookmark();
    assert!(!story.is_bookmarked());
    story.toggle_bookmark();
    assert!(story.is_bookmarked());
}
