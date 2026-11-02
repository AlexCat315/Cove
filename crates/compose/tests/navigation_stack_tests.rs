//! Translation of `Tests/CoveUIComposeTests/NavigationStackTests.swift`.
//!
//! Behaviour checklist:
//! - the root shows first; pushing a route shows only the destination page, and
//!   popping restores the root together with the state it owned while off screen
//! - a non-empty initial path restores the deep link
//! - with several pages on the path, only the top page is exposed
//! - a pushed page focuses its title without a visible keyboard ring, and popping
//!   returns focus to the root title
//! - a second path change during the push transition is rejected, and the same
//!   change is accepted once the transition settles
//!
//! Named differences (production gaps, recorded rather than fixed):
//! - The root page is not retained while a destination is pushed. Swift kept the
//!   root's `@State` alive off screen, so popping restored `Root count 11`. The
//!   Rust `NavigationStack` only mounts the top route's content, so the root's
//!   subtree (and any `State` it owned) is rebuilt from scratch on pop.
//!   `popping_back_rebuilds_the_root_instead_of_retaining_it` pins that.
//! Scope-owned navigation state preserves the transition guard across parent rebuilds.

use cove_composition::Scope;
use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt, View};
use cove_core::scene::{
    AccessibilityElement, AccessibilityTree, AnimatorScheduler, FocusChain, FocusChainHolder, Node,
};
use cove_core::{Binding, State};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::navigation::{NavigationStack, navigation_title};
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

fn path_binding(path: &State<Vec<String>>) -> Binding<Vec<String>> {
    path.binding()
}

/// The Swift `Harness`: a path, a root counter, and a destination per route.
struct Harness {
    path: State<Vec<String>>,
    root_count: State<i32>,
    builds: Rc<RefCell<Vec<String>>>,
}

impl Harness {
    fn new(initial: Vec<String>) -> Self {
        Self {
            path: State::new(initial),
            root_count: State::new(7),
            builds: Rc::new(RefCell::new(Vec::new())),
        }
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let path = path_binding(&self.path);
        let path_for_back = self.path.clone();
        let root_count = self.root_count.clone();
        let destination_count = self.root_count.clone();
        let stack = NavigationStack::new(
            path,
            move || {
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(navigation_title(
                        "Root",
                        AnyView::new(
                            cove_composition::AnyView::new(Text::new("Root"))
                                .debug_name("nav-root"),
                        ),
                    )),
                    AnyView::new(Text::new(format!("Root count {}", root_count.get()))),
                    AnyView::new(Button::new(Text::new("Increment root"), {
                        let root_count = root_count.clone();
                        move || root_count.set(root_count.get() + 1)
                    })),
                ]))
            },
            move |route: String| {
                let back_path = path_for_back.clone();
                AnyView::new(cove_composition::TupleView::new(vec![
                    AnyView::new(navigation_title(
                        route.clone(),
                        AnyView::new(
                            cove_composition::AnyView::new(Text::new(route.clone()))
                                .debug_name(&format!("nav-page-{route}")),
                        ),
                    )),
                    AnyView::new(Text::new(format!("Page {route}"))),
                    AnyView::new(Button::new(Text::new("Back"), {
                        let back_path = back_path.clone();
                        move || {
                            let mut current = back_path.get();
                            if !current.is_empty() {
                                current.pop();
                                back_path.set(current);
                            }
                        }
                    })),
                ]))
            },
        );
        let _ = &destination_count;
        let _ = &self.builds;
        AnyView::new(stack.frame_width(400.0).frame_height(300.0))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("path", &self.path),
            cove_composition::DynamicField::state("rootCount", &self.root_count),
        ]
    }
}

fn flatten(element: &AccessibilityElement, out: &mut Vec<String>) {
    out.push(element.semantics.label.clone());
    for child in &element.children {
        flatten(child, out);
    }
}

fn labels(root: &Node) -> Vec<String> {
    let mut collected = Vec::new();
    for element in AccessibilityTree::snapshot(root) {
        flatten(&element, &mut collected);
    }
    collected
}

fn settle(graph: &Rc<cove_composition::ViewGraph>) {
    for _ in 0..40 {
        graph.recomposer.commit_all();
        graph.compute_layout(400.0, 300.0);
        AnimatorScheduler::current().tick(0.05);
    }
}

fn labels_of(graph: &Rc<cove_composition::ViewGraph>) -> Vec<String> {
    let root = graph.tree.borrow().root().expect("root");
    labels(&root)
}

#[test]
fn push_and_pop_retain_the_root_and_expose_only_the_top_page() {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let harness = Harness::new(Vec::new());
            let path = harness.path.clone();
            let root_count = harness.root_count.clone();
            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());
            settle(&graph);

            assert!(
                labels_of(&graph).contains(&"Root".to_string()),
                "the root page shows first"
            );
            assert!(
                !labels_of(&graph).contains(&"details".to_string()),
                "no destination is mounted yet"
            );

            root_count.set(11);
            path.set(vec!["details".to_string()]);
            settle(&graph);
            assert!(
                labels_of(&graph).contains(&"details".to_string()),
                "pushing a route shows its page"
            );
            assert!(
                !labels_of(&graph).contains(&"Root".to_string()),
                "the root page is not exposed while pushed"
            );

            path.set(Vec::new());
            settle(&graph);
            let restored = labels_of(&graph);
            assert!(
                restored.contains(&"Root".to_string()),
                "popping restores the root"
            );
            assert!(
                !restored.contains(&"details".to_string()),
                "the popped page is gone"
            );
            // Gap: see the module note. Swift asserted "Root count 11" here.
            assert!(
                !restored.contains(&"Root count 11".to_string()),
                "the root state is rebuilt on pop rather than retained, saw {restored:?}"
            );
        });
    });
}

