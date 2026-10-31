//! Translation of Primitives/Controls/Toggle.swift.
//!
//! Boolean on/off controls: a switch track with a movable thumb, and a check box
//! that additionally reports a mixed state for partially selected collections.
use std::rc::Rc;

use crate::foundation::ColorAdjustExt;
use crate::modifiers::interaction::{AccessibilityExt, CursorExt};
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::adaptive::apply_touch_control_extent;
use crate::primitives::text::Text;
use crate::theme::{SemanticFontRef, theme_for_node};
use crate::{
    AnyView, PrimitiveView, Row, TupleView, VerticalAlignment, View, ViewExt, impl_primitive_view,
};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventPhase, EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, Node, PointerCaptureHolder, PointerPhase, Rect,
    WindowRevealContext,
};
use cove_core::{Binding, Color};
use cove_platform::{MouseButton, SystemCursor};

use super::button::ActionControlAccessibility;

const PRESSED: &str = "__bool_control_pressed";
const HOVERED: &str = "__bool_control_hovered";
const ON: &str = "__bool_control_on";
const VARIANT: &str = "__bool_control_variant";
const VISUAL_PROGRESS: &str = "__bool_visual_progress";
const VISUAL_FILL: &str = "__bool_visual_fill";

/// Which boolean control is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BoolControlVariant {
    #[default]
    Toggle,
    Checkbox,
    MixedCheckbox,
}

impl BoolControlVariant {
    /// Intrinsic control size in logical points.
    pub fn layout_size(self) -> (f32, f32) {
        match self {
            // 36x20 track with a 16-point knob.
            Self::Toggle => (36.0, 20.0),
            Self::Checkbox | Self::MixedCheckbox => (16.0, 16.0),
        }
    }

    fn is_switch(self) -> bool {
        matches!(self, Self::Toggle)
    }
}

/// Check box state, including the mixed state for partial selections.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CheckboxState {
    #[default]
    Off,
    On,
    /// Mixed state represents a partially selected collection. Activation
    /// selects it fully.
    Mixed,
}

impl CheckboxState {
    pub fn is_on(self) -> bool {
        self == Self::On
    }

    /// Activation turns mixed into on, on into off, off into on.
    pub fn activated(self) -> Self {
        match self {
            Self::On => Self::Off,
            Self::Off | Self::Mixed => Self::On,
        }
    }
}

/// Boolean switch.
pub struct Toggle {
    pub is_on: Binding<bool>,
    pub is_enabled: bool,
    pub label: Option<String>,
}

impl Toggle {
    pub fn new(is_on: Binding<bool>) -> Self {
        Self {
            is_on,
            is_enabled: true,
            label: None,
        }
    }

