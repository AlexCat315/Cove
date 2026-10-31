//! Translation of Primitives/Controls/Toolbar.swift.
//!
//! Groups controls with shared density and arrow-key navigation along the axis.
//! Child inputs get first refusal for caret movement. Disabling the toolbar blocks
//! all hosted input and removes its descendants from focus traversal.
pub mod commands;
pub mod overflow_layout;

use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::button::ButtonIconSource;
use crate::primitives::overlays::roving_focus::SelectionNavigation;
use crate::theme::{ControlSize, ControlSizeEnvironment, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, CompositionLocal, EventResult, FlexDirection,
    FocusChainHolder, InputHandlerRoute, InteractionRegistryHolder, Justify, LayoutNode, Node,
    Wrap,
};

/// Which edge carries the toolbar's divider.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToolbarDividerEdge {
    None,
    Top,
    #[default]
    Bottom,
    /// `Leading`/`Trailing` suit vertical toolbars docked beside content.
    Leading,
    Trailing,
}

/// Main axis of a toolbar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToolbarAxis {
    #[default]
    Horizontal,
    Vertical,
}

/// Shared toolbar height for the current control size.
pub fn toolbar_height(size: ControlSize) -> f32 {
    size.button_height() + 8.0
}

/// Toolbar density derived from the control size.
pub struct ToolbarMetrics;

impl ToolbarMetrics {
    /// Height at the regular control size.
    pub fn height() -> f32 {
        Self::height_for(ControlSize::Regular)
    }

    /// Control height inside a regular toolbar.
    pub fn control_height() -> f32 {
        Self::height() - 8.0
    }

    pub fn height_for(size: ControlSize) -> f32 {
        size.button_height() + 8.0
    }
}

pub struct ToolbarAxisEnvironment;
impl ToolbarAxisEnvironment {
    pub fn axis() -> Rc<CompositionLocal<ToolbarAxis>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<ToolbarAxis>> =
                Rc::new(CompositionLocal::constant(ToolbarAxis::Horizontal));
        }
        KEY.with(Rc::clone)
    }
}

/// Authored toolbar options.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolbarOptions {
    pub divider: ToolbarDividerEdge,
    pub is_enabled: bool,
    pub label: String,
    pub axis: ToolbarAxis,
}

impl Default for ToolbarOptions {
    fn default() -> Self {
        Self {
            divider: ToolbarDividerEdge::Bottom,
            is_enabled: true,
            label: "Toolbar".to_string(),
            axis: ToolbarAxis::Horizontal,
        }
    }
}

impl ToolbarOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Where one command sits in the bar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolbarItemPlacement {
    /// Larger priorities remain in the bar longer. Ties retain source order.
    #[default]
    Automatic,
    AutomaticWithPriority(i32),
    /// Keep a primary command in the bar, scrolling if necessary.
    Always,
    /// Always in the overflow menu.
    Overflow,
}

impl ToolbarItemPlacement {
    /// Larger stays in the bar longer; `Always` wins, `Overflow` never inlines.
    pub fn priority(self) -> i32 {
        match self {
            Self::Always => i32::MAX,
            Self::Automatic => 0,
            Self::AutomaticWithPriority(value) => value,
            Self::Overflow => i32::MIN,
        }
    }
}

/// One command shared by the bar and its overflow menu. Actions and enabled,
/// selected and destructive semantics come from the same `MenuItem` value.
#[derive(Clone)]
pub struct ToolbarItem {
    pub command: crate::primitives::overlays::menu::MenuItem,
    pub icon: Option<ButtonIconSource>,
    pub placement: ToolbarItemPlacement,
}

impl ToolbarItem {
    pub fn new(command: crate::primitives::overlays::menu::MenuItem) -> Self {
        Self {
            command,
            icon: None,
            placement: ToolbarItemPlacement::Automatic,
        }
    }

    pub fn icon(mut self, icon: ButtonIconSource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn placement(mut self, placement: ToolbarItemPlacement) -> Self {
        self.placement = placement;
        self
    }
}

/// Groups controls with arrow-key navigation along the toolbar axis.
pub use commands::{ToolbarCommandPresentation, ToolbarCommands};
pub use overflow_layout::{ToolbarArrangement, ToolbarOverflowLayout};

/// A toolbar's items and options.
pub struct Toolbar {
    pub options: ToolbarOptions,
    /// Explicit minimum height; `None` follows the control size.
    pub minimum_height: Option<f32>,
    pub vertical_padding: f32,
    content: AnyView,
}

impl Toolbar {
    pub fn new(content: impl View + 'static) -> Self {
        Self {
            options: ToolbarOptions::default(),
            minimum_height: None,
            vertical_padding: 4.0,
            content: AnyView::new(content),
        }
    }

