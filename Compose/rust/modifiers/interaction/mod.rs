//! Interaction and accessibility view modifiers.
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, FocusChainHolder, HoverPhase,
    InteractionRegistryHolder, Node, PointerCaptureHolder,
};
use cove_platform::SystemCursor;
use crate::{ModifiedContent, View, ViewExt, ViewModifier};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

fn collect_semantic_controls(node: &Node, targets: &mut Vec<Node>) {
    if node.accessibility().is_some_and(|semantics| {
        !matches!(semantics.role, AccessibilityRole::StaticText | AccessibilityRole::Group)
    }) {
        targets.push(node.clone());
        return;
    }
    for child in node.children() { collect_semantic_controls(&child, targets); }
}

fn is_container_role(role: AccessibilityRole) -> bool {
    matches!(role,
        AccessibilityRole::Group | AccessibilityRole::Dialog | AccessibilityRole::TabPanel |
        AccessibilityRole::Table | AccessibilityRole::Tree | AccessibilityRole::List |
        AccessibilityRole::Row | AccessibilityRole::Cell | AccessibilityRole::Status)
}

/// Applies semantics to the wrapper unless it unambiguously wraps one
/// actionable descendant, in which case its label/help customizes that control.
pub struct AccessibilityModifier {
    configure: Rc<dyn Fn(&mut AccessibilitySemantics)>,
}
impl AccessibilityModifier {
    pub fn new(configure: impl Fn(&mut AccessibilitySemantics) + 'static) -> Self {
        Self { configure: Rc::new(configure) }
    }
}
impl ViewModifier for AccessibilityModifier {
    fn apply_node(&self, node: &Node) {
        let mut targets = Vec::new();
        if node.accessibility().is_none() {
            for child in node.children() { collect_semantic_controls(&child, &mut targets); }
        }
        let target = if targets.len() == 1 { targets[0].clone() } else { node.clone() };
        let mut semantics = target.accessibility().unwrap_or_else(|| AccessibilitySemantics::new(AccessibilityRole::Group));
        (self.configure)(&mut semantics);
        if &target != node && is_container_role(semantics.role) {
            let mut wrapper = AccessibilitySemantics::new(AccessibilityRole::Group);
            (self.configure)(&mut wrapper);
            node.set_accessibility(Some(wrapper));
        } else {
            target.set_accessibility(Some(semantics));
        }
    }
}

pub struct CursorModifier(pub SystemCursor);
impl ViewModifier for CursorModifier {
    fn apply_node(&self, node: &Node) { node.set_cursor(Some(self.0)); }
}

pub struct HitTestingModifier(pub bool);
impl ViewModifier for HitTestingModifier {
    fn apply_node(&self, node: &Node) {
        node.set_allows_hit_testing(self.0);
        if self.0 { return; }
        if let Some(capture) = PointerCaptureHolder::current()
            && capture.target().is_some_and(|target| !target.accepts_subtree_input())
        {
            capture.release();
        }
        if let Some(focus) = FocusChainHolder::current()
            && focus.focused().is_some_and(|focused| !focused.accepts_subtree_input())
        {
            focus.clear();
        }
    }
}

pub struct HoverModifier {
    on_change: Rc<dyn Fn(bool)>,
}
impl HoverModifier {
    pub fn new(on_change: impl Fn(bool) + 'static) -> Self { Self { on_change: Rc::new(on_change) } }
}
impl ViewModifier for HoverModifier {
    fn apply_node(&self, node: &Node) {
        let Some(registry) = InteractionRegistryHolder::current() else { return };
        let on_change = self.on_change.clone();
        registry.set_hover(node, Rc::new(move |phase| on_change(phase == HoverPhase::Enter)));
    }
}

const FOCUS_REQUEST_ATTACHMENT: &str = "__focus.request";
pub struct FocusRequestModifier {
    request_id: Option<u64>,
}
impl FocusRequestModifier {
    pub fn new(request_id: Option<u64>) -> Self { Self { request_id } }
}
impl ViewModifier for FocusRequestModifier {
    fn apply_node(&self, node: &Node) {
        let Some(request_id) = self.request_id else { return };
        if !node.is_focusable() || node.attachment::<u64>(FOCUS_REQUEST_ATTACHMENT) == Some(request_id) { return; }
        node.set_attachment(FOCUS_REQUEST_ATTACHMENT, request_id);
        if let Some(focus) = FocusChainHolder::current() {
            focus.focus(Some(node), focus.is_focus_visible());
        }
    }
}

pub trait AccessibilityExt: View + Sized + 'static {
    fn accessibility(self, configure: impl Fn(&mut AccessibilitySemantics) + 'static) -> ModifiedContent {
        self.modifier(AccessibilityModifier::new(configure))
    }
    fn accessibility_label(self, label: impl Into<String>) -> ModifiedContent {
        let label = label.into();
        self.accessibility(move |semantics| semantics.label = label.clone())
    }
    fn accessibility_identifier(self, identifier: impl Into<String>) -> ModifiedContent {
        let identifier = identifier.into();
        self.accessibility(move |semantics| semantics.identifier = identifier.clone())
    }
    fn accessibility_described_by(self, identifier: impl Into<String>) -> ModifiedContent {
        let identifier = identifier.into();
        self.accessibility(move |semantics| semantics.described_by = Some(identifier.clone()))
    }
    fn accessibility_hidden(self, hidden: bool) -> ModifiedContent {
        self.accessibility(move |semantics| semantics.is_hidden = hidden)
    }
}
impl<T: View + Sized + 'static> AccessibilityExt for T {}

pub trait CursorExt: View + Sized + 'static {
    fn cursor(self, cursor: SystemCursor) -> ModifiedContent { self.modifier(CursorModifier(cursor)) }
}
impl<T: View + Sized + 'static> CursorExt for T {}

pub trait HitTestingExt: View + Sized + 'static {
    fn allows_hit_testing(self, enabled: bool) -> ModifiedContent { self.modifier(HitTestingModifier(enabled)) }
}
impl<T: View + Sized + 'static> HitTestingExt for T {}

pub trait HoverExt: View + Sized + 'static {
    fn on_hover(self, on_change: impl Fn(bool) + 'static) -> ModifiedContent {
        self.modifier(HoverModifier::new(on_change))
    }
}
impl<T: View + Sized + 'static> HoverExt for T {}

pub trait FocusRequestExt: View + Sized + 'static {
    fn focus_request(self, request_id: Option<u64>) -> ModifiedContent {
        self.modifier(FocusRequestModifier::new(request_id))
    }
    fn focus_request_key<K: Hash>(self, request_id: Option<K>) -> ModifiedContent {
        let request_id = request_id.map(|key| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
        });
        self.focus_request(request_id)
    }
}
impl<T: View + Sized + 'static> FocusRequestExt for T {}
