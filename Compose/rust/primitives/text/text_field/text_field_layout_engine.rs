//! Translation of Primitives/Text/TextField/TextFieldLayoutEngine.swift.
//!
//! Render-state derivation, document caching, viewport projection and the
//! scrollbar geometry a text field paints. All caches hang off attachments so a
//! repeated frame reuses the shaped rows instead of measuring again.
use std::collections::HashMap;
use std::rc::Rc;

use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_document_layout::{TextDocumentLayout, TextLineGeometry};
use crate::primitives::text::text_field::text_field_horizontal_scroll::HorizontalScrollState;
use cove_core::scene::{Node, Point, Rect};
use cove_core::{Font, TextLayoutResult, TextLine};
use cove_text::TextEnvironment;

pub const RENDER_CACHE_KEY: &str = "__textfield_render_cache";
pub const DOCUMENT_LAYOUT_KEY: &str = "__textfield_document_layout";
pub const FIELD_STATE_KEY: &str = "__textfield_state";

/// What the field paints for the current interaction state.
#[derive(Clone, Debug)]
pub struct RenderState {
    pub buffer: TextBuffer,
    pub cursor_index: usize,
    pub composition_range: Option<(usize, usize)>,
    pub shows_placeholder: bool,
    pub is_composing: bool,
}

/// Cache identity for one shaped render pass.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderCacheKey {
    pub root: usize,
    pub visible_first_row: usize,
    pub visible_row_count: usize,
    pub total_rows: usize,
    pub font: Font,
    pub line_height: f32,
    pub letter_spacing: f32,
    pub atlas: u64,
    pub available_text_width: f32,
    pub secure: bool,
    pub hidden_lines: Vec<std::ops::Range<usize>>,
}

