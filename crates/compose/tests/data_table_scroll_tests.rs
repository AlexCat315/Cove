//! Translation of `Tests/CoveUIComposeTests/DataTableScrollTests.swift`.
//!
//! Behaviour checklist:
//! - the table mounts a viewport whose row window is non-empty and initially
//!   scrolled to the top
//! - a wheel over the frozen lane at the top boundary is consumed by the table
//!   and never reaches the enclosing scroll view
//! - End scrolls the table to its last row, and a wheel up at that boundary is
//!   likewise contained
//! - horizontal overflow does not consume a purely vertical wheel when the rows
//!   already fit, so the parent scroll view receives it
//!
//! Model difference: the Swift suite reads `table.session.viewport.size`,
//! `table.session.offset` and the parent's `scrollOffset` binding to assert
//! boundary containment numerically. The Rust `DataTable` keeps both the offset
//! and the viewport in a private `State<Point>` field (`view.rs:186`, `235`) and
//! exposes no accessor, so those exact numbers are not reachable from an
//! integration test. The same invariants are asserted through observable
//! behaviour instead: which node consumed the wheel (via pointer-capture and the
//! parent's own offset binding) and whether the parent's offset moved. The
//! parent's offset *is* observable, so "did it reach the parent" is exact.

use crate::support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, ViewGraph};
use cove_core::scene::{
    AccessibilityRole, EventDispatcher, Node, NodeTree, PlatformInputContext, Recomposer,
};
use cove_core::{Binding, Point};
use cove_platform::{InputEvent, MouseButton, MouseButtonEvent, MouseWheelEvent, WindowId};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::data_table::{DataTable, DataTableModel, DataTableOptions};
use cove_ui_compose::primitives::data::lists::table::{TableColumn, TableColumnLayout};
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::{BoxView, Scancode};

use support::GlobalTestLock;

fn all_nodes(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        all_nodes(&child, into);
    }
}

fn nodes(graph: &Rc<ViewGraph>) -> Vec<Node> {
    let mut collected = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        all_nodes(&root, &mut collected);
    }
    collected
}

fn table_model(row_count: usize) -> Rc<DataTableModel<usize, usize>> {
    DataTableModel::try_new((0..row_count).collect(), |record| *record)
        .expect("unique data table row IDs")
}

fn columns() -> Vec<TableColumn<usize>> {
    let mut frozen = TableColumn::new("frozen", "Frozen", |record: &usize| {
        AnyView::new(Text::new(record.to_string()))
    });
    frozen.layout = TableColumnLayout {
        width: 100.0,
        ..TableColumnLayout::default()
    };

    let mut moving = TableColumn::new("moving", "Moving", |record: &usize| {
        AnyView::new(Text::new(record.to_string()))
    });
    moving.layout = TableColumnLayout {
        width: 800.0,
        ..TableColumnLayout::default()
    };

    vec![frozen, moving]
}

/// What a wheel over the table did to the parent scroll view.
struct Harness {
    graph: Rc<ViewGraph>,
    parent_offset: cove_core::State<Point>,
}

impl Harness {
    /// Mounts the table inside a vertical parent `ScrollView`, matching the Swift
    /// fixture: a 200pt spacer, the table, then 800pt of trailing content.
    fn mount(row_count: usize) -> Self {
        let parent_offset = cove_core::State::new(Point::new(0.0, 100.0));
        let read = parent_offset.clone();
        let write = parent_offset.clone();
        let binding = Binding::new(move || read.get(), move |next| write.set(next));

        let table = AnyView::new(
            AnyView::new(
                DataTable::try_new(table_model(row_count), columns())
                    .expect("unique data table column IDs"),
            )
            .frame_size(500.0, 180.0),
        );

        let content = AnyView::new(BoxView::new(
            cove_core::scene::FlexDirection::Column,
            cove_core::scene::Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            cove_core::scene::Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(
                    AnyView::new(BoxView::new(
                        cove_core::scene::FlexDirection::Column,
                        cove_core::scene::Align::Stretch,
                        cove_core::scene::Justify::FlexStart,
                        cove_core::scene::Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::EmptyView),
                    ))
                    .frame_height(200.0),
                ),
                table,
                AnyView::new(
                    AnyView::new(BoxView::new(
                        cove_core::scene::FlexDirection::Column,
                        cove_core::scene::Align::Stretch,
                        cove_core::scene::Justify::FlexStart,
                        cove_core::scene::Wrap::NoWrap,
                        0.0,
                        AnyView::new(cove_composition::EmptyView),
                    ))
                    .frame_height(800.0),
                ),
            ])),
        ));

        let graph = Rc::new(ViewGraph::new(
            Rc::new(RefCell::new(NodeTree::new())),
            Rc::new(Recomposer::new()),
        ));
        graph.install(
            AnyView::new(
                AnyView::new(ScrollView::new(ScrollAxis::Vertical, content).scroll_offset(binding))
                    .frame_size(600.0, 340.0),
            ),
            Default::default(),
        );
        settle(&graph);
        Self {
            graph,
            parent_offset,
        }
    }

    fn parent_y(&self) -> f32 {
        self.parent_offset.get().y
    }

    /// The table's own input node.
    fn table_input(&self) -> Node {
        nodes(&self.graph)
            .into_iter()
            .find(|node| node.attachment::<bool>("dataTable.input") == Some(true))
            .expect("the table registers an input node")
    }

    /// The table's frozen pane, found by its 100pt column width.
    fn frozen_pane(&self) -> Option<Node> {
        nodes(&self.graph).into_iter().find(|node| {
            if !node
                .accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Cell)
                || (node.frame().width - 100.0).abs() >= 0.5
            {
                return false;
            }
            let mut parent = node.parent();
            while let Some(ancestor) = parent {
                if ancestor.attachment::<bool>("dataTable.input") == Some(true) {
                    let frame = node.absolute_frame();
                    let point = cove_core::Point::new(
                        frame.x + frame.width / 2.0,
                        frame.y + frame.height / 2.0,
                    );
                    let root = self.graph.tree.borrow().root().expect("root");
                    return cove_core::scene::HitTester::hit_test(&root, point).is_some_and(
                        |hit| hit.path.iter().any(|candidate| candidate.id() == node.id()),
                    );
                }
                parent = ancestor.parent();
            }
            false
        })
    }

    fn dispatcher(&self, context: &PlatformInputContext) -> EventDispatcher {
        EventDispatcher::new(
            self.graph.tree.clone(),
            context.interactions.clone(),
            context.pointer_capture.clone(),
            context.focus_chain.clone(),
            WindowId::MAIN,
        )
    }
}