    pub fn labeled(label: impl Into<String>, is_on: Binding<bool>) -> Self {
        Self {
            is_on,
            is_enabled: true,
            label: Some(label.into()),
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

impl View for Toggle {
    fn body(&self) -> AnyView {
        let control = AnyView::new(BoolControlHost {
            is_on: self.is_on.clone(),
            is_enabled: self.is_enabled,
            variant: BoolControlVariant::Toggle,
        });
        match self.label.clone() {
            Some(label) => BoolControlLabel::view(label, self.is_enabled, control),
            None => control,
        }
    }
}

/// Check box with an optional mixed state.
pub struct Checkbox {
    pub state: Binding<CheckboxState>,
    pub is_enabled: bool,
    pub label: Option<String>,
}

impl Checkbox {
    pub fn new(state: Binding<CheckboxState>) -> Self {
        Self {
            state,
            is_enabled: true,
            label: None,
        }
    }

    /// Convenience over a plain boolean binding; the mixed state is never seen.
    pub fn from_boolean(is_on: Binding<bool>) -> Self {
        let state = Binding::new(
            {
                let is_on = is_on.clone();
                move || {
                    if is_on.get() {
                        CheckboxState::On
                    } else {
                        CheckboxState::Off
                    }
                }
            },
            {
                let is_on = is_on.clone();
                move |value: CheckboxState| is_on.set(value == CheckboxState::On)
            },
        );
        Self::new(state)
    }

    pub fn labeled(label: impl Into<String>, state: Binding<CheckboxState>) -> Self {
        Self {
            state,
            is_enabled: true,
            label: Some(label.into()),
        }
    }

    /// Boolean convenience that also carries a visible label.
    ///
    /// `from_boolean` erases to tri-state internally and has no label slot, so
    /// without this every caller holding a `Binding<bool>` plus a label had to
    /// hand-roll a tri-state adapter. `Toggle` already has `labeled(_:is_on:)`,
    /// so this closes the only asymmetry between the two controls.
    pub fn labeled_from_boolean(label: impl Into<String>, is_on: Binding<bool>) -> Self {
        let mut value = Self::from_boolean(is_on);
        value.label = Some(label.into());
        value
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

impl View for Checkbox {
    fn body(&self) -> AnyView {
        let mixed = self.state.get() == CheckboxState::Mixed;
        let is_on = Binding::new(
            {
                let state = self.state.clone();
                move || state.get() == CheckboxState::On
            },
            {
                let state = self.state.clone();
                move |value: bool| {
                    state.set(if value {
                        CheckboxState::On
                    } else {
                        CheckboxState::Off
                    })
                }
            },
        );
        let control = AnyView::new(BoolControlHost {
            is_on,
            is_enabled: self.is_enabled,
            variant: if mixed {
                BoolControlVariant::MixedCheckbox
            } else {
                BoolControlVariant::Checkbox
            },
        });
        match self.label.clone() {
            Some(label) => BoolControlLabel::view(label, self.is_enabled, control),
            None => control,
        }
    }
}

/// Gives the control its accessible name without re-wrapping an `AnyView`
/// (which would drop the provider modifier).
struct BoolControlLabelled {
    content: AnyView,
    label: String,
}

impl PrimitiveView for BoolControlLabelled {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_composition_value(
            &ActionControlAccessibility::label(),
            Some(self.label.clone()),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(BoolControlLabelled);

/// Label beside a boolean control; the control owns the accessible name.
struct BoolControlLabel;

impl BoolControlLabel {
    fn view(label: String, is_enabled: bool, control: AnyView) -> AnyView {
        let accessible_label = label.clone();
        AnyView::new(
            Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(BoolControlLabelled {
                        content: control,
                        label: accessible_label,
                    }),
                    AnyView::new(
                        Text::new(label)
                            .semantic_font(SemanticFontRef::body())
                            .semantic_foreground(if is_enabled {
                                crate::theme::SemanticColorRef::on_surface()
                            } else {
                                crate::theme::SemanticColorRef::on_surface_muted()
                            })
                            .modifier(crate::modifiers::interaction::AccessibilityModifier::new(
                                |semantics| semantics.is_hidden = true,
                            )),
                    ),
                ])),
            )
            .frame_min_width(0.0),
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
struct PaintIdentity {
    is_on: bool,
    is_enabled: bool,
    variant: BoolControlVariant,
}

fn interaction_pressed(node: &Node) -> bool {
    node.attachment::<bool>(PRESSED) == Some(true)
}

fn interaction_hovered(node: &Node) -> bool {
    node.attachment::<bool>(HOVERED) == Some(true)
}

/// Perceptually flat mix used for the neutral track's hover and press states.
fn mix(base: Color, toward: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: base.r + (toward.r - base.r) * amount,
        g: base.g + (toward.g - base.g) * amount,
        b: base.b + (toward.b - base.b) * amount,
        a: base.a + (toward.a - base.a) * amount,
    }
}

/// Centered within a touch-sized target, leading-aligned when stretched wider.
pub fn leading_offset(available: f32, content: f32) -> f32 {
    let excess = (available - content).max(0.0);
    if available <= 44.0 { excess * 0.5 } else { 0.0 }
}

fn update_appearance(node: &Node, is_on: bool, is_enabled: bool) {
    let theme = theme_for_node(node);
    let colors = theme.colors;
    let pressed = interaction_pressed(node);
    let hovered = interaction_hovered(node);
    let is_switch = node
        .attachment::<u8>(VARIANT)
        .map(|value| value == 0)
        .unwrap_or(true);
    // Off fill differs by control: the switch has a solid neutral track, the
    // check box shows the window background inside its border.
    let off_base = if is_switch {
        colors.border_strong
    } else {
        colors.background
    };
    let base = if !is_enabled {
        if is_switch {
            colors.border.multiplied_alpha(0.8)
        } else {
            colors.surface
        }
    } else if is_on {
        colors.accent
    } else {
        off_base
    };
    // An unchecked box keeps its fill; only the border reacts to hover.
    let off_hover = if is_switch {
        mix(colors.border_strong, colors.on_surface, 0.04)
    } else {
        colors.background
    };
    let off_press = if is_switch {
        mix(colors.border_strong, colors.on_surface, 0.08)
    } else {
        colors.background
    };
    let fill = if !is_enabled {
        base
    } else if pressed {
        if is_on {
            colors.accent_pressed
        } else {
            off_press
        }
    } else if hovered {
        if is_on {
            colors.accent_hover
        } else {
            off_hover
        }
    } else {
        base
    };
    let previous = node.attachment::<f32>(VISUAL_PROGRESS);
    let write_progress = {
        let weak = node.downgrade();
        let target = if is_on { 1.0_f32 } else { 0.0_f32 };
        let current = previous.unwrap_or(target);
        move |_node: &Node| {
            if let Some(owner) = Node::upgrade(&weak) {
                let follow = owner.downgrade();
                owner.animatable_set_with("bool.progress", current, target, move |value| {
                    if let Some(inner) = Node::upgrade(&follow) {
                        inner.set_attachment(VISUAL_PROGRESS, value);
                        inner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                            "bool.progress",
                        ));
                    }
                });
            }
        }
    };
    let write_fill = {
        let weak = node.downgrade();
        let current = node.attachment::<Color>(VISUAL_FILL).unwrap_or(fill);
        move |_node: &Node| {
            if let Some(owner) = Node::upgrade(&weak) {
                let follow = owner.downgrade();
                owner.animatable_set_optional(
                    "bool.fill",
                    Some(current),
                    Some(fill),
                    move |value| {
                        if let Some(inner) = Node::upgrade(&follow) {
                            if let Some(value) = value {
                                inner.set_attachment(VISUAL_FILL, value);
                            }
                            inner.mark_render_dirty(
                                cove_core::scene::InvalidationSource::style_set("bool.fill"),
                            );
                        }
                    },
                );
            }
        }
    };
    if previous.is_some() {
        write_progress(node);
        write_fill(node);
    } else {
        write_progress(node);
        write_fill(node);
    }
}

fn set_interaction(node: &Node, key: &str, value: bool) {
    if node.attachment::<bool>(key) == Some(value) {
        return;
    }
    node.set_attachment(key, value);
    update_appearance(
        node,
        node.attachment::<bool>(ON) == Some(true),
        node.is_focusable(),
    );
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(key));
}

/// Stateful boolean control surface shared by the switch and the check box.
pub struct BoolControlHost {
    pub is_on: Binding<bool>,
    pub is_enabled: bool,
    pub variant: BoolControlVariant,
}

impl PrimitiveView for BoolControlHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        let (width, height) = self.variant.layout_size();
        layout.set_width(Some(width));
        layout.set_height(Some(height));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        let (width, height) = self.variant.layout_size();
        layout.set_width(Some(width));
        layout.set_height(Some(height));
    }

