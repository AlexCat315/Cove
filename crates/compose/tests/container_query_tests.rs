//! Translation of `Tests/CoveUIComposeTests/ContainerQueryTests.swift`.
//!
//! Behaviour checklist:
//! - the measured content box is published after layout; the first, unmeasured
//!   build receives zero and the next frame publishes the real size
//! - a width or height change republishes, the rebuilt subtree keeps node
//!   identity, and further idle frames do not rebuild
//! - the query reports the parent's allocation, not the branch's intrinsic
//!   width, and excludes padding and border
//! - a row query uses the flex allocation even when the content's intrinsic
//!   width is larger
//! - exactly half a point of change is still published
//! - sub-tolerance jitter does not republish

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::adaptive::ContainerSizeReader;
use cove_composition::{AnyView, TupleView, View, ViewGraph};
use cove_core::scene::{Align, FlexDirection, Justify, Node, NodeTree, Recomposer, Wrap};
use cove_core::{Size, State};
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::{BoxView, LayoutDebugExt};

use support::GlobalTestLock;

fn empty_box() -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(cove_composition::EmptyView),
    )
}

/// The Swift `isApproximatelyEqual(to:)`, a 0.05pt tolerance.
fn approximately(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= 0.05
}

#[derive(Default)]
struct Recorder {
    builds: RefCell<usize>,
    sizes: RefCell<Vec<Size>>,
}

impl Recorder {
    fn content(&self, size: Size) -> AnyView {
        *self.builds.borrow_mut() += 1;
        self.sizes.borrow_mut().push(size);
        AnyView::new(empty_box().debug_name("probe-child"))
    }

    fn build_count(&self) -> usize {
        *self.builds.borrow()
    }

    fn sizes(&self) -> Vec<Size> {
        self.sizes.borrow().clone()
    }
}

/// `Harness` from the Swift suite: a reader inside a sized frame, driven by
/// observed width and height.
struct Harness {
    width: State<f32>,
    height: State<f32>,
    recorder: Rc<Recorder>,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let width = self.width.clone();
        let height = self.height.clone();
        let recorder = self.recorder.clone();
        AnyView::new(
            AnyView::new(ContainerSizeReader::new(move |size: Size| {
                recorder.content(size)
            }))
            .frame_size(width.get(), height.get()),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("width", &self.width),
            cove_composition::DynamicField::state("height", &self.height),
        ]
    }
}

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

fn named(graph: &Rc<ViewGraph>, name: &str) -> Option<Node> {
    let root = graph.tree.borrow().root()?;
    let mut nodes = Vec::new();
    all_nodes(&root, &mut nodes);
    nodes
        .into_iter()
        .find(|node| support::debug_name(node).as_deref() == Some(name))
}

/// One settle cycle: layout publishes the frame, the queued recompose rebuilds
/// with the measured size, layout runs again to leave the graph consistent.
fn settle(graph: &Rc<ViewGraph>, width: f32, height: f32) {
    graph.recomposer.commit_all();
    graph.compute_layout(width, height);
    graph.recomposer.commit_all();
    graph.compute_layout(width, height);
}

#[test]
fn reports_the_measured_content_box_after_layout() {
    GlobalTestLock::locked(|| {
        let recorder = Rc::new(Recorder::default());
        let harness = Harness {
            width: State::new(300.0),
            height: State::new(100.0),
            recorder: recorder.clone(),
        };
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(harness, Default::default());
        settle(&graph, 300.0, 100.0);

        let sizes = recorder.sizes();
        assert_eq!(recorder.build_count(), 2, "the reader builds twice");
        assert_eq!(sizes.len(), 2, "each build reports one size");
        assert_eq!(sizes[0], Size::default(), "the unmeasured build sees zero");
        assert!(
            approximately(sizes[1].width, 300.0),
            "the measured width is published (got {})",
            sizes[1].width
        );
        assert!(
            approximately(sizes[1].height, 100.0),
            "the measured height is published (got {})",
            sizes[1].height
        );
        assert!(
            !graph.recomposer.has_pending(),
            "the settle cycle drains the queue"
        );
    });
}

#[test]
fn republishes_on_resize_keeps_identity_and_stays_idle() {
    GlobalTestLock::locked(|| {
        let recorder = Rc::new(Recorder::default());
        let width = State::new(300.0);
        let height = State::new(100.0);
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            Harness {
                width: width.clone(),
                height: height.clone(),
                recorder: recorder.clone(),
            },
            Default::default(),
        );
        settle(&graph, 300.0, 100.0);
        let child = named(&graph, "probe-child").expect("probe child");

        width.set(120.0);
        height.set(240.0);
        settle(&graph, 120.0, 240.0);

        let last = *recorder.sizes().last().expect("a published size");
        assert!(
            approximately(last.width, 120.0),
            "the new width is published (got {})",
            last.width
        );
        assert!(
            approximately(last.height, 240.0),
            "the new height is published (got {})",
            last.height
        );
        assert!(!graph.recomposer.has_pending());

        let resized = named(&graph, "probe-child").expect("probe child survives");
        assert_eq!(
            resized.id(),
            child.id(),
            "a stable subtree keeps its node across settle cycles"
        );

        let builds = recorder.build_count();
        for _ in 0..3 {
            graph.recomposer.commit_all();
            graph.compute_layout(120.0, 240.0);
        }
        assert_eq!(
            recorder.build_count(),
            builds,
            "idle frames do not rebuild the content"
        );
    });
}

