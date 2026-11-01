//! Translation of Primitives/Controls/Fields/Numeric/NumberField.swift.
//!
//! Numeric text field that keeps an editable draft and commits parsed floats on
//! submit or blur.
//!
//! With a `prefix`, a leading label chip is drawn inside the field. Dragging the
//! chip horizontally scrubs the value (one step per 4 pt; hold Shift when
//! starting for x10, Option/Alt for x0.1). With a `reset_value`, double-clicking
//! the chip restores it.
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource, ButtonRole};
use crate::primitives::text::Text;
use crate::primitives::text::text_buffer::TextBuffer;
use crate::primitives::text::text_field::{TextField, TextFieldEvents, TextFieldSize};
use crate::theme::styles::buttons::{ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt};
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View};
use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color};

use super::prefix_style::NumericPrefixFieldStyle;
use super::scrub::{NumericScrub, NumericScrubHandle};
use super::text_format::NumericTextFormat;

/// A short leading label, such as an axis name, that doubles as the scrub handle.
#[derive(Clone)]
pub struct NumberFieldPrefix {
    pub title: String,
    pub tint: SemanticColorRef,
}

impl PartialEq for NumberFieldPrefix {
    fn eq(&self, other: &Self) -> bool {
        // Color slots resolve against a theme; the title is the authored identity.
        self.title == other.title
    }
}

impl NumberFieldPrefix {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            tint: SemanticColorRef::on_surface_muted(),
        }
    }

    pub fn tint(mut self, tint: SemanticColorRef) -> Self {
        self.tint = tint;
        self
    }
}

/// Numeric field with draft/commit editing.
pub struct NumberField {
    pub value: Binding<f32>,
    pub decimals: usize,
    pub size: TextFieldSize,
    pub is_enabled: bool,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
    pub step: Option<f32>,
    pub shows_stepper: bool,
    pub mixed_value_label: Option<String>,
    pub prefix: Option<NumberFieldPrefix>,
    pub reset_value: Option<f32>,
    pub format: NumericTextFormat,
    draft: cove_core::State<TextBuffer>,
    is_editing: cove_core::State<bool>,
    draft_changed: cove_core::State<bool>,
}

