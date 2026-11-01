//! Translation of `Tests/CoveUIComposeTests/TextDocumentLayoutTests.swift`.
//!
//! Behaviour checklist:
//! - a 200K-line document shapes only the visible window, and the height total
//!   comes from the line index rather than from shaping every line
//! - the first shaped line starts at the byte offset of the requested row
//! - a repeated read of the same window does not shape more lines
//! - editing far down the document keeps the index in sync with the buffer
//! - wrapped line heights survive a newline edit, and the row mapping shifts by
//!   exactly one row
//! - changing the width resets the wrapping index
//! - caret and hit testing round-trip on every grapheme stop of a mixed line
//! - a secure geometry masks the same grapheme stops

mod support;

use cove_core::TextLayoutResult;
use cove_text::{TextEnvironment, TextEnvironmentHolder};
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_document_layout::{
    TextDocumentLayout, TextLineGeometry,
};

use support::GlobalTestLock;

fn environment() -> TextEnvironment {
    support::test_text_environment(14.0, 20.0)
}

fn geometry(env: &TextEnvironment, width: f32, secure: bool) -> TextLineGeometry {
    TextLineGeometry {
        font: cove_core::Font::system(14.0, cove_core::FontWeight::Regular),
        line_height: 20.0,
        letter_spacing: 0.0,
        atlas: env.atlas.identity(),
        width,
        secure,
    }
}

fn with_environment<T>(body: impl FnOnce(&TextEnvironment) -> T) -> T {
    let env = environment();
    let previous = TextEnvironmentHolder::current();
    TextEnvironmentHolder::set_current(Some(env.clone()));
    let result = body(&env);
    TextEnvironmentHolder::set_current(previous);
    result
}

fn visible(
    layout: &mut TextDocumentLayout,
    first_row: usize,
    rows: usize,
    env: &TextEnvironment,
) -> TextLayoutResult {
    layout.visible_layout(first_row, rows, env)
}

#[test]
fn large_document_shapes_only_a_visible_window() {
    GlobalTestLock::locked(|| {
        with_environment(|env| {
            let buffer = TextBuffer::new(&"let example = 123\n".repeat(200_000));
            let mut layout =
                TextDocumentLayout::new(buffer.clone(), geometry(env, f32::INFINITY, false));

            let first = visible(&mut layout, 100_000, 40, env);
            assert_eq!(first.lines.len(), 40, "the window shapes 40 visual rows");
            assert_eq!(
                layout.shaped_line_count, 40,
                "only the visible window is shaped"
            );
            assert!(
                layout.shaped_utf8_count < 1_000,
                "the shaped byte count stays small, was {}",
                layout.shaped_utf8_count
            );
            assert_eq!(
                first.total_height,
                buffer.line_count() as f32 * 20.0,
                "the height total comes from the line index"
            );
            let expected_start =
                buffer.utf8_offset_for_character_index(buffer.line_range(100_000).start);
            assert_eq!(
                first.lines[0].start_cluster as usize, expected_start,
                "the first shaped line starts at the requested row's byte offset"
            );

            let _ = visible(&mut layout, 100_000, 40, env);
            assert_eq!(
                layout.shaped_line_count, 40,
                "re-reading the same window shapes nothing new"
            );

            let mut buffer = buffer;
            for _ in 0..100 {
                let range = buffer.line_range(100_020);
                buffer = buffer.insert("a", range.end);
                layout.update(buffer.clone(), geometry(env, f32::INFINITY, false));
                let _ = visible(&mut layout, 100_000, 40, env);
            }
            assert!(
                layout.shaped_line_count <= 140,
                "100 edits near the window retain a bounded cache, was {}",
                layout.shaped_line_count
            );
            assert_eq!(
                layout.index().line_count(),
                buffer.line_count(),
                "the index tracks the buffer's line count"
            );
            assert_eq!(
                layout.index().row_count(),
                buffer.line_count(),
                "unwrapped rows equal lines"
            );
        });
    });
}

#[test]
fn wrapped_heights_survive_newline_edits_and_reset_on_width_change() {
    GlobalTestLock::locked(|| {
        with_environment(|env| {
            let mut buffer = TextBuffer::new("abcdefghijklmno\nsecond wrapped sentence\nend");
            let metrics = geometry(env, 50.0, false);
            let mut layout = TextDocumentLayout::new(buffer.clone(), metrics.clone());
            let _ = visible(&mut layout, 0, 100, env);

            let third_row = layout.index().first_row_of_line(2);
            assert!(
                third_row > 2,
                "the third logical line starts past row 2 once line 0 wraps, was {third_row}"
            );
            let measured = layout.shaped_line_count;

            buffer = buffer.insert("\n", buffer.line_range(1).start);
            layout.update(buffer.clone(), metrics.clone());
            let _ = visible(&mut layout, 0, 100, env);
            assert_eq!(layout.index().line_count(), 4, "the newline adds a line");
            assert_eq!(
                layout.shaped_line_count,
                measured + 2,
                "the split line is reshaped into its two halves"
            );
            assert_eq!(
                layout.index().first_row_of_line(3),
                third_row + 1,
                "the following lines shift by exactly one row"
            );

            layout.update(buffer.clone(), geometry(env, f32::INFINITY, false));
            assert_eq!(
                layout.index().row_count(),
                4,
                "a width change resets wrapping to one row per line"
            );
            let _ = visible(&mut layout, 0, 100, env);
            assert_eq!(
                layout.index().row_count(),
                4,
                "measuring after the reset keeps one row per line"
            );
        });
    });
}

#[test]
fn caret_and_hit_testing_share_grapheme_stops() {
    GlobalTestLock::locked(|| {
        with_environment(|env| {
            let buffer = TextBuffer::new("Ae\u{301}👩🏽‍💻中Z\n\nend");
            let first_line_end = buffer.line_range(0).end;
            for secure in [false, true] {
                let mut layout =
                    TextDocumentLayout::new(buffer.clone(), geometry(env, f32::INFINITY, secure));
                let result = visible(&mut layout, 0, 10, env);
                assert_eq!(result.lines.len(), 3, "three visual rows");

                for character in 0..=first_line_end {
                    let (x, row) = layout.caret(character, false, env);
                    assert_eq!(row, 0, "every caret on the first line is on row 0");
                    let hit = layout.character(row, x, env);
                    assert_eq!(
                        hit, character,
                        "caret stop {character} round-trips through hit testing (secure {secure})"
                    );
                }

                let blank_start = buffer.line_range(1).start;
                let (blank_x, blank_row) = layout.caret(blank_start, false, env);
                assert_eq!(
                    (blank_x, blank_row),
                    (0.0, 1),
                    "the blank line caret sits at its origin"
                );

                for glyph in result.lines.iter().flat_map(|line| line.glyphs.iter()) {
                    let character = buffer.character_index_for_utf8_offset(glyph.cluster as usize);
                    assert_eq!(
                        buffer.utf8_offset_for_character_index(character),
                        glyph.cluster as usize,
                        "a shaped cluster is a character boundary (secure {secure})"
                    );
                }
            }
        });
    });
}
