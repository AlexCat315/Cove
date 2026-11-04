//! Translation of Primitives/Controls/Fields/ColorField.swift (the view half).
//!
//! Compact colour picker: a coloured swatch button that opens an RGBA channel
//! editor popover. The popover holds per-channel sliders in 0–1, mirrored
//! integer readouts and a hex text field.
//!
//! Hex formatting, parsing and the panel width clamp live in
//! [`hex_string`], [`parse_hex`] and [`panel_width`].
use crate::Scope;
use std::sync::{Arc, Mutex};

use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align, EventResult,
    FlexDirection, InputHandlerRoute, InteractionRegistryHolder, Justify, Node,
    PointerCaptureHolder, PointerPhase, Rect, Wrap,
};
use cove_core::{Binding, Color, DrawList, RoundedRect};
use cove_platform::{MouseButton, SystemCursor};

use crate::LayoutDebugExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{SemanticStyleViewExt, SurfaceStyleExt};
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::controls::fields::color_field_model::{
    ColorFormat, format_color, hex_string, panel_width, parse_color,
};
use crate::primitives::controls::fields::color_wheel::ColorWheel;
use crate::primitives::controls::fields::numeric::number_field::NumberField;
use crate::primitives::controls::fields::numeric::options::NumberFieldOptions;
use crate::primitives::controls::fields::slider::Slider;
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use crate::primitives::overlays::popover::{Popover, PopoverTriggerDescription};
use crate::primitives::text::Text;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{TextField, TextFieldEvents};
use crate::theme::SemanticFontRef;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, impl_primitive_view,
};

/// Swatch size in the trigger.
const SWATCH_WIDTH: f32 = 52.0;
const SWATCH_HEIGHT: f32 = 22.0;
/// Preview swatch height inside the panel.
const PREVIEW_HEIGHT: f32 = 28.0;
/// Channel label column width.
const CHANNEL_LABEL_WIDTH: f32 = 10.0;
/// Editable channel value field width.
const CHANNEL_VALUE_WIDTH: f32 = 44.0;

/// Compact colour picker.
pub struct ColorField {
    pub color: Binding<Color>,
    pub is_enabled: bool,
    pub show_alpha: bool,
    pub shows_inline_values: bool,
    pub shows_inline_channels: bool,
    /// Shows the hue/saturation wheel above the channel sliders.
    pub shows_wheel: bool,
    /// Shows the preset palette under the hex field.
    pub shows_swatches: bool,
    pub swatches: Vec<Color>,
    /// Host-provided screen sampler. When set, an eyedropper button appears and
    /// the host calls the completion with the sampled colour.
    pub on_sample: Option<std::rc::Rc<dyn Fn(std::rc::Rc<dyn Fn(Color)>)>>,
    draft: Arc<Mutex<TextBuffer>>,
    is_editing: Arc<Mutex<bool>>,
    format: Arc<Mutex<ColorFormat>>,
    channel_draft: Arc<Mutex<Option<TextBuffer>>>,
    channel_editing: Arc<Mutex<Option<Channel>>>,
    recent: Arc<Mutex<Vec<Color>>>,
}

impl ColorField {
    /// Most recent colours kept for the "recent" row.
    pub const RECENT_LIMIT: usize = 8;

