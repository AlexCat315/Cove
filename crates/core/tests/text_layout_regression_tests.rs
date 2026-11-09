//! Translation of `Tests/CoveUIRuntimeTests/TextLayoutRegressionTests.swift`.
//!
//! Line boundaries are the contract every text surface reads: which byte starts
//! a line, which one ends it, and where the baseline sits. An off-by-one in a
//! CRLF or a U+2028 separator shifts every selection, caret and accessibility
//! range after it, so the source ranges are pinned exactly.
//!
//! Behaviour checklist:
//! - CRLF is one break; empty and trailing lines preserve their source ranges
//!   and baselines
//! - mixed fallback fonts share a baseline; a short line height distributes
//!   leading on both sides
//! - Unicode line and paragraph separators agree with the text buffer's line
//!   boundaries
//! - tracking is between clusters and wrapping never separates glyphs in one
//!   cluster
//! - ideographs wrap between characters and carry the previous one with
//!   closing punctuation

use cove_core::{
    GlyphLineMetrics, GlyphMetrics, GlyphMetricsProvider, ShapedGlyph, TextLayoutOptions,
    layout_text,
};

/// The Swift `Metrics` provider: no glyph metrics at all, so every measurement
/// comes from the line metrics alone.
struct Metrics;

impl GlyphMetricsProvider for Metrics {
    fn glyph_metrics(&self, _glyph_index: u32, _font_id: i64) -> Option<GlyphMetrics> {
        None
    }

    fn line_metrics(&self, font_id: i64) -> Option<GlyphLineMetrics> {
        Some(GlyphLineMetrics {
            ascent: if font_id == 0 { 12.0 } else { 14.0 },
            descent: if font_id == 0 { 4.0 } else { 6.0 },
            line_height: 20.0,
        })
    }
}

fn glyph(cluster: u32, advance: f32, font_id: i64) -> ShapedGlyph {
    ShapedGlyph {
        glyph_id: 1,
        x_offset: 0.0,
        y_offset: 0.0,
        x_advance: advance,
        y_advance: 0.0,
        cluster,
        font_id,
    }
}

fn options(max_width: f32, line_height: f32, letter_spacing: f32) -> TextLayoutOptions {
    TextLayoutOptions {
        max_width,
        line_height,
        letter_spacing,
        ..TextLayoutOptions::default()
    }
}

#[test]
fn crlf_is_one_break_and_empty_and_trailing_lines_preserve_their_source_ranges() {
    let text = "A\r\n\r\nB\n";
    let shaped: Vec<ShapedGlyph> = (0..text.len() as u32)
        .map(|cluster| glyph(cluster, 5.0, 0))
        .collect();
    let result = layout_text(&shaped, text, &Metrics, options(f32::INFINITY, 20.0, 0.0));

    assert_eq!(
        (result.lines.len(), result.total_height),
        (4, 80.0),
        "CRLF ends one line, not two",
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.start_cluster)
            .collect::<Vec<_>>(),
        vec![0, 3, 5, 7]
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.end_cluster)
            .collect::<Vec<_>>(),
        vec![1, 3, 6, 7]
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.baseline_y)
            .collect::<Vec<_>>(),
        vec![14.0, 34.0, 54.0, 74.0]
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.glyphs.len())
            .collect::<Vec<_>>(),
        vec![1, 0, 1, 0]
    );
}

#[test]
fn mixed_fallback_fonts_share_a_baseline_and_short_line_height_splits_the_leading() {
    let result = layout_text(
        &[glyph(0, 5.0, 0), glyph(1, 5.0, 0), glyph(2, 5.0, 1)],
        "A\nB",
        &Metrics,
        options(f32::INFINITY, 16.0, 0.0),
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.baseline_y)
            .collect::<Vec<_>>(),
        vec![12.0, 28.0],
        "both lines centre the leading on the shared 20pt font line height",
    );
}