    fn update_node(&self, node: &Node) {
        apply_touch_control_extent(node, true);
        if !crate::primitives::controls::adaptive::supports_control_hover(node) {
            node.set_attachment(HOVERED, false);
        }
        node.set_is_focusable(self.is_enabled);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: if self.variant.is_switch() {
                AccessibilityRole::Toggle
            } else {
                AccessibilityRole::Checkbox
            },
            label: node
                .composition_value(&ActionControlAccessibility::label())
                .unwrap_or_default(),
            value: if self.variant == BoolControlVariant::MixedCheckbox {
                "mixed".to_string()
            } else if self.is_on.get() {
                "1".to_string()
            } else {
                "0".to_string()
            },
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                is_selected: self.is_on.get(),
                ..Default::default()
            },
            ..Default::default()
        }));
        let is_on = self.is_on.clone();
        let weak = node.downgrade();
        let activate: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(owner) = Node::upgrade(&weak) {
                WindowRevealContext::record_activation(&owner);
            }
            is_on.set(!is_on.get());
        });
        node.set_accessibility_actions(AccessibilityActions {
            activate: if self.is_enabled {
                Some(activate.clone())
            } else {
                None
            },
            ..Default::default()
        });

        if !self.is_enabled {
            node.set_attachment(PRESSED, false);
            node.set_attachment(HOVERED, false);
            if PointerCaptureHolder::current()
                .and_then(|capture| capture.target())
                .is_some_and(|target| target == *node)
                && let Some(capture) = PointerCaptureHolder::current()
            {
                capture.release();
            }
            if let Some(chain) = FocusChainHolder::current()
                && chain.focused().is_some_and(|focused| focused == *node)
            {
                chain.clear();
            }
        } else {
            if node.attachment::<bool>(PRESSED).is_none() {
                node.set_attachment(PRESSED, false);
            }
            if node.attachment::<bool>(HOVERED).is_none() {
                node.set_attachment(HOVERED, false);
            }
        }
        node.set_attachment(
            ON,
            self.is_on.get() || self.variant == BoolControlVariant::MixedCheckbox,
        );
        node.set_attachment(
            VARIANT,
            match self.variant {
                BoolControlVariant::Toggle => 0_u8,
                BoolControlVariant::Checkbox => 1,
                BoolControlVariant::MixedCheckbox => 2,
            },
        );
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));

        update_appearance(
            node,
            self.is_on.get() || self.variant == BoolControlVariant::MixedCheckbox,
            self.is_enabled,
        );

        let weak = node.downgrade();
        let variant = self.variant;
        let enabled = self.is_enabled;
        let snapshot_on = self.is_on.get();
        node.update_draw(
            PaintIdentity {
                is_on: snapshot_on,
                is_enabled: enabled,
                variant,
            },
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                match variant {
                    BoolControlVariant::Toggle => render_toggle(&owner, origin, list, variant),
                    BoolControlVariant::Checkbox | BoolControlVariant::MixedCheckbox => {
                        render_checkbox(&owner, origin, list, variant, enabled)
                    }
                }
            },
        );

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }

        let hover_node = node.downgrade();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let Some(owner) = Node::upgrade(&hover_node) else {
                    return;
                };
                match phase {
                    HoverPhase::Enter => set_interaction(
                        &owner,
                        HOVERED,
                        crate::primitives::controls::adaptive::supports_control_hover(&owner),
                    ),
                    HoverPhase::Leave => set_interaction(&owner, HOVERED, false),
                }
            }),
        );

        let pointer_node = node.downgrade();
        let pointer_on = self.is_on.clone();
        let pointer_enabled = self.is_enabled;
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, _event_phase| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        set_interaction(&owner, PRESSED, true);
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        let was_pressed = owner.attachment::<bool>(PRESSED) == Some(true);
                        set_interaction(&owner, PRESSED, false);
                        if !was_pressed {
                            return EventResult::Ignored;
                        }
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        let frame = owner.absolute_frame();
                        if !(event.x >= frame.x
                            && event.x <= frame.x + frame.width
                            && event.y >= frame.y
                            && event.y <= frame.y + frame.height)
                        {
                            return EventResult::Handled;
                        }
                        pointer_on.set(!pointer_on.get());
                        owner.set_attachment(ON, pointer_on.get());
                        update_appearance(&owner, pointer_on.get(), pointer_enabled);
                        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                            "boolControlValue",
                        ));
                        EventResult::Handled
                    }
                }
            }),
        );

        let key_node = node.downgrade();
        let key_on = self.is_on.clone();
        let key_enabled = self.is_enabled;
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target || event.is_repeat {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    crate::Scancode::RETURN
                    | crate::Scancode::SPACE
                    | crate::Scancode::KEYPAD_ENTER => {
                        let Some(owner) = Node::upgrade(&key_node) else {
                            return EventResult::Ignored;
                        };
                        key_on.set(!key_on.get());
                        owner.set_attachment(ON, key_on.get());
                        update_appearance(&owner, key_on.get(), key_enabled);
                        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                            "boolControlValue",
                        ));
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }),
        );
    }
}
impl_primitive_view!(BoolControlHost);

