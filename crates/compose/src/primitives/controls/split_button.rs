//! Translation of Primitives/Controls/SplitButton.swift.
//!
//! Primary action plus an adjacent menu trigger. The primary button runs the
//! default action; the arrow opens the same [`MenuEntry`] menu a `MenuButton`
//! uses. The pair shares one Tab stop, and Left/Right moves between the zones.
use crate::Scope;
use std::rc::Rc;

use cove_core::scene::{
    Align, EventPhase, EventResult, FlexDirection, FocusChainHolder, InputHandlerRoute,
    InteractionRegistryHolder, Justify, Node, Wrap,
};

use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::primitives::controls::button::Button;
use crate::primitives::controls::menu_button::MenuButton;
use crate::primitives::overlays::menu::MenuEntry;
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::theme::theme_for_node;
use crate::{AnyView, BoxView, PrimitiveView, View, impl_primitive_view};

/// Height floor for the pair.
const SPLIT_MIN_HEIGHT: f32 = 32.0;
/// Divider width between the two zones.
const DIVIDER_WIDTH: f32 = 1.0;
/// Menu trigger padding.
const MENU_PADDING_HORIZONTAL: f32 = 10.0;
const MENU_PADDING_VERTICAL: f32 = 7.0;

/// Primary action plus an adjacent menu trigger.
pub struct SplitButton {
    pub title: String,
    pub action: Rc<dyn Fn()>,
    pub entries: Vec<MenuEntry>,
    pub is_enabled: bool,
    pub menu_width: Option<f32>,
    /// Accessible name for the menu zone.
    pub menu_accessibility_label: String,
}

impl SplitButton {
    pub fn new(
        title: impl Into<String>,
        entries: Vec<MenuEntry>,
        action: impl Fn() + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            action: Rc::new(action),
            entries,
            is_enabled: true,
            menu_width: None,
            menu_accessibility_label: crate::i18n::tr("More actions"),
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn menu_width(mut self, menu_width: f32) -> Self {
        self.menu_width = Some(menu_width);
        self
    }

    pub fn menu_accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.menu_accessibility_label = label.into();
        self
    }
}

impl Clone for SplitButton {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            action: self.action.clone(),
            entries: self.entries.clone(),
            is_enabled: self.is_enabled,
            menu_width: self.menu_width,
            menu_accessibility_label: self.menu_accessibility_label.clone(),
        }
    }
}

impl View for SplitButton {
    fn body(&self, _cx: &Scope) -> AnyView {
        let action = self.action.clone();
        // The group draws the one capsule bezel; each zone only answers the
        // pointer, so the two halves read as a single macOS split button.
        let primary = Button::title(self.title.clone(), move || action())
            .enabled(self.is_enabled)
            .button_style(crate::theme::styles::buttons::GhostButtonStyle);

        let menu_label = AnyView::new(
            AnyView::new(crate::primitives::controls::button::ButtonIcon::new(
                crate::primitives::controls::button::ButtonIconSource::resource(
                    crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down(),
                ),
                Some(12.0),
                None,
            ))
            .padding_edges(MENU_PADDING_HORIZONTAL, MENU_PADDING_VERTICAL),
        );
        let menu = MenuButton::new(self.entries.clone(), menu_label)
            .ghost()
            .enabled(self.is_enabled)
            .accessibility_label(self.menu_accessibility_label.clone());
        let menu = match self.menu_width {
            Some(width) => menu.width(width),
            None => menu,
        };

        AnyView::new(SplitButtonGroup {
            is_enabled: self.is_enabled,
            primary: AnyView::new(AnyView::new(primary).flex_with(1.0, 1.0, Some(0.0))),
            menu: AnyView::new(menu),
        })
    }
}

/// The two zones plus the divider, with the shared Tab stop and Left/Right
/// movement between them.
struct SplitButtonGroup {
    is_enabled: bool,
    primary: AnyView,
    menu: AnyView,
}

impl PrimitiveView for SplitButtonGroup {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_min_height(Some(SPLIT_MIN_HEIGHT));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_min_height(Some(SPLIT_MIN_HEIGHT));
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        // A capsule push-button bezel around both zones.
        node.set_background_color(Some(theme.colors.surface));
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
        node.set_corner_radius(SPLIT_MIN_HEIGHT * 0.5);
        node.set_shadow(theme.elevation.low);
        node.set_layout_did_update(Some(Rc::new(|owner| {
            let zone = owner.attachment::<usize>("split.zone").unwrap_or(0);
            apply_tab_stops(owner, zone);
        })));
        if !self.is_enabled {
            return;
        }
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let weak = node.downgrade();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Bubble || !event.modifiers.is_empty() {
                    return EventResult::Ignored;
                }
                if event.scancode != crate::Scancode::ARROW_LEFT
                    && event.scancode != crate::Scancode::ARROW_RIGHT
                {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&weak) else {
                    return EventResult::Ignored;
                };
                let buttons = focusable_buttons(&owner);
                let Some(chain) = FocusChainHolder::current() else {
                    return EventResult::Ignored;
                };
                let Some(focused) = chain.focused() else {
                    return EventResult::Ignored;
                };
                let Some(index) = buttons.iter().position(|button| button == &focused) else {
                    return EventResult::Ignored;
                };
                let target = if event.scancode == crate::Scancode::ARROW_RIGHT {
                    buttons.get((index + 1).min(buttons.len().saturating_sub(1)))
                } else {
                    buttons.get(index.saturating_sub(1))
                };
                let Some(target) = target else {
                    return EventResult::Ignored;
                };
                let zone = if buttons.len() > 1 && target == &buttons[1] {
                    1usize
                } else {
                    0usize
                };
                owner.set_attachment("split.zone", zone);
                apply_tab_stops(&owner, zone);
                chain.focus(Some(target), true);
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(crate::TupleView::new(vec![
                self.primary.clone(),
                AnyView::new(SplitDivider),
                self.menu.clone(),
            ])),
        ))]
    }
}
impl_primitive_view!(SplitButtonGroup);

fn apply_tab_stops(node: &Node, zone: usize) {
    for (index, button) in focusable_buttons(node).into_iter().enumerate() {
        button.set_is_tab_stop(index == zone);
    }
}

/// The pressable buttons inside the pair, in visual order.
fn focusable_buttons(node: &Node) -> Vec<Node> {
    let mut found = Vec::new();
    fn walk(candidate: &Node, found: &mut Vec<Node>) {
        if candidate.is_focusable() {
            found.push(candidate.clone());
        }
        for child in candidate.children() {
            walk(&child, found);
        }
    }
    walk(node, &mut found);
    found
}

/// Shared chrome, not a third control.
struct SplitDivider;

impl PrimitiveView for SplitDivider {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(DIVIDER_WIDTH));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(DIVIDER_WIDTH));
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        node.set_background_color(Some(theme.colors.border));
    }
}
impl_primitive_view!(SplitDivider);
