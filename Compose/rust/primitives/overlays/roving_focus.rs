//! Translation of Primitives/Overlays/RovingFocus.swift.
//!
//! Shared keyboard navigation policy for single-selection groups. Disabled
//! items are excluded by callers.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{
    EventPhase, EventResult, FocusChainHolder, InputHandlerRoute, InteractionRegistryHolder,
    LayoutNode, Node, NodeResource,
};
use cove_platform::KeyEvent;
use crate::{impl_primitive_view, AnyView, PrimitiveView, View};

const ROVING_ITEM: &str = "__roving.item";
const ROVING_TAB_STOP: &str = "__roving.tabStop";
const ROVING_FOCUS_REQUEST: &str = "__roving.focusRequest";

/// Navigation command shared by single-selection groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SelectionNavigation {
    Previous,
    Next,
    First,
    Last,
}

impl SelectionNavigation {
    pub fn command_for(scancode: u32) -> Option<Self> {
        match scancode {
            crate::Scancode::ARROW_UP | crate::Scancode::ARROW_LEFT => Some(Self::Previous),
            crate::Scancode::ARROW_DOWN | crate::Scancode::ARROW_RIGHT => Some(Self::Next),
            crate::Scancode::HOME => Some(Self::First),
            crate::Scancode::END => Some(Self::Last),
            _ => None,
        }
    }

    pub fn destination<Id: PartialEq + Clone>(self, items: &[Id], selection: Option<&Id>, wraps: bool) -> Option<Id> {
        if items.is_empty() {
            return None;
        }
        let index = selection.and_then(|selection| items.iter().position(|item| item == selection));
        match self {
            Self::First => items.first().cloned(),
            Self::Last => items.last().cloned(),
            Self::Next => match index {
                None => items.first().cloned(),
                Some(index) => items.get(if wraps {
                    (index + 1) % items.len()
                } else {
                    (index + 1).min(items.len() - 1)
                }).cloned(),
            },
            Self::Previous => match index {
                None => items.last().cloned(),
                Some(index) => items.get(if wraps {
                    (index + items.len() - 1) % items.len()
                } else {
                    index.saturating_sub(1)
                }).cloned(),
            },
        }
    }
}

/// A request can outlive the frame that selected an item hidden by layout.
#[derive(Clone, Debug, Default)]
pub struct RovingFocusRequest {
    pub target: Option<u64>,
    pub visible: bool,
}

impl RovingFocusRequest {
    pub fn new() -> Self {
        Self { target: None, visible: true }
    }
}

/// A keyboard boundary whose enabled items have one sequential Tab stop.
pub struct RovingFocusHost<Id: PartialEq + Clone + 'static> {
    pub enabled_items: Vec<Id>,
    pub selection: Rc<dyn Fn() -> Option<Id>>,
    pub on_select: Rc<dyn Fn(Id)>,
    pub wraps: bool,
    pub command: Rc<dyn Fn(&KeyEvent) -> Option<SelectionNavigation>>,
    pub focus_request: Option<Rc<RefCell<RovingFocusRequest>>>,
    content: AnyView,
    identity: Rc<dyn Fn(&Id) -> u64>,
}

impl<Id: PartialEq + Clone + 'static> RovingFocusHost<Id> {
    pub fn new(
        enabled_items: Vec<Id>,
        selection: impl Fn() -> Option<Id> + 'static,
        on_select: impl Fn(Id) + 'static,
        identity: impl Fn(&Id) -> u64 + 'static,
        content: impl View + 'static,
    ) -> Self {
        Self {
            enabled_items,
            selection: Rc::new(selection),
            on_select: Rc::new(on_select),
            wraps: true,
            command: Rc::new(|event| SelectionNavigation::command_for(event.scancode)),
            focus_request: None,
            content: AnyView::new(content),
            identity: Rc::new(identity),
        }
    }

    pub fn wraps(mut self, wraps: bool) -> Self {
        self.wraps = wraps;
        self
    }

    pub fn command(mut self, command: impl Fn(&KeyEvent) -> Option<SelectionNavigation> + 'static) -> Self {
        self.command = Rc::new(command);
        self
    }

    pub fn focus_request(mut self, request: Rc<RefCell<RovingFocusRequest>>) -> Self {
        self.focus_request = Some(request);
        self
    }
}

fn locate(node: &Node, token: u64) -> Option<Node> {
    let mut pending = vec![node.clone()];
    while let Some(item) = pending.pop() {
        if item.attachment::<u64>(ROVING_ITEM) == Some(token) && item.is_focusable() {
            return Some(item);
        }
        pending.extend(item.children().into_iter().rev());
    }
    None
}

fn update_tab_stops(node: &Node, target: Option<u64>) {
    if let Some(id) = node.attachment::<u64>(ROVING_ITEM) {
        node.set_is_tab_stop(target == Some(id));
    }
    for child in node.children() {
        update_tab_stops(&child, target);
    }
}

