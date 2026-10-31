//! Translation of `Tests/CoveUIComposeTests/TextIndentGuidesTests.swift`.
//!
//! Behaviour checklist:
//! - nested scopes draw in the preceding whitespace and join across blank lines
//!   without a per-row gap
//! - a scrolled window recovers blank-line context, tabs share columns, and
//!   offscreen depths are bounded
//! - soft-wrapped words do not receive a guide through their first glyph

mod support;

use cove_core::{TextLayoutResult, TextLine};
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::indent_guides::TextIndentGuides;

/// Builds the flat line layout the Swift helper produces: one line per logical
/// line, no glyphs, 20pt rows.
fn layout(source: &TextBuffer, rows: Option<std::ops::Range<usize>>) -> TextLayoutResult {
    let range = rows.unwrap_or(0..source.line_count());
    let lines: Vec<TextLine> = range
        .map(|index| {
            let line = source.line_range(index);
            TextLine {
                glyphs: Vec::new(),
                baseline_y: index as f32 * 20.0 + 16.0,
                top_y: index as f32 * 20.0,
                width: 100.0,
                start_cluster: source.character_index_for_utf8_offset(line.start) as u32,
                end_cluster: source.character_index_for_utf8_offset(line.end) as u32,
            }
        })
        .collect();
    TextLayoutResult {
        lines,
        total_width: 100.0,
        total_height: source.line_count() as f32 * 20.0,
    }
}

#[test]
fn continuous_scopes_join_across_blank_lines() {
    let source = TextBuffer::from(
        "func demo() {\n    first()\n\n    if ready {\n        next()\n    }\n}\n",
    );
    let guides =
        TextIndentGuides::segments(&layout(&source, None), &source, 4, 8.0, 20.0, (0.0, 200.0));
    assert_eq!(guides.len(), 2, "two scopes are open in this snippet");
    let first = guides.first().expect("outer scope guide");
    assert_eq!(first.x, 0.5);
    assert_eq!(first.top, 20.0);
    assert_eq!(
        first.height, 100.0,
        "the outer guide spans its body across the blank line without a gap"
    );
    let last = guides.last().expect("inner scope guide");
    assert_eq!(last.x, 32.5);
    assert_eq!(last.top, 80.0);
    assert_eq!(last.height, 20.0);
}

#[test]
fn visible_window_recovers_context_and_bounds_depth() {
    let source = TextBuffer::from("root\n\tfirst\n\n\t\tsecond\nend");
    let guides = TextIndentGuides::segments(
        &layout(&source, Some(2..4)),
        &source,
        4,
        8.0,
        20.0,
        (0.0, 200.0),
    );
    assert_eq!(guides.len(), 2, "both tabs in the window produce a guide");
    let first = guides.first().expect("depth-one guide");
    assert_eq!(first.top, 40.0);
    assert_eq!(
        first.height, 40.0,
        "the blank line keeps the run continuous"
    );
    let last = guides.last().expect("depth-two guide");
    assert_eq!(last.x, 32.5, "tabs land on shared columns");
    assert_eq!(last.top, 60.0);

    assert!(
        TextIndentGuides::segments(&layout(&source, None), &source, 4, 8.0, 20.0, (64.0, 200.0),)
            .is_empty(),
        "depths outside the visible x range are not drawn"
    );
}

#[test]
fn soft_wrapped_words_get_no_guide_through_first_glyph() {
    let source = TextBuffer::from("    a long wrapped statement");
    let rows = TextLayoutResult {
        lines: vec![
            TextLine {
                glyphs: Vec::new(),
                baseline_y: 16.0,
                top_y: 0.0,
                width: 100.0,
                start_cluster: 0,
                end_cluster: 11,
            },
            TextLine {
                glyphs: Vec::new(),
                baseline_y: 36.0,
                top_y: 20.0,
                width: 100.0,
                start_cluster: 11,
                end_cluster: source.utf8_length() as u32,
            },
        ],
        total_width: 100.0,
        total_height: 40.0,
    };
    let guides = TextIndentGuides::segments(&rows, &source, 4, 8.0, 20.0, (0.0, 200.0));
    assert_eq!(guides.len(), 1, "one leading scope only");
    let guide = guides.first().expect("leading guide");
    assert_eq!(guide.top, 0.0);
    assert_eq!(
        guide.height, 20.0,
        "the continuation row is not given its own guide"
    );
}
