//! Translation of `Tests/Headless/Tests/CoveUISceneTests/StructuralModifierTests.swift`.
//!
//! Swift's `ViewBuilder` produces a structural view for `if` and `for` bodies;
//! Rust spells the same shapes `Option<T>` and `Vec<T>`. The suite pins that a
//! modifier applied above a structural view does not disturb the child scopes
//! underneath it — only an identity change resets child-local state.
//!
//! Model note: Swift `@State` is owned by the child's view scope, and the Rust
//! graph carries it the same way — `ViewScope::replace_view` matches state
//! members by name and copies the runtime value across, so each `body`
//! evaluation may construct a fresh `State` value. The tests below therefore
//! build a new `Counter` per body, exactly like the Swift original.
//!
//! The graphs share process-global composition state, so every test runs under
//! [`GlobalTestLock`].
//!
//! Behaviour checklist:
//! - updating a modifier on an unchanged conditional preserves its child scope
//!   and local state, and toggling the branch away resets that state
//! - modified arrays preserve keyed child state through reorder, insertion and
//!   removal
//! - an identified conditional survives multiple outer modifiers and resets only
//!   when its identity changes
//! - identified arrays reorder as groups and reset removed groups on return

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};

use cove_composition::{
    AnyView, DynamicField, LayoutDebugExt, PrimitiveView, View, ViewExt, ViewGraph, ViewKind,
};
use cove_core::scene::{AccessibilityActions, Node, NodeTree, Recomposer};
use cove_core::{ElementId, State};

struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn locked<T>(body: impl FnOnce() -> T) -> T {
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        body()
    }
}

/// Stamps the probe name and count and installs the activate action, the Swift
/// `Probe`.
struct Probe {
    name: String,
    count: i32,
    action: Rc<dyn Fn()>,
}

impl PrimitiveView for Probe {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, node: &Node) {
        node.set_attachment("probe-name", self.name.clone());
        node.set_attachment("probe-count", self.count);
        node.set_accessibility_actions(AccessibilityActions {
            activate: Some(self.action.clone()),
            ..AccessibilityActions::default()
        });
    }
    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }
}

impl View for Probe {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// `Counter`: a named probe whose count is child-local `@State`.
struct Counter {
    name: String,
    count: State<i32>,
}

impl Counter {
    fn new(name: &str) -> Self {
        Self { name: name.to_string(), count: State::new(0) }
    }
}

/// `Option<T>: View` requires `Clone`; cloning shares the same `State` handle.
impl Clone for Counter {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), count: self.count.clone() }
    }
}