    pub fn new(color: Binding<Color>) -> Self {
        Self {
            color,
            is_enabled: true,
            show_alpha: true,
            shows_inline_values: false,
            shows_inline_channels: true,
            shows_wheel: true,
            shows_swatches: true,
            swatches: crate::primitives::controls::fields::color_field_model::default_swatches(),
            on_sample: None,
            draft: Arc::new(Mutex::new(TextBuffer::empty())),
            is_editing: Arc::new(Mutex::new(false)),
            format: Arc::new(Mutex::new(ColorFormat::default())),
            channel_draft: Arc::new(Mutex::new(None)),
            channel_editing: Arc::new(Mutex::new(None)),
            recent: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn show_alpha(mut self, show_alpha: bool) -> Self {
        self.show_alpha = show_alpha;
        self
    }

    pub fn shows_inline_values(mut self, shows_inline_values: bool) -> Self {
        self.shows_inline_values = shows_inline_values;
        self
    }

    pub fn shows_inline_channels(mut self, shows_inline_channels: bool) -> Self {
        self.shows_inline_channels = shows_inline_channels;
        self
    }

    /// Shows or hides the hue/saturation wheel in the popover.
    pub fn shows_wheel(mut self, shows_wheel: bool) -> Self {
        self.shows_wheel = shows_wheel;
        self
    }

    /// Shows or hides the preset palette.
    pub fn shows_swatches(mut self, shows_swatches: bool) -> Self {
        self.shows_swatches = shows_swatches;
        self
    }

    /// Replaces the preset palette.
    pub fn swatches(mut self, swatches: Vec<Color>) -> Self {
        self.swatches = swatches;
        self
    }

    /// Supplies a screen colour sampler for the eyedropper button. The host
    /// starts sampling and calls the completion with the picked colour.
    pub fn on_sample(mut self, handler: impl Fn(std::rc::Rc<dyn Fn(Color)>) + 'static) -> Self {
        self.on_sample = Some(std::rc::Rc::new(handler));
        self
    }

    /// Seeds the "recent" row.
    pub fn recent_colors(mut self, recent: Vec<Color>) -> Self {
        *self
            .recent
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = recent;
        self
    }

    /// The text field's current format.
    pub fn color_format(&self) -> ColorFormat {
        self.format()
    }

    /// The most recently committed colours, newest first.
    pub fn recent(&self) -> Vec<Color> {
        self.recent
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// The popover width for a viewport, clamped between 176 and 280.
    pub fn panel_width(viewport: f32) -> f32 {
        panel_width(viewport)
    }

    fn snapshot(&self) -> ColorField {
        ColorField {
            color: self.color.clone(),
            is_enabled: self.is_enabled,
            show_alpha: self.show_alpha,
            shows_inline_values: self.shows_inline_values,
            shows_inline_channels: self.shows_inline_channels,
            shows_wheel: self.shows_wheel,
            shows_swatches: self.shows_swatches,
            swatches: self.swatches.clone(),
            on_sample: self.on_sample.clone(),
            draft: self.draft.clone(),
            is_editing: self.is_editing.clone(),
            format: self.format.clone(),
            channel_draft: self.channel_draft.clone(),
            channel_editing: self.channel_editing.clone(),
            recent: self.recent.clone(),
        }
    }

    /// Records a colour as recently used: newest first, de-duplicated, capped.
    fn push_recent(&self, color: Color) {
        let mut recent = self
            .recent
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        recent.retain(|existing| *existing != color);
        recent.insert(0, color);
        recent.truncate(Self::RECENT_LIMIT);
    }

    /// The text field's current format.
    fn format(&self) -> ColorFormat {
        *self
            .format
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn set_format(&self, format: ColorFormat) {
        *self
            .format
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = format;
        // Re-seed the text field from the colour in the new format.
        *self
            .is_editing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
    }

    /// Writes a colour, records it as recent and keeps the text draft's format.
    fn write_color(&self, color: Color) {
        self.color.set(color);
        self.push_recent(color);
    }

    /// Whether an eyedropper is available: an authored handler or the host's
    /// installed sampler.
    fn sampler_available(&self) -> bool {
        self.on_sample.is_some() || cove_core::scene::ColorSampler::is_installed()
    }

    /// Starts a screen pick and writes the sampled colour back.
    fn request_sample(&self) {
        let target = self.clone();
        if let Some(sample) = self.on_sample.clone() {
            sample(std::rc::Rc::new(move |color| target.write_color(color)));
        } else {
            cove_core::scene::ColorSampler::request(Box::new(move |color| {
                if let Some(color) = color {
                    target.write_color(color);
                }
            }));
        }
    }

    /// Writes one channel, clamped to 0…1.
    fn set_channel(&self, channel: Channel, value: f64) {
        let mut color = self.color.get();
        let value = (value as f32).clamp(0.0, 1.0);
        match channel {
            Channel::Red => color.r = value,
            Channel::Green => color.g = value,
            Channel::Blue => color.b = value,
            Channel::Alpha => color.a = value,
        }
        self.color.set(color);
    }

    fn channel(&self, channel: Channel) -> f64 {
        let color = self.color.get();
        match channel {
            Channel::Red => color.r as f64,
            Channel::Green => color.g as f64,
            Channel::Blue => color.b as f64,
            Channel::Alpha => color.a as f64,
        }
    }

    /// Writes a colour chosen programmatically and records it as recent.
    pub fn set_color(&self, color: Color) {
        self.write_color(color);
    }

    /// Writes one channel and records the resulting colour as recent.
    fn commit_channel(&self, channel: Channel, value: f64) {
        self.set_channel(channel, value);
        self.push_recent(self.color.get());
    }

    /// Commits the text draft when it parses in the current format.
    fn commit_text(&self) {
        let draft = self
            .draft
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(color) = parse_color(&draft.string_value(), self.format()) {
            self.write_color(color);
        }
    }
}

impl View for ColorField {
    fn body(&self, cx: &Scope) -> AnyView {
        let mut field = self.snapshot();
        field.draft = cx.remember(|| self.draft.clone());
        field.is_editing = cx.remember(|| self.is_editing.clone());
        field.format = cx.remember(|| self.format.clone());
        field.channel_draft = cx.remember(|| self.channel_draft.clone());
        field.channel_editing = cx.remember(|| self.channel_editing.clone());
        field.recent = cx.remember(|| self.recent.clone());
        let presented: Binding<bool> = cx.state(|| false).into();
        let trigger = AnyView::new(ColorSwatch {
            color: self.color.get(),
            is_enabled: self.is_enabled,
            show_alpha: self.show_alpha,
            shows_inline_values: self.shows_inline_values,
            shows_inline_channels: self.shows_inline_channels,
        });
        let content = AnyView::new(crate::adaptive::SurfaceReader::new(move |surface| {
            AnyView::new(ColorPickerPanel {
                field: field.clone(),
                width: panel_width(surface.viewport.width),
            })
        }));
        AnyView::new(
            Popover::new(trigger, content)
                .is_presented(presented)
                .enabled(self.is_enabled)
                .surface(PopoverSurface::Content)
                .overflow(PopoverOverflow::Flip)
                .trigger_description(PopoverTriggerDescription {
                    label: crate::i18n::tr("Choose color"),
                    value: hex_string(self.color.get(), self.show_alpha),
                    help: None,
                }),
        )
    }
}

impl Clone for ColorField {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

/// The four editable channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Channel {
    Red,
    Green,
    Blue,
    Alpha,
}

impl Channel {
    fn label(self) -> &'static str {
        match self {
            Self::Red => "R",
            Self::Green => "G",
            Self::Blue => "B",
            Self::Alpha => "A",
        }
    }
}

/// Swatch button: a coloured tile, optionally with inline hex and channel text.
struct ColorSwatch {
    color: Color,
    is_enabled: bool,
    show_alpha: bool,
    shows_inline_values: bool,
    shows_inline_channels: bool,
}

impl PrimitiveView for ColorSwatch {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = crate::theme::theme_for_node(node);
        let swatch = AnyView::new(SwatchTile {
            color: if self.is_enabled {
                self.color
            } else {
                self.color.multiplied_alpha(0.4)
            },
            radius: theme.radius.sm,
            border: theme.colors.border,
            size: Some((SWATCH_WIDTH, SWATCH_HEIGHT)),
        });
        if !self.shows_inline_values {
            return vec![swatch];
        }
        let mut row: Vec<AnyView> = vec![
            swatch,
            AnyView::new(
                Text::new(hex_string(self.color, self.show_alpha))
                    .semantic_font(SemanticFontRef::mono())
                    .semantic_foreground(if self.is_enabled {
                        crate::theme::SemanticColorRef::on_surface_variant()
                    } else {
                        crate::theme::SemanticColorRef::on_surface_muted()
                    }),
            ),
        ];
        if self.shows_inline_channels {
            row.push(AnyView::new(
                Text::new(rgb_string(self.color, self.show_alpha))
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted())
                    .flex(1.0),
            ));
        }
        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            8.0,
            AnyView::new(TupleView::new(row)),
        ))]
    }
}
impl_primitive_view!(ColorSwatch);

