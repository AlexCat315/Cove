//! Static text and divider primitives from `Primitives/Text/Text.swift`.
use super::rich::text_selection::{TextSelectionChrome, TextSelectionScope};
use super::text_document_layout::MeasuredTextLine;
use crate::modifiers::style::{
    FONT, LETTER_SPACING, LINE_HEIGHT, TEXT_STYLE_SCALER, inherited_layout_style, text_style_font,
    text_style_letter_spacing, text_style_line_height,
};
use crate::theme::{TextScaler, TextStyleToken, theme_for_node};
use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{AccessibilityRole, AccessibilitySemantics, LayoutNode, MeasureMode, Node};
use cove_core::{
    Color, Font, TextAlignment, TextLayoutCacheEntry, TextLayoutCacheKey, TextLayoutResult,
    TextMeasureInputs,
};
use cove_text::{DrawListText, TextEnvironment, TextEnvironmentHolder};
use std::rc::Rc;

#[derive(Clone)]
pub struct Text {
    pub string: String,
    pub alignment: TextAlignment,
    pub color: Option<Color>,
    pub line_limit: Option<usize>,
    truncates: bool,
}

#[derive(Clone, PartialEq)]
struct PaintIdentity {
    string: String,
    alignment: TextAlignment,
    color: Option<Color>,
    line_limit: Option<usize>,
    truncates: bool,
}

#[derive(Clone)]
struct TextMetrics {
    font: Font,
    line_height: f32,
    letter_spacing: f32,
}

impl Text {
    pub fn new(string: impl Into<String>) -> Self {
        Self {
            string: string.into(),
            alignment: TextAlignment::Leading,
            color: None,
            line_limit: None,
            truncates: false,
        }
    }

    pub fn localized(key: impl Into<crate::i18n::LocalizedStringKey>) -> Self {
        Self::new(key.into().resolved())
    }