    pub fn options(mut self, options: ToolbarOptions) -> Self {
        self.options = options;
        self
    }

    pub fn minimum_height(mut self, height: f32) -> Self {
        self.minimum_height = Some(height);
        self
    }

    pub fn vertical_padding(mut self, padding: f32) -> Self {
        self.vertical_padding = padding;
        self
    }
}

/// Collects the focusable Tab stops a nav command can move between.
fn navigation_items(node: &Node) -> Vec<Node> {
    if !node.accepts_subtree_input() {
        return Vec::new();
    }
    let mut items = Vec::new();
    if node.is_focusable() && node.is_tab_stop() {
        items.push(node.clone());
    }
    for child in node.children() {
        items.extend(navigation_items(&child));
    }
    items
}

impl PrimitiveView for Toolbar {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_allows_hit_testing(self.options.is_enabled);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Toolbar,
            label: self.options.label.clone(),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.options.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
        node.set_attachment(
            "toolbar.axis",
            if self.options.axis == ToolbarAxis::Vertical {
                1_u8
            } else {
                0_u8
            },
        );
        if !self.options.is_enabled {
            // Disabling must not leave capture or focus stranded inside the bar.
            if let (Some(chain), Some(captured)) = (
                FocusChainHolder::current(),
                cove_core::scene::PointerCaptureHolder::current_target(),
            ) && chain.contains(&captured, node)
                && let Some(capture) = cove_core::scene::PointerCaptureHolder::current()
            {
                capture.release();
            }
            if let Some(chain) = FocusChainHolder::current()
                && let Some(focused) = chain.focused()
                && chain.contains(&focused, node)
            {
                chain.clear();
            }
        }
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let enabled = self.options.is_enabled;
        let vertical = self.options.axis == ToolbarAxis::Vertical;
        let key_node = node.downgrade();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if !enabled || phase != cove_core::scene::EventPhase::Bubble {
                    return EventResult::Ignored;
                }
                if !event.modifiers.is_empty() {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&key_node) else {
                    return EventResult::Ignored;
                };
                let command = match event.scancode {
                    crate::Scancode::ARROW_LEFT if !vertical => SelectionNavigation::Previous,
                    crate::Scancode::ARROW_UP if vertical => SelectionNavigation::Previous,
                    crate::Scancode::ARROW_RIGHT if !vertical => SelectionNavigation::Next,
                    crate::Scancode::ARROW_DOWN if vertical => SelectionNavigation::Next,
                    crate::Scancode::HOME => SelectionNavigation::First,
                    crate::Scancode::END => SelectionNavigation::Last,
                    _ => return EventResult::Ignored,
                };
                let candidates = navigation_items(&owner);
                if candidates.is_empty() {
                    return EventResult::Ignored;
                }
                let focused = FocusChainHolder::current().and_then(|chain| chain.focused());
                let current = focused.as_ref().and_then(|focused| {
                    candidates.iter().position(|candidate| candidate == focused)
                });
                let indices: Vec<usize> = (0..candidates.len()).collect();
                let Some(index) = command.destination(&indices, current.as_ref(), true) else {
                    return EventResult::Ignored;
                };
                let Some(target) = candidates.get(index) else {
                    return EventResult::Ignored;
                };
                if let Some(chain) = FocusChainHolder::current() {
                    chain.focus(Some(target), true);
                }
                crate::primitives::layout::ScrollReveal::reveal(target, false);
                EventResult::Handled
            }),
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let size = node.composition_value(&ControlSizeEnvironment::key());
        let extent = self.minimum_height.unwrap_or_else(|| toolbar_height(size));
        let vertical = self.options.axis == ToolbarAxis::Vertical;
        let side_divider = matches!(
            self.options.divider,
            ToolbarDividerEdge::Leading | ToolbarDividerEdge::Trailing
        );
        let mut column: Vec<AnyView> = Vec::new();
        if self.options.divider == ToolbarDividerEdge::Top {
            column.push(AnyView::new(crate::primitives::text::Divider::new(
                None,
                1.0,
                crate::primitives::text::DividerAxis::Horizontal,
            )));
        }
        if self.options.divider == ToolbarDividerEdge::Leading {
            column.push(AnyView::new(crate::primitives::text::Divider::new(
                None,
                1.0,
                crate::primitives::text::DividerAxis::Vertical,
            )));
        }
        column.push(if vertical {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    theme.spacing.xs,
                    self.content.clone(),
                )
                .padding_edges(self.vertical_padding, theme.spacing.sm)
                .frame_min_width(extent)
                .flex_with(1.0, 1.0, None)
                .frame(None, None, None, Some(0.0), None, None),
            )
        } else {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    theme.spacing.xs,
                    self.content.clone(),
                )
                .padding_edges(theme.spacing.sm, self.vertical_padding)
                .frame(None, None, None, Some(extent), None, None),
            )
        });
        if self.options.divider == ToolbarDividerEdge::Trailing {
            column.push(AnyView::new(crate::primitives::text::Divider::new(
                None,
                1.0,
                crate::primitives::text::DividerAxis::Vertical,
            )));
        }
        if self.options.divider == ToolbarDividerEdge::Bottom {
            column.push(AnyView::new(crate::primitives::text::Divider::new(
                None,
                1.0,
                crate::primitives::text::DividerAxis::Horizontal,
            )));
        }
        vec![AnyView::new(ToolbarChrome {
            axis: self.options.axis,
            is_enabled: self.options.is_enabled,
            side_divider,
            content: AnyView::new(BoxView::new(
                if side_divider {
                    FlexDirection::Row
                } else {
                    FlexDirection::Column
                },
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            )),
        })]
    }
}
crate::impl_primitive_view!(Toolbar);

