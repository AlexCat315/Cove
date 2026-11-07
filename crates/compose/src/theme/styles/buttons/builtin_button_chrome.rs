//! Translation of Primitives/Controls/Button/Chrome.swift.
//!
//! `ButtonStyle` is a declarative protocol, but the built-in button faces need
//! to answer the pointer without a recomposition. This chrome primitive owns
//! that node-driven repaint: it stores its kind/metrics on the node and reads
//! the host button's live pressed/hovered/focus attachments every time it is
//! applied.
use cove_core::scene::{
    ActiveAnimationContext, Align, Animation, Edge, FlexDirection, FocusChainHolder, Justify,
    LayoutNode, Node,
};
use cove_core::{Color, InputCapabilities};

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::theme::{
    ControlSize, SemanticColorRef, SemanticFontRef, SemanticMotionRef, SemanticStyleViewExt, Theme,
    theme_for_node,
};
use crate::{AnyView, PrimitiveView, View, ViewKind};

use super::button_style::ButtonStyleConfiguration;

/// Attachment keys owned by `ButtonHost` (Primitives/Controls/Button.swift).
pub const BUTTON_HOST_MARKER_KEY: &str = "__button_host";
pub const BUTTON_HOST_PRESSED_KEY: &str = "__button_pressed";
pub const BUTTON_HOST_HOVERED_KEY: &str = "__button_hovered";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuiltinButtonChromeKind {
    Primary,
    Secondary,
    Ghost,
    Destructive,
    Toggle { min_width: f32, height: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuiltinButtonChromeMetrics {
    pub height: f32,
    pub min_width: Option<f32>,
    pub horizontal_padding: f32,
    pub radius: f32,
    pub vertical_padding: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BuiltinButtonChromeState {
    kind: BuiltinButtonChromeKind,
    is_enabled: bool,
    is_selected: bool,
    is_icon_only: bool,
    metrics: BuiltinButtonChromeMetrics,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct BuiltinButtonChromeValues {
    background: Color,
    border: Color,
    border_width: f32,
    radius: f32,
    opacity: f32,
    scale: f32,
}

/// Background + metrics for a filled/tonal/ghost/toggle button face.
pub struct BuiltinButtonChrome {
    kind: BuiltinButtonChromeKind,
    is_enabled: bool,
    is_selected: bool,
    is_icon_only: bool,
    foreground: SemanticColorRef,
    label: AnyView,
    metrics: BuiltinButtonChromeMetrics,
}
impl BuiltinButtonChrome {
    pub fn new(
        kind: BuiltinButtonChromeKind,
        configuration: &ButtonStyleConfiguration,
        foreground: SemanticColorRef,
    ) -> Self {
        Self {
            kind,
            is_enabled: configuration.is_enabled,
            is_selected: configuration.is_selected,
            is_icon_only: configuration.is_icon_only,
            foreground,
            label: configuration.label.clone(),
            metrics: kind.metrics(
                &configuration.theme,
                configuration.control_size,
                configuration.is_icon_only,
                configuration.input,
            ),
        }
    }

    fn state(&self) -> BuiltinButtonChromeState {
        BuiltinButtonChromeState {
            kind: self.kind,
            is_enabled: self.is_enabled,
            is_selected: self.is_selected,
            is_icon_only: self.is_icon_only,
            metrics: self.metrics,
        }
    }
}

impl BuiltinButtonChromeKind {
    pub fn metrics(
        self,
        theme: &Theme,
        size: ControlSize,
        is_icon_only: bool,
        input: InputCapabilities,
    ) -> BuiltinButtonChromeMetrics {
        let touch = input.contains(InputCapabilities::TOUCH);
        let height = if touch {
            size.touch_height()
        } else {
            size.button_height()
        };
        let line_height = theme.typography.label.line_height;
        match self {
            Self::Primary | Self::Secondary | Self::Ghost | Self::Destructive => {
                BuiltinButtonChromeMetrics {
                    height,
                    min_width: if is_icon_only { Some(height) } else { None },
                    horizontal_padding: if is_icon_only {
                        (height - line_height) / 2.0
                    } else if touch {
                        size.touch_padding()
                    } else {
                        size.horizontal_padding()
                    }
                    .max(if is_icon_only { 2.0 } else { 0.0 }),
                    // Push buttons are capsules; icon-only buttons become circles.
                    radius: height * 0.5,
                    vertical_padding: ((height - line_height) / if touch { 4.0 } else { 2.0 })
                        .max(if touch { 4.0 } else { 2.0 }),
                }
            }
            Self::Toggle {
                min_width,
                height: authored_height,
            } => {
                let height = if touch {
                    height.max(authored_height)
                } else {
                    authored_height
                };
                BuiltinButtonChromeMetrics {
                    height,
                    min_width: Some(min_width),
                    horizontal_padding: if touch { 12.0 } else { 7.0 },
                    radius: if touch {
                        10.0f32.max(theme.radius.sm)
                    } else {
                        theme.radius.sm
                    },
                    vertical_padding: ((height - line_height) / if touch { 4.0 } else { 2.0 })
                        .max(if touch { 4.0 } else { 2.0 }),
                }
            }
        }
    }
}

impl PrimitiveView for BuiltinButtonChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("__builtin_button_chrome_state", self.state());
        apply_builtin_button_chrome(node);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        // The host owns the hit region. Built-in chrome should occupy that same
        // region when a form/toolbar gives the button extra width, while
        // retaining its intrinsic label width when the host is unconstrained.
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        layout.set_min_height(Some(self.metrics.height));
        if let Some(min_width) = self.metrics.min_width {
            layout.set_min_width(Some(min_width));
        }
        layout.set_padding(self.metrics.vertical_padding, Edge::Top);
        layout.set_padding(self.metrics.horizontal_padding, Edge::Start);
        layout.set_padding(self.metrics.vertical_padding, Edge::Bottom);
        layout.set_padding(self.metrics.horizontal_padding, Edge::End);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let label = AnyView::new(self.label.clone())
            .frame_min_width(0.0)
            .shrink(1.0)
            .semantic_font(SemanticFontRef::label())
            .semantic_foreground(self.foreground.clone());
        vec![AnyView::new(label)]
    }
}

impl View for BuiltinButtonChrome {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Recursively re-applies chrome after an interaction change. This keeps the
/// built-in styles on their node-driven fast path while still updating colors,
/// focus rings, and press transforms immediately.
pub fn update_builtin_button_chrome_descendants(button_node: &Node) {
    for child in button_node.children() {
        apply_builtin_button_chrome(&child);
        update_builtin_button_chrome_descendants(&child);
    }
}

fn apply_builtin_button_chrome(chrome_node: &Node) {
    let Some(state) =
        chrome_node.attachment::<BuiltinButtonChromeState>("__builtin_button_chrome_state")
    else {
        return;
    };
    let button_node = nearest_button_host_ancestor(chrome_node);
    let values = builtin_button_chrome_values(state, chrome_node, button_node.as_ref());

    // Hover and press answer on the same frame: a control that eases toward
    // the pointer reads as a delayed answer. Release is the moment the face
    // springs back, so only the way out of a press travels.
    let previous_scale = chrome_node.transform().scale;
    let releasing = values.scale > previous_scale && !chrome_node.prefers_reduced_motion();
    let theme = theme_for_node(chrome_node);
    let release_fill = (releasing && ActiveAnimationContext::current().animation.is_none())
        .then(|| SemanticMotionRef::fast().resolve(&theme));
    let owner = chrome_node.clone();
    let current = chrome_node.background_color();
    within(release_fill, || {
        chrome_node.animatable_set_optional(
            "backgroundColor",
            current,
            Some(values.background),
            move |value| {
                owner.set_background_color(value);
            },
        );
    });
    let owner = chrome_node.clone();
    let current = chrome_node.border_color();
    chrome_node.animatable_set_optional(
        "borderColor",
        current,
        Some(values.border),
        move |value| {
            owner.set_border_color(value);
        },
    );
    let owner = chrome_node.clone();
    let current = chrome_node.border_width();
    chrome_node.animatable_set_with("borderWidth", current, values.border_width, move |value| {
        owner.set_border_width(value);
    });
    let owner = chrome_node.clone();
    let current = chrome_node.corner_radius();
    chrome_node.animatable_set_with("cornerRadius", current, values.radius, move |value| {
        owner.set_corner_radius(value);
    });
    let owner = chrome_node.clone();
    let current = chrome_node.opacity();
    chrome_node.animatable_set_with("opacity", current, values.opacity, move |value| {
        owner.set_opacity(value);
    });

    let mut transform = chrome_node.transform();
    transform.anchor = cove_core::Point::new(0.5, 0.5);
    chrome_node.set_transform(transform);
    // A bordered push button rests on a faint contact shadow that a press
    // tucks away, as on macOS.
    let rests_raised = matches!(state.kind, BuiltinButtonChromeKind::Secondary)
        && !state.is_icon_only
        && state.is_enabled
        && values.scale >= 1.0;
    chrome_node.set_shadow(if rests_raised {
        theme.elevation.low
    } else {
        cove_core::SurfaceShadow::NONE
    });
    let release = releasing.then(|| button_release_spring(&theme));
    let owner = chrome_node.clone();
    within(release, || {
        chrome_node.animatable_set_with(
            "button.pressScale",
            previous_scale,
            values.scale,
            move |value| {
                let mut transform = owner.transform();
                transform.scale = value.max(0.0);
                owner.set_transform(transform);
            },
        );
    });
}

/// Runs `body` inside `animation` when there is one, and otherwise leaves the
/// caller's transaction (a theme cross-fade, for instance) in charge.
fn within(animation: Option<Animation>, body: impl FnOnce()) {
    match animation {
        Some(animation) => ActiveAnimationContext::with(Some(animation), None, None, body),
        None => body(),
    }
}

/// The release eases the face back to rest over the fast duration: a press
/// answers at once and lets go softly, without a bounce.
pub fn button_release_spring(theme: &Theme) -> Animation {
    Animation::new(
        theme.motion.fast * 1.5,
        crate::theme::Easing::RESPONSIVE.as_animation_curve(),
    )
}

fn builtin_button_chrome_values(
    state: BuiltinButtonChromeState,
    chrome_node: &Node,
    button_node: Option<&Node>,
) -> BuiltinButtonChromeValues {
    let theme = theme_for_node(chrome_node);
    let pressed = state.is_enabled
        && button_node
            .and_then(|node| node.attachment::<bool>(BUTTON_HOST_PRESSED_KEY))
            .unwrap_or(false);
    let hovered = state.is_enabled
        && button_node
            .and_then(|node| node.attachment::<bool>(BUTTON_HOST_HOVERED_KEY))
            .unwrap_or(false);
    let focus_chain = FocusChainHolder::current();
    let focused = state.is_enabled
        && focus_chain
            .as_ref()
            .is_some_and(|chain| chain.is_focus_visible())
        && button_node.is_some_and(|button| {
            focus_chain
                .as_ref()
                .and_then(|chain| chain.focused())
                .as_ref()
                == Some(button)
        });
    let metrics = state.metrics;
    let clear = Color::CLEAR;

    let mut has_border = false;
    let background = match state.kind {
        BuiltinButtonChromeKind::Primary => {
            if !state.is_enabled {
                // Disabled primary shares the control face and keeps its text
                // muted, rather than a washed-out accent.
                theme.colors.surface
            } else if pressed {
                theme.colors.accent_pressed
            } else if hovered {
                theme.colors.accent_hover
            } else {
                theme.colors.accent
            }
        }
        // A glyph-only push button has no bezel on macOS: it answers the
        // pointer with a state layer, like a ghost button.
        BuiltinButtonChromeKind::Secondary if state.is_icon_only => {
            if pressed {
                theme.colors.state_layer_pressed
            } else if hovered {
                theme.colors.state_layer_hover
            } else {
                clear
            }
        }
        BuiltinButtonChromeKind::Secondary => {
            // A solid control face (surface) with a 1-px border, darkening on
            // hover and press — matching the reference button treatment.
            has_border = state.is_enabled;
            if !state.is_enabled {
                theme.colors.surface
            } else if pressed {
                theme.colors.surface_pressed
            } else if hovered {
                theme.colors.surface_variant
            } else {
                theme.colors.surface
            }
        }
        BuiltinButtonChromeKind::Ghost => {
            if pressed {
                theme.colors.state_layer_pressed
            } else if hovered {
                theme.colors.state_layer_hover
            } else {
                clear
            }
        }
        BuiltinButtonChromeKind::Destructive => {
            let error = theme.colors.error;
            if !state.is_enabled {
                theme.colors.surface
            } else if pressed {
                error.composited(theme.colors.state_layer_pressed)
            } else if hovered {
                error.composited(theme.colors.state_layer_hover)
            } else {
                error
            }
        }
        BuiltinButtonChromeKind::Toggle { .. } => {
            // Selection still conveys the current mode when its command cannot
            // be repeated (e.g. Play while already playing). Keep the paired
            // accent/onAccent colors even when disabled.
            if state.is_selected {
                if pressed {
                    theme.colors.accent_pressed
                } else if hovered {
                    theme.colors.accent_hover
                } else {
                    theme.colors.accent
                }
            } else if pressed {
                theme.colors.state_layer_pressed
            } else if hovered {
                theme.colors.state_layer_hover
            } else {
                clear
            }
        }
    };

    // Keyboard focus paints a 2-px accent ring; otherwise the secondary face
    // carries its 1-px hairline.
    let (border, border_width) = if focused {
        (theme.colors.focus_ring, 2.0)
    } else if has_border {
        (theme.colors.border, 1.0)
    } else {
        (clear, 0.0)
    };

    let press_scale = theme.motion.press_scale;
    let scale = if pressed && !chrome_node.prefers_reduced_motion() {
        if press_scale.is_finite() {
            press_scale.clamp(0.8, 0.99)
        } else {
            0.97
        }
    } else {
        1.0
    };

    BuiltinButtonChromeValues {
        background,
        border,
        border_width,
        radius: metrics.radius,
        opacity: if state.is_enabled || theme.text_emphasis.disabled.is_some() {
            1.0
        } else if state.is_selected {
            0.75
        } else {
            0.55
        },
        scale,
    }
}

fn nearest_button_host_ancestor(node: &Node) -> Option<Node> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate
            .attachment::<bool>(BUTTON_HOST_MARKER_KEY)
            .unwrap_or(false)
        {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}