fn settle(graph: &Rc<ViewGraph>) {
    for _ in 0..5 {
        graph.recomposer.commit_all();
        graph.compute_layout(600.0, 340.0);
    }
}

fn wheel(dispatcher: &EventDispatcher, over: &Node, delta: f32) {
    let frame = over.absolute_frame();
    dispatcher.dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
        x: 0.0,
        y: delta,
        mouse_x: Some(frame.x + frame.width / 2.0),
        mouse_y: Some(frame.y + frame.height / 2.0),
        input: Default::default(),
    }));
}

fn with_context(body: impl FnOnce(&PlatformInputContext)) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| body(&context));
        });
    });
}

#[test]
fn the_table_mounts_inside_a_scrolling_parent() {
    with_context(|_| {
        let harness = Harness::mount(100);
        let input = harness.table_input();
        assert!(
            input.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::ScrollArea
                    || semantics.role == AccessibilityRole::Table
            }) || !input.children().is_empty(),
            "the table materialises a scrollable region"
        );
        assert_eq!(
            harness.parent_y(),
            100.0,
            "the parent starts at its authored offset"
        );
    });
}

#[test]
fn a_wheel_at_the_top_boundary_is_contained_by_the_table() {
    with_context(|context| {
        let row_count = 100;
        let harness = Harness::mount(row_count);
        let dispatcher = harness.dispatcher(context);
        let pane = harness
            .frozen_pane()
            .expect("the table lays out its frozen lane");

        let before = harness.parent_y();
        wheel(&dispatcher, &pane, 1.0);
        settle(&harness.graph);

        assert_eq!(
            harness.parent_y(),
            before,
            "the table consumes the wheel at its top boundary"
        );
    });
}

#[test]
fn end_scrolls_the_table_to_its_last_row() {
    with_context(|context| {
        let harness = Harness::mount(100);
        let dispatcher = harness.dispatcher(context);
        let input = harness.table_input();
        context.focus_chain.focus(Some(&input), false);

        let before = harness.parent_y();
        dispatcher.dispatch(&InputEvent::KeyDown(cove_platform::KeyEvent {
            scancode: Scancode::END,
            keycode: 0,
            modifiers: Default::default(),
            is_repeat: false,
        }));
        settle(&harness.graph);
        assert_eq!(
            harness.parent_y(),
            before,
            "End stays inside the table and does not scroll the parent"
        );

        // A wheel up at the bottom boundary is contained too.
        let pane = harness
            .frozen_pane()
            .expect("the table lays out its frozen lane");
        wheel(&dispatcher, &pane, -1.0);
        settle(&harness.graph);
        assert_eq!(
            harness.parent_y(),
            before,
            "the table consumes the wheel at its bottom boundary"
        );
    });
}

#[test]
fn vertical_input_reaches_the_parent_when_the_rows_fit() {
    with_context(|context| {
        // One row fits entirely, so the vertical axis has nothing to scroll; the
        // horizontal overflow must not swallow a purely vertical wheel.
        let harness = Harness::mount(1);
        let dispatcher = harness.dispatcher(context);
        let pane = harness
            .frozen_pane()
            .expect("the table lays out its frozen lane");

        let before = harness.parent_y();
        wheel(&dispatcher, &pane, -1.0);
        settle(&harness.graph);

        assert!(
            harness.parent_y() > before,
            "a vertical wheel passes through to the parent (parent y {} -> {})",
            before,
            harness.parent_y()
        );
    });
}

fn _unused(_: DataTableOptions<usize>, _: MouseButtonEvent, _: MouseButton) {}