fn rgb_string(color: Color, show_alpha: bool) -> String {
    let channel = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as i32;
    if show_alpha {
        format!(
            "{}, {}, {}, {}",
            channel(color.r),
            channel(color.g),
            channel(color.b),
            channel(color.a)
        )
    } else {
        format!(
            "{}, {}, {}",
            channel(color.r),
            channel(color.g),
            channel(color.b)
        )
    }
}

/// Paints a light/dark checkerboard inside `rect`, clipped to its rounded edge.
fn paint_checkerboard(list: &mut DrawList, rect: Rect, radius: f32) {
    const CELL: f32 = 6.0;
    let light = Color::from_rgba8(255, 255, 255, 255);
    let dark = Color::from_rgba8(204, 204, 210, 255);
    let _ = list.add_rounded_rect(rect, radius, light);
    let columns = (rect.width / CELL).ceil() as i32;
    let rows = (rect.height / CELL).ceil() as i32;
    for row in 0..rows {
        for column in 0..columns {
            if (row + column) % 2 == 0 {
                continue;
            }
            let x = rect.x + column as f32 * CELL;
            let y = rect.y + row as f32 * CELL;
            let width = CELL.min(rect.x + rect.width - x);
            let height = CELL.min(rect.y + rect.height - y);
            if width > 0.0 && height > 0.0 {
                let _ = list.add_rect(Rect::new(x, y, width, height), dark);
            }
        }
    }
}

