//! Translation of `Tests/CoveUIComposeTests/SnippetTests.swift`.
//!
//! Behaviour checklist:
//! - snippet syntax expands placeholders, choices, escapes, variables and the
//!   final stop, and reports each stop's character range
//! - Tab visits stops in order, edits inside a stop shift the later ones, the
//!   final stop ends the session, and an outside edit ends it without consuming
//!   Tab
//! - a very long unwrapped line shapes in segments with monotonic glyphs and
//!   exact carets
//! - horizontal culling keeps only the glyphs near the viewport of a long row

use crate::support;

use cove_core::{PositionedGlyph, TextLayoutResult};
use cove_platform::{KeyEvent, KeyModifiers};
use cove_text::TextEnvironment;
use cove_ui_compose::Scancode;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_document_layout::{
    TextDocumentLayout, TextLineGeometry,
};
use cove_ui_compose::primitives::text::text_field::snippet::{TextSnippet, TextSnippetSession};
use cove_ui_compose::primitives::text::text_field::text_field_layout_engine::visible_layout;

use support::GlobalTestLock;

fn key(scancode: u32, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers,
        is_repeat: false,
    }
}

fn ranges(snippet: &TextSnippet) -> Vec<(usize, usize)> {
    snippet
        .stops
        .iter()
        .map(|stop| (stop.start, stop.end))
        .collect()
}

#[test]
fn snippet_syntax_expands_placeholders_choices_and_escapes() {
    let snippet =
        TextSnippet::new("for ${1:item} in ${2|items,list|} {\n\t$0\n} \\$x ${TM_FILENAME:file}");
    assert_eq!(snippet.text, "for item in items {\n\t\n} $x file");
    assert_eq!(
        snippet
            .stops
            .iter()
            .map(|stop| stop.number)
            .collect::<Vec<_>>(),
        [1, 2, 0],
        "numbered stops come first, then the final caret"
    );
    assert_eq!(
        ranges(&snippet),
        [(4, 8), (12, 17), (21, 21)],
        "each stop records its expanded character range"
    );

    let implicit = TextSnippet::new("print($1)");
    assert_eq!(implicit.text, "print()");
    assert_eq!(
        ranges(&implicit),
        [(6, 6), (7, 7)],
        "a bare `$1` is an empty stop and `$0` defaults past the end"
    );

    let nested = TextSnippet::new("${1:outer ${2:inner}}");
    assert_eq!(nested.text, "outer inner");
    assert_eq!(
        (nested.stops[0].start, nested.stops[0].end),
        (0, 11),
        "the outer placeholder spans the whole expansion"
    );
    assert_eq!(
        (nested.stops[1].start, nested.stops[1].end),
        (6, 11),
        "the nested stop sits inside its parent's range"
    );
}

#[test]
fn tab_visits_stops_and_edits_shift_the_later_ones() {
    let session = TextSnippetSession::new();
    let snippet = TextSnippet::new("f(${1:a}, ${2:b})$0");
    let mut buffer = TextBuffer::new(&format!("x = {}", snippet.text));
    let first = session.start(&snippet, 4, &buffer);
    assert_eq!(
        first,
        Some((6, 7)),
        "the first stop is selected after insertion"
    );

    buffer = buffer.replace(6..7, "alpha");
    let tab = key(Scancode::TAB, KeyModifiers::default());
    assert_eq!(
        session.command(&tab, &buffer),
        Some(Some((13, 14))),
        "Tab moves to the next stop, shifted by the longer placeholder"
    );
    let back = key(Scancode::TAB, KeyModifiers::LEFT_SHIFT);
    assert_eq!(
        session.command(&back, &buffer),
        Some(Some((6, 11))),
        "Shift+Tab returns to the widened first stop"
    );
    assert_eq!(
        session.command(&tab, &buffer),
        Some(Some((13, 14))),
        "Tab moves forward again"
    );
    assert_eq!(
        session.command(&tab, &buffer),
        Some(Some((15, 15))),
        "the last Tab selects the final stop and ends the session"
    );
    assert!(!session.is_active(), "the final stop ends the session");

    // An edit outside the current stop ends the session without consuming Tab.
    let _ = session.start(&snippet, 4, &buffer);
    let elsewhere = buffer.replace(0..1, "y");
    assert_eq!(
        session.command(&tab, &elsewhere),
        None,
        "an outside edit ends the session and the Tab is not consumed"
    );
    assert!(!session.is_active());
}

