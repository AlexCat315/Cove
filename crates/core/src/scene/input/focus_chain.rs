//! Translation of CoveUIScene/Input/FocusChain.swift.
use crate::scene::{
    Node, ObservableStateRegistrar, StateAccessKey, TextEditActions, TextEditCommand,
    TextInputAttachmentKey,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Scope {
    root: Option<Node>,
    previous: Option<Node>,
    previous_tree_root: Option<Node>,
    visible: bool,
    restores_commands: bool,
}
impl Scope {
    fn new(root: &Node, previous: Option<Node>, visible: bool, restores_commands: bool) -> Self {
        Self {
            root: Some(root.clone()),
            previous_tree_root: previous.as_ref().map(tree_root),
            previous,
            visible,
            restores_commands,
        }
    }
}

struct Accessory {
    root: Option<Node>,
    owner: Option<Node>,
    previous: Option<Node>,
    previous_tree_root: Option<Node>,
    visible: bool,
}
impl Accessory {
    fn new(root: &Node, owner: &Node, previous: Option<Node>, visible: bool) -> Self {
        Self {
            root: Some(root.clone()),
            owner: Some(owner.clone()),
            previous_tree_root: previous.as_ref().map(tree_root),
            previous,
            visible,
        }
    }
}

fn tree_root(node: &Node) -> Node {
    let mut root = node.clone();
    while let Some(parent) = root.parent() {
        root = parent;
    }
    root
}

fn is_descendant(node: &Node, root: &Node) -> bool {
    let mut cursor = Some(node.clone());
    while let Some(current) = cursor {
        if &current == root {
            return true;
        }
        cursor = current.parent();
    }
    false
}

/// Tracks the currently focused node and provides tab-order traversal.
///
/// Focus chain order = depth-first, in tree order, including only nodes with
/// `is_focusable == true`. Recomputed lazily by walking the live `Node` tree.
#[derive(Default)]
pub struct FocusChain {
    focused: RefCell<Option<Node>>,
    is_focus_visible: Cell<bool>,
    scopes: RefCell<Vec<Scope>>,
    accessories: RefCell<Vec<Accessory>>,
    registrar: ObservableStateRegistrar,
}
impl FocusChain {
    pub fn new() -> Self {
        Self {
            focused: RefCell::new(None),
            is_focus_visible: Cell::new(true),
            scopes: RefCell::new(Vec::new()),
            accessories: RefCell::new(Vec::new()),
            registrar: ObservableStateRegistrar::new(),
        }
    }
    pub fn focused(&self) -> Option<Node> {
        self.focused.borrow().clone()
    }
    pub fn is_focus_visible(&self) -> bool {
        self.is_focus_visible.get()
    }
    pub fn active_scope_root(&self) -> Option<Node> {
        self.scopes
            .borrow()
            .last()
            .and_then(|scope| scope.root.clone())
    }
    pub fn has_modal_scope(&self) -> bool {
        self.scopes
            .borrow()
            .iter()
            .any(|scope| !scope.restores_commands && scope.root.is_some())
    }
    pub fn modal_root(&self) -> Option<Node> {
        self.scopes
            .borrow()
            .iter()
            .rev()
            .find(|scope| !scope.restores_commands)
            .and_then(|scope| scope.root.clone())
    }
    pub fn current_focus(&self) -> Option<Node> {
        self.registrar.access(StateAccessKey::text("focus"));
        self.focused()
    }
    /// Menus keep text editing commands directed at the control they cover.
    pub fn command_target(&self) -> Option<Node> {
        self.registrar.access(StateAccessKey::text("focus"));
        let mut target = self.focused();
        for scope in self.scopes.borrow().iter().rev() {
            if !scope.restores_commands {
                break;
            }
            target = scope.previous.clone();
        }
        target
    }
    pub fn permits_input(&self, node: &Node) -> bool {
        for scope in self.scopes.borrow().iter().rev() {
            if !scope.restores_commands {
                break;
            }
            if let Some(root) = &scope.root
                && self.belongs_to_scope(node, root, &mut Vec::new())
            {
                return true;
            }
        }
        let modal = self
            .scopes
            .borrow()
            .iter()
            .rev()
            .find(|scope| !scope.restores_commands)
            .and_then(|scope| scope.root.clone());
        modal.is_none_or(|root| self.belongs_to_scope(node, &root, &mut Vec::new()))
    }
    pub fn permits_keyboard_input(&self, node: &Node) -> bool {
        self.active_scope_root()
            .is_none_or(|root| self.belongs_to_scope(node, &root, &mut Vec::new()))
    }
    /// A detached interactive portal belongs to its trigger's scope without
    /// taking keyboard focus merely because the pointer revealed it.
    pub fn attach_accessory(&self, root: &Node, owner: &Node) {
        let mut accessories = self.accessories.borrow_mut();
        accessories.retain(|accessory| accessory.root.is_some() && accessory.owner.is_some());
        if accessories
            .iter()
            .any(|accessory| accessory.root.as_ref() == Some(root))
        {
            return;
        }
        accessories.push(Accessory::new(
            root,
            owner,
            self.focused(),
            self.is_focus_visible.get(),
        ));
    }
    pub fn detach_accessory(&self, root: &Node) {
        let removed = {
            let mut accessories = self.accessories.borrow_mut();
            accessories
                .iter()
                .position(|accessory| accessory.root.as_ref() == Some(root))
                .map(|index| accessories.remove(index))
        };
        let Some(accessory) = removed else { return };
        let focused = self.focused();
        if focused.is_none()
            || focused
                .as_ref()
                .is_some_and(|focused| is_descendant(focused, root))
        {
            let restorable = accessory
                .previous
                .as_ref()
                .zip(accessory.previous_tree_root.as_ref())
                .is_some_and(|(previous, tree_root)| {
                    previous.parent().is_some()
                        && is_descendant(previous, tree_root)
                        && previous.is_focusable()
                        && previous.accepts_subtree_input()
                        && self.permits_input(previous)
                        && self.permits_keyboard_input(previous)
                });
            if restorable {
                self.focus(accessory.previous.as_ref(), accessory.visible);
            } else {
                self.clear();
            }
        }
    }
    /// Native accessibility starts at the modal root, including only accessory
    /// portals owned by that scope.
    pub fn accessory_roots(&self) -> Vec<Node> {
        let modal = self.modal_root();
        self.accessories
            .borrow()
            .iter()
            .filter_map(|accessory| {
                let root = accessory.root.as_ref()?;
                let owner = accessory.owner.as_ref()?;
                if !owner.accepts_subtree_input() || !self.permits_input(owner) {
                    return None;
                }
                if modal
                    .as_ref()
                    .is_some_and(|modal| is_descendant(root, modal))
                {
                    return None;
                }
                Some(root.clone())
            })
            .collect()
    }
    fn belongs_to_scope(
        &self,
        node: &Node,
        root: &Node,
        visited: &mut Vec<crate::ElementId>,
    ) -> bool {
        if is_descendant(node, root) {
            return true;
        }
        if visited.contains(&node.id()) {
            return false;
        }
        visited.push(node.id());
        let accessories = self.accessories.borrow();
        accessories.iter().any(|accessory| {
            let (Some(accessory_root), Some(owner)) =
                (accessory.root.as_ref(), accessory.owner.as_ref())
            else {
                return false;
            };
            owner.accepts_subtree_input()
                && is_descendant(node, accessory_root)
                && self.belongs_to_scope(owner, root, visited)
        })
    }
    pub fn push_scope(&self, root: &Node, restores_commands: bool) {
        if self
            .scopes
            .borrow()
            .iter()
            .any(|scope| scope.root.as_ref() == Some(root))
        {
            return;
        }
        self.scopes.borrow_mut().push(Scope::new(
            root,
            self.focused(),
            self.is_focus_visible.get(),
            restores_commands,
        ));
        let target = self
            .focusables(root)
            .first()
            .cloned()
            .unwrap_or_else(|| root.clone());
        self.focus(Some(&target), self.is_focus_visible.get());
    }
    pub fn settle_scope(&self, root: &Node) {
        if self.active_scope_root().as_ref() != Some(root) {
            return;
        }
        let focused = self.focused();
        if focused.is_none() || focused.as_ref() == Some(root) {
            let candidates: Vec<Node> = self
                .focusables(root)
                .into_iter()
                .filter(|candidate| candidate != root)
                .collect();
            let target = candidates
                .iter()
                .find(|candidate| {
                    candidate
                        .attachment::<super::TextInputAreaResolver>(
                            TextInputAttachmentKey::AREA_RESOLVER,
                        )
                        .is_some()
                })
                .or_else(|| candidates.first())
                .cloned();
            if let Some(target) = target {
                self.focus(Some(&target), self.is_focus_visible.get());
            }
        }
    }
    pub fn pop_scope(&self, root: &Node) {
        let index = self
            .scopes
            .borrow()
            .iter()
            .position(|scope| scope.root.as_ref() == Some(root));
        let Some(index) = index else { return };
        let should_restore = self
            .focused()
            .is_none_or(|focused| is_descendant(&focused, root));
        let scope = self.scopes.borrow_mut().remove(index);
        if index < self.scopes.borrow().len() {
            let mut scopes = self.scopes.borrow_mut();
            let next = &mut scopes[index];
            if let Some(previous) = next.previous.as_ref()
                && is_descendant(previous, root)
            {
                next.previous = scope.previous;
                next.previous_tree_root = scope.previous_tree_root;
            }
            return;
        }
        if !should_restore
            && let Some(focused) = self.focused()
            && focused.parent().is_some()
            && self.permits_input(&focused)
        {
            return;
        }
        let restorable = scope
            .previous
            .as_ref()
            .zip(scope.previous_tree_root.as_ref())
            .is_some_and(|(previous, tree_root)| {
                is_descendant(previous, tree_root)
                    && previous.parent().is_some()
                    // A style host wraps the focusable surface; restoring the
                    // wrapper keeps the editor's focus target reachable.
                    && (previous.is_focusable() || !self.focusables(previous).is_empty())
                    && previous.accepts_subtree_input()
                    && self.permits_input(previous)
            });
        if restorable {
            self.focus(scope.previous.as_ref(), scope.visible);
        } else if let Some(active) = self.active_scope_root() {
            let target = self.focusables(&active).first().cloned().unwrap_or(active);
            self.focus(Some(&target), self.is_focus_visible.get());
        } else {
            self.focus(None, scope.visible);
        }
    }
    pub fn focus(&self, node: Option<&Node>, visible: bool) {
        if let Some(node) = node
            && (!self.permits_input(node) || !node.accepts_subtree_input())
        {
            return;
        }
        let visibility_changed = self.is_focus_visible.get() != visible;
        self.is_focus_visible.set(visible);
        if self.focused.borrow().as_ref() == node {
            if visibility_changed {
                self.registrar.invalidate(&StateAccessKey::text("focus"));
                self.notify_focus_change(node, true);
            }
            return;
        }
        let previous = std::mem::replace(&mut *self.focused.borrow_mut(), node.cloned());
        self.registrar.invalidate(&StateAccessKey::text("focus"));
        self.notify_focus_change(previous.as_ref(), false);
        self.notify_focus_change(node, true);
    }
    /// Move focus to the next focusable node in tree order, wrapping around.
    pub fn focus_next(&self, root: &Node) -> Option<Node> {
        let scope_root = self.active_scope_root().unwrap_or_else(|| root.clone());
        let chain = self.focusables(&scope_root);
        if chain.is_empty() {
            // Keyboard traversal makes the focus ring visible, matching Swift's
            // `focus(_:visible:)` default of `true`. Carrying the previous
            // visibility over would leave the ring hidden after a pointer-driven
            // session, so the user could tab through controls with no indication.
            self.focus(None, true);
            return None;
        }
        let focused = self.focused();
        let index = focused
            .as_ref()
            .and_then(|focused| chain.iter().position(|candidate| candidate == focused));
        let next = match index {
            Some(index) => chain[(index + 1) % chain.len()].clone(),
            None => chain[0].clone(),
        };
        self.focus(Some(&next), true);
        Some(next)
    }
    pub fn focus_previous(&self, root: &Node) -> Option<Node> {
        let scope_root = self.active_scope_root().unwrap_or_else(|| root.clone());
        let chain = self.focusables(&scope_root);
        if chain.is_empty() {
            self.focus(None, self.is_focus_visible.get());
            return None;
        }
        let focused = self.focused();
        let index = focused
            .as_ref()
            .and_then(|focused| chain.iter().position(|candidate| candidate == focused));
        let previous = match index {
            Some(index) => chain[(index + chain.len() - 1) % chain.len()].clone(),
            None => chain[chain.len() - 1].clone(),
        };
        self.focus(Some(&previous), self.is_focus_visible.get());
        Some(previous)
    }
    pub fn clear(&self) {
        let visible = self.is_focus_visible.get();
        self.focus(None, visible);
    }
    pub fn contains(&self, node: &Node, root: &Node) -> bool {
        is_descendant(node, root)
    }
    pub fn begin_modal(&self, root: &Node) {
        self.push_scope(root, false);
    }
    pub fn end_modal(&self, root: &Node) {
        self.pop_scope(root);
    }
    pub fn ensure_modal_focus(&self) {
        if let Some(root) = self.active_scope_root() {
            self.settle_scope(&root);
        }
    }
    pub fn text_edit_availability(&self, command: TextEditCommand) -> Option<bool> {
        self.text_edit_actions()
            .map(|actions| (actions.can_perform)(command))
    }
    pub fn perform_text_edit(&self, command: TextEditCommand) -> bool {
        let Some(actions) = self.text_edit_actions() else {
            return false;
        };
        if (actions.can_perform)(command) {
            (actions.perform)(command);
        }
        true
    }
    fn text_edit_actions(&self) -> Option<TextEditActions> {
        let target = self.command_target()?;
        if !target.accepts_subtree_input() || !self.permits_input(&target) {
            return None;
        }
        target.attachment::<TextEditActions>(TextInputAttachmentKey::EDIT_ACTIONS)
    }
    fn focusables(&self, root: &Node) -> Vec<Node> {
        let mut out = Vec::new();
        collect(root, &mut out);
        out
    }
    fn notify_focus_change(&self, node: Option<&Node>, is_focused: bool) {
        if let Some(handler) = node.and_then(|node| {
            node.attachment::<super::TextInputFocusChangeHandler>(
                TextInputAttachmentKey::FOCUS_CHANGE_HANDLER,
            )
        }) {
            handler(is_focused);
        }
    }
}

fn collect(node: &Node, out: &mut Vec<Node>) {
    if !node.accepts_subtree_input() {
        return;
    }
    if node.is_focusable() && node.is_tab_stop() {
        out.push(node.clone());
    }
    for child in node.children() {
        collect(&child, out);
    }
}

thread_local! {
    static CURRENT_CHAIN: RefCell<Option<Rc<FocusChain>>> = const { RefCell::new(None) };
}
pub struct FocusChainHolder;
impl FocusChainHolder {
    pub fn current() -> Option<Rc<FocusChain>> {
        CURRENT_CHAIN.with(|slot| slot.borrow().clone())
    }
    pub fn set_current(chain: Option<Rc<FocusChain>>) {
        CURRENT_CHAIN.with(|slot| *slot.borrow_mut() = chain);
    }
    pub fn current_focused() -> Option<Node> {
        Self::current().and_then(|chain| chain.focused())
    }
    pub fn clear_current() {
        if let Some(chain) = Self::current() {
            chain.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> (Node, Node, Node, Node) {
        let root = Node::new();
        root.set_frame(crate::Rect::new(0.0, 0.0, 100.0, 100.0));
        let first = Node::new();
        first.set_is_focusable(true);
        let second = Node::new();
        second.set_is_focusable(true);
        root.add_child(&first);
        root.add_child(&second);
        (root, first, second, Node::new())
    }

    #[test]
    fn tab_traversal_wraps_in_tree_order() {
        let (root, first, second, _) = tree();
        let chain = FocusChain::new();
        assert_eq!(chain.focus_next(&root), Some(first.clone()));
        assert_eq!(chain.focus_next(&root), Some(second.clone()));
        assert_eq!(chain.focus_next(&root), Some(first.clone()));
        assert_eq!(chain.focus_previous(&root), Some(second));
    }

    #[test]
    fn modal_scope_restores_previous_focus() {
        let (root, first, _, _) = tree();
        let chain = FocusChain::new();
        chain.focus(Some(&first), true);
        let modal = Node::new();
        let inside = Node::new();
        inside.set_is_focusable(true);
        modal.add_child(&inside);
        root.add_child(&modal);
        chain.begin_modal(&modal);
        assert_eq!(chain.focused(), Some(inside));
        chain.end_modal(&modal);
        assert_eq!(chain.focused(), Some(first));
    }

    #[test]
    fn hidden_subtree_is_skipped() {
        let (root, first, _, _) = tree();
        first.set_layout_visibility(false);
        let chain = FocusChain::new();
        let next = chain.focus_next(&root).unwrap();
        assert_ne!(next, first);
    }
}
