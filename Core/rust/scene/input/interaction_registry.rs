//! Translation of CoveUIScene/Input/InteractionRegistry.swift.
use crate::scene::Node;
use crate::scene::NodeResource;
use cove_platform::{
    KeyEvent, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent, TextEditingEvent,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::{Rc, Weak};

/// Phase of event delivery in the capture/target/bubble model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

/// Result of a handler — `Handled` stops bubble (and capture if returned in capture phase).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
    Handled,
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPhase {
    Down,
    Up,
}

/// Boundary transition derived from mouse-motion hit-test changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoverPhase {
    Enter,
    Leave,
}

/// Semantic role attached to an input handler.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InputHandlerRole {
    Control,
    Scroll,
    ScrollChrome,
    TextInput,
    Viewport,
    Workspace,
    Drag,
    Overlay,
    Shortcut,
    Custom(String),
}

/// Coarse priority band used by the dispatcher before falling back to tree order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InputRoutingPriority(pub i32);
impl InputRoutingPriority {
    pub const BACKGROUND: Self = Self(0);
    pub const NORMAL: Self = Self(100);
    pub const FOCUSED: Self = Self(200);
    pub const CHROME: Self = Self(300);
    pub const CAPTURE: Self = Self(400);
    pub const MODAL: Self = Self(500);
    pub const SYSTEM: Self = Self(600);
}

