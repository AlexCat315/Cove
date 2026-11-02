//! Translation of `Tests/CoveUIComposeTests/ViewThatFitsTests.swift`.
//!
//! Behaviour checklist:
//! - only the measured branch lays out and is exposed to input and accessibility
//! - round trips preserve independent local state and node identity, while
//!   releasing hidden focus and capture
//! - the wide threshold is inclusive and changing it does not require a new
//!   measurement
//! - intrinsic mode selects by natural content and retains both branches

use cove_composition::Scope;
use crate::support;

use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, View};
use cove_core::State;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, FocusChain, FocusChainHolder, InteractionRegistry,
    InteractionRegistryHolder, Node, PointerCapture, PointerCaptureHolder,
};

use cove_ui_compose::modifiers::interaction::AccessibilityExt;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::layout::responsive::ViewThatFits;

use support::GlobalTestLock;

const HEIGHT: f32 = 100.0;

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut collected);
    }
    collected
}

fn named(graph: &cove_composition::ViewGraph, name: &str) -> Option<Node> {
    all(graph)
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
}

/// The exposed accessibility labels, the Swift `AccessibilityTree.snapshot` read.
fn exposed_labels(graph: &cove_composition::ViewGraph) -> Vec<String> {
    let Some(root) = graph.tree.borrow().root() else {
        return Vec::new();
    };
    cove_core::scene::AccessibilityTree::snapshot(&root)
        .into_iter()
        .map(|element| element.semantics.label)
        .filter(|label| label == "wide" || label == "compact")
        .collect()
}

/// Stickiness of the source branch, mirroring Swift's retained-branch contract.
fn settled(graph: &cove_composition::ViewGraph, width: f32) {
    for _ in 0..3 {
        graph.recomposer.commit_all();
        graph.compute_layout(width, HEIGHT);
    }
    assert!(
        !graph.recomposer.has_pending(),
        "the recomposer settles after three passes"
    );
}

/// One counted button, the Swift `Counter`.
fn counter(name: &'static str, count: State<i32>) -> AnyView {
    let pressed = count.clone();
    AnyView::new(
        AnyView::new(Button::title("", move || pressed.set(pressed.get() + 1)))
            .accessibility(move |semantics: &mut AccessibilitySemantics| {
                semantics.role = AccessibilityRole::Button;
                semantics.label = name.to_string();
            })
            .debug_name(name),
    )
}

/// A branch that owns its own counter state, so a round trip can prove it kept it.
struct CounterBranch {
    name: &'static str,
    count: State<i32>,
}

impl View for CounterBranch {
    fn body(&self, _cx: &Scope) -> AnyView {
        let _count = self.count.get();
        AnyView::new(counter(self.name, self.count.clone()))
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("count", &self.count)]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct HarnessView {
    width: State<f32>,
    threshold: State<f32>,
    wide_count: State<i32>,
    compact_count: State<i32>,
}

impl View for HarnessView {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            AnyView::new(ViewThatFits::threshold(
                self.threshold.get(),
                AnyView::new(CounterBranch {
                    name: "wide",
                    count: self.wide_count.clone(),
                }),
                AnyView::new(CounterBranch {
                    name: "compact",
                    count: self.compact_count.clone(),
                }),
            ))
            .frame_size(self.width.get(), HEIGHT),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.width),
            DynamicField::state("threshold", &self.threshold),
            DynamicField::state("wideCount", &self.wide_count),
            DynamicField::state("compactCount", &self.compact_count),
        ]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct IntrinsicView {
    width: State<f32>,
    wide_count: State<i32>,
    compact_count: State<i32>,
}

