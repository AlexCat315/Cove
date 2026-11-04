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

use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color};

use crate::LayoutDebugExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt, SurfaceStyleExt};
use crate::primitives::controls::fields::color_field_model::{hex_string, panel_width, parse_hex};
use crate::primitives::controls::fields::slider::Slider;
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use crate::primitives::overlays::popover::{Popover, PopoverTriggerDescription};
use crate::primitives::text::Text;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{TextField, TextFieldEvents};
use crate::theme::SemanticFontRef;
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
/// Integer readout width.
const CHANNEL_VALUE_WIDTH: f32 = 26.0;

/// Compact colour picker.
pub struct ColorField {
    pub color: Binding<Color>,
    pub is_enabled: bool,
    pub show_alpha: bool,
    pub shows_inline_values: bool,
    pub shows_inline_channels: bool,
    draft: Arc<Mutex<TextBuffer>>,
    is_editing: Arc<Mutex<bool>>,
}

impl ColorField {
    pub fn new(color: Binding<Color>) -> Self {
        Self {
            color,
            is_enabled: true,
            show_alpha: true,
            shows_inline_values: false,
            shows_inline_channels: true,
            draft: Arc::new(Mutex::new(TextBuffer::empty())),
            is_editing: Arc::new(Mutex::new(false)),
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
            draft: self.draft.clone(),
            is_editing: self.is_editing.clone(),
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

    /// Commits the hex draft when it parses.
    fn commit_hex(&self) {
        let draft = self
            .draft
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(color) = parse_hex(&draft.string_value()) {
            self.color.set(color);
        }
    }
}

impl View for ColorField {
    fn body(&self, cx: &Scope) -> AnyView {
        let mut field = self.snapshot();
        field.draft = cx.remember(|| self.draft.clone());
        field.is_editing = cx.remember(|| self.is_editing.clone());
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
        let swatch = AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            )
            .frame_size(SWATCH_WIDTH, SWATCH_HEIGHT)
            .background_color(if self.is_enabled {
                self.color
            } else {
                self.color.multiplied_alpha(0.4)
            })
            .corner_radius(theme.radius.sm)
            .border_color(theme.colors.border, 1.0),
        );
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
        let mut column: Vec<AnyView> = vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            )
            .frame_height(PREVIEW_HEIGHT)
            .background_color(field.color.get())
            .corner_radius(theme.radius.sm)
            .border_color(theme.colors.border, 1.0),
        )];
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
        let slider = {
            let target = field.clone();
            Slider::new(Binding::new(
                move || value,
                move |next: f64| target.set_channel(channel, next),
            ))
            .range((0.0, 1.0))
        };
        let readout = (value as f32 * 255.0).round().clamp(0.0, 255.0) as i32;
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
                AnyView::new(
                    Text::new(readout.to_string())
                        .semantic_font(SemanticFontRef::mono())
                        .semantic_foreground(crate::theme::SemanticColorRef::on_surface_variant())
                        .frame_width(CHANNEL_VALUE_WIDTH),
                ),
            ])),
        ))]
    }
}
impl_primitive_view!(ChannelRow);

/// The hex input, committing on submit and blur.
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
        let committed = hex_string(field.color.get(), field.show_alpha);
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
        let input = TextField::new("Hex color", text).events(TextFieldEvents {
            on_submit: Some(std::rc::Rc::new(move || commit.commit_hex())),
            on_focus: Some(std::rc::Rc::new(move || {
                let mut editing = focus
                    .is_editing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if !*editing {
                    *focus
                        .draft
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                        TextBuffer::new(&hex_string(focus.color.get(), focus.show_alpha));
                    *editing = true;
                }
            })),
            on_blur: Some(std::rc::Rc::new(move || {
                blur.commit_hex();
                *blur
                    .is_editing
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
            })),
            ..Default::default()
        });
        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            6.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(
                    Text::new("HEX")
                        .semantic_font(SemanticFontRef::label())
                        .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted()),
                ),
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