/// One cached measure pass: its key, the visible rows and the owning document.
#[derive(Clone)]
pub struct RenderCacheEntry {
    pub key: RenderCacheKey,
    pub layout: TextLayoutResult,
    pub document: Rc<std::cell::RefCell<TextDocumentLayout>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CaretLocation {
    pub x: f32,
    pub top_y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportMetrics {
    pub text_origin_x: f32,
    pub text_origin_y: f32,
    pub available_text_width: f32,
    pub raw_caret: CaretLocation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollbarMetrics {
    pub track_rect: Rect,
    pub thumb_rect: Rect,
}

/// Vertical and horizontal scroll for one field session.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldScrollState {
    pub horizontal: HorizontalScrollState,
    pub offset_y: f32,
    pub max_y: f32,
    pub visible_height: f32,
    pub content_height: f32,
    pub needs_caret_reveal: bool,
}

impl FieldScrollState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FieldSelectionState {
    pub cursor_index: usize,
    pub anchor: Option<usize>,
    pub line_end_affinity: bool,
    pub preferred_caret_x: Option<u32>,
}

impl FieldSelectionState {
    pub fn preferred_caret_x(&self) -> Option<f32> {
        self.preferred_caret_x.map(f32::from_bits)
    }

    pub fn set_preferred_caret_x(&mut self, value: Option<f32>) {
        self.preferred_caret_x = value.map(f32::to_bits);
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldCompositionState {
    pub text: String,
    pub start: usize,
    pub length: usize,
}

impl FieldCompositionState {
    pub fn is_active(&self) -> bool {
        !self.text.is_empty()
    }
}

/// Cached composition preview so a keystroke does not rebuild the preview
/// buffer while the IME composition is unchanged.
#[derive(Clone)]
pub struct CompositionPreview {
    pub source: TextBuffer,
    pub range: (usize, usize),
    pub text: String,
    pub buffer: TextBuffer,
}

/// Render-draft caches for the placeholder and the active composition.
#[derive(Clone, Default)]
pub struct RenderDraftState {
    pub placeholder: String,
    pub placeholder_buffer: Option<TextBuffer>,
    pub preview: Option<CompositionPreview>,
}

/// Layout inputs the engine needs from the field's authored configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldLayoutInputs {
    pub is_vertical: bool,
    pub wraps_lines: bool,
    pub secure: bool,
    pub horizontal_inset: f32,
    pub vertical_inset: f32,
    pub text_origin_y_offset: f32,
}

/// Derives the render state for the current buffer, selection and composition.
pub fn make_render_state(
    inputs: &FieldLayoutInputs,
    draft: &mut RenderDraftState,
    current: &TextBuffer,
    selection: &FieldSelectionState,
    composition: &FieldCompositionState,
    placeholder: &str,
    is_focused: bool,
) -> RenderState {
    let _ = inputs;
    if !(is_focused && composition.is_active()) {
        if current.is_empty() {
            if draft.placeholder != placeholder {
                draft.placeholder = placeholder.to_string();
                draft.placeholder_buffer = Some(TextBuffer::new(placeholder));
            }
            return RenderState {
                buffer: draft
                    .placeholder_buffer
                    .clone()
                    .unwrap_or_else(|| TextBuffer::new(placeholder)),
                cursor_index: 0,
                composition_range: None,
                shows_placeholder: true,
                is_composing: false,
            };
        }
        return RenderState {
            buffer: current.clone(),
            cursor_index: selection.cursor_index.min(current.character_count()),
            composition_range: None,
            shows_placeholder: false,
            is_composing: false,
        };
    }
    let cursor = selection.cursor_index.min(current.character_count());
    let range = selection_range(current, selection).unwrap_or((cursor, cursor));
    let preview = match draft.preview.as_ref() {
        Some(cached)
            if cached.source.has_same_contents(current)
                && cached.range == range
                && cached.text == composition.text =>
        {
            cached.buffer.clone()
        }
        _ => {
            let buffer = current.replace(range.0..range.1, &composition.text);
            draft.preview = Some(CompositionPreview {
                source: current.clone(),
                range,
                text: composition.text.clone(),
                buffer: buffer.clone(),
            });
            buffer
        }
    };
    let start_byte = current.utf8_offset_for_character_index(range.0);
    let end_byte = start_byte + composition.text.len();
    let offset = if composition.length > 0 {
        composition.start + composition.length
    } else {
        composition.text.chars().count()
    };
    let prefix: String = composition
        .text
        .chars()
        .take(offset.min(composition.text.chars().count()))
        .collect();
    let caret_byte = start_byte + prefix.len();
    let mut first = preview.character_index_for_utf8_offset(start_byte);
    if preview.utf8_offset_for_character_index(first) > start_byte {
        first = first.saturating_sub(1);
    }
    RenderState {
        cursor_index: preview.character_index_for_utf8_offset(caret_byte),
        composition_range: Some((first, preview.character_index_for_utf8_offset(end_byte))),
        buffer: preview,
        shows_placeholder: false,
        is_composing: true,
    }
}

pub fn selection_range(
    buffer: &TextBuffer,
    selection: &FieldSelectionState,
) -> Option<(usize, usize)> {
    let anchor = selection.anchor?;
    if anchor == selection.cursor_index {
        return None;
    }
    let count = buffer.character_count();
    let lower = anchor.min(selection.cursor_index).min(count);
    let upper = anchor.max(selection.cursor_index).min(count);
    if lower == upper {
        None
    } else {
        Some((lower, upper))
    }
}

/// Geometry for one document revision.
pub fn document_geometry(
    inputs: &FieldLayoutInputs,
    font: Font,
    line_height: f32,
    letter_spacing: f32,
    atlas: u64,
    available_text_width: f32,
    is_placeholder: bool,
) -> TextLineGeometry {
    let width = if inputs.is_vertical && inputs.wraps_lines {
        available_text_width.max(1.0)
    } else {
        f32::INFINITY
    };
    TextLineGeometry {
        font,
        line_height,
        letter_spacing,
        atlas,
        width,
        secure: inputs.secure && !is_placeholder,
    }
}

/// Update the scroll bounds for a freshly measured document.
pub fn update_scroll(
    scroll: &mut FieldScrollState,
    document: &TextDocumentLayout,
    line_height: f32,
) {
    scroll.content_height = document.index().row_count() as f32 * line_height;
    scroll.max_y = (scroll.content_height - scroll.visible_height).max(0.0);
    scroll.offset_y = scroll.offset_y.clamp(0.0, scroll.max_y);
}

/// Rows inside the vertical window. With `visible_x`, a long row keeps only
/// glyphs near the horizontal window, so a minified line draws in proportion to
/// the viewport rather than its length.
pub fn visible_layout(
    layout: &TextLayoutResult,
    scroll_offset_y: f32,
    visible_height: f32,
    line_height: f32,
    visible_x: Option<(f32, f32)>,
) -> TextLayoutResult {
    let mut lines: Vec<TextLine> = layout
        .lines
        .iter()
        .filter(|line| {
            line.baseline_y + line_height >= scroll_offset_y - line_height
                && line.baseline_y - line_height <= scroll_offset_y + visible_height + line_height
        })
        .cloned()
        .collect();
    if let Some((low, high)) = visible_x {
        lines = lines
            .into_iter()
            .map(|line| {
                // Bidi rows are not ordered by x; they keep every glyph.
                let sorted = line.glyphs.windows(2).all(|pair| pair[0].x <= pair[1].x);
                if line.glyphs.len() <= 512 || !sorted {
                    return line;
                }
                let margin = (high - low).max(64.0);
                let first_at_least = |x: f32| -> usize {
                    let (mut lower, mut upper) = (0usize, line.glyphs.len());
                    while lower < upper {
                        let middle = (lower + upper) / 2;
                        if line.glyphs[middle].x < x {
                            lower = middle + 1
                        } else {
                            upper = middle
                        }
                    }
                    lower
                };
                let lower = first_at_least(low - margin);
                let upper = first_at_least(high + margin);
                TextLine {
                    glyphs: line.glyphs[lower.min(line.glyphs.len())..upper.min(line.glyphs.len())]
                        .to_vec(),
                    ..line
                }
            })
            .collect();
    }
    TextLayoutResult {
        lines,
        total_width: layout.total_width,
        total_height: layout.total_height,
    }
}

/// Vertical scrollbar geometry, or `None` while the document fits.
pub fn scrollbar_metrics(
    scroll: &FieldScrollState,
    node: &Node,
    origin: Point,
    thickness: f32,
    inset: f32,
) -> Option<ScrollbarMetrics> {
    if scroll.max_y <= 0.0 || scroll.content_height <= scroll.visible_height {
        return None;
    }
    let frame = node.frame();
    let x = origin.x + frame.width - thickness - inset;
    let y = origin.y + inset;
    let height = (frame.height - inset * 2.0).max(thickness * 2.0);
    let thumb_height =
        (height * scroll.visible_height / scroll.content_height.max(1.0)).max(thickness * 2.0);
    let progress = scroll.offset_y / scroll.max_y;
    Some(ScrollbarMetrics {
        track_rect: Rect::new(x, y, thickness, height),
        thumb_rect: Rect::new(
            x,
            y + (height - thumb_height) * progress,
            thickness,
            thumb_height,
        ),
    })
}

/// Horizontal scrollbar geometry, or `None` while the row fits.
pub fn horizontal_scrollbar_metrics(
    scroll: &FieldScrollState,
    node: &Node,
    origin: Point,
    thickness: f32,
    inset: f32,
) -> Option<ScrollbarMetrics> {
    let horizontal = &scroll.horizontal;
    if horizontal.maximum() <= 0.0 || horizontal.viewport_width <= 0.0 {
        return None;
    }
    let frame = node.frame();
    let width = (horizontal.viewport_width - inset * 2.0).max(thickness * 2.0);
    let thumb = (width * horizontal.viewport_width / horizontal.content_width).max(thickness * 2.0);
    let x = origin.x + horizontal.leading_inset + inset;
    let y = origin.y + frame.height - thickness - inset;
    Some(ScrollbarMetrics {
        track_rect: Rect::new(x, y, width, thickness),
        thumb_rect: Rect::new(
            x + (width - thumb) * horizontal.offset / horizontal.maximum(),
            y,
            thumb,
            thickness,
        ),
    })
}

/// Glyph x for a UTF-8 byte inside one shaped row, clamped to the row ends.
pub fn row_x_at_byte(line: &TextLine, byte: usize) -> f32 {
    if byte <= line.start_cluster as usize {
        return 0.0;
    }
    if byte >= line.end_cluster as usize {
        return line.width;
    }
    line.glyphs
        .iter()
        .find(|glyph| glyph.cluster as usize >= byte)
        .map_or(line.width, |glyph| glyph.x)
}

/// Per-node cache slots for the document and the render pass.
pub struct FieldLayoutCache {
    pub document: std::cell::RefCell<Option<Rc<std::cell::RefCell<TextDocumentLayout>>>>,
    pub render: std::cell::RefCell<Option<RenderCacheEntry>>,
}

impl FieldLayoutCache {
    pub fn new() -> Self {
        Self {
            document: std::cell::RefCell::new(None),
            render: std::cell::RefCell::new(None),
        }
    }
}

impl Default for FieldLayoutCache {
    fn default() -> Self {
        Self::new()
    }
}

const _: fn(&TextEnvironment) -> HashMap<usize, usize> = |_| HashMap::new();