    pub fn with_alignment(mut self, alignment: TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn line_limit(mut self, limit: Option<usize>) -> Self {
        self.line_limit = limit;
        self
    }

    /// Draw one fitted line and record whether it was shortened.
    pub fn truncated(mut self) -> Self {
        self.truncates = true;
        self
    }

    fn metrics_for_layout(layout: &LayoutNode, environment: &TextEnvironment) -> TextMetrics {
        let token = layout
            .attachment::<TextStyleToken>(crate::theme::ThemeEnvironment::BODY_STYLE_ATTACHMENT);
        let scaler = layout
            .attachment::<TextScaler>(TEXT_STYLE_SCALER)
            .unwrap_or_default();
        let authored_font = inherited_layout_style::<Font>(layout, FONT);
        let font = scaler.scale_font(
            authored_font
                .as_ref()
                .or_else(|| token.as_ref().map(|token| &token.font))
                .unwrap_or(&environment.default_font),
        );
        let line_height = inherited_layout_style(layout, LINE_HEIGHT)
            .or_else(|| {
                token
                    .as_ref()
                    .map(|token| token.line_height_for(authored_font.as_ref()))
            })
            .map(|height| scaler.scale(height))
            .unwrap_or_else(|| environment.resolved_line_height(&font, None));
        let letter_spacing = inherited_layout_style(layout, LETTER_SPACING)
            .or_else(|| token.map(|token| token.letter_spacing))
            .map(|spacing| scaler.scale(spacing))
            .unwrap_or(0.0);
        TextMetrics {
            font,
            line_height,
            letter_spacing,
        }
    }

    fn metrics_for_node(node: &Node, environment: &TextEnvironment) -> TextMetrics {
        let font = text_style_font(node).unwrap_or_else(|| {
            crate::theme::node_text_scaler(node).scale_font(&environment.default_font)
        });
        let line_height = text_style_line_height(node)
            .unwrap_or_else(|| environment.resolved_line_height(&font, None));
        let letter_spacing = text_style_letter_spacing(node).unwrap_or(0.0);
        TextMetrics {
            font,
            line_height,
            letter_spacing,
        }
    }

    fn update_selection(&self, node: &Node) {
        let Some(handle) = node
            .composition_value(&TextSelectionScope::key())
            .filter(|_| !TextSelectionChrome::is_inside_control(node))
        else {
            if node.attachment::<bool>("__selectable_text") == Some(true) {
                super::selection_input::disable(node);
                node.remove_attachment("__selectable_text");
            }
            return;
        };
        let owner = node.downgrade();
        let string = self.string.clone();
        TextSelectionScope::register(
            &handle.0,
            node,
            Rc::new(move || string.clone()),
            Rc::new(move |point| {
                let Some(node) = Node::upgrade(&owner) else {
                    return 0;
                };
                let Some(layout) = node.layout_node() else {
                    return 0;
                };
                let Some(entry) = layout.text_measure() else {
                    return 0;
                };
                let padding = layout.resolved_padding();
                let frame = node.absolute_frame();
                let text = &entry.key.text;
                let measured =
                    MeasuredTextLine::new(text, text, &entry.result, entry.key.line_height);
                let row = ((point.y - frame.y - padding.top) / entry.key.line_height) as usize;
                let index = measured.character(row, point.x - frame.x - padding.left);
                text.char_indices()
                    .nth(index)
                    .map_or(text.len(), |(offset, _)| offset)
            }),
        );
        super::selection_input::install(node, &handle.0);
        node.set_attachment("__selectable_text", true);
    }

    fn layout_result(
        &self,
        environment: &TextEnvironment,
        layout: Option<&LayoutNode>,
        metrics: &TextMetrics,
        width: f32,
    ) -> (TextLayoutResult, bool) {
        // A zero or non-finite authored width means the label has no room: it
        // must not fall back to "unbounded", or a truncating label at width 0
        // shapes and paints the entire string. Swift passes the authored width
        // through and paints nothing.
        let max_width = if width.is_finite() && width > 0.0 {
            width
        } else if width.is_finite() {
            0.0
        } else {
            f32::INFINITY
        };
        let single_line = self.truncates || self.line_limit == Some(1);
        let normalized = if single_line {
            self.string.lines().collect::<Vec<_>>().join(" ")
        } else {
            self.string.clone()
        };
        let text = if self.truncates {
            crate::primitives::text::single_line::SingleLineText::fitting(
                &normalized,
                max_width,
                |candidate| {
                    environment
                        .cached_layout(
                            candidate,
                            Some(&metrics.font),
                            Some(metrics.line_height),
                            f32::INFINITY,
                            self.alignment,
                            metrics.letter_spacing,
                        )
                        .total_width
                },
            )
        } else {
            normalized
        };
        let layout_width = if single_line {
            f32::INFINITY
        } else {
            max_width
        };
        let key = TextLayoutCacheKey {
            text: text.clone(),
            font: metrics.font.clone(),
            line_height: metrics.line_height,
            alignment: self.alignment,
            max_width: layout_width,
            letter_spacing: metrics.letter_spacing,
            atlas_id: environment.atlas.identity(),
        };
        let mut result = layout
            .and_then(|layout| layout.text_measure())
            .filter(|cached| cached.key == key)
            .map(|cached| cached.result)
            .unwrap_or_else(|| {
                (*environment.cached_layout(
                    &text,
                    Some(&metrics.font),
                    Some(metrics.line_height),
                    layout_width,
                    self.alignment,
                    metrics.letter_spacing,
                ))
                .clone()
            });
        let line_limit = self.line_limit;
        if let Some(limit) = line_limit.filter(|limit| result.lines.len() > *limit) {
            result.lines.truncate(limit);
            result.total_width = result
                .lines
                .iter()
                .map(|line| line.width)
                .fold(0.0, f32::max);
            result.total_height = result
                .lines
                .last()
                .map(|line| line.top_y + metrics.line_height)
                .unwrap_or(0.0);
        }
        if let Some(layout) = layout {
            layout.set_text_measure(Some(TextLayoutCacheEntry {
                key,
                result: result.clone(),
            }));
        }
        let truncated = self.truncates && text != self.string.lines().collect::<Vec<_>>().join(" ");
        (result, truncated)
    }

    /// Queues the label for the next shaping batch, so one batch shapes every
    /// label that a layout pass measures.
    fn request_shape(&self, layout: &LayoutNode) {
        let Some(environment) = TextEnvironmentHolder::current() else {
            return;
        };
        let weak_layout = layout.downgrade();
        let string = self.string.clone();
        let single_line = self.truncates || self.line_limit == Some(1);
        environment.request_shape(move |environment| {
            let layout = LayoutNode::upgrade(&weak_layout)?;
            if layout.text_measure().is_some() {
                return None;
            }
            let font = Self::metrics_for_layout(&layout, environment).font;
            let text = if single_line {
                string.lines().collect::<Vec<_>>().join(" ")
            } else {
                string
            };
            Some((text, font))
        });
    }

    fn install_measure(layout: &LayoutNode, snapshot: Self) {
        let weak_layout = layout.downgrade();
        layout.set_measure_func(Some(Rc::new(
            move |width, width_mode, _height, _height_mode| {
                let Some(environment) = TextEnvironmentHolder::current() else {
                    return cove_core::Size::default();
                };
                let Some(layout) = LayoutNode::upgrade(&weak_layout) else {
                    return cove_core::Size::default();
                };
                let metrics = Self::metrics_for_layout(&layout, &environment);
                if width_mode == MeasureMode::MinContent {
                    // Measure unbreakable runs without emergency character wrapping.
                    // Truncating text can shrink to zero; nowrap text keeps its natural width.
                    if snapshot.truncates {
                        return cove_core::Size::default();
                    }
                    if snapshot.line_limit != Some(1) {
                        let mut breaks: Vec<_> =
                            cove_core::line_break_opportunities(&snapshot.string)
                                .into_iter()
                                .collect();
                        breaks.push(snapshot.string.len());
                        breaks.sort_unstable();
                        breaks.dedup();
                        let mut start = 0;
                        let mut minimum = 0.0_f32;
                        for end in breaks {
                            if end > start && snapshot.string.is_char_boundary(end) {
                                let run = snapshot.string[start..end].trim();
                                let measured = environment.cached_layout(
                                    run,
                                    Some(&metrics.font),
                                    Some(metrics.line_height),
                                    f32::INFINITY,
                                    snapshot.alignment,
                                    metrics.letter_spacing,
                                );
                                minimum = minimum.max(measured.total_width);
                                start = end;
                            }
                        }
                        let (result, _) =
                            snapshot.layout_result(&environment, None, &metrics, minimum);
                        return cove_core::Size {
                            width: minimum,
                            height: result.total_height,
                        };
                    }
                }
                let width = match width_mode {
                    MeasureMode::Undefined | MeasureMode::MinContent => f32::INFINITY,
                    MeasureMode::Exactly | MeasureMode::AtMost => width.max(0.0),
                };
                let (result, _) =
                    snapshot.layout_result(&environment, Some(&layout), &metrics, width);
                cove_core::Size {
                    width: result.total_width,
                    height: result.total_height,
                }
            },
        )));
    }
}

impl PrimitiveView for Text {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        crate::theme::sync_text_style_environment(node);
        self.update_selection(node);
        if node.accessibility().is_none_or(|semantics| {
            semantics.role != AccessibilityRole::StaticText || semantics.label != self.string
        }) {
            let mut semantics = AccessibilitySemantics::new(AccessibilityRole::StaticText);
            semantics.label.clone_from(&self.string);
            node.set_accessibility(Some(semantics));
        }
        let snapshot = self.clone();
        let identity = PaintIdentity {
            string: self.string.clone(),
            alignment: self.alignment,
            color: self.color,
            line_limit: self.line_limit,
            truncates: self.truncates,
        };
        let weak_node = node.downgrade();
        node.update_draw(identity, move |list, origin| {
            let Some(node) = Node::upgrade(&weak_node) else {
                return;
            };
            let Some(environment) = TextEnvironmentHolder::current() else {
                return;
            };
            let layout = node.layout_node();
            let metrics = Self::metrics_for_node(&node, &environment);
            let padding = layout
                .as_ref()
                .map(LayoutNode::resolved_padding)
                .unwrap_or_default();
            let width = (node.frame().width - padding.horizontal()).max(0.0);
            let (result, truncated) =
                snapshot.layout_result(&environment, layout.as_ref(), &metrics, width);
            node.set_attachment("__text_truncated", truncated);
            let color = node
                .debug_foreground_color()
                .or(snapshot.color)
                .or_else(|| node.inherited_foreground_color())
                .unwrap_or_else(|| theme_for_node(&node).colors.on_surface)
                .multiplied_alpha(node.opacity());
            if let Some(scope) = node.composition_value(&TextSelectionScope::key())
                && let Some(range) = scope.0.borrow_mut().range_for(&node)
            {
                let text = &snapshot.string;
                let measured = MeasuredTextLine::new(text, text, &result, metrics.line_height);
                let color = theme_for_node(&node)
                    .colors
                    .selection
                    .multiplied_alpha(node.opacity());
                for (index, line) in result.lines.iter().enumerate() {
                    let start = range.start.max(line.start_cluster as usize);
                    let end = range.end.min(line.end_cluster as usize);
                    if start >= end {
                        continue;
                    }
                    let (x1, _) =
                        measured.caret(measured.display_character_index_for_byte(start), false);
                    let (x2, _) =
                        measured.caret(measured.display_character_index_for_byte(end), true);
                    let _ = list.add_rect(
                        cove_core::Rect::new(
                            origin.x + padding.left + x1.min(x2),
                            origin.y + padding.top + measured.rows[index].top_y,
                            (x2 - x1).abs(),
                            metrics.line_height,
                        ),
                        color,
                    );
                }
            }
            let _ = list.add_text(
                &result,
                cove_core::Point::new(origin.x + padding.left, origin.y + padding.top),
                color,
                environment.atlas_texture_id,
                Some(&environment.atlas),
                None,
            );
        });
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        Self::install_measure(&layout, self.clone());
        layout.set_text_inputs(Some(TextMeasureInputs {
            text: self.string.clone(),
            alignment: self.alignment,
            line_limit: self.line_limit,
        }));
        self.request_shape(&layout);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        Self::install_measure(layout, self.clone());
        let next = TextMeasureInputs {
            text: self.string.clone(),
            alignment: self.alignment,
            line_limit: self.line_limit,
        };
        if layout.text_inputs().as_ref() != Some(&next) {
            layout.set_text_inputs(Some(next));
            layout.set_text_measure(None);
            layout.mark_dirty();
        }
        if layout.text_measure().is_none() {
            self.request_shape(layout);
        }
    }
}
impl_primitive_view!(Text);

