//! Measured lines, glyphs and span fragments of one rich paragraph at one
//! width. Offsets are UTF-8 byte offsets into the paragraph text.

use super::run::{RichTextRun, RichTextTraits};
use cove_core::{
    line_break_opportunities, BidiParagraphs, Font, PositionedGlyph, TextAlignment, TextLayoutResult,
    TextLine,
};
use cove_text::TextEnvironment;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct RichParagraphSpan {
    pub text: String,
    pub font: Font,
    pub line_height: f32,
    pub traits: RichTextTraits,
    /// Index into the paragraph's link destinations.
    pub link: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Hash)]
pub struct RichParagraph {
    pub spans: Vec<RichParagraphSpan>,
    pub alignment: TextAlignment,
    /// Code listings keep authored lines and scroll instead of wrapping.
    pub wraps: bool,
}

impl RichParagraph {
    pub fn new(spans: Vec<RichParagraphSpan>) -> Self {
        Self {
            spans,
            alignment: TextAlignment::Leading,
            wraps: true,
        }
    }

    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

impl Default for RichParagraph {
    fn default() -> Self {
        Self {
            spans: Vec::new(),
            alignment: TextAlignment::Leading,
            wraps: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RichTextCaret {
    pub offset: usize,
    pub x: f32,
}

#[derive(Clone, Debug)]
pub struct RichTextLine {
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
    pub width: f32,
    pub start_offset: usize,
    pub end_offset: usize,
    /// Sorted by offset; the last entry is the visual end of the line.
    pub carets: Vec<RichTextCaret>,
    /// Base direction of the paragraph this line belongs to.
    pub is_right_to_left: bool,
}

/// One span's extent on one line.
#[derive(Clone, Debug)]
pub struct RichTextFragment {
    pub span: usize,
    pub line: usize,
    pub min_x: f32,
    pub max_x: f32,
    pub ascent: f32,
    pub descent: f32,
}

#[derive(Clone)]
pub struct RichTextFlow {
    pub text: String,
    pub lines: Vec<RichTextLine>,
    pub fragments: Vec<RichTextFragment>,
    pub glyphs: TextLayoutResult,
    pub span_starts: Vec<usize>,
    pub width: f32,
    pub height: f32,
}

impl RichTextFlow {
    pub fn span(&self, offset: usize) -> Option<usize> {
        if self.span_starts.is_empty() {
            return None;
        }
        let mut low = 0usize;
        let mut high = self.span_starts.len() - 1;
        while low < high {
            let mid = (low + high + 1) / 2;
            if self.span_starts[mid] <= offset {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        Some(low)
    }

    pub fn fragment(&self, x: f32, y: f32) -> Option<&RichTextFragment> {
        let line = self.line_index(y)?;
        if y < 0.0 || y >= self.height {
            return None;
        }
        self.fragments
            .iter()
            .find(|fragment| fragment.line == line && x >= fragment.min_x && x < fragment.max_x)
    }

    pub fn line_index(&self, y: f32) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        Some(
            self.lines
                .iter()
                .rposition(|line| line.top <= y)
                .unwrap_or(0),
        )
    }

    /// The caret offset nearest to a point, clamped to the paragraph.
    pub fn offset(&self, x: f32, y: f32) -> usize {
        let Some(index) = self.line_index(y) else {
            return 0;
        };
        if y < 0.0 {
            return 0;
        }
        if y >= self.height {
            return self.text.len();
        }
        let carets = &self.lines[index].carets;
        let Some(mut best) = carets.first().copied() else {
            return self.lines[index].start_offset;
        };
        for caret in carets {
            if (caret.x - x).abs() < (best.x - x).abs() {
                best = *caret;
            }
        }
        best.offset
    }

    pub fn caret_x(&self, offset: usize, line: usize) -> f32 {
        let carets = &self.lines[line].carets;
        carets
            .iter()
            .rev()
            .find(|caret| caret.offset <= offset)
            .or_else(|| carets.first())
            .map_or(0.0, |caret| caret.x)
    }

    /// Zero-width caret rectangle at a UTF-8 offset, in paragraph-local space.
    pub fn caret_rect(&self, offset: usize) -> (f32, f32, f32, f32) {
        if self.lines.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let index = self
            .lines
            .iter()
            .rposition(|line| line.start_offset <= offset)
            .unwrap_or(0);
        let line = &self.lines[index];
        (self.caret_x(offset, index), line.top, 0.0, line.height)
    }

    /// Highlight rectangles (x, y, width, height) for a UTF-8 range.
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<(f32, f32, f32, f32)> {
        if range.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            let lower = range.start.max(line.start_offset);
            let upper = range.end.min(line.end_offset);
            if !(lower < upper
                || (range.start <= line.start_offset
                    && line.start_offset < range.end
                    && line.start_offset == line.end_offset))
            {
                continue;
            }
            let start = self.caret_x(lower, index);
            let mut end = if upper >= line.end_offset {
                line.carets.last().map_or(start, |caret| caret.x)
            } else {
                self.caret_x(upper, index)
            };
            // A selected line break shows as a short tail, as in text editors.
            if range.end > line.end_offset && index + 1 < self.lines.len() {
                end += line.height * 0.3;
            }
            result.push((start, line.top, (end - start).max(2.0), line.height));
        }
        result
    }

    pub fn layout(
        paragraph: &RichParagraph,
        max_width: f32,
        env: &TextEnvironment,
    ) -> Rc<RichTextFlow> {
        let limit = if paragraph.wraps && max_width.is_finite() && max_width > 0.0 {
            max_width
        } else {
            f32::INFINITY
        };
        let key = FlowCacheKey {
            paragraph: paragraph.clone(),
            max_width: limit,
            atlas: env.atlas.identity(),
        };
        CACHE.with(|cache| {
            if let Some(hit) = cache.borrow().get(&key) {
                return hit.clone();
            }
            let result = Rc::new(FlowBuilder {
                paragraph: paragraph.clone(),
                max_width: limit,
                env: env.clone(),
            }
            .build());
            let mut table = cache.borrow_mut();
            // Widths change continuously while a window resizes; a bounded
            // table keeps that from growing without limit.
            if table.len() >= 512 {
                table.clear();
            }
            table.insert(key, result.clone());
            result
        })
    }
}

#[derive(Clone)]
struct FlowCacheKey {
    paragraph: RichParagraph,
    max_width: f32,
    atlas: u64,
}

impl PartialEq for FlowCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.paragraph == other.paragraph
            && self.atlas == other.atlas
            && float_bits(self.max_width) == float_bits(other.max_width)
    }
}
impl Eq for FlowCacheKey {}
impl Hash for FlowCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.paragraph.hash(state);
        self.atlas.hash(state);
        float_bits(self.max_width).hash(state);
    }
}
fn float_bits(value: f32) -> u32 {
    if value == 0.0 {
        0
    } else {
        value.to_bits()
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<FlowCacheKey, Rc<RichTextFlow>>> = RefCell::new(HashMap::new());
}

#[derive(Clone)]
struct Item {
    glyph: Option<cove_core::ShapedGlyph>,
    span: usize,
    cluster: usize,
    advance: f32,
    pad_before: f32,
    pad_after: f32,
    is_space: bool,
    break_before: bool,
}

impl Item {
    fn hard_break(span: usize, cluster: usize) -> Self {
        Self {
            glyph: None,
            span,
            cluster,
            advance: 0.0,
            pad_before: 0.0,
            pad_after: 0.0,
            is_space: false,
            break_before: false,
        }
    }

    fn is_hard_break(&self) -> bool {
        self.glyph.is_none()
    }

    fn extent(&self) -> f32 {
        self.pad_before + self.advance + self.pad_after
    }
}

struct FlowBuilder {
    paragraph: RichParagraph,
    max_width: f32,
    env: TextEnvironment,
}

struct PlacedLine {
    line: RichTextLine,
    fragments: Vec<RichTextFragment>,
    glyphs: TextLine,
}

impl FlowBuilder {
    fn build(&self) -> RichTextFlow {
        let text = self.paragraph.text();
        let bidi: Option<BidiParagraphs> = if BidiParagraphs::contains_right_to_left(&text) {
            Some(BidiParagraphs::new(&text, false))
        } else {
            None
        };
        let mut starts = Vec::new();
        let mut offset = 0usize;
        for span in &self.paragraph.spans {
            starts.push(offset);
            offset += span.text.len();
        }
        let items = self.make_items(&text, &starts, bidi.as_ref());
        let mut lines: Vec<RichTextLine> = Vec::new();
        let mut fragments: Vec<RichTextFragment> = Vec::new();
        let mut glyph_lines: Vec<TextLine> = Vec::new();
        let mut top: f32 = 0.0;
        for range in self.wrap(&items) {
            let placed = self.place(
                &items,
                range,
                &text,
                top,
                lines.len(),
                bidi.as_ref(),
            );
            lines.push(placed.line.clone());
            fragments.extend(placed.fragments);
            glyph_lines.push(placed.glyphs);
            top += placed.line.height;
        }
        if lines.is_empty() {
            if let Some(first) = self.paragraph.spans.first() {
                let empty = RichTextLine {
                    top: 0.0,
                    height: first.line_height,
                    baseline: first.line_height * 0.75,
                    width: 0.0,
                    start_offset: 0,
                    end_offset: 0,
                    carets: vec![RichTextCaret { offset: 0, x: 0.0 }],
                    is_right_to_left: false,
                };
                lines.push(empty);
                top = first.line_height;
            }
        }
        let width = lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let aligned = self.align(&lines, &glyph_lines, &fragments, width);
        RichTextFlow {
            text,
            lines: aligned.0,
            fragments: aligned.1,
            glyphs: TextLayoutResult {
                lines: aligned.2,
                total_width: width,
                total_height: top,
            },
            span_starts: starts,
            width,
            height: top,
        }
    }

    fn make_items(
        &self,
        text: &str,
        starts: &[usize],
        bidi: Option<&BidiParagraphs>,
    ) -> Vec<Item> {
        let breaks: HashSet<usize> = line_break_opportunities(text);
        let mut items: Vec<Item> = Vec::new();
        for (index, span) in self.paragraph.spans.iter().enumerate() {
            let pad: f32 = if span.traits.contains(RichTextTraits::CODE) {
                (span.font.size() * 0.32).round()
            } else {
                0.0
            };
            let first_item = items.len();
            let mut segment_start = starts[index];
            let segments: Vec<&str> = span.text.split('\n').collect();
            for (segment_index, segment) in segments.iter().enumerate() {
                if segment_index > 0 {
                    items.push(Item::hard_break(index, segment_start.saturating_sub(1)));
                }
                let utf8 = segment.as_bytes();
                let mut shaped: Vec<Item> = Vec::new();
                for glyph in self
                    .env
                    .shape(segment, Some(&span.font))
                    .iter()
                {
                    let local = glyph.cluster as usize;
                    let byte = utf8.get(local).copied().unwrap_or(0);
                    let cluster = segment_start + local;
                    shaped.push(Item {
                        glyph: Some(glyph.clone()),
                        span: index,
                        cluster,
                        advance: glyph.x_advance,
                        pad_before: 0.0,
                        pad_after: 0.0,
                        is_space: byte == 0x20 || byte == 0x09,
                        break_before: breaks.contains(&cluster),
                    });
                }
                // Wrapping works in logical order; right-to-left runs arrive visual.
                if let Some(bidi) = bidi {
                    let mut ordered: Vec<(usize, Item)> = shaped.into_iter().enumerate().collect();
                    ordered.sort_by(|(left_offset, left), (right_offset, right)| {
                        if left.cluster != right.cluster {
                            return left.cluster.cmp(&right.cluster);
                        }
                        if bidi.level_at_byte(left.cluster) & 1 == 1 {
                            right_offset.cmp(left_offset)
                        } else {
                            left_offset.cmp(right_offset)
                        }
                    });
                    shaped = ordered.into_iter().map(|(_, item)| item).collect();
                }
                items.extend(shaped);
                segment_start += utf8.len() + 1;
            }
            if pad > 0.0 && items.len() > first_item {
                if let Some(first) = items[first_item..]
                    .iter()
                    .position(|item| !item.is_hard_break())
                    .map(|offset| first_item + offset)
                {
                    items[first].pad_before = pad;
                }
                if let Some(last) = items[first_item..]
                    .iter()
                    .rposition(|item| !item.is_hard_break())
                    .map(|offset| first_item + offset)
                {
                    items[last].pad_after = pad;
                }
            }
        }
        // Runs shape separately, so a break between runs comes from the text.
        for index in 1..items.len() {
            if items[index].cluster != items[index - 1].cluster {
                let cluster = items[index].cluster;
                items[index].break_before = items[index].break_before || breaks.contains(&cluster);
            }
        }
        items
    }

    /// Greedy wrapping into item ranges; hard breaks end a line and are
    /// represented by the range that follows them.
    fn wrap(&self, items: &[Item]) -> Vec<Range<usize>> {
        let mut ranges: Vec<Range<usize>> = Vec::new();
        let mut start = 0usize;
        let mut width: f32 = 0.0;
        let mut index = 0usize;
        while index < items.len() {
            let item = &items[index];
            if item.is_hard_break() {
                ranges.push(start..index);
                start = index + 1;
                width = 0.0;
                index += 1;
                if start == items.len() {
                    ranges.push(start..start);
                }
                continue;
            }
            let breaks_here = index > start && items[index - 1].cluster != item.cluster;
            if width + item.extent() > self.max_width && breaks_here && !item.is_space {
                let soft = (start + 1..=index)
                    .rev()
                    .find(|position| {
                        items[*position].break_before && !items[*position].is_space
                    });
                if let Some(soft) = soft.filter(|soft| *soft > start) {
                    ranges.push(start..soft);
                    start = soft;
                    width = items[soft..index].iter().map(Item::extent).sum();
                } else {
                    ranges.push(start..index);
                    start = index;
                    width = 0.0;
                }
            }
            width += item.extent();
            index += 1;
        }
        if start < items.len() || (ranges.is_empty() && !items.is_empty()) {
            ranges.push(start..items.len());
        }
        ranges
    }

    fn place(
        &self,
        items: &[Item],
        range: Range<usize>,
        text: &str,
        top: f32,
        index: usize,
        bidi: Option<&BidiParagraphs>,
    ) -> PlacedLine {
        let logical = &items[range.clone()];
        let line_start = logical.first().map_or_else(
            || {
                if range.start > 0 {
                    items[range.start - 1].cluster + 1
                } else {
                    0
                }
            },
            |item| item.cluster,
        );
        let right_to_left = bidi
            .is_some_and(|bidi| bidi.paragraph_at_byte(line_start).is_right_to_left());
        let content: Vec<Item> = match bidi {
            Some(bidi) => self.visual_order(logical, bidi),
            None => logical.to_vec(),
        };
        let levels: Vec<u8> = content
            .iter()
            .map(|item| bidi.map_or(0, |bidi| bidi.level_at_byte(item.cluster)))
            .collect();
        let fallback_span = content.first().map_or_else(
            || {
                if range.start > 0 {
                    items[range.start - 1].span
                } else {
                    0
                }
            },
            |item| item.span,
        );
        let mut ascent: f32 = 0.0;
        let mut descent: f32 = 0.0;
        let mut height: f32 = 0.0;
        let mut metrics_by_span: HashMap<usize, (f32, f32)> = HashMap::new();
        for item in &content {
            let span = &self.paragraph.spans[item.span];
            height = height.max(span.line_height);
            if !metrics_by_span.contains_key(&item.span) {
                if let Some(glyph) = &item.glyph {
                    let metrics = self.env.atlas.line_metrics(glyph.font_id);
                    metrics_by_span.insert(
                        item.span,
                        (
                            metrics.as_ref().map_or(span.font.size() * 0.8, |m| m.ascent),
                            metrics.as_ref().map_or(span.font.size() * 0.2, |m| m.descent),
                        ),
                    );
                }
            }
            if let Some((a, d)) = metrics_by_span.get(&item.span) {
                ascent = ascent.max(*a);
                descent = descent.max(*d);
            }
        }
        if height == 0.0 {
            if let Some(span) = self.paragraph.spans.get(fallback_span) {
                height = span.line_height;
                ascent = span.font.size() * 0.8;
                descent = span.font.size() * 0.2;
            }
        }
        let baseline = (top + (height - ascent - descent) / 2.0 + ascent).round();

        let mut pen: f32 = 0.0;
        let mut visible_width: f32 = 0.0;
        let mut carets: Vec<RichTextCaret> = Vec::new();
        let mut positioned: Vec<PositionedGlyph> = Vec::new();
        let mut fragments: Vec<RichTextFragment> = Vec::new();
        for (position, item) in content.iter().enumerate() {
            let Some(glyph) = &item.glyph else { continue };
            if carets.last().map(|caret| caret.offset) != Some(item.cluster) {
                // A right-to-left character starts at its right edge.
                let odd = levels[position] & 1 == 1;
                carets.push(RichTextCaret {
                    offset: item.cluster,
                    x: if odd {
                        pen + item.extent() - item.pad_after
                    } else {
                        pen + item.pad_before
                    },
                });
            }
            let (a, d) = metrics_by_span
                .get(&item.span)
                .copied()
                .unwrap_or((ascent, descent));
            if fragments.last().map(|fragment| fragment.span) == Some(item.span) {
                if let Some(last) = fragments.last_mut() {
                    last.max_x = pen + item.extent();
                }
            } else {
                fragments.push(RichTextFragment {
                    span: item.span,
                    line: index,
                    min_x: pen,
                    max_x: pen + item.extent(),
                    ascent: a,
                    descent: d,
                });
            }
            positioned.push(PositionedGlyph {
                glyph_id: glyph.glyph_id,
                font_id: glyph.font_id,
                cluster: item.cluster as u32,
                x: pen + item.pad_before + glyph.x_offset,
                y: baseline + glyph.y_offset,
                atlas_info: None,
            });
            pen += item.extent();
            if !item.is_space {
                visible_width = pen;
            }
        }
        let start_offset = line_start;
        let end_offset = if range.end < items.len() {
            items[range.end].cluster
        } else {
            text.len()
        };
        carets.push(RichTextCaret {
            offset: end_offset,
            x: if right_to_left { 0.0 } else { pen },
        });
        if bidi.is_some() {
            carets.sort_by_key(|caret| caret.offset);
        }
        if carets.first().map(|caret| caret.offset) != Some(start_offset) {
            carets.insert(
                0,
                RichTextCaret {
                    offset: start_offset,
                    x: if right_to_left { pen } else { 0.0 },
                },
            );
        }
        let line = RichTextLine {
            top,
            height,
            baseline,
            width: visible_width,
            start_offset,
            end_offset,
            carets,
            is_right_to_left: right_to_left,
        };
        PlacedLine {
            line,
            fragments,
            glyphs: TextLine {
                glyphs: positioned,
                baseline_y: baseline,
                top_y: top,
                width: visible_width,
                start_cluster: start_offset as u32,
                end_cluster: end_offset as u32,
            },
        }
    }

    /// L1 returns trailing whitespace to the paragraph level; L2 then reverses
    /// runs from the highest level down to the lowest odd one.
    fn visual_order(&self, items: &[Item], bidi: &BidiParagraphs) -> Vec<Item> {
        let line: Vec<Item> = items.to_vec();
        let Some(first) = line.first() else {
            return line;
        };
        let base = bidi.paragraph_at_byte(first.cluster).base_level;
        let mut levels: Vec<u8> = line
            .iter()
            .map(|item| bidi.level_at_byte(item.cluster))
            .collect();
        let mut index = line.len();
        while index > 0 {
            index -= 1;
            if line[index].is_space || bidi.is_trailing_whitespace(line[index].cluster) {
                levels[index] = base;
            } else {
                break;
            }
        }
        BidiParagraphs::visual_order(&levels)
            .into_iter()
            .map(|position| line[position].clone())
            .collect()
    }

    fn align(
        &self,
        lines: &[RichTextLine],
        glyphs: &[TextLine],
        fragments: &[RichTextFragment],
        width: f32,
    ) -> (Vec<RichTextLine>, Vec<RichTextFragment>, Vec<TextLine>) {
        let box_width = if self.max_width.is_finite() {
            self.max_width
        } else {
            width
        };
        // Leading and trailing follow each line's paragraph direction.
        let factor = |line: &RichTextLine| -> f32 {
            match self.paragraph.alignment {
                TextAlignment::Leading => {
                    if line.is_right_to_left {
                        1.0
                    } else {
                        0.0
                    }
                }
                TextAlignment::Center => 0.5,
                TextAlignment::Trailing => {
                    if line.is_right_to_left {
                        0.0
                    } else {
                        1.0
                    }
                }
            }
        };
        if !lines.iter().any(|line| factor(line) > 0.0) {
            return (lines.to_vec(), fragments.to_vec(), glyphs.to_vec());
        }
        let shifts: Vec<f32> = lines
            .iter()
            .map(|line| ((box_width - line.width) * factor(line)).round())
            .collect();
        let shifted_lines: Vec<RichTextLine> = lines
            .iter()
            .zip(&shifts)
            .map(|(line, shift)| {
                let mut copy = line.clone();
                copy.carets = line
                    .carets
                    .iter()
                    .map(|caret| RichTextCaret {
                        offset: caret.offset,
                        x: caret.x + shift,
                    })
                    .collect();
                copy
            })
            .collect();
        let shifted_glyphs: Vec<TextLine> = glyphs
            .iter()
            .zip(&shifts)
            .map(|(line, shift)| TextLine {
                glyphs: line
                    .glyphs
                    .iter()
                    .map(|glyph| PositionedGlyph {
                        x: glyph.x + shift,
                        ..glyph.clone()
                    })
                    .collect(),
                ..line.clone()
            })
            .collect();
        let shifted_fragments: Vec<RichTextFragment> = fragments
            .iter()
            .map(|fragment| {
                let mut copy = fragment.clone();
                copy.min_x += shifts[fragment.line];
                copy.max_x += shifts[fragment.line];
                copy
            })
            .collect();
        (shifted_lines, shifted_fragments, shifted_glyphs)
    }
}