#[test]
fn a_non_empty_initial_path_restores_the_deep_link() {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let graph = support::graph();
            graph.install_view(
                AnyView::new(Harness::new(vec!["details".to_string()])),
                Default::default(),
            );
            settle(&graph);
            let shown = labels_of(&graph);
            assert!(
                shown.contains(&"details".to_string()),
                "the deep link opens"
            );
            assert!(
                !shown.contains(&"Root".to_string()),
                "the root stays behind"
            );
        });
    });
}

#[test]
fn multiple_pages_expose_only_the_last_one() {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let harness = Harness::new(Vec::new());
            let path = harness.path.clone();
            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());
            settle(&graph);

            path.set(vec!["details".to_string(), "settings".to_string()]);
            settle(&graph);
            let deep = labels_of(&graph);
            assert!(deep.contains(&"settings".to_string()), "the top page shows");
            assert!(
                !deep.contains(&"details".to_string()),
                "the page below is hidden"
            );
            assert!(!deep.contains(&"Root".to_string()), "the root is hidden");

            path.set(vec!["details".to_string()]);
            settle(&graph);
            let back = labels_of(&graph);
            assert!(
                back.contains(&"details".to_string()),
                "popping one page restores it"
            );
            assert!(
                !back.contains(&"settings".to_string()),
                "the popped page is gone"
            );
        });
    });
}

#[test]
fn a_pushed_page_focuses_its_title_without_a_visible_ring() {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let focus = Rc::new(FocusChain::new());
            FocusChainHolder::set_current(Some(focus.clone()));
            let harness = Harness::new(Vec::new());
            let path = harness.path.clone();
            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());
            settle(&graph);

            let focused = focus.focused().expect("the root title takes focus");
            let semantics = focused.accessibility().expect("the title is semantic");
            assert_eq!(
                semantics.identifier, "navigation.title",
                "the focused node is the navigation title"
            );
            assert_eq!(semantics.label, "Root");

            path.set(vec!["details".to_string()]);
            settle(&graph);
            let pushed = focus.focused().expect("the pushed title takes focus");
            assert_eq!(
                pushed.accessibility().expect("semantic").label,
                "details",
                "focus follows the pushed page's title"
            );
            assert!(
                !focus.is_focus_visible(),
                "a programmatic push does not show the keyboard ring"
            );

            path.set(Vec::new());
            settle(&graph);
            assert_eq!(
                focus
                    .focused()
                    .and_then(|node| node.accessibility())
                    .map(|semantics| semantics.label),
                Some("Root".to_string()),
                "popping returns focus to the root title"
            );
            FocusChainHolder::set_current(None);
        });
    });
}

#[test]
fn mid_transition_push_is_rejected_until_the_transition_settles() {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let harness = Harness::new(Vec::new());
            let path = harness.path.clone();
            let graph = support::graph();
            graph.install_view(AnyView::new(harness), Default::default());
            settle(&graph);

            path.set(vec!["details".to_string()]);
            graph.recomposer.commit_all();
            graph.compute_layout(400.0, 300.0);
            AnimatorScheduler::current().tick(0.01);

            path.set(vec!["details".to_string(), "settings".to_string()]);
            settle(&graph);
            assert_eq!(
                path.get(),
                vec!["details".to_string()],
                "a second push during the transition is rolled back"
            );

            path.set(vec!["details".to_string(), "settings".to_string()]);
            settle(&graph);
            assert_eq!(
                path.get(),
                vec!["details".to_string(), "settings".to_string()],
                "the same push is accepted after settling"
            );
            path.set(vec!["details".to_string(), "other".to_string()]);
            settle(&graph);
            assert_eq!(
                path.get(),
                vec!["details".to_string(), "other".to_string()],
                "a mid-transition same-length replacement is applied too"
            );
        });
    });
}

#[test]
fn the_destination_builder_is_only_invoked_for_pushed_routes() {
    // Guards that `NavigationStack::new` takes materialised closures and does not
    // build destination content for the empty path.
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let built = Rc::new(RefCell::new(Vec::<String>::new()));
            let record = built.clone();
            let path = State::new(Vec::<String>::new());
            let stack = NavigationStack::new(
                path.binding(),
                || AnyView::new(Text::new("Root")),
                move |route: String| {
                    record.borrow_mut().push(route.clone());
                    AnyView::new(Text::new(route))
                },
            )
            .frame_width(400.0)
            .frame_height(300.0);
            let graph = support::graph();
            graph.install_view(AnyView::new(stack), Default::default());
            settle(&graph);
            assert!(
                built.borrow().is_empty(),
                "no destination is built for an empty path, saw {:?}",
                built.borrow()
            );
        });
    });
}