/// Thin divider that resolves its default tint from the active theme.
#[derive(Clone, Copy)]
pub struct Divider {
    pub color: Option<Color>,
    pub thickness: f32,
    pub axis: DividerAxis,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DividerAxis {
    Horizontal,
    Vertical,
}
impl Divider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn horizontal() -> Self {
        Self::new()
    }

    pub fn vertical() -> Self {
        Self::new().axis(DividerAxis::Vertical)
    }

    pub fn axis(mut self, axis: DividerAxis) -> Self {
        self.axis = axis;
        self
    }

    pub fn color(mut self, color: impl Into<Option<Color>>) -> Self {
        self.color = color.into();
        self
    }

    pub fn thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness.max(0.0);
        self
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self {
            color: None,
            thickness: 1.0,
            axis: DividerAxis::Horizontal,
        }
    }
}
impl PrimitiveView for Divider {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(
            self.color
                .unwrap_or_else(|| theme_for_node(node).colors.divider),
        ));
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        match self.axis {
            DividerAxis::Horizontal => {
                layout.set_height(Some(self.thickness));
                layout.set_width_percent(100.0);
            }
            DividerAxis::Vertical => {
                layout.set_width(Some(self.thickness));
                layout.set_height_percent(100.0);
            }
        }
    }
}
impl_primitive_view!(Divider);