/// Paints a checkerboard, then the colour over it, so alpha is visible.
fn paint_alpha_backdrop(list: &mut DrawList, rect: Rect, radius: f32, color: Color, border: Color) {
    list.push_mask(RoundedRect::new(rect, radius));
    paint_checkerboard(list, rect, radius);
    let _ = list.add_rect(rect, color);
    if border.a > 0.0 {
        let _ = list.add_rounded_rect_stroke(rect, radius, 1.0, border);
    }
    list.pop_mask();
}

/// Alpha slider whose track is a checkerboard under a transparent→opaque
/// gradient, so the channel reads as transparency rather than a flat fill.
struct AlphaSlider {
    color: Binding<Color>,
    is_enabled: bool,
}

impl PrimitiveView for AlphaSlider {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(self.is_enabled);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_height(Some(24.0));
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        layout.set_flex_basis(0.0);
        layout.set_measure_func(Some(std::rc::Rc::new(|width, width_mode, _h, _hm| {
            cove_core::Size {
                width: if width_mode == cove_core::scene::MeasureMode::Undefined {
                    120.0
                } else {
                    width
                },
                height: 24.0,
            }
        })));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        node.set_is_focusable(self.is_enabled);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));
        let alpha = self.color.get().a;
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: crate::i18n::tr("Alpha"),
            value: format!("{}%", (alpha * 100.0).round() as i64),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));

        let weak = node.downgrade();
        let color_for_draw = self.color.clone();
        let enabled = self.is_enabled;
        node.update_draw((self.color.get().rgba8(), enabled), move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let frame = owner.frame();
            if frame.width <= 1.0 {
                return;
            }
            let color = color_for_draw.get();
            let fraction = color.a.clamp(0.0, 1.0);
            draw_alpha_track(
                list,
                Rect::new(origin.x, origin.y, frame.width, frame.height),
                color,
                fraction,
                enabled,
            );
        });

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }

        let pointer_color = self.color.clone();
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            std::rc::Rc::new(move |event, phase, _| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, false);
                        }
                        owner.set_attachment(ALPHA_PRESSED, true);
                        write_alpha(&owner, &pointer_color, event.x);
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        owner.set_attachment(ALPHA_PRESSED, false);
                        EventResult::Handled
                    }
                }
            }),
        );

        let motion_color = self.color.clone();
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            std::rc::Rc::new(move |event, _| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                if owner.attachment::<bool>(ALPHA_PRESSED) != Some(true) {
                    return EventResult::Ignored;
                }
                write_alpha(&owner, &motion_color, event.x);
                EventResult::Handled
            }),
        );
    }
}
const ALPHA_PRESSED: &str = "__color_alpha_pressed";

fn write_alpha(node: &Node, color: &Binding<Color>, x: f32) {
    let frame = node.frame();
    if frame.width <= 1.0 {
        return;
    }
    let origin = node.absolute_origin();
    let fraction = ((x - origin.x) / frame.width).clamp(0.0, 1.0);
    let mut next = color.get();
    next.a = fraction;
    color.set(next);
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
        "alphaSlider",
    ));
}
impl_primitive_view!(AlphaSlider);

/// Track (checkerboard + transparent→colour ramp), border and thumb.
fn draw_alpha_track(list: &mut DrawList, rect: Rect, color: Color, fraction: f32, enabled: bool) {
    const TRACK_HEIGHT: f32 = 8.0;
    let track = Rect::new(
        rect.x,
        rect.y + (rect.height - TRACK_HEIGHT) / 2.0,
        rect.width,
        TRACK_HEIGHT,
    );
    let radius = TRACK_HEIGHT / 2.0;
    let alpha = if enabled { 1.0 } else { 0.45 };
    list.push_mask(RoundedRect::new(track, radius));
    paint_checkerboard(list, track, radius);
    let _ = list.add_linear_gradient(
        track,
        0.0,
        cove_core::LinearGradient {
            start: Color { a: 0.0, ..color },
            end: Color { a: alpha, ..color },
            angle: 90.0,
        },
    );
    list.pop_mask();
    let _ = list.add_rounded_rect_stroke(track, radius, 1.0, Color::from_rgba8(0, 0, 0, 40));
    // Thumb.
    let center = track.x + fraction.clamp(0.0, 1.0) * track.width;
    let thumb = Rect::new(
        center - 7.0,
        rect.y + (rect.height - 16.0) / 2.0,
        14.0,
        16.0,
    );
    let _ = list.add_rounded_rect_stroke(thumb, 4.0, 1.0, Color::from_rgba8(0, 0, 0, 60));
    list.push_mask(RoundedRect::new(thumb, 4.0));
    let _ = list.add_rect(
        thumb,
        Color {
            a: alpha,
            ..Color::WHITE
        },
    );
    list.pop_mask();
}