#[test]
fn long_unwrapped_line_shapes_in_segments_with_exact_carets() {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let environment = support::test_text_environment(16.0, 20.0);
            let line = "abcdefghij ".repeat(3_000);
            let geometry = TextLineGeometry {
                font: environment.default_font.clone(),
                line_height: 20.0,
                letter_spacing: 0.0,
                atlas: 1,
                width: f32::INFINITY,
                secure: false,
            };
            let mut document = TextDocumentLayout::new(TextBuffer::new(&line), geometry);
            let measured = document.measured_line(0, &environment);

            assert_eq!(measured.rows.len(), 1, "an unwrapped line stays one row");
            let glyphs = &measured.rows[0].glyphs;
            assert!(
                glyphs.len() >= line.chars().count() - 3_000,
                "most characters shape, got {} glyphs for {} characters",
                glyphs.len(),
                line.chars().count()
            );
            assert!(
                glyphs.windows(2).all(|pair| pair[0].x <= pair[1].x),
                "glyph positions advance monotonically"
            );
            assert!(
                glyphs.last().map(|glyph| glyph.cluster).unwrap_or(0) as usize >= line.len() - 2,
                "the final glyph reaches the end of the line"
            );

            let whole = environment.cached_layout(
                &line[..110],
                Some(&environment.default_font),
                Some(20.0),
                f32::INFINITY,
                cove_core::TextAlignment::Leading,
                0.0,
            );
            let caret = document.caret(110, false, &environment).0;
            assert!(
                (caret - whole.lines[0].width).abs() < 2.0,
                "the caret at character 110 matches the shaped width of the prefix: {caret} vs {}",
                whole.lines[0].width
            );
            assert_eq!(
                document.character(0, measured.rows[0].width + 10.0, &environment),
                line.chars().count(),
                "a point past the row's end maps to the line's last character"
            );
        });
    });
}

#[test]
fn horizontal_culling_keeps_only_glyphs_near_the_viewport() {
    let glyphs: Vec<PositionedGlyph> = (0..2_000)
        .map(|index| PositionedGlyph {
            glyph_id: 1,
            font_id: 0,
            cluster: index as u32,
            x: index as f32 * 10.0,
            y: 0.0,
            atlas_info: None,
        })
        .collect();
    let layout = TextLayoutResult {
        lines: vec![cove_core::TextLine {
            glyphs,
            baseline_y: 10.0,
            top_y: 0.0,
            width: 20_000.0,
            start_cluster: 0,
            end_cluster: 2_000,
        }],
        total_width: 20_000.0,
        total_height: 20.0,
    };

    let culled = visible_layout(&layout, 0.0, 20.0, 20.0, Some((5_000.0, 5_300.0)));
    let kept = &culled.lines[0].glyphs;
    assert!(
        kept.len() < 100,
        "culling keeps a window of glyphs, kept {}",
        kept.len()
    );
    assert!(
        kept.first().map(|glyph| glyph.x).unwrap_or(0.0) <= 5_000.0
            && kept.last().map(|glyph| glyph.x).unwrap_or(0.0) >= 5_300.0,
        "the kept window brackets the requested viewport"
    );
    assert_eq!(
        culled.lines[0].width, 20_000.0,
        "culling preserves the row's full width"
    );
}

const _: fn() = || {
    let _ = |_: &TextEnvironment| {};
};