fn fill_color(
    node: &Node,
    colors: &crate::theme::ColorScheme,
    is_on: bool,
    is_enabled: bool,
    is_switch: bool,
) -> Color {
    if let Some(fill) = node.attachment::<Color>(VISUAL_FILL) {
        return fill.multiplied_alpha(node.opacity());
    }
    if !is_enabled {
        return if is_switch {
            colors.border.multiplied_alpha(0.8)
        } else {
            colors.surface
        };
    }
    let pressed = interaction_pressed(node);
    let hovered = interaction_hovered(node);
    if is_on {
        return if pressed {
            colors.accent_pressed
        } else if hovered {
            colors.accent_hover
        } else {
            colors.accent
        };
    }
    if !is_switch {
        return colors.background;
    }
    // Off is a solid neutral track (border darkened toward text): a grey track
    // rather than a translucent wash.
    if pressed {
        mix(colors.border_strong, colors.on_surface, 0.08)
    } else if hovered {
        mix(colors.border_strong, colors.on_surface, 0.04)
    } else {
        colors.border_strong
    }
}

fn render_toggle(
    node: &Node,
    origin: cove_core::Point,
    list: &mut cove_core::DrawList,
    variant: BoolControlVariant,
) {
    let theme = theme_for_node(node);
    let colors = theme.colors;
    let frame = node.frame();
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return;
    }
    let (track_width_intrinsic, _) = variant.layout_size();
    let track_height = 20.0_f32;
    let thumb_diameter = 16.0_f32;
    let thumb_inset = 2.0_f32;
    // The switch keeps its intrinsic size. A touch extent centers it in the
    // 44 pt target; a layout that stretches the control leaves it at the leading
    // edge instead of painting a full-width pill.
    let track_width = frame.width.min(track_width_intrinsic);
    let track = Rect::new(
        origin.x + leading_offset(frame.width, track_width),
        origin.y + (frame.height - track_height) * 0.5,
        track_width,
        track_height,
    );
    let is_on = node.attachment::<bool>(ON) == Some(true);
    let is_enabled = node.is_focusable();
    let _ = list.add_rounded_rect(
        track,
        track_height * 0.5,
        fill_color(node, &colors, is_on, is_enabled, true),
    );
    let thumb_travel = track.width - 2.0 * thumb_inset - thumb_diameter;
    let progress = node
        .attachment::<f32>(VISUAL_PROGRESS)
        .unwrap_or(if is_on { 1.0 } else { 0.0 });
    let thumb_x = track.x + thumb_inset + thumb_travel * progress;
    let thumb = Rect::new(
        thumb_x,
        track.y + (track.height - thumb_diameter) * 0.5,
        thumb_diameter,
        thumb_diameter,
    );
    // The knob is a white disc with a small key shadow: a raised disc rather
    // than an outlined one.
    let thumb_fill = if is_enabled {
        Color::WHITE
    } else {
        Color::WHITE.multiplied_alpha(0.6)
    };
    let _ = list.add_rounded_rect(thumb, thumb.height * 0.5, thumb_fill);

    // The ring belongs outside the whole control, with a gap: flush against the
    // fill it cannot be told apart from it.
    if focus_visible(node) {
        let outset = 3.0;
        let ring = Rect::new(
            track.x - outset,
            track.y - outset,
            track.width + 2.0 * outset,
            track.height + 2.0 * outset,
        );
        let _ = list.add_rounded_rect_stroke(ring, ring.height * 0.5, 2.0, colors.focus_ring);
    }
}