impl NumberField {
    pub fn new(value: Binding<f32>) -> Self {
        Self {
            value,
            decimals: 2,
            size: TextFieldSize::Automatic,
            is_enabled: true,
            min_value: None,
            max_value: None,
            step: None,
            shows_stepper: false,
            mixed_value_label: None,
            prefix: None,
            reset_value: None,
            format: NumericTextFormat::invariant(2),
            draft: cove_core::State::new(TextBuffer::empty()),
            is_editing: cove_core::State::new(false),
            draft_changed: cove_core::State::new(false),
        }
    }

    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals.min(6);
        self.format = NumericTextFormat::invariant(self.decimals);
        self
    }

    pub fn size(mut self, size: TextFieldSize) -> Self {
        self.size = size;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn range(mut self, min_value: Option<f32>, max_value: Option<f32>) -> Self {
        self.min_value = min_value;
        self.max_value = max_value;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn shows_stepper(mut self, shows_stepper: bool) -> Self {
        self.shows_stepper = shows_stepper;
        self
    }

    pub fn mixed_value_label(mut self, label: impl Into<String>) -> Self {
        self.mixed_value_label = Some(label.into());
        self
    }

    pub fn prefix(mut self, prefix: NumberFieldPrefix) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub fn reset_value(mut self, reset_value: f32) -> Self {
        self.reset_value = Some(reset_value);
        self
    }

    pub fn with_format(mut self, format: NumericTextFormat) -> Self {
        self.format = format;
        self
    }

    /// Format a value for display.
    pub fn format_value(&self, value: f32) -> String {
        self.format.format(value)
    }

    /// Parse a draft; strict, so a numeric prefix is not accepted.
    pub fn parse_value(&self, text: &str) -> Option<f32> {
        self.format.parse(text)
    }

    pub fn clamped(&self, value: f32) -> f32 {
        let mut out = value;
        if let Some(min) = self.min_value {
            out = out.max(min);
        }
        if let Some(max) = self.max_value {
            out = out.min(max);
        }
        out
    }

    /// Step is an increment, not a quantization rule for typed values.
    pub fn normalized(&self, value: f32) -> f32 {
        self.clamped(value)
    }

    pub fn resolved_step(&self) -> f32 {
        match self.step {
            Some(step) if step.is_finite() && step > 0.0 => step,
            _ => 1.0,
        }
    }
}

impl View for NumberField {
    fn body(&self) -> AnyView {
        let committed = match self.mixed_value_label.clone() {
            Some(_) => String::new(),
            None => self.format_value(self.normalized(self.value.get())),
        };
        let committed_buffer = TextBuffer::new(&committed);
        let draft = self.draft.clone();
        let is_editing = self.is_editing.clone();
        let draft_changed = self.draft_changed.clone();
        let _value = self.value.clone();
        let field = self.snapshot();

        let committed_for_get = committed_buffer.clone();
        let text = Binding::new(
            {
                let is_editing = is_editing.clone();
                let draft = draft.clone();
                let committed = committed_for_get.clone();
                move || {
                    if is_editing.get() {
                        draft.get()
                    } else {
                        committed.clone()
                    }
                }
            },
            {
                let draft = draft.clone();
                let draft_changed = draft_changed.clone();
                move |buffer: TextBuffer| {
                    draft.set(buffer);
                    draft_changed.set(true);
                }
            },
        );

        let input = TextField::new(self.mixed_value_label.clone().unwrap_or_default(), text)
            .decoration(crate::primitives::text::text_field::TextFieldDecoration {
                size: self.size,
                ..Default::default()
            })
            .behavior(crate::primitives::text::text_field::TextFieldBehavior {
                disabled: !self.is_enabled,
                ..Default::default()
            })
            .events(TextFieldEvents {
                // Swift tracks the draft in the *binding setter*
                // (`NumberField.swift:90`: `set: { draft = $0; draftChanged = true }`),
                // so `draftChanged` is set by every edit. Rust has no setter hook,
                // so without `on_change` the flag stayed false forever and
                // `commit_draft` always returned early — Return and blur fired but
                // committed nothing.
                on_change: Some({
                    let draft = draft.clone();
                    let draft_changed = draft_changed.clone();
                    Rc::new(move |buffer: &TextBuffer| {
                        draft.set(buffer.clone());
                        draft_changed.set(true);
                    })
                }),
                on_submit: Some({
                    let field = field.clone();
                    let draft = draft.clone();
                    let draft_changed = draft_changed.clone();
                    Rc::new(move || commit_draft(&field, &draft, &draft_changed))
                }),
                on_key_down: Some({
                    let field = field.clone();
                    let draft = draft.clone();
                    let draft_changed = draft_changed.clone();
                    Rc::new(move |event: &cove_platform::KeyEvent| {
                        if event.scancode != crate::Scancode::ARROW_UP
                            && event.scancode != crate::Scancode::ARROW_DOWN
                        {
                            return false;
                        }
                        let multiplier = if event.modifiers.has_shift() {
                            10.0
                        } else if event.modifiers.has_alt() {
                            0.1
                        } else {
                            1.0
                        };
                        let by = if event.scancode == crate::Scancode::ARROW_UP {
                            multiplier
                        } else {
                            -multiplier
                        };
                        adjust(&field, &draft, &draft_changed, by);
                        true
                    })
                }),
                on_focus: Some({
                    let draft = draft.clone();
                    let is_editing = is_editing.clone();
                    let draft_changed = draft_changed.clone();
                    let committed = committed_buffer.clone();
                    Rc::new(move || {
                        if !is_editing.get() {
                            draft.set(committed.clone());
                            draft_changed.set(false);
                            is_editing.set(true);
                        }
                    })
                }),
                on_blur: Some({
                    let field = field.clone();
                    let draft = draft.clone();
                    let is_editing = is_editing.clone();
                    let draft_changed = draft_changed.clone();
                    Rc::new(move || {
                        commit_draft(&field, &draft, &draft_changed);
                        is_editing.set(false);
                    })
                }),
                ..Default::default()
            });

        let styled = match self.prefix.clone() {
            Some(prefix) => AnyView::new(NumberFieldPrefixed {
                input: AnyView::new(input),
                prefix,
                field: field.clone(),
                draft: draft.clone(),
                draft_changed: draft_changed.clone(),
                is_editing: is_editing.clone(),
            }),
            None => AnyView::new(input),
        };
        if !self.shows_stepper {
            return styled;
        }
        AnyView::new(NumberFieldStepper {
            input: styled,
            field,
            draft,
            draft_changed,
            is_editing,
        })
    }
}

impl Clone for NumberField {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

impl NumberField {
    fn snapshot(&self) -> NumberField {
        NumberField {
            value: self.value.clone(),
            decimals: self.decimals,
            size: self.size,
            is_enabled: self.is_enabled,
            min_value: self.min_value,
            max_value: self.max_value,
            step: self.step,
            shows_stepper: self.shows_stepper,
            mixed_value_label: self.mixed_value_label.clone(),
            prefix: self.prefix.clone(),
            reset_value: self.reset_value,
            format: self.format.clone(),
            draft: self.draft.clone(),
            is_editing: self.is_editing.clone(),
            draft_changed: self.draft_changed.clone(),
        }
    }
}

/// The value an edit starts from: the parsed draft while editing, else the bound
/// value.
fn current_value(
    field: &NumberField,
    draft: &cove_core::State<TextBuffer>,
    is_editing: &cove_core::State<bool>,
) -> f32 {
    if is_editing.get() {
        field
            .parse_value(&draft.get().string_value())
            .unwrap_or_else(|| field.value.get())
    } else {
        field.value.get()
    }
}

fn set_value(
    field: &NumberField,
    draft: &cove_core::State<TextBuffer>,
    draft_changed: &cove_core::State<bool>,
    raw: f32,
) {
    let next = field.normalized(raw);
    if field.value.get() != next || field.mixed_value_label.is_some() {
        field.value.set(next);
    }
    draft.set(TextBuffer::new(&field.format_value(next)));
    draft_changed.set(false);
}

fn commit_draft(
    field: &NumberField,
    draft: &cove_core::State<TextBuffer>,
    draft_changed: &cove_core::State<bool>,
) {
    if !draft_changed.get() {
        return;
    }
    draft_changed.set(false);
    if let Some(parsed) = field.parse_value(&draft.get().string_value()) {
        let next = field.normalized(parsed);
        if field.mixed_value_label.is_some() || field.value.get() != next {
            field.value.set(next);
        }
    }
    let committed = field.normalized(field.value.get());
    if committed != field.value.get() {
        field.value.set(committed);
    }
    draft.set(TextBuffer::new(&field.format_value(committed)));
}

fn adjust(
    field: &NumberField,
    draft: &cove_core::State<TextBuffer>,
    draft_changed: &cove_core::State<bool>,
    multiplier: f32,
) {
    let next = field.normalized(field.value.get() + field.resolved_step() * multiplier);
    if field.value.get() != next {
        field.value.set(next);
    }
    draft.set(TextBuffer::new(&field.format_value(next)));
    draft_changed.set(false);
}

/// Field plus its leading scrub chip, sharing one chrome.
struct NumberFieldPrefixed {
    input: AnyView,
    prefix: NumberFieldPrefix,
    field: NumberField,
    draft: cove_core::State<TextBuffer>,
    draft_changed: cove_core::State<bool>,
    is_editing: cove_core::State<bool>,
}

impl NumberFieldPrefixed {
    /// Height follows the size and text scale, so chip and text grow together.
    fn prefix_height(&self) -> f32 {
        match self.field.size {
            TextFieldSize::Large => 40.0,
            TextFieldSize::Automatic | TextFieldSize::Regular => 32.0,
            TextFieldSize::Small => 28.0,
            TextFieldSize::Mini => 22.0,
        }
    }
}

impl PrimitiveView for NumberFieldPrefixed {
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
        let height = self.prefix_height();
        let field = self.field.snapshot();
        let draft = self.draft.clone();
        let draft_changed = self.draft_changed.clone();
        let is_editing = self.is_editing.clone();
        let prefix = self.prefix.clone();
        let chip_value = current_value(&field, &draft, &is_editing);
        let scrub = NumericScrub::new(field.resolved_step() / 4.0, field.reset_value, {
            let field = field.clone();
            move |value: f32| field.clamped(value)
        });
        let title = prefix.title.clone();
        let tint = prefix.tint.clone();
        let chip = NumericScrubHandle::new(
            chip_value,
            {
                let field = field.clone();
                let draft = draft.clone();
                let draft_changed = draft_changed.clone();
                move |value: f32| set_value(&field, &draft, &draft_changed, value)
            },
            scrub,
            field.is_enabled,
            NumberFieldChip {
                title: title.clone(),
                tint: tint.clone(),
                is_enabled: field.is_enabled,
            },
        );
        let input = AnyView::new(NumericPrefixStyled {
            input: self.input.clone(),
            style: NumericPrefixFieldStyle::new(chip),
        });
        let _ = node;
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(input.flex_with(1.0, 1.0, Some(0.0))),
            )
            .frame(None, None, None, Some(height), None, None),
        )]
    }
}
crate::impl_primitive_view!(NumberFieldPrefixed);