/// A colour tile drawn over an alpha checkerboard. `size` fixes the extents;
/// `None` fills whatever the surrounding layout gives it.
struct SwatchTile {
    color: Color,
    radius: f32,
    border: Color,
    size: Option<(f32, f32)>,
}

impl PrimitiveView for SwatchTile {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        if let Some((width, height)) = self.size {
            layout.set_width(Some(width));
            layout.set_height(Some(height));
            layout.set_flex_shrink(0.0);
        }
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let color = self.color;
        let radius = self.radius;
        let border = self.border;
        let weak = node.downgrade();
        node.update_draw(
            (color.rgba8(), radius.to_bits(), border.rgba8()),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let frame = owner.frame();
                if frame.width <= 0.0 || frame.height <= 0.0 {
                    return;
                }
                let rect = Rect::new(origin.x, origin.y, frame.width, frame.height);
                paint_alpha_backdrop(list, rect, radius, color, border);
            },
        );
    }
}
impl_primitive_view!(SwatchTile);

/// The channel editor inside the popover.
struct ColorPickerPanel {
    field: ColorField,
    width: f32,
}

impl PrimitiveView for ColorPickerPanel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let field = &self.field;
        let theme = crate::theme::theme_for_node(node);
        let mut column: Vec<AnyView> = Vec::new();
        if field.shows_wheel {
            column.push(AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(ColorWheel::new(field.color.clone()).enabled(field.is_enabled)),
            )));
        }
        column.push(AnyView::new(
            AnyView::new(SwatchTile {
                color: field.color.get(),
                radius: theme.radius.sm,
                border: theme.colors.border,
                size: None,
            })
            .frame_height(PREVIEW_HEIGHT),
        ));
        for channel in [Channel::Red, Channel::Green, Channel::Blue] {
            column.push(AnyView::new(ChannelRow {
                field: field.clone(),
                channel,
            }));
        }
        if field.show_alpha {
            column.push(AnyView::new(ChannelRow {
                field: field.clone(),
                channel: Channel::Alpha,
            }));
        }
        column.push(AnyView::new(HexRow {
            field: field.clone(),
        }));
        if field.shows_swatches && !field.swatches.is_empty() {
            column.push(swatch_row(
                field,
                &field.swatches,
                theme.radius.sm,
                theme.colors.border,
            ));
        }
        let recent = field.recent();
        if !recent.is_empty() {
            column.push(swatch_row(
                field,
                &recent,
                theme.radius.sm,
                theme.colors.border,
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(column)),
            )
            .padding(10.0)
            .frame_width(self.width)
            .floating_surface(theme.radius.md)
            .debug_name("color-picker-panel"),
        )]
    }
}
impl_primitive_view!(ColorPickerPanel);

/// A wrapped row of clickable colour swatches.
fn swatch_row(field: &ColorField, colors: &[Color], radius: f32, border: Color) -> AnyView {
    let enabled = field.is_enabled;
    let show_alpha = field.show_alpha;
    let mut cells: Vec<AnyView> = Vec::new();
    for color in colors {
        let target = field.clone();
        let color = *color;
        cells.push(AnyView::new(
            Button::with_options(
                AnyView::new(SwatchTile {
                    color,
                    radius,
                    border,
                    size: Some((20.0, 20.0)),
                }),
                move || target.write_color(color),
                ButtonRole::Normal,
                enabled,
                false,
                false,
                None,
            )
            .accessibility_text(hex_string(color, show_alpha))
            .button_style(GhostButtonStyle)
            .debug_name("color-swatch"),
        ));
    }
    AnyView::new(BoxView::new(
        FlexDirection::Row,
        Align::Center,
        Justify::FlexStart,
        Wrap::Wrap,
        6.0,
        AnyView::new(TupleView::new(cells)),
    ))
}

