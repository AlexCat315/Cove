//! Translation of Primitives/Overlays/Menu/ContextMenu.swift.
//!
//! Right-click (or Ctrl-click) presents a menu at the pointer. A native backend
//! shows the system menu and runs the chosen command; otherwise the self-drawn
//! panel is presented through the portal. Touch surfaces have no secondary
//! click, so a long press opens the same menu as a sheet.
use std::rc::Rc;

use cove_core::scene::{
    Align, EventPhase, EventResult, FlexDirection, InputHandlerRoute, InteractionRegistryHolder,
    LayoutNode, Node, PointerPhase,
};
use cove_core::{Point, Rect};
use cove_platform::MouseButton;

use crate::primitives::overlays::focus_scope::FocusScope;
use crate::primitives::overlays::menu::{Menu, MenuEntry};
use crate::primitives::overlays::portal::{
    PortalPlacementMode, PortalResource, PortalStoreHolder,
};
use crate::animation::transition::Transition;
use cove_core::scene::NodeResource;
use crate::{impl_primitive_view, AnyView, PrimitiveView, View};

const HOST_MARKER: &str = "context-menu-anchor";

/// Right-click menu attached to its content.
pub struct ContextMenu {
    pub width: f32,
    pub entries: Rc<dyn Fn() -> Vec<MenuEntry>>,
    pub on_open: Rc<dyn Fn()>,
    /// Whether a press at this window point should open the menu. A host that
    /// covers a whole document uses it to stay out of the way of presses that
    /// belong to the control underneath.
    pub is_interested: Rc<dyn Fn(Point) -> bool>,
    content: AnyView,
    point: cove_core::State<Option<Point>>,
}

impl ContextMenu {
    pub fn new(
        entries: impl Fn() -> Vec<MenuEntry> + 'static,
        content: impl View + 'static,
    ) -> Self {
        Self {
            width: 220.0,
            entries: Rc::new(entries),
            on_open: Rc::new(|| {}),
            is_interested: Rc::new(|_| true),
            content: AnyView::new(content),
            point: cove_core::State::new(None),
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = if width.is_finite() { width.max(80.0) } else { 220.0 };
        self
    }

    pub fn on_open(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_open = Rc::new(handler);
        self
    }

    pub fn is_interested(mut self, predicate: impl Fn(Point) -> bool + 'static) -> Self {
        self.is_interested = Rc::new(predicate);
        self
    }
}

impl View for ContextMenu {
    fn body(&self) -> AnyView {
        let point = self.point.get();
        let point_state = self.point.clone();
        let on_open = self.on_open.clone();
        let on_dismiss_state = self.point.clone();
        let entries = self.entries.clone();
        let interested = self.is_interested.clone();
        AnyView::new(ContextMenuHost {
            content: self.content.clone(),
            entries: if point.is_some() { entries() } else { Vec::new() },
            width: self.width,
            point,
            interested,
            on_open: Rc::new(move |position: Point| {
                on_open();
                point_state.set(Some(position));
            }),
            on_dismiss: Rc::new(move || on_dismiss_state.set(None)),
        })
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct ContextMenuHost {
    content: AnyView,
    entries: Vec<MenuEntry>,
    width: f32,
    point: Option<Point>,
    interested: Rc<dyn Fn(Point) -> bool>,
    on_open: Rc<dyn Fn(Point)>,
    on_dismiss: Rc<dyn Fn()>,
}

impl PrimitiveView for ContextMenuHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.add_resource(PortalResource::new());
        node.set_attachment(HOST_MARKER, true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        if let Some(registry) = InteractionRegistryHolder::current() {
            let interested = self.interested.clone();
            let on_open = self.on_open.clone();
            registry.set_pointer(
                node,
                InputHandlerRoute::new(
                    cove_core::scene::InputHandlerRole::Control,
                    cove_core::scene::InputRoutingPriority::CHROME,
                    "context-menu",
                ),
                Rc::new(move |event, phase, event_phase| {
                    let here = Point::new(event.x, event.y);
                    let secondary = event.button == MouseButton::Right
                        || (event.button == MouseButton::Left && event.modifiers.has_ctrl());
                    if !secondary || !interested(here) {
                        return EventResult::Ignored;
                    }
                    if phase == PointerPhase::Down && event_phase == EventPhase::Target {
                        on_open(here);
                    }
                    EventResult::Handled
                }),
            );
        }
        let resource = node.first_resource::<PortalResource>();
        let Some(resource) = resource else { return };
        let Some(point) = self.point else {
            resource.unmount(node);
            return;
        };
        let entries = self.entries.clone();
        let width = self.width;
        let dismiss = self.on_dismiss.clone();
        let content = AnyView::new(FocusScope::with_restoring_commands(
            true,
            AnyView::new(
                Menu::new(entries)
                    .width(width)
                    .max_visible_rows(12)
                    .on_item_activated({
                        let dismiss = dismiss.clone();
                        move || dismiss()
                    })
                    .on_dismiss(move || dismiss()),
            ),
        ));
        resource.present(
            Some(PortalStoreHolder::current()),
            point,
            Some(width),
            content,
            false,
            PortalPlacementMode::Fixed,
            None,
            Some(Transition::opacity()),
        );
        let dismiss_anchor = Rc::new(|| Rect::new(0.0, 0.0, 0.0, 0.0));
        resource.set_dismissal(dismiss_anchor, self.on_dismiss.clone());
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(ContextMenuHost);