/// Applies the prefix style through the field-style environment.
struct NumericPrefixStyled {
    input: AnyView,
    style: NumericPrefixFieldStyle,
}

impl PrimitiveView for NumericPrefixStyled {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_composition_value(
            &crate::theme::styles::components::TextFieldStyleEnvironment::key(),
            crate::theme::styles::components::AnyTextFieldStyle::new(self.style.clone()),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.input.clone()]
    }
}
crate::impl_primitive_view!(NumericPrefixStyled);

impl NumberField {}

/// The prefix chip's label.
struct NumberFieldChip {
    title: String,
    tint: SemanticColorRef,
    is_enabled: bool,
}

impl PrimitiveView for NumberFieldChip {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let resolved = self.tint.resolve(&theme_for_node(node));
        node.set_background_color(Some(resolved.multiplied_alpha(if self.is_enabled {
            0.12
        } else {
            0.05
        })));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(
                    Text::new(self.title.clone())
                        .semantic_font(SemanticFontRef::label())
                        .semantic_foreground(self.tint.clone()),
                ),
            )
            .frame_min_width(22.0)
            .padding_edges(5.0, 0.0),
        )]
    }
}
crate::impl_primitive_view!(NumberFieldChip);

/// Field plus the optional compact spinner.
struct NumberFieldStepper {
    input: AnyView,
    field: NumberField,
    draft: cove_core::State<TextBuffer>,
    draft_changed: cove_core::State<bool>,
    is_editing: cove_core::State<bool>,
}

