//! Translation of Primitives/Overlays/ShortcutHost.swift.
//!
//! Invisible host for application-level keyboard shortcuts.
//!
//! The dispatcher delivers normal focused key handlers first, then falls back to
//! handlers registered with the shortcut route, so text fields and focused
//! controls keep priority while app-level commands still work without panel
//! focus.
use std::rc::Rc;

use cove_core::scene::{
    EventResult, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node,
};
use cove_platform::KeyEvent;

use crate::{impl_primitive_view, AnyView, PrimitiveView};

type KeyHandler = Rc<dyn Fn(&KeyEvent, cove_core::scene::EventPhase) -> EventResult>;

/// Invisible host for application-level keyboard shortcuts.
pub struct ShortcutHost {
    pub route: InputHandlerRoute,
    on_key_down: KeyHandler,
    on_key_up: Option<KeyHandler>,
}

impl ShortcutHost {
    /// `true` from the handler means the shortcut was handled.
    pub fn new(on_key_down: impl Fn(&KeyEvent) -> bool + 'static) -> Self {
        Self {
            route: InputHandlerRoute::shortcut(),
            on_key_down: Rc::new(move |event, _| {
                if on_key_down(event) { EventResult::Handled } else { EventResult::Ignored }
            }),
            on_key_up: None,
        }
    }

    pub fn with_results(
        on_key_down: impl Fn(&KeyEvent, cove_core::scene::EventPhase) -> EventResult + 'static,
    ) -> Self {
        Self {
            route: InputHandlerRoute::shortcut(),
            on_key_down: Rc::new(on_key_down),
            on_key_up: None,
        }
    }

    pub fn route(mut self, route: InputHandlerRoute) -> Self {
        self.route = route;
        self
    }

    pub fn on_key_up(
        mut self,
        handler: impl Fn(&KeyEvent, cove_core::scene::EventPhase) -> EventResult + 'static,
    ) -> Self {
        self.on_key_up = Some(Rc::new(handler));
        self
    }
}

impl PrimitiveView for ShortcutHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_is_focusable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(0.0));
        layout.set_height(Some(0.0));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else { return };
        registry.remove(node);
        registry.set_key(node, self.route.clone(), self.on_key_down.clone());
        if let Some(on_key_up) = self.on_key_up.clone() {
            registry.set_key_up(node, self.route.clone(), on_key_up);
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(ShortcutHost);