/// Metadata for a node handler in the input routing table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputHandlerRoute {
    pub role: InputHandlerRole,
    pub priority: InputRoutingPriority,
    pub debug_name: String,
}
impl InputHandlerRoute {
    pub fn new(
        role: InputHandlerRole,
        priority: InputRoutingPriority,
        debug_name: impl Into<String>,
    ) -> Self {
        Self {
            role,
            priority,
            debug_name: debug_name.into(),
        }
    }
    pub fn control() -> Self {
        Self::new(
            InputHandlerRole::Control,
            InputRoutingPriority::NORMAL,
            "control",
        )
    }
    pub fn scroll() -> Self {
        Self::new(
            InputHandlerRole::Scroll,
            InputRoutingPriority::NORMAL,
            "scroll",
        )
    }
    pub fn scroll_chrome() -> Self {
        Self::new(
            InputHandlerRole::ScrollChrome,
            InputRoutingPriority::CHROME,
            "scroll.chrome",
        )
    }
    pub fn text_input() -> Self {
        Self::new(
            InputHandlerRole::TextInput,
            InputRoutingPriority::FOCUSED,
            "text.input",
        )
    }
    pub fn viewport() -> Self {
        Self::new(
            InputHandlerRole::Viewport,
            InputRoutingPriority::FOCUSED,
            "viewport",
        )
    }
    pub fn workspace_drag() -> Self {
        Self::new(
            InputHandlerRole::Workspace,
            InputRoutingPriority::CAPTURE,
            "workspace.drag",
        )
    }
    pub fn overlay() -> Self {
        Self::new(
            InputHandlerRole::Overlay,
            InputRoutingPriority::MODAL,
            "overlay",
        )
    }
    pub fn shortcut() -> Self {
        Self::new(
            InputHandlerRole::Shortcut,
            InputRoutingPriority::SYSTEM,
            "shortcut",
        )
    }
}
impl Default for InputHandlerRoute {
    fn default() -> Self {
        Self::control()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputDispatchKind {
    PointerDown,
    PointerUp,
    Motion,
    Wheel,
    KeyDown,
    KeyUp,
    Editing,
    Text,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputDispatchTrace {
    pub kind: InputDispatchKind,
    pub node_id: crate::ElementId,
    pub phase: EventPhase,
    pub route: Option<InputHandlerRoute>,
    pub result: EventResult,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum InputHandlerSlot {
    Pointer,
    Motion,
    Wheel,
    Key,
    KeyUp,
    Editing,
    Text,
}

#[derive(Clone, Debug)]
pub(crate) struct RoutedInputNode {
    pub node: Node,
    pub route: InputHandlerRoute,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct RouteIndexKey {
    slot: InputHandlerSlot,
    role: InputHandlerRole,
}

pub type PointerHandler = Rc<dyn Fn(&MouseButtonEvent, PointerPhase, EventPhase) -> EventResult>;
pub type HoverHandler = Rc<dyn Fn(HoverPhase)>;
pub type MotionHandler = Rc<dyn Fn(&MouseMotionEvent, EventPhase) -> EventResult>;
pub type WheelHandler = Rc<dyn Fn(&MouseWheelEvent, EventPhase) -> EventResult>;
pub type KeyHandler = Rc<dyn Fn(&KeyEvent, EventPhase) -> EventResult>;
pub type EditingHandler = Rc<dyn Fn(&TextEditingEvent, EventPhase) -> EventResult>;
pub type TextHandler = Rc<dyn Fn(&str, EventPhase) -> EventResult>;

/// Per-node handler closures registered by Compose-layer modifiers.
#[derive(Clone, Default)]
pub struct Handlers {
    pub pointer: Option<PointerHandler>,
    pub hover: Option<HoverHandler>,
    pub motion: Option<MotionHandler>,
    pub wheel: Option<WheelHandler>,
    pub key: Option<KeyHandler>,
    pub key_up: Option<KeyHandler>,
    pub editing: Option<EditingHandler>,
    pub text: Option<TextHandler>,
    pub pointer_route: Option<InputHandlerRoute>,
    pub motion_route: Option<InputHandlerRoute>,
    pub wheel_route: Option<InputHandlerRoute>,
    pub key_route: Option<InputHandlerRoute>,
    pub key_up_route: Option<InputHandlerRoute>,
    pub editing_route: Option<InputHandlerRoute>,
    pub text_route: Option<InputHandlerRoute>,
}
impl Handlers {
    pub fn is_empty(&self) -> bool {
        self.pointer.is_none()
            && self.hover.is_none()
            && self.motion.is_none()
            && self.wheel.is_none()
            && self.key.is_none()
            && self.key_up.is_none()
            && self.editing.is_none()
            && self.text.is_none()
    }
    fn route(&self, slot: InputHandlerSlot) -> Option<InputHandlerRoute> {
        match slot {
            InputHandlerSlot::Pointer => self.pointer_route.clone(),
            InputHandlerSlot::Motion => self.motion_route.clone(),
            InputHandlerSlot::Wheel => self.wheel_route.clone(),
            InputHandlerSlot::Key => self.key_route.clone(),
            InputHandlerSlot::KeyUp => self.key_up_route.clone(),
            InputHandlerSlot::Editing => self.editing_route.clone(),
            InputHandlerSlot::Text => self.text_route.clone(),
        }
    }
}

#[derive(Default)]
struct RegistryInner {
    table: RefCell<HashMap<Node, Handlers>>,
    routed_index: RefCell<HashMap<RouteIndexKey, HashSet<Node>>>,
}
impl RegistryInner {
    fn update_route_index(
        &self,
        node: &Node,
        slot: InputHandlerSlot,
        old_route: Option<InputHandlerRoute>,
        new_route: Option<InputHandlerRoute>,
    ) {
        let mut index = self.routed_index.borrow_mut();
        if let Some(old) = old_route {
            let key = RouteIndexKey {
                slot,
                role: old.role,
            };
            if let Some(nodes) = index.get_mut(&key) {
                nodes.remove(node);
                if nodes.is_empty() {
                    index.remove(&key);
                }
            }
        }
        if let Some(new) = new_route {
            index
                .entry(RouteIndexKey {
                    slot,
                    role: new.role,
                })
                .or_default()
                .insert(node.clone());
        }
    }
    fn remove(&self, node: &Node) {
        self.table.borrow_mut().remove(node);
        let mut index = self.routed_index.borrow_mut();
        index.retain(|_, nodes| {
            nodes.remove(node);
            !nodes.is_empty()
        });
    }
}

/// Per-window side table of input handlers keyed by node identity.
#[derive(Default)]
pub struct InteractionRegistry {
    inner: Rc<RegistryInner>,
}
impl InteractionRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn handlers(&self, node: &Node) -> Handlers {
        self.inner
            .table
            .borrow()
            .get(node)
            .cloned()
            .unwrap_or_default()
    }
    fn update(&self, node: &Node, change: impl FnOnce(&mut Handlers)) {
        let mut table = self.inner.table.borrow_mut();
        let entry = table.entry(node.clone()).or_default();
        change(entry);
    }
    fn bind_cleanup(&self, node: &Node) {
        if node
            .first_resource::<InteractionCleanupResource>()
            .is_none()
        {
            node.add_resource(Rc::new(InteractionCleanupResource {
                registry: Rc::downgrade(&self.inner),
            }));
        }
    }
    pub fn set_pointer(&self, node: &Node, route: InputHandlerRoute, handler: PointerHandler) {
        let old = self.handlers(node).pointer_route;
        self.update(node, |entry| {
            entry.pointer = Some(handler);
            entry.pointer_route = Some(route.clone());
        });
        self.inner
            .update_route_index(node, InputHandlerSlot::Pointer, old, Some(route));
        self.bind_cleanup(node);
    }
    pub fn set_hover(&self, node: &Node, handler: HoverHandler) {
        self.update(node, |entry| entry.hover = Some(handler));
        self.bind_cleanup(node);
    }
    pub fn set_motion(&self, node: &Node, route: InputHandlerRoute, handler: MotionHandler) {
        let old = self.handlers(node).motion_route;
        self.update(node, |entry| {
            entry.motion = Some(handler);
            entry.motion_route = Some(route.clone());
        });
        self.inner
            .update_route_index(node, InputHandlerSlot::Motion, old, Some(route));
        self.bind_cleanup(node);
    }
    pub fn set_wheel(&self, node: &Node, route: InputHandlerRoute, handler: WheelHandler) {
        let old = self.handlers(node).wheel_route;
        self.update(node, |entry| {
            entry.wheel = Some(handler);
            entry.wheel_route = Some(route.clone());
        });
        self.inner
            .update_route_index(node, InputHandlerSlot::Wheel, old, Some(route));
        self.bind_cleanup(node);
    }
    pub fn set_key(&self, node: &Node, route: InputHandlerRoute, handler: KeyHandler) {
        let old = self.handlers(node).key_route;
        self.update(node, |entry| {
            entry.key = Some(handler);
            entry.key_route = Some(route.clone());
        });
        self.inner
            .update_route_index(node, InputHandlerSlot::Key, old, Some(route));
        self.bind_cleanup(node);
    }
    pub fn set_key_up(&self, node: &Node, route: InputHandlerRoute, handler: KeyHandler) {
        let old = self.handlers(node).key_up_route;
        self.update(node, |entry| {
            entry.key_up = Some(handler);
            entry.key_up_route = Some(route.clone());
        });
        self.inner
            .update_route_index(node, InputHandlerSlot::KeyUp, old, Some(route));
        self.bind_cleanup(node);
    }
    pub fn set_editing(&self, node: &Node, route: InputHandlerRoute, handler: EditingHandler) {
        let old = self.handlers(node).editing_route;
        self.update(node, |entry| {
            entry.editing = Some(handler);
            entry.editing_route = Some(route.clone());
        });
        self.inner
            .update_route_index(node, InputHandlerSlot::Editing, old, Some(route));
        self.bind_cleanup(node);
    }
    pub fn set_text(&self, node: &Node, route: InputHandlerRoute, handler: TextHandler) {
        let old = self.handlers(node).text_route;
        self.update(node, |entry| {
            entry.text = Some(handler);
            entry.text_route = Some(route.clone());
        });
        self.inner
            .update_route_index(node, InputHandlerSlot::Text, old, Some(route));
        self.bind_cleanup(node);
    }
    pub fn remove(&self, node: &Node) {
        self.inner.remove(node);
    }
    pub fn count(&self) -> usize {
        self.inner.table.borrow().len()
    }
    pub(crate) fn routed_nodes(
        &self,
        slot: InputHandlerSlot,
        role: &InputHandlerRole,
        min_priority: InputRoutingPriority,
    ) -> Vec<RoutedInputNode> {
        let index = self.inner.routed_index.borrow();
        let Some(nodes) = index.get(&RouteIndexKey {
            slot,
            role: role.clone(),
        }) else {
            return Vec::new();
        };
        let table = self.inner.table.borrow();
        nodes
            .iter()
            .filter_map(|node| {
                let handlers = table.get(node)?;
                let route = handlers.route(slot)?;
                (route.role == *role && route.priority >= min_priority).then(|| RoutedInputNode {
                    node: node.clone(),
                    route,
                })
            })
            .collect()
    }
}

/// Node-owned cleanup for interaction handlers.
pub struct InteractionCleanupResource {
    registry: Weak<RegistryInner>,
}
impl NodeResource for InteractionCleanupResource {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, node: &Node) {
        if let Some(registry) = self.registry.upgrade() {
            registry.remove(node);
        }
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn handlers_are_released_with_the_node() {
        let registry = InteractionRegistry::new();
        let parent = Node::new();
        let node = Node::new();
        parent.add_child(&node);
        registry.set_pointer(
            &node,
            InputHandlerRoute::control(),
            Rc::new(|_, _, _| EventResult::Handled),
        );
        assert_eq!(registry.count(), 1);
        assert!(registry.handlers(&node).pointer.is_some());
        parent.remove_child(&node);
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn routed_lookup_filters_by_priority() {
        let registry = InteractionRegistry::new();
        let node = Node::new();
        let seen = Rc::new(Cell::new(0));
        let counter = seen.clone();
        registry.set_key(
            &node,
            InputHandlerRoute::overlay(),
            Rc::new(move |_, _| {
                counter.set(counter.get() + 1);
                EventResult::Handled
            }),
        );
        assert_eq!(
            registry
                .routed_nodes(
                    InputHandlerSlot::Key,
                    &InputHandlerRole::Overlay,
                    InputRoutingPriority::MODAL
                )
                .len(),
            1
        );
        assert!(
            registry
                .routed_nodes(
                    InputHandlerSlot::Key,
                    &InputHandlerRole::Overlay,
                    InputRoutingPriority::SYSTEM
                )
                .is_empty()
        );
    }
}