impl PrimitiveView for NumberFieldStepper {
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
        let current = current_value(&self.field, &self.draft, &self.is_editing);
        // Stepping past a bound is a no-op, so the button reports it instead of
        // swallowing the press.
        let can_step_up =
            self.field.is_enabled && self.field.max_value.is_none_or(|max| current < max);
        let can_step_down =
            self.field.is_enabled && self.field.min_value.is_none_or(|min| current > min);
        let up = step_button("Increment", can_step_up, {
            let field = self.field.snapshot();
            let draft = self.draft.clone();
            let draft_changed = self.draft_changed.clone();
            move || adjust(&field, &draft, &draft_changed, 1.0)
        });
        let down = step_button("Decrement", can_step_down, {
            let field = self.field.snapshot();
            let draft = self.draft.clone();
            let draft_changed = self.draft_changed.clone();
            move || adjust(&field, &draft, &draft_changed, -1.0)
        });
        vec![AnyView::new(Row::new(
            VerticalAlignment::Center,
            2.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(self.input.clone().flex(1.0)),
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        1.0,
                        AnyView::new(TupleView::new(vec![up, down])),
                    )
                    .frame_min_width(22.0),
                ),
            ])),
        ))]
    }
}
crate::impl_primitive_view!(NumberFieldStepper);

fn step_button(title: &str, is_enabled: bool, action: impl Fn() + 'static) -> AnyView {
    let icon = if title == "Increment" {
        crate::primitives::media::ui_icon_catalog::navigation::chevron_up()
    } else {
        crate::primitives::media::ui_icon_catalog::navigation::chevron_down()
    };
    let label = AnyView::new(ButtonIcon::new(
        ButtonIconSource::resource(icon),
        Some(12.0),
        Some(theme_colors().1),
    ));
    let button = Button::with_options(
        label,
        action,
        ButtonRole::Normal,
        is_enabled,
        false,
        false,
        Some(title.to_string()),
    )
    .button_style(NumberStepButtonStyle);
    AnyView::new(button)
}

/// Fallback colors for the stepper glyph before a node exists.
fn theme_colors() -> (Color, Color) {
    (Color::WHITE, Color::from_rgba8(120, 120, 120, 255))
}

/// Compact spinner button chrome.
struct NumberStepButtonStyle;

impl ButtonStyle for NumberStepButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let fill = if configuration.is_pressed {
            theme.colors.state_layer_pressed
        } else if configuration.is_hovered {
            theme.colors.state_layer_hover
        } else {
            Color::CLEAR
        };
        AnyView::new(NumberStepChrome {
            label: configuration.label.clone(),
            fill,
            border: if configuration.is_focused {
                theme.colors.focus_ring
            } else {
                Color::CLEAR
            },
            border_width: if configuration.is_focused { 1.0 } else { 0.0 },
            radius: theme.radius.sm,
            alpha: if configuration.is_enabled { 1.0 } else { 0.45 },
        })
    }
}

/// Spinner button body with its press/hover face.
struct NumberStepChrome {
    label: AnyView,
    fill: Color,
    border: Color,
    border_width: f32,
    radius: f32,
    alpha: f32,
}

impl PrimitiveView for NumberStepChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(self.fill));
        node.set_corner_radius(self.radius);
        node.set_border_color(Some(self.border));
        node.set_border_width(self.border_width);
        node.set_opacity(self.alpha);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                self.label.clone(),
            )
            .padding_edges(5.0, 1.0)
            .frame(None, None, None, Some(15.0), None, None),
        )]
    }
}
crate::impl_primitive_view!(NumberStepChrome);