/// One labelled channel slider with its integer readout.
struct ChannelRow {
    field: ColorField,
    channel: Channel,
}

impl PrimitiveView for ChannelRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = &self.field;
        let channel = self.channel;
        let value = field.channel(channel);
        // Alpha gets a checkerboard track; RGB keep the plain slider.
        let slider: AnyView = if channel == Channel::Alpha {
            AnyView::new(AlphaSlider {
                color: field.color.clone(),
                is_enabled: field.is_enabled,
            })
        } else {
            let target = field.clone();
            AnyView::new(
                Slider::new(Binding::new(
                    move || value,
                    move |next: f64| target.set_channel(channel, next),
                ))
                .range((0.0, 1.0)),
            )
        };
        let number = {
            let target = field.clone();
            NumberField::new(Binding::new(
                move || (value * 255.0).round().clamp(0.0, 255.0) as f32,
                move |next: f32| target.commit_channel(channel, (next as f64) / 255.0),
            ))
            .options(
                NumberFieldOptions::default()
                    .decimals(0)
                    .range(Some(0.0), Some(255.0)),
            )
            .enabled(field.is_enabled)
            .frame_width(CHANNEL_VALUE_WIDTH)
            .debug_name("color-channel-value")
        };
        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Text::new(channel.label())
                        .semantic_font(SemanticFontRef::label())
                        .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted())
                        .frame_width(CHANNEL_LABEL_WIDTH),
                ),
                AnyView::new(AnyView::new(slider).flex(1.0)),
                AnyView::new(number),
            ])),
        ))]
    }
}
impl_primitive_view!(ChannelRow);

/// The text input: HEX or HSL selected by a small toggle, committing on submit
/// and blur.
struct HexRow {
    field: ColorField,
}

impl PrimitiveView for HexRow {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let field = &self.field;
        let format = field.format();
        let committed = format_color(field.color.get(), field.show_alpha, format);
        let draft = field.draft.clone();
        let editing = field.is_editing.clone();
        let text = Binding::new(
            {
                let draft = draft.clone();
                let editing = editing.clone();
                let committed = committed.clone();
                move || {
                    if *editing
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                    {
                        draft
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .clone()
                    } else {
                        TextBuffer::new(&committed)
                    }
                }
            },
            {
                let draft = draft.clone();
                move |buffer: TextBuffer| {
                    *draft
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) = buffer;
                }
            },
        );
        let commit = field.clone();
        let focus = field.clone();
        let blur = field.clone();
        let input = TextField::new("Color value", text).events(TextFieldEvents {
            on_submit: Some(std::rc::Rc::new(move || commit.commit_text())),
            on_focus: Some(std::rc::Rc::new(move || {
                let mut editing = focus
                    .is_editing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if !*editing {
                    *focus
                        .draft
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) = TextBuffer::new(
                        &format_color(focus.color.get(), focus.show_alpha, focus.format()),
                    );
                    *editing = true;
                }
            })),
            on_blur: Some(std::rc::Rc::new(move || {
                blur.commit_text();
                *blur
                    .is_editing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
            })),
            ..Default::default()
        });
        // Format toggle: one button per format, the active one selected.
        let mut toggles: Vec<AnyView> = Vec::new();
        if field.sampler_available() {
            let target = field.clone();
            toggles.push(AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::resource(
                        crate::primitives::media::ui_icon_catalog::action::crosshair(),
                    ),
                    Some(14.0),
                    None,
                    ButtonRole::Normal,
                    field.is_enabled,
                    false,
                    false,
                    Some(crate::i18n::tr("Pick color from screen")),
                    move || target.request_sample(),
                )
                .button_style(GhostButtonStyle)
                .debug_name("color-eyedropper"),
            ));
        }
        for candidate in [ColorFormat::Hex, ColorFormat::Hsl] {
            let target = field.clone();
            toggles.push(AnyView::new(
                Button::title(candidate.label(), move || target.set_format(candidate))
                    .selected(candidate == format)
                    .enabled(field.is_enabled)
                    .button_style(GhostButtonStyle)
                    .debug_name("color-format"),
            ));
        }
        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(toggles)),
                )),
                AnyView::new(AnyView::new(input).flex(1.0)),
            ])),
        ))]
    }
}
impl_primitive_view!(HexRow);

impl ColorField {
    /// The panel width for a viewport, exposed for hosts that size the popover.
    pub fn width_for_viewport(viewport: f32) -> f32 {
        panel_width(viewport)
    }
}