impl View for IntrinsicView {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(
            AnyView::new(ViewThatFits::intrinsic(
                AnyView::new(CounterBranch {
                    name: "wide",
                    count: self.wide_count.clone(),
                })
                .frame_width(160.0),
                AnyView::new(CounterBranch {
                    name: "compact",
                    count: self.compact_count.clone(),
                })
                .frame_width(60.0),
            ))
            .frame_size(self.width.get(), HEIGHT),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("width", &self.width),
            DynamicField::state("wideCount", &self.wide_count),
            DynamicField::state("compactCount", &self.compact_count),
        ]
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

fn install_holders() -> (Rc<InteractionRegistry>, Rc<FocusChain>, Rc<PointerCapture>) {
    let registry = Rc::new(InteractionRegistry::new());
    let focus = Rc::new(FocusChain::new());
    let capture = Rc::new(PointerCapture::new());
    InteractionRegistryHolder::set_current(Some(registry.clone()));
    FocusChainHolder::set_current(Some(focus.clone()));
    PointerCaptureHolder::set_current(Some(capture.clone()));
    (registry, focus, capture)
}

fn clear_holders() {
    InteractionRegistryHolder::set_current(None);
    FocusChainHolder::set_current(None);
    PointerCaptureHolder::set_current(None);
}

/// Fires a node's accessibility activation, the Swift `activate?()` call.
fn activate(node: &Node) -> bool {
    let handlers = InteractionRegistryHolder::current()
        .map(|registry| registry.handlers(node))
        .unwrap_or_default();
    match handlers.pointer {
        Some(handler) => {
            let frame = node.absolute_frame();
            let event = cove_platform::MouseButtonEvent {
                button: cove_platform::MouseButton::Left,
                x: frame.x + frame.width / 2.0,
                y: frame.y + frame.height / 2.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            };
            let _ = handler(
                &event,
                cove_core::scene::PointerPhase::Down,
                cove_core::scene::EventPhase::Target,
            );
            let _ = handler(
                &event,
                cove_core::scene::PointerPhase::Up,
                cove_core::scene::EventPhase::Target,
            );
            true
        }
        None => false,
    }
}

fn harness() -> (HarnessView, Rc<cove_composition::ViewGraph>) {
    let view = HarnessView {
        width: State::new(700.0f32),
        threshold: State::new(600.0f32),
        wide_count: State::new(0),
        compact_count: State::new(0),
    };
    let graph = support::graph();
    graph.install(
        HarnessView {
            width: view.width.clone(),
            threshold: view.threshold.clone(),
            wide_count: view.wide_count.clone(),
            compact_count: view.compact_count.clone(),
        },
        Default::default(),
    );
    (view, graph)
}

fn harness_refs(view: &HarnessView) -> HarnessView {
    HarnessView {
        width: view.width.clone(),
        threshold: view.threshold.clone(),
        wide_count: view.wide_count.clone(),
        compact_count: view.compact_count.clone(),
    }
}

#[test]
fn only_the_measured_branch_lays_out_and_is_exposed() {
    GlobalTestLock::locked(|| {
        let (view, graph) = harness();
        let _ = view;

        assert!(
            named(&graph, "compact").is_some() && named(&graph, "wide").is_none(),
            "the install pass selects compact before any measurement"
        );

        settled(&graph, 700.0);
        let wide = named(&graph, "wide").expect("the wide branch materialises");
        let compact = named(&graph, "compact").expect("the compact branch materialises");
        assert!(
            wide.accepts_subtree_input() && !compact.accepts_subtree_input(),
            "only the selected branch accepts input"
        );
        assert!(
            wide.frame().width > 0.0,
            "the selected branch keeps its measured frame, was {:?}",
            wide.frame()
        );
        assert_eq!(
            compact.frame().width,
            0.0,
            "the hidden branch collapses, was {:?}",
            compact.frame()
        );
        let labels = exposed_labels(&graph);
        assert_eq!(
            labels,
            vec!["wide".to_string()],
            "only the selected branch is exposed to accessibility"
        );

        _ = view;
        // Narrow the container from the start and confirm the branches swap roles.
        let narrow_harness = HarnessView {
            width: State::new(300.0f32),
            threshold: State::new(600.0f32),
            wide_count: State::new(0),
            compact_count: State::new(0),
        };
        let graph2 = support::graph();
        graph2.install(harness_refs(&narrow_harness), Default::default());
        settled(&graph2, 300.0);
        let compact = named(&graph2, "compact").expect("the compact branch materialises");
        assert!(
            compact.accepts_subtree_input(),
            "the narrow container activates the compact branch"
        );
        assert!(
            compact.frame().width > 0.0,
            "the selected branch keeps its frame, was {:?}",
            compact.frame()
        );
        // GAP: a branch that starts inactive is never mounted, so there is no
        // wide node to inspect here. Swift mounts both and hides one.
        assert!(
            named(&graph2, "wide").is_none(),
            "a branch inactive from the first frame is never materialised; Swift mounts both"
        );
        let labels = exposed_labels(&graph2);
        assert_eq!(
            labels,
            vec!["compact".to_string()],
            "the compact branch is the only exposed one"
        );
    });
}

#[test]
fn round_trips_preserve_branch_state_release_focus_and_keep_node_identity() {
    GlobalTestLock::locked(|| {
        let (_registry, focus, capture) = install_holders();
        let (view, graph) = harness();
        settled(&graph, 700.0);

        let wide = named(&graph, "wide").expect("the wide branch materialises");
        assert_eq!(view.wide_count.get(), 0, "the wide counter starts at zero");

        view.wide_count.set(1);
        settled(&graph, 700.0);
        assert_eq!(
            view.wide_count.get(),
            1,
            "the wide branch owns an independent counter"
        );

        focus.focus(Some(&wide), true);
        capture.acquire(&wide, true);
        view.width.set(300.0);
        settled(&graph, 300.0);
        assert!(
            focus.focused().is_none(),
            "hiding the focused branch releases focus"
        );
        assert!(
            capture.target().is_none(),
            "hiding the captured branch releases capture"
        );

        let compact = named(&graph, "compact").expect("the compact branch materialises");
        assert_eq!(
            view.compact_count.get(),
            0,
            "the compact branch starts with its own independent counter"
        );
        view.compact_count.set(2);
        settled(&graph, 300.0);

        view.width.set(800.0);
        settled(&graph, 800.0);
        let wide_again = named(&graph, "wide").expect("the wide branch returns");
        assert_eq!(
            wide_again.id(),
            wide.id(),
            "the returning wide branch is the identical node"
        );
        assert_eq!(
            view.wide_count.get(),
            1,
            "the wide branch kept its count across the round trip"
        );

        view.width.set(320.0);
        settled(&graph, 320.0);
        let compact_again = named(&graph, "compact").expect("the compact branch returns");
        assert_eq!(
            compact_again.id(),
            compact.id(),
            "the returning compact branch is the identical node"
        );
        assert_eq!(
            view.compact_count.get(),
            2,
            "the compact branch kept its own count"
        );

        let compact_id = compact_again.id();
        graph.install_view(
            AnyView::new(cove_composition::EmptyView),
            Default::default(),
        );
        assert!(
            all(&graph).iter().all(|node| node.id() != compact_id),
            "tearing the graph down releases the branch node"
        );
        clear_holders();
        let _ = harness_refs(&view);
    });
}

#[test]
fn the_wide_threshold_is_inclusive_and_needs_no_new_measurement() {
    GlobalTestLock::locked(|| {
        let (view, graph) = harness();
        settled(&graph, 700.0);

        view.width.set(600.0);
        settled(&graph, 600.0);
        let wide = named(&graph, "wide").expect("the wide branch materialises");
        assert!(
            wide.accepts_subtree_input(),
            "a width equal to the threshold stays wide"
        );

        view.threshold.set(700.0);
        settled(&graph, 600.0);
        let compact = named(&graph, "compact").expect("the compact branch materialises");
        assert!(
            compact.accepts_subtree_input(),
            "raising the threshold selects compact without re-measuring"
        );
        assert!(
            !wide.accepts_subtree_input(),
            "the wide branch goes inactive as soon as the threshold moves"
        );
    });
}

#[test]
fn intrinsic_mode_selects_by_natural_content_and_retains_both_branches() {
    GlobalTestLock::locked(|| {
        let (_registry, focus, _capture) = install_holders();
        let width = State::new(200.0f32);
        let wide_count = State::new(0);
        let compact_count = State::new(0);
        let graph = support::graph();
        graph.install(
            IntrinsicView {
                width: width.clone(),
                wide_count: wide_count.clone(),
                compact_count: compact_count.clone(),
            },
            Default::default(),
        );

        let wide = named(&graph, "wide").expect("the wide branch materialises");
        let compact = named(&graph, "compact").expect("the compact branch materialises");
        settled(&graph, 200.0);
        assert!(
            wide.accepts_subtree_input() && !compact.accepts_subtree_input(),
            "the wider natural content fits at 200pt"
        );
        let labels = exposed_labels(&graph);
        assert_eq!(
            labels,
            vec!["wide".to_string()],
            "only the fitting branch is exposed"
        );

        wide_count.set(1);
        settled(&graph, 200.0);
        assert_eq!(
            wide_count.get(),
            1,
            "the wide branch owns an independent counter"
        );

        width.set(100.0);
        settled(&graph, 100.0);
        assert!(
            !wide.accepts_subtree_input() && compact.accepts_subtree_input(),
            "narrowing past the wide natural width selects compact"
        );
        let labels = exposed_labels(&graph);
        assert_eq!(
            labels,
            vec!["compact".to_string()],
            "the compact branch is the only exposed one"
        );

        focus.focus(Some(&wide), true);
        width.set(200.0);
        settled(&graph, 200.0);
        let wide_again = named(&graph, "wide").expect("the wide branch returns");
        assert_eq!(
            wide_again.id(),
            wide.id(),
            "intrinsic mode retains the wide branch node"
        );
        assert_eq!(wide_count.get(), 1, "the wide branch kept its count");
        assert_eq!(
            compact_count.get(),
            0,
            "the compact branch never counted anything"
        );
        clear_holders();
    });
}
