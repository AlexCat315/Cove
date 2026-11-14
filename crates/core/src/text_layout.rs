//! Translation of CoveUICore/TextLayout.swift.
use crate::{
    BidiParagraphs, GlyphAtlasInfo, GlyphMetrics, GlyphMetricsProvider, ShapedGlyph, is_newline,
    line_break_opportunities,
};
use std::collections::HashMap;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlignment {
    #[default]
    Leading,
    Center,
    Trailing,
}
#[derive(Clone, Debug, Default)]
pub struct PositionedGlyph {
    pub glyph_id: u32,
    pub font_id: i64,
    pub cluster: u32,
    pub x: f32,
    pub y: f32,
    pub atlas_info: Option<GlyphAtlasInfo>,
}
#[derive(Clone, Debug, Default)]
pub struct TextLine {
    pub glyphs: Vec<PositionedGlyph>,
    pub baseline_y: f32,
    pub top_y: f32,
    pub width: f32,
    pub start_cluster: u32,
    pub end_cluster: u32,
}
#[derive(Clone, Debug, Default)]
pub struct TextLayoutResult {
    pub lines: Vec<TextLine>,
    pub total_width: f32,
    pub total_height: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct TextLayoutOptions {
    pub max_width: f32,
    pub line_height: f32,
    pub alignment: TextAlignment,
    pub letter_spacing: f32,
}
impl Default for TextLayoutOptions {
    fn default() -> Self {
        Self {
            max_width: f32::INFINITY,
            line_height: 16.0,
            alignment: TextAlignment::Leading,
            letter_spacing: 0.0,
        }
    }
}
type Measured = (ShapedGlyph, Option<GlyphMetrics>);

/// Slack allowed when fitting text into a width.
///
/// A container sizes itself from its text's natural width and then hands the
/// text back `container - padding - siblings`. Those f32 sums lose a few ulps,
/// so the text can be offered a hair less than the width it reported. Without
/// slack that hair wraps the last glyph or truncates the label. 1/64 px is the
/// sub-pixel unit browsers lay text out in and is far below one device pixel.
pub const TEXT_FIT_TOLERANCE: f32 = 1.0 / 64.0;

/// Whether text of `extent` does not fit in `limit`.
#[inline]
pub fn text_overflows(extent: f32, limit: f32) -> bool {
    extent > limit + TEXT_FIT_TOLERANCE
}
pub fn layout_text(
    shaped: &[ShapedGlyph],
    text: &str,
    atlas: &dyn GlyphMetricsProvider,
    options: TextLayoutOptions,
) -> TextLayoutResult {
    if shaped.is_empty() {
        return TextLayoutResult::default();
    }
    let utf8 = text.as_bytes();
    let bidi =
        BidiParagraphs::contains_right_to_left(text).then(|| BidiParagraphs::new(text, false));
    let shaped = if let Some(bidi) = &bidi {
        logical_glyph_order(shaped, bidi)
    } else {
        shaped.to_vec()
    };
    let newlines: HashMap<u32, u32> = text
        .grapheme_indices(true)
        .filter(|(_, c)| is_newline(c))
        .map(|(i, c)| (i as u32, c.len() as u32))
        .collect();
    let spacing = if options.letter_spacing.is_finite() {
        options.letter_spacing
    } else {
        0.0
    };
    let options = TextLayoutOptions {
        letter_spacing: spacing,
        ..options
    };
    let soft_breaks = if options.max_width.is_finite() {
        line_break_opportunities(text)
    } else {
        Default::default()
    };
    let measured: Vec<_> = shaped
        .iter()
        .map(|g| (*g, atlas.glyph_metrics(g.glyph_id, g.font_id)))
        .collect();
    let baseline = centered_baseline_y(&measured, atlas, 0.0, options.line_height);
    let mut last_was_newline = false;
    let mut lines = Vec::new();
    let mut current: Vec<Measured> = Vec::new();
    let mut start = 0;
    let mut pen_x = 0.0;
    let mut last_break = None;
    for (glyph, metrics) in measured {
        let cluster = glyph.cluster as usize;
        let byte = utf8.get(cluster).copied().unwrap_or(0);
        if newlines.contains_key(&glyph.cluster) || byte == b'\n' || byte == b'\r' {
            if glyph.cluster < start {
                continue;
            }
            last_was_newline = true;
            lines.push(build_line(
                &current,
                baseline,
                lines.len(),
                start,
                glyph.cluster,
                options,
                bidi.as_ref(),
            ));
            current.clear();
            start = glyph.cluster + newlines.get(&glyph.cluster).copied().unwrap_or(1);
            pen_x = 0.0;
            last_break = None;
            continue;
        }
        last_was_newline = false;
        if matches!(byte, b' ' | b'\t')
            || (soft_breaks.contains(&cluster)
                && current.last().is_none_or(|g| g.0.cluster != glyph.cluster)
                && cluster > 0
                && utf8
                    .get(cluster - 1)
                    .is_some_and(|b| !matches!(b, b' ' | b'\t')))
        {
            last_break = Some(current.len());
        }
        let gap = if current.last().is_some_and(|g| g.0.cluster != glyph.cluster) {
            spacing
        } else {
            0.0
        };
        let mut next_pen_x = pen_x + gap + glyph.x_advance;
        let mut wrapped = false;
        if text_overflows(next_pen_x, options.max_width)
            && !current.is_empty()
            && current.last().is_some_and(|g| g.0.cluster != glyph.cluster)
        {
            wrapped = true;
            if let Some(index) = last_break.filter(|&i| i > 0) {
                let remaining: Vec<_> = current[index..]
                    .iter()
                    .copied()
                    .skip_while(|g| {
                        utf8.get(g.0.cluster as usize)
                            .is_some_and(|b| matches!(b, b' ' | b'\t'))
                    })
                    .collect();
                let next_start = remaining
                    .first()
                    .map(|g| g.0.cluster)
                    .unwrap_or(glyph.cluster);
                lines.push(build_line(
                    &current[..index],
                    baseline,
                    lines.len(),
                    start,
                    next_start,
                    options,
                    bidi.as_ref(),
                ));
                current = remaining;
                start = next_start;
                pen_x = glyph_width(&current, spacing);
                let remaining_gap = if current.last().is_some_and(|g| g.0.cluster != glyph.cluster)
                {
                    spacing
                } else {
                    0.0
                };
                next_pen_x = pen_x + remaining_gap + glyph.x_advance;
                last_break = None;
            } else {
                lines.push(build_line(
                    &current,
                    baseline,
                    lines.len(),
                    start,
                    glyph.cluster,
                    options,
                    bidi.as_ref(),
                ));
                current.clear();
                start = glyph.cluster;
                next_pen_x = glyph.x_advance;
                last_break = None;
            }
        }
        // The space that overflowed hangs at the end of the line it ended,
        // the same as spaces after an earlier break, instead of indenting the
        // next line.
        if wrapped && current.is_empty() && matches!(byte, b' ' | b'\t') {
            pen_x = 0.0;
            continue;
        }
        if current.is_empty() {
            start = glyph.cluster;
        }
        current.push((glyph, metrics));
        pen_x = next_pen_x;
    }
    if !current.is_empty() || last_was_newline {
        lines.push(build_line(
            &current,
            baseline,
            lines.len(),
            start,
            text.len() as u32,
            options,
            bidi.as_ref(),
        ));
    }
    TextLayoutResult {
        total_width: lines.iter().map(|l| l.width).fold(0.0, f32::max),
        total_height: lines.len() as f32 * options.line_height,
        lines,
    }
}
fn build_line(
    glyphs: &[Measured],
    baseline: f32,
    index: usize,
    start: u32,
    end: u32,
    options: TextLayoutOptions,
    bidi: Option<&BidiParagraphs>,
) -> TextLine {
    let glyphs = bidi.map_or_else(|| glyphs.to_vec(), |b| visual_glyph_order(glyphs, b));
    let rtl = bidi.is_some_and(|b| b.paragraph_at_byte(start as usize).is_right_to_left());
    let top_y = index as f32 * options.line_height;
    let baseline_y = top_y + baseline;
    let mut positioned = Vec::with_capacity(glyphs.len());
    let mut pen_x = 0.0;
    let mut previous = None;
    for (shaped, _) in glyphs {
        if previous.is_some_and(|cluster| cluster != shaped.cluster) {
            pen_x += options.letter_spacing;
        }
        previous = Some(shaped.cluster);
        positioned.push(PositionedGlyph {
            glyph_id: shaped.glyph_id,
            font_id: shaped.font_id,
            cluster: shaped.cluster,
            x: pen_x + shaped.x_offset,
            y: baseline_y + shaped.y_offset,
            atlas_info: None,
        });
        pen_x += shaped.x_advance;
    }
    let offset = match options.alignment {
        TextAlignment::Leading if rtl => options.max_width - pen_x,
        TextAlignment::Center => (options.max_width - pen_x) / 2.0,
        TextAlignment::Trailing if !rtl => options.max_width - pen_x,
        _ => 0.0,
    };
    if offset != 0.0 && offset.is_finite() {
        for glyph in &mut positioned {
            glyph.x += offset;
        }
    }
    TextLine {
        glyphs: positioned,
        baseline_y,
        top_y,
        width: pen_x,
        start_cluster: start,
        end_cluster: end,
    }
}
pub fn logical_glyph_order(glyphs: &[ShapedGlyph], bidi: &BidiParagraphs) -> Vec<ShapedGlyph> {
    let mut indexed: Vec<_> = glyphs.iter().copied().enumerate().collect();
    indexed.sort_by(|(i, a), (j, b)| {
        a.cluster.cmp(&b.cluster).then_with(|| {
            if bidi.level_at_byte(a.cluster as usize) & 1 == 1 {
                j.cmp(i)
            } else {
                i.cmp(j)
            }
        })
    });
    indexed.into_iter().map(|(_, glyph)| glyph).collect()
}
pub fn visual_glyph_order<T: Clone>(
    glyphs: &[(ShapedGlyph, T)],
    bidi: &BidiParagraphs,
) -> Vec<(ShapedGlyph, T)> {
    let Some(first) = glyphs.first() else {
        return vec![];
    };
    let base = bidi.paragraph_at_byte(first.0.cluster as usize).base_level;
    let mut levels: Vec<_> = glyphs
        .iter()
        .map(|g| bidi.level_at_byte(g.0.cluster as usize))
        .collect();
    for index in (0..glyphs.len()).rev() {
        if !bidi.is_trailing_whitespace(glyphs[index].0.cluster as usize) {
            break;
        }
        levels[index] = base;
    }
    BidiParagraphs::visual_order(&levels)
        .into_iter()
        .map(|i| glyphs[i].clone())
        .collect()
}
fn glyph_width(glyphs: &[Measured], spacing: f32) -> f32 {
    let mut width = 0.0;
    let mut previous = None;
    for (glyph, _) in glyphs {
        if previous.is_some_and(|cluster| cluster != glyph.cluster) {
            width += spacing;
        }
        previous = Some(glyph.cluster);
        width += glyph.x_advance;
    }
    width
}
fn centered_baseline_y(
    glyphs: &[Measured],
    atlas: &dyn GlyphMetricsProvider,
    line_top: f32,
    line_height: f32,
) -> f32 {
    let mut ascent: f32 = 0.0;
    let mut descent: f32 = 0.0;
    for (shaped, metrics) in glyphs {
        if let Some(metrics) = atlas.line_metrics(shaped.font_id) {
            ascent = ascent.max(metrics.ascent);
            descent = descent.max(metrics.descent);
        } else if let Some(metrics) = metrics.filter(|m| m.height > 0.0) {
            let top = shaped.y_offset - metrics.bearing_y;
            ascent = ascent.max(-top);
            descent = descent.max(top + metrics.height);
        }
    }
    if ascent <= 0.0 && descent <= 0.0 {
        line_top + line_height
    } else {
        line_top + (line_height - ascent - descent) * 0.5 + ascent
    }
}