impl<Id: PartialEq + Clone + 'static> PrimitiveView for RovingFocusHost<Id> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let request = self
            .focus_request
            .clone()
            .or_else(|| node.attachment::<Rc<RefCell<RovingFocusRequest>>>(ROVING_FOCUS_REQUEST))
            .unwrap_or_else(|| Rc::new(RefCell::new(RovingFocusRequest::new())));
        node.set_attachment(ROVING_FOCUS_REQUEST, request.clone());

        let enabled_tokens: Vec<u64> = self.enabled_items.iter().map(|item| (self.identity)(item)).collect();
        {
            let mut state = request.borrow_mut();
            if let Some(target) = state.target
                && !enabled_tokens.contains(&target)
            {
                state.target = None;
            }
        }

        let selection = self.selection.clone();
        let identity = self.identity.clone();
        let enabled = self.enabled_items.clone();
        let tab_stop_fn: Rc<dyn Fn() -> Option<u64>> = Rc::new(move || {
            let selected = selection().map(|item| identity(&item));
            selected
                .filter(|token| enabled.iter().any(|item| identity(item) == *token))
                .or_else(|| enabled.first().map(|item| identity(item)))
        });
        node.set_attachment(ROVING_TAB_STOP, tab_stop_fn.clone());

        let weak = node.downgrade();
        let resolve_identity = self.identity.clone();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let Some(owner_node) = Node::upgrade(&weak) else { return };
            let Some(request) = owner_node.attachment::<Rc<RefCell<RovingFocusRequest>>>(ROVING_FOCUS_REQUEST) else {
                return;
            };
            let state = request.borrow().clone();
            let Some(target) = state.target else { return };
            let Some(item) = locate(owner, target) else { return };
            if !item.accepts_subtree_input() {
                return;
            }
            if let Some(chain) = FocusChainHolder::current() {
                chain.focus(Some(item.clone()), state.visible);
            }
            crate::primitives::layout::ScrollReveal::reveal(&item, false);
            request.borrow_mut().target = None;
            let _ = &resolve_identity;
        }) as cove_core::scene::LayoutUpdateHook));

        let Some(registry) = InteractionRegistryHolder::current() else { return };
        let key_request = request.clone();
        let key_selection = self.selection.clone();
        let key_enabled = self.enabled_items.clone();
        let key_identity = self.identity.clone();
        let key_on_select = self.on_select.clone();
        let key_command = self.command.clone();
        let key_wraps = self.wraps;
        let key_node = node.downgrade();
        registry.set_key(node, InputHandlerRoute::control(), Rc::new(move |event: &KeyEvent, phase| {
            if phase != EventPhase::Bubble {
                return EventResult::Ignored;
            }
            let Some(command) = (key_command)(event) else { return EventResult::Ignored };
            let Some(owner) = Node::upgrade(&key_node) else { return EventResult::Ignored };
            let mut source = key_request.borrow().target;
            if let Some(focused) = FocusChainHolder::current().and_then(|chain| chain.focused()) {
                let mut candidate = Some(focused);
                let mut focused_item = None;
                while let Some(current) = candidate {
                    if current == owner {
                        break;
                    }
                    if let Some(id) = current.attachment::<u64>(ROVING_ITEM) {
                        focused_item = Some(id);
                        break;
                    }
                    candidate = current.parent();
                }
                // Text fields and other controls inside a panel keep their
                // unhandled arrow keys; only group headers participate here.
                let Some(focused_item) = focused_item else { return EventResult::Ignored };
                let focused_in_enabled = key_enabled.iter().any(|item| (key_identity)(item) == focused_item);
                if !focused_in_enabled {
                    return EventResult::Ignored;
                }
                source = key_request.borrow().target.or(Some(focused_item));
            }
            let source_item = source.and_then(|token| {
                key_enabled.iter().find(|item| (key_identity)(item) == token).cloned()
            });
            let Some(target) = command.destination(&key_enabled, source_item.as_ref(), key_wraps) else {
                return EventResult::Ignored;
            };
            (key_on_select)(target.clone());
            let target_token = (key_identity)(&target);
            {
                let mut state = key_request.borrow_mut();
                state.target = Some(target_token);
                state.visible = true;
            }
            update_tab_stops(&owner, Some(target_token));
            let request = key_request.clone();
            if let Some(item) = locate(&owner, target_token)
                && item.accepts_subtree_input()
                && let Some(chain) = FocusChainHolder::current()
            {
                chain.focus(Some(item.clone()), true);
                crate::primitives::layout::ScrollReveal::reveal(&item, false);
                request.borrow_mut().target = None;
            }
            EventResult::Handled
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl<Id: PartialEq + Clone + 'static> View for RovingFocusHost<Id> {}

/// Applies the roving Tab-stop policy to one item.
pub struct RovingItem<'a, Id: PartialEq + Clone + 'static> {
    pub id: &'a Id,
    pub is_tab_stop: bool,
    pub identity: Rc<dyn Fn(&Id) -> u64>,
}

impl<'a, Id: PartialEq + Clone + 'static> RovingItem<'a, Id> {
    pub fn new(id: &'a Id, is_tab_stop: bool, identity: Rc<dyn Fn(&Id) -> u64>) -> Self {
        Self { id, is_tab_stop, identity }
    }
}

impl<'a, Id: PartialEq + Clone + 'static> cove_composition::ViewModifier for RovingItem<'a, Id> {
    fn apply_node(&self, node: &Node) {
        let token = (self.identity)(self.id);
        node.set_attachment(ROVING_ITEM, token);
        // A caller modifier can replay after the button updates locally.
        // Resolve the current group selection instead of restoring its old
        // snapshot.
        let mut ancestor = node.parent();
        while let Some(parent) = ancestor {
            if let Some(current) = parent.attachment::<Rc<dyn Fn() -> Option<u64>>>(ROVING_TAB_STOP) {
                node.set_is_tab_stop(current() == Some(token));
                return;
            }
            ancestor = parent.parent();
        }
        node.set_is_tab_stop(self.is_tab_stop);
    }
}

/// Convenience: hashes any hashable identity to the token used by the group.
pub fn roving_identity<T: std::hash::Hash>(value: &T) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Node resource marker so a roving group can clear the focus request on unmount.
pub struct RovingFocusLifetime;

impl NodeResource for RovingFocusLifetime {
    fn mount(&self, _node: &Node) {}

    fn unmount(&self, node: &Node) {
        node.remove_attachment(ROVING_FOCUS_REQUEST);
        node.remove_attachment(ROVING_TAB_STOP);
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