#[test]
fn unicode_line_and_paragraph_separators_agree_with_the_text_buffer() {
    let result = layout_text(
        &[
            glyph(0, 5.0, 0),
            glyph(1, 5.0, 0),
            glyph(4, 5.0, 0),
            glyph(5, 5.0, 0),
            glyph(8, 5.0, 0),
        ],
        "A\u{2028}B\u{2029}C",
        &Metrics,
        options(f32::INFINITY, 20.0, 0.0),
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.start_cluster)
            .collect::<Vec<_>>(),
        vec![0, 4, 8]
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.end_cluster)
            .collect::<Vec<_>>(),
        vec![1, 5, 9]
    );
    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.glyphs.len())
            .collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
}

#[test]
fn tracking_is_between_clusters_and_wrapping_never_separates_one_cluster() {
    let glyphs = [glyph(0, 3.0, 0), glyph(0, 2.0, 0), glyph(3, 4.0, 0)];
    let spaced = layout_text(
        &glyphs,
        "a\u{301}B",
        &Metrics,
        options(f32::INFINITY, 20.0, 2.0),
    );
    assert_eq!(spaced.total_width, 11.0);
    assert_eq!(
        spaced.lines[0]
            .glyphs
            .iter()
            .map(|glyph| glyph.x)
            .collect::<Vec<_>>(),
        vec![0.0, 3.0, 7.0],
        "the second glyph of one cluster takes no extra tracking",
    );

    let wrapped = layout_text(&glyphs, "a\u{301}B", &Metrics, options(4.0, 20.0, 0.0));
    assert_eq!(
        wrapped
            .lines
            .iter()
            .map(|line| line.glyphs.len())
            .collect::<Vec<_>>(),
        vec![2, 1],
        "wrapping keeps the combining mark with its base",
    );

    let tracked_wrap = layout_text(
        &[glyph(0, 5.0, 0), glyph(1, 5.0, 0)],
        "AB",
        &Metrics,
        options(10.0, 20.0, 1.0),
    );
    assert_eq!(
        tracked_wrap
            .lines
            .iter()
            .map(|line| line.width)
            .collect::<Vec<_>>(),
        vec![5.0, 5.0],
        "a tracked line reports its advance without the trailing tracking",
    );
}

#[test]
fn ideographs_wrap_between_characters_and_carry_the_previous_one_with_closing_punctuation() {
    let text = "了解什么。";
    let clusters: Vec<u32> = text.char_indices().map(|(index, _)| index as u32).collect();
    let shaped: Vec<ShapedGlyph> = clusters
        .iter()
        .map(|cluster| glyph(*cluster, 10.0, 0))
        .collect();
    let result = layout_text(&shaped, text, &Metrics, options(40.0, 20.0, 0.0));

    assert_eq!(
        result
            .lines
            .iter()
            .map(|line| line.glyphs.len())
            .collect::<Vec<_>>(),
        vec![3, 2],
        "\u{3002} would overflow a four-character line, so it takes the previous glyph with it",
    );
    assert_eq!(result.lines[1].start_cluster, clusters[3]);
}

#[test]
fn a_space_that_overflows_hangs_instead_of_indenting_the_next_line() {
    // "ab cd" at 5pt per glyph in a 10pt column: the space is the glyph that
    // overflows, so the second line must start at "c", not at the space.
    let text = "ab cd";
    let shaped: Vec<ShapedGlyph> = (0..text.len() as u32)
        .map(|cluster| glyph(cluster, 5.0, 0))
        .collect();
    let result = layout_text(&shaped, text, &Metrics, options(10.0, 20.0, 0.0));
    assert_eq!(result.lines.len(), 2);
    assert_eq!(result.lines[1].start_cluster, 3);
    assert_eq!(
        result.lines[1]
            .glyphs
            .iter()
            .map(|glyph| glyph.cluster)
            .collect::<Vec<_>>(),
        vec![3, 4]
    );
}