/// Publishes the toolbar axis, its surface and the ghost button style.
struct ToolbarChrome {
    axis: ToolbarAxis,
    is_enabled: bool,
    side_divider: bool,
    content: AnyView,
}

impl PrimitiveView for ToolbarChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_composition_value(&ToolbarAxisEnvironment::axis(), self.axis);
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface));
        node.set_opacity(if self.is_enabled { 1.0 } else { 0.6 });
        node.set_attachment("toolbar.sideDivider", self.side_divider);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(ToolbarChrome);

/// A bordered or labelled run of toolbar controls.
pub struct ToolbarGroup {
    pub is_bordered: bool,
    pub label: String,
    content: AnyView,
}

impl ToolbarGroup {
    pub fn new(content: impl View + 'static) -> Self {
        Self {
            is_bordered: false,
            label: String::new(),
            content: AnyView::new(content),
        }
    }

    pub fn bordered(mut self, is_bordered: bool) -> Self {
        self.is_bordered = is_bordered;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
}

impl PrimitiveView for ToolbarGroup {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: self.label.clone(),
            ..Default::default()
        }));
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let size = node.composition_value(&ControlSizeEnvironment::key());
        let theme = theme_for_node(node);
        let vertical =
            node.composition_value(&ToolbarAxisEnvironment::axis()) == ToolbarAxis::Vertical;
        vec![AnyView::new(
            BoxView::new(
                if vertical {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                2.0,
                self.content.clone(),
            )
            .padding_edges(
                if self.is_bordered && !vertical {
                    2.0
                } else {
                    0.0
                },
                if self.is_bordered && vertical {
                    2.0
                } else {
                    0.0
                },
            )
            .background_color(if self.is_bordered {
                theme.colors.surface_variant
            } else {
                cove_core::Color::CLEAR
            })
            .corner_radius(if self.is_bordered {
                theme.radius.sm
            } else {
                0.0
            })
            .frame(
                None,
                None,
                if vertical {
                    Some(size.button_height())
                } else {
                    None
                },
                if vertical {
                    None
                } else {
                    Some(size.button_height())
                },
                None,
                None,
            ),
        )]
    }
}
crate::impl_primitive_view!(ToolbarGroup);

/// Thin rule between toolbar groups, drawn across the toolbar axis.
pub struct ToolbarSeparator;

impl PrimitiveView for ToolbarSeparator {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let vertical =
            node.composition_value(&ToolbarAxisEnvironment::axis()) == ToolbarAxis::Vertical;
        let divider = if vertical {
            AnyView::new(
                crate::primitives::text::Divider::new(
                    None,
                    1.0,
                    crate::primitives::text::DividerAxis::Horizontal,
                )
                .frame_size(18.0, 1.0)
                .padding_edges(0.0, theme.spacing.xs),
            )
        } else {
            AnyView::new(
                crate::primitives::text::Divider::new(
                    None,
                    1.0,
                    crate::primitives::text::DividerAxis::Vertical,
                )
                .frame_size(1.0, 18.0)
                .padding_edges(theme.spacing.xs, 0.0),
            )
        };
        vec![AnyView::new(divider.with_modifier(
            crate::modifiers::interaction::AccessibilityModifier::new(|semantics| {
                semantics.is_hidden = true
            }),
        ))]
    }
}
crate::impl_primitive_view!(ToolbarSeparator);