#[test]
fn queries_the_parent_allocation_and_excludes_insets() {
    GlobalTestLock::locked(|| {
        let recorder = Rc::new(Recorder::default());
        let read_recorder = recorder.clone();
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(
                        AnyView::new(ContainerSizeReader::new(move |size: Size| {
                            read_recorder.content(size)
                        }))
                        .padding_edges(12.0, 8.0)
                        .frame_height(100.0)
                        .debug_name("inset-reader"),
                    ),
                ))
                .frame_width(400.0),
            ),
            Default::default(),
        );

        let probe = named(&graph, "inset-reader").expect("inset reader");
        if let Some(layout) = probe.layout_node() {
            for edge in [
                cove_core::scene::Edge::Top,
                cove_core::scene::Edge::Bottom,
                cove_core::scene::Edge::Left,
                cove_core::scene::Edge::Right,
            ] {
                layout.set_border(2.0, edge);
            }
        }
        settle(&graph, 900.0, 300.0);

        let last = *recorder.sizes().last().expect("a published size");
        assert_eq!(
            (last.width, last.height),
            (372.0, 80.0),
            "the query excludes horizontal and vertical insets"
        );
        assert!(!graph.recomposer.has_pending());
    });
}

#[test]
fn row_queries_use_the_flex_allocation() {
    GlobalTestLock::locked(|| {
        let recorder = Rc::new(Recorder::default());
        let read_recorder = recorder.clone();
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            AnyView::new(empty_box())
                                .frame_size(100.0, 40.0)
                                .flex_with(0.0, 0.0, None),
                        ),
                        AnyView::new(
                            AnyView::new(ContainerSizeReader::new(move |size: Size| {
                                AnyView::new(read_recorder.content(size)).frame_size(950.0, 40.0)
                            }))
                            .flex_with(1.0, 1.0, Some(0.0)),
                        ),
                    ])),
                ))
                .frame_width(400.0),
            ),
            Default::default(),
        );
        settle(&graph, 900.0, 300.0);

        let last = *recorder.sizes().last().expect("a published size");
        assert_eq!(
            last.width, 300.0,
            "the row allocation wins over the 950pt intrinsic width"
        );
        assert!(!graph.recomposer.has_pending());
    });
}

#[test]
fn exactly_half_a_point_is_published() {
    GlobalTestLock::locked(|| {
        let recorder = Rc::new(Recorder::default());
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            Harness {
                width: State::new(300.0),
                height: State::new(100.0),
                recorder: recorder.clone(),
            },
            Default::default(),
        );
        settle(&graph, 300.0, 100.0);

        let root = graph.tree.borrow().root().expect("root");
        let mut nodes = Vec::new();
        all_nodes(&root, &mut nodes);
        let probe = nodes
            .into_iter()
            .find(|node| node.layout_did_update().is_some())
            .expect("the reader installs a layout-did-update hook");

        probe.set_frame(cove_core::Rect::new(0.0, 0.0, 300.5, probe.frame().height));
        let hook = probe.layout_did_update().expect("hook");
        hook(&probe);
        graph.recomposer.commit_all();

        let last = *recorder.sizes().last().expect("a published size");
        assert_eq!(
            last.width, 300.5,
            "half a point meets the publication threshold"
        );
        assert!(!graph.recomposer.has_pending());
    });
}

#[test]
fn sub_tolerance_jitter_does_not_republish() {
    GlobalTestLock::locked(|| {
        let recorder = Rc::new(Recorder::default());
        let width = State::new(300.0);
        let height = State::new(100.0);
        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            Harness {
                width: width.clone(),
                height: height.clone(),
                recorder: recorder.clone(),
            },
            Default::default(),
        );
        settle(&graph, 300.0, 100.0);

        width.set(300.4);
        settle(&graph, 300.4, 100.0);
        assert!(
            recorder
                .sizes()
                .iter()
                .all(|size| !approximately(size.width, 300.4)),
            "a 0.4pt width jitter is never published"
        );
        assert!(
            approximately(recorder.sizes().last().expect("size").width, 300.0),
            "the last published width stays at 300"
        );
        assert!(!graph.recomposer.has_pending());

        height.set(100.3);
        settle(&graph, 300.4, 100.3);
        assert!(
            recorder
                .sizes()
                .iter()
                .all(|size| !approximately(size.height, 100.3)),
            "a 0.3pt height jitter is never published"
        );
        assert!(
            approximately(recorder.sizes().last().expect("size").height, 100.0),
            "the last published height stays at 100"
        );
        assert!(!graph.recomposer.has_pending());
    });
}
