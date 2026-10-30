//! Menu level host: key, text and pointer routing plus the panel chrome.
//!
//! The host is a layout-less pass-through; the panel child carries the measured
//! frame, and the level reports it so sibling levels can anchor submenus.
use std::rc::Rc;

use cove_core::scene::{
    Align, EventResult, FlexDirection, InputHandlerRoute, InteractionRegistryHolder, Justify, Node,
    Wrap,
};
use cove_core::Point;
use crate::modifiers::layout::PaddingExt;
use crate::modifiers::style::{FloatingSurfaceElevation, SurfaceStyleExt};
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, View};

use super::menu_navigation::{MenuSession, MENU_LEVEL_MARKER, MENU_SCROLL_INSET, MENU_SCROLL_MARKER};

/// The panel's content gutter: rows sit inside it, so their highlight bands keep
/// an even edge on every side. Revealing a row preserves it.
pub const PANEL_INSET: f32 = 4.0;

pub struct MenuLevelHost {
    pub session: Rc<MenuSession>,
    pub width: Option<f32>,
    pub max_visible_rows: usize,
    pub rows: AnyView,
    pub on_key: Rc<dyn Fn(&cove_platform::KeyEvent, &Node) -> bool>,
    pub on_text: Rc<dyn Fn(&str, &Node) -> bool>,
}

impl PrimitiveView for MenuLevelHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let hover: Rc<dyn cove_core::scene::NodeResource> = self.session.hover_intent.clone();
        node.add_resource(hover);
        node.set_attachment(MENU_LEVEL_MARKER, true);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else { return };
        let on_key = self.on_key.clone();
        let on_text = self.on_text.clone();
        let key_node = node.downgrade();
        registry.set_key(node, InputHandlerRoute::control(), Rc::new(move |event, phase| {
            if phase == cove_core::scene::EventPhase::Capture {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&key_node) else { return EventResult::Ignored };
            if on_key(event, &owner) { EventResult::Handled } else { EventResult::Ignored }
        }));
        let text_node = node.downgrade();
        registry.set_text(node, InputHandlerRoute::control(), Rc::new(move |text: &str, phase| {
            if phase == cove_core::scene::EventPhase::Capture {
                return EventResult::Ignored;
            }
            let Some(owner) = Node::upgrade(&text_node) else { return EventResult::Ignored };
            if on_text(text, &owner) { EventResult::Handled } else { EventResult::Ignored }
        }));
        let pointer_session = self.session.clone();
        registry.set_motion(node, InputHandlerRoute::control(), Rc::new(move |event, phase| {
            if phase == cove_core::scene::EventPhase::Capture {
                pointer_session.pointer.borrow_mut().record(Point::new(event.x, event.y));
            }
            EventResult::Ignored
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        // Overlay, not `stable`: a reserved lane is a right-only inset, so every
        // row highlight would sit closer to one edge. Menus fill their panel
        // edge to edge and only need the bar while the pointer is over them.
        let scroller = crate::primitives::layout::ScrollView::vertical(
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 1.0,
                    self.rows.clone(),
                )
                .padding(PANEL_INSET),
            ),
        )
        .scrollbar_gutter(crate::primitives::layout::ScrollbarGutter::Overlay)
        .consume_policy(cove_core::scene::ScrollConsumePolicy::Always);
        let panel = AnyView::new(scroller)
            .modifier(MenuScrollMarker { inset: PANEL_INSET })
            .modifier(MenuWindowBounds { max_visible_rows: self.max_visible_rows });
        let panel = match self.width {
            Some(width) => AnyView::new(panel.frame_width(width.max(80.0))),
            None => panel,
        };
        // The dropdown shares the chrome's shadow, but its corners match the
        // tighter popup radius rather than the wider panel tier, so it reads as
        // a menu, not a floating window.
        vec![AnyView::new(panel.floating_surface_with_elevation(
            crate::theme::ThemeMetric::group_radius(),
            FloatingSurfaceElevation::Panel,
        ))]
    }
}
impl_primitive_view!(MenuLevelHost);

/// Marks the scroll viewport so revealing a row keeps the panel gutter.
struct MenuScrollMarker {
    inset: f32,
}

impl cove_composition::ViewModifier for MenuScrollMarker {
    fn apply_node(&self, node: &Node) {
        node.set_attachment(MENU_SCROLL_MARKER, true);
        node.set_attachment(MENU_SCROLL_INSET, self.inset);
    }
}

/// Clamps the panel to the visible row budget and the window bounds.
struct MenuWindowBounds {
    max_visible_rows: usize,
}

impl cove_composition::ViewModifier for MenuWindowBounds {
    fn apply_node(&self, node: &Node) {
        let Some(layout) = node.layout_node() else { return };
        let row_height = 30.0_f32.max(
            node.composition_value(&crate::theme::ControlSizeEnvironment::key()).button_height(),
        );
        let preferred = self.max_visible_rows as f32 * row_height;
        let weak = node.downgrade();
        let max_visible_rows = self.max_visible_rows;
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let Some(node) = Node::upgrade(&weak) else { return };
            let Some(layout) = node.layout_node() else { return };
            let bounds = super::super::portal::portal_window_bounds(&node);
            let height = preferred.min((bounds.height - 12.0).max(0.0));
            if layout.max_height() != Some(height) {
                layout.set_max_height(Some(height));
            }
            let _ = (owner, max_visible_rows);
        }) as cove_core::scene::LayoutUpdateHook));
        layout.set_max_height(Some(preferred));
    }
}