fn render_checkbox(
    node: &Node,
    origin: cove_core::Point,
    list: &mut cove_core::DrawList,
    variant: BoolControlVariant,
    is_enabled: bool,
) {
    let theme = theme_for_node(node);
    let colors = theme.colors;
    let frame = node.frame();
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return;
    }
    let is_on = node.attachment::<bool>(ON) == Some(true);
    let edge = frame.width.min(frame.height).min(variant.layout_size().0);
    let box_rect = Rect::new(
        origin.x + leading_offset(frame.width, edge),
        origin.y + (frame.height - edge) * 0.5,
        edge,
        edge,
    );
    // A 16x16 box: background fill with a 1-px inset border.
    let fill = fill_color(node, &colors, is_on, is_enabled, false);
    let border = if !is_enabled {
        colors.on_surface.multiplied_alpha(0.3)
    } else if is_on {
        fill
    } else if interaction_hovered(node) {
        colors.border_strong
    } else {
        colors.border
    };
    let _ = list.add_rounded_rect(box_rect, 4.0, fill);
    let _ = list.add_rounded_rect_stroke(box_rect, 4.0, 1.0, border);
    if focus_visible(node) {
        let outset = 3.0;
        let ring = Rect::new(
            box_rect.x - outset,
            box_rect.y - outset,
            box_rect.width + 2.0 * outset,
            box_rect.height + 2.0 * outset,
        );
        let _ = list.add_rounded_rect_stroke(ring, 4.0 + outset, 2.0, colors.focus_ring);
    }

    let progress = node
        .attachment::<f32>(VISUAL_PROGRESS)
        .unwrap_or(if is_on { 1.0 } else { 0.0 });
    if progress <= 0.0 {
        return;
    }
    let inset = (edge * 0.18).max(3.0);
    let x0 = box_rect.x + inset;
    let y0 = box_rect.y + edge * 0.55;
    let x1 = box_rect.x + edge * 0.42;
    let y1 = box_rect.y + edge - inset;
    let x2 = box_rect.x + edge - inset;
    let y2 = box_rect.y + inset;
    let line_color = (if is_enabled {
        colors.on_accent
    } else {
        colors.on_surface_muted
    })
    .multiplied_alpha(progress * node.opacity());
    if variant == BoolControlVariant::MixedCheckbox {
        let _ = list.add_line(
            cove_core::Point::new(x0, box_rect.y + edge / 2.0),
            cove_core::Point::new(x2, box_rect.y + edge / 2.0),
            2.0,
            line_color,
        );
        return;
    }
    let _ = list.add_line(
        cove_core::Point::new(x0, y0),
        cove_core::Point::new(x1, y1),
        2.0,
        line_color,
    );
    let _ = list.add_line(
        cove_core::Point::new(x1, y1),
        cove_core::Point::new(x2, y2),
        2.0,
        line_color,
    );
}

fn focus_visible(node: &Node) -> bool {
    FocusChainHolder::current().is_some_and(|chain| {
        chain.focused().is_some_and(|focused| focused == *node) && chain.is_focus_visible()
    })
}