impl View for Counter {
    fn body(&self) -> AnyView {
        let count = self.count.clone();
        AnyView::new(Probe {
            name: self.name.clone(),
            count: self.count.get(),
            action: Rc::new(move || {
                let next = count.get() + 1;
                count.set(next);
            }),
        })
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("count", &self.count)]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn probe(root: &Node, name: &str) -> Option<Node> {
    if root.attachment::<String>("probe-name").as_deref() == Some(name) {
        return Some(root.clone());
    }
    root.children().iter().find_map(|child| probe(child, name))
}

fn root_of(graph: &Rc<ViewGraph>) -> Node {
    graph.tree.borrow().root().expect("an installed graph has a root")
}

fn probe_count(node: &Node) -> i32 {
    node.attachment::<i32>("probe-count").expect("a probe stamps its count")
}

fn activate(node: &Node) {
    let actions = node.accessibility_actions();
    let activate = actions.activate.expect("a probe installs an activate action");
    activate();
}

fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

/// `if shown { Counter }` under a revision-renamed modifier.
struct ConditionalHarness {
    revision: State<i32>,
    shown: State<bool>,
}

impl View for ConditionalHarness {
    fn body(&self) -> AnyView {
        let revision = self.revision.get();
        let content = if self.shown.get() {
            AnyView::new(Some(Counter::new("counter")))
        } else {
            AnyView::new(Option::<Counter>::None)
        };
        AnyView::new(AnyView::new(content).debug_name(&format!("conditional-{revision}")))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("revision", &self.revision),
            DynamicField::state("shown", &self.shown),
        ]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn conditional() {
    GlobalTestLock::locked(|| {
        let graph = graph();
        let harness = ConditionalHarness { revision: State::new(0), shown: State::new(true) };
        let revision = harness.revision.clone();
        let shown = harness.shown.clone();
        graph.install(harness, Default::default());

        let first = probe(&root_of(&graph), "counter").expect("the conditional counter is installed");
        let id: ElementId = first.id();
        activate(&first);
        graph.recomposer.commit_all();

        // Renaming the modifier must not disturb the child scope below it.
        for value in 1..=4 {
            revision.set(value);
            graph.recomposer.commit_all();
            let node = probe(&root_of(&graph), "counter").expect("still installed");
            assert_eq!(node.id(), id, "revision {value} keeps the child identity");
            assert_eq!(probe_count(&node), 1, "revision {value} keeps the child-local count");
        }

        // Toggling the branch away drops the child, and its state with it.
        shown.set(false);
        graph.recomposer.commit_all();
        assert!(probe(&root_of(&graph), "counter").is_none(), "the branch is gone");

        shown.set(true);
        graph.recomposer.commit_all();
        let restored = probe(&root_of(&graph), "counter").expect("the branch is back");
        assert_eq!(probe_count(&restored), 0, "a restored branch starts from fresh state");
    });
}

/// `names.map { Counter($0).id($0) }` under a revision-renamed modifier.
struct ArrayHarness {
    names: State<Vec<String>>,
    revision: State<i32>,
}

impl View for ArrayHarness {
    fn body(&self) -> AnyView {
        let revision = self.revision.get();
        let views: Vec<AnyView> = self
            .names
            .get()
            .iter()
            .map(|name| AnyView::new(Counter::new(name).id(name.clone())))
            .collect();
        AnyView::new(AnyView::new(views).debug_name(&format!("array-{revision}")))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("names", &self.names),
            DynamicField::state("revision", &self.revision),
        ]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn arrays() {
    GlobalTestLock::locked(|| {
        let graph = graph();
        let harness =
            ArrayHarness { names: State::new(strings(&["a", "b"])), revision: State::new(0) };
        let names = harness.names.clone();
        let revision = harness.revision.clone();
        graph.install(harness, Default::default());

        let first = probe(&root_of(&graph), "a").expect("`a` is installed");
        let id = first.id();
        activate(&first);
        graph.recomposer.commit_all();

        let edits: [&[&str]; 3] = [&["b", "a"], &["c", "a", "b"], &["a"]];
        for next in edits {
            names.set(strings(next));
            revision.set(revision.get() + 1);
            graph.recomposer.commit_all();
            let node = probe(&root_of(&graph), "a").expect("`a` survives every edit");
            assert_eq!(node.id(), id, "the key `a` keeps its identity through {next:?}");
            assert_eq!(probe_count(&node), 1, "the key `a` keeps its state through {next:?}");
        }
    });
}

/// `if … { Counter("identified") }.id(identity)` with two outer `debug_name`
/// modifiers.
struct IdentifiedConditional {
    revision: State<i32>,
    identity: State<i32>,
}

impl View for IdentifiedConditional {
    fn body(&self) -> AnyView {
        let revision = self.revision.get();
        let identity = self.identity.get();
        AnyView::new(
            AnyView::new(
                AnyView::new(Counter::new("identified").id(identity))
                    .debug_name(&format!("identified-{revision}")),
            )
            .debug_name("outer"),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("revision", &self.revision),
            DynamicField::state("identity", &self.identity),
        ]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn identified_conditional() {
    GlobalTestLock::locked(|| {
        let graph = graph();
        let harness = IdentifiedConditional { revision: State::new(0), identity: State::new(0) };
        let revision = harness.revision.clone();
        let identity = harness.identity.clone();
        graph.install(harness, Default::default());

        let first =
            probe(&root_of(&graph), "identified").expect("the identified counter is installed");
        let id = first.id();
        activate(&first);
        graph.recomposer.commit_all();

        // Two outer modifiers change on every revision; identity does not.
        for value in 1..=4 {
            revision.set(value);
            graph.recomposer.commit_all();
            let node = probe(&root_of(&graph), "identified").expect("still installed");
            assert_eq!(node.id(), id, "revision {value} survives both outer modifiers");
            assert_eq!(probe_count(&node), 1, "revision {value} keeps the child-local count");
        }

        // A new identity is the one thing that rebuilds the subtree.
        identity.set(1);
        graph.recomposer.commit_all();
        let replaced = probe(&root_of(&graph), "identified").expect("still installed");
        assert_ne!(replaced.id(), id, "a new identity replaces the node");
        assert_eq!(probe_count(&replaced), 0, "the replacement starts from fresh state");
    });
}

/// `groups.map { AnyView(names.map { Counter("group-name").id(name) }).id(group) }`.
struct IdentifiedArrays {
    groups: State<Vec<String>>,
    names: State<Vec<String>>,
}

impl View for IdentifiedArrays {
    fn body(&self) -> AnyView {
        let names = self.names.get();
        let nested: Vec<AnyView> = self
            .groups
            .get()
            .iter()
            .map(|group| {
                let children: Vec<AnyView> = names
                    .iter()
                    .map(|name| {
                        AnyView::new(Counter::new(&format!("{group}-{name}")).id(name.clone()))
                    })
                    .collect();
                AnyView::new(AnyView::new(children).id(group.clone()))
            })
            .collect();
        AnyView::new(AnyView::new(nested).debug_name("groups"))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("groups", &self.groups),
            DynamicField::state("names", &self.names),
        ]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

#[test]
fn identified_arrays() {
    GlobalTestLock::locked(|| {
        let graph = graph();
        let harness = IdentifiedArrays {
            groups: State::new(strings(&["first", "second"])),
            names: State::new(strings(&["a", "b"])),
        };
        let groups = harness.groups.clone();
        let names = harness.names.clone();
        graph.install(harness, Default::default());

        let first = probe(&root_of(&graph), "first-a").expect("`first-a` is installed");
        let second = probe(&root_of(&graph), "second-a").expect("`second-a` is installed");
        let first_id = first.id();
        let second_id = second.id();
        activate(&first);
        graph.recomposer.commit_all();

        // Reverse the group order and insert a child: keyed identity holds.
        groups.set(strings(&["second", "first"]));
        names.set(strings(&["b", "a", "c"]));
        graph.recomposer.commit_all();

        let reordered_first = probe(&root_of(&graph), "first-a").expect("`first-a` survives");
        let reordered_second = probe(&root_of(&graph), "second-a").expect("`second-a` survives");
        assert_eq!(
            reordered_first.id(),
            first_id,
            "groups reorder without rebuilding their children",
        );
        assert_eq!(reordered_second.id(), second_id);
        assert_eq!(probe_count(&reordered_first), 1, "the first group keeps its child state");
        assert_eq!(probe_count(&reordered_second), 0, "the untouched group keeps its own state");

        // Dropping a group releases it; bringing it back starts fresh.
        groups.set(strings(&["second"]));
        graph.recomposer.commit_all();
        assert!(probe(&root_of(&graph), "first-a").is_none(), "the removed group is gone");

        groups.set(strings(&["first", "second"]));
        graph.recomposer.commit_all();
        let restored = probe(&root_of(&graph), "first-a").expect("the group is back");
        assert_ne!(restored.id(), first_id, "a returned group builds new nodes");
        assert_eq!(probe_count(&restored), 0, "a returned group starts from fresh state");
    });
}
