//! Translation of `Tests/CoveUIComposeTests/RouteAndPreloadTests.swift`.
//!
//! Behaviour checklist:
//! - only the active route is materialised, selecting another route swaps the
//!   page without re-running the shell body, and returning rebuilds the page
//! - an explicitly memoised child skips equal parent updates while a local
//!   state write, an input change and a text-scale change each rebuild it
//! - a programmatically focused sidebar row survives a hover motion: it stays
//!   the only roving tab stop and selection does not change
//! - `VirtualWindow` keeps a refill buffer at both ends, responds to viewport
//!   size changes, and ignores empty or hidden content
//! - buffered wheel motion does not rebuild mounted rows for a 1pt step, a
//!   larger scroll only constructs entering rows, and returning to an old
//!   offset still consumes a later refill there (fixed, measured and estimated
//!   row heights)
//!
//! Named differences from the Swift suite:
//! - Swift identifies a sidebar row as `sidebar-item-<id>` via `debugName`;
//!   the Rust `Sidebar` does not stamp that debug name, so the case resolves
//!   the row by its accessibility label `"Item 3"` and role `Button`. The
//!   focused row, tab-stop count and selected state asserted afterwards are
//!   unchanged.
//! - Swift parameterises one `buffered scroll` case over `[0, 1, 2]`; Rust has
//!   no argumentised tests, so the three row-height strategies are three
//!   `#[test]`s sharing one scenario helper.

mod support;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use cove_composition::MemoizedView;
use cove_composition::{AnyView, LayoutDebugExt, PrimitiveView, View, ViewGraph};
use cove_core::scene::{AccessibilityRole, LayoutNode, Node};
use cove_core::{Binding, Point, Size, State};
use cove_platform::{
    InputEvent, MouseMotionEvent, MouseWheelEvent, ScrollDevice, ScrollInput, ScrollUnit,
};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::layout::sidebar::{Sidebar, SidebarItem, SidebarSection};
use cove_ui_compose::primitives::layout::virtualization::{VirtualStack, VirtualWindow};
use cove_ui_compose::primitives::navigation::route_outlet::RouteOutlet;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::{TextScaler, TextScalerViewExt};

use support::InteractiveScene;

#[derive(Default)]
struct Counts {
    shell: usize,
    pages: Vec<i32>,
    rows: HashMap<usize, usize>,
    child: usize,
}

struct Routed {
    selection: State<i32>,
    counts: Rc<RefCell<Counts>>,
}

impl View for Routed {
    fn body(&self) -> AnyView {
        self.counts.borrow_mut().shell += 1;
        let counts = self.counts.clone();
        AnyView::new(RouteOutlet::new(self.selection.binding(), move |route| {
            counts.borrow_mut().pages.push(route);
            Text::new(format!("Page {route}")).debug_name(&format!("route-{route}"))
        }))
    }
}

struct MemoHarness {
    parent: State<i32>,
    input: State<i32>,
    scale: State<f32>,
    local: State<i32>,
    counts: Rc<RefCell<Counts>>,
}

impl View for MemoHarness {
    fn body(&self) -> AnyView {
        let input = self.input.get();
        let counts = self.counts.clone();
        let local = self.local.clone();
        AnyView::new(
            MemoizedView::new(input, move || MemoChild {
                input,
                local: local.clone(),
                counts: counts.clone(),
            })
            .text_scaler(TextScaler::new(self.scale.get())),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("parent", &self.parent),
            cove_composition::DynamicField::state("input", &self.input),
            cove_composition::DynamicField::state("scale", &self.scale),
        ]
    }
}

struct MemoChild {
    input: i32,
    local: State<i32>,
    counts: Rc<RefCell<Counts>>,
}

impl View for MemoChild {
    fn body(&self) -> AnyView {
        self.counts.borrow_mut().child += 1;
        let local = self.local.clone();
        AnyView::new(
            Button::new(
                Text::new(format!("{}:{}", self.input, self.local.get())),
                move || local.set(local.get() + 1),
            )
            .debug_name("memo-child"),
        )
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state("local", &self.local)]
    }
}

struct Row {
    index: usize,
    counts: Rc<RefCell<Counts>>,
}

impl PrimitiveView for Row {
    fn make_node(&self) -> Node {
        Node::new()
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
}
cove_composition::impl_primitive_view!(Row);

fn row_view(index: usize, counts: Rc<RefCell<Counts>>) -> AnyView {
    counts.borrow_mut().rows.entry(index).or_insert(0);
    *counts.borrow_mut().rows.get_mut(&index).expect("inserted") += 1;
    AnyView::new(Row { index, counts })
        .frame_height(10.0)
        .debug_name(&format!("preload-row-{index}"))
}

#[test]
fn only_the_active_route_is_created() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let counts = Rc::new(RefCell::new(Counts::default()));
        let selection = State::new(0_i32);
        scene.install(Routed {
            selection: selection.clone(),
            counts: counts.clone(),
        });

        let before = counts.borrow().shell;
        assert_eq!(
            counts.borrow().pages,
            vec![0],
            "installing materialises only route 0"
        );
        let first = scene.named("route-0").expect("route 0 is mounted");

        selection.set(1);
        scene.settle(5, 1.0);
        assert_eq!(
            counts.borrow().shell,
            before,
            "swapping the route leaves the shell body untouched"
        );
        assert_eq!(
            counts.borrow().pages,
            vec![0, 1],
            "selecting route 1 materialises it once"
        );
        assert!(
            scene.named("route-0").is_none(),
            "the previous route is torn down"
        );
        assert!(
            scene.named("route-1").is_some(),
            "the selected route is mounted"
        );

        selection.set(0);
        scene.settle(5, 1.0);
        let rebuilt = scene.named("route-0").expect("route 0 is mounted again");
        assert_ne!(
            rebuilt, first,
            "returning to route 0 rebuilds the page instead of reusing the old node"
        );
    });
}

#[test]
fn explicit_memoization_skips_equal_parent_updates_but_keeps_local_invalidation() {
    InteractiveScene::run(800.0, 600.0, |scene| {
        let counts = Rc::new(RefCell::new(Counts::default()));
        let harness = MemoHarness {
            parent: State::new(0_i32),
            input: State::new(1_i32),
            scale: State::new(1.0_f32),
            local: State::new(0_i32),
            counts: counts.clone(),
        };
        let parent = harness.parent.clone();
        let input = harness.input.clone();
        let scale = harness.scale.clone();
        scene.install(harness);

        let before = counts.borrow().child;
        parent.set(parent.get() + 1);
        scene.settle(5, 1.0);
        assert_eq!(
            counts.borrow().child,
            before,
            "an equal parent update does not rebuild the memoised child"
        );

        let child = scene
            .named("memo-child")
            .expect("the memo child is mounted");
        scene.click(&child, 1);
        assert!(
            counts.borrow().child > before,
            "a local state write still invalidates the memoised child"
        );

        input.set(2);
        scene.settle(5, 1.0);
        assert!(
            scene.labelled("2:1", None).is_some(),
            "a new memo key rebuilds the child and preserves its local state"
        );

        let before_environment = counts.borrow().child;
        scale.set(2.0);
        scene.settle(5, 1.0);
        assert!(
            counts.borrow().child > before_environment,
            "a text-scale change rebuilds the memoised child"
        );
    });
}

#[test]
fn sidebar_focus_survives_a_hover_without_changing_selection() {
    InteractiveScene::run(260.0, 400.0, |scene| {
        let sidebar = Sidebar::new(
            Binding::constant(Some(0_i32)),
            vec![SidebarSection::new(
                "main",
                (0..6)
                    .map(|index| SidebarItem::new(index, format!("Item {index}")))
                    .collect(),
            )],
        );
        scene.install(sidebar.frame_size(260.0, 400.0));

        let row = scene
            .labelled("Item 3", Some(AccessibilityRole::Button))
            .expect("sidebar item 3 is mounted");
        scene.focus.focus(Some(&row), true);
        scene.settle(5, 1.0);
        let frame = row.absolute_frame();
        scene
            .dispatcher()
            .dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                x: frame.x + frame.width / 2.0,
                y: frame.y + frame.height / 2.0,
                delta_x: 0.0,
                delta_y: 0.0,
                kind: Default::default(),
            }));
        scene.settle(5, 1.0);

        assert!(row.is_tab_stop(), "the focused row stays a tab stop");
        assert!(
            !row.accessibility().expect("semantics").state.is_selected,
            "hovering does not select the focused row"
        );
        let stops = scene
            .all(|node| node.attachment::<u64>("__roving.item").is_some() && node.is_tab_stop())
            .len();
        assert_eq!(stops, 1, "exactly one roving item remains a tab stop");
    });
}

#[test]
fn virtual_window_refill_thresholds_and_preparation() {
    let size = Size::new(200.0, 1_000.0);
    let mut window = VirtualWindow::default();
    window.record(180..320, 200..300, 10_000, size);

    assert!(
        !window.needs_refill(200..300, size),
        "the recorded window needs no refill"
    );
    assert!(
        !window.needs_refill(205..305, size),
        "a small shift stays inside the buffer"
    );
    assert!(
        window.needs_refill(211..311, size),
        "crossing the trailing guard refreshes"
    );
    assert!(
        window.needs_refill(189..289, size),
        "crossing the leading guard refreshes"
    );
    assert!(
        window.needs_refill(200..300, Size::new(201.0, 1_000.0)),
        "a viewport size change always refreshes"
    );

    let (offset, height) = VirtualWindow::preparation(2_000.0, 1_000.0, 0.3);
    assert!(
        (offset - 1_850.0).abs() < 0.01 && (height - 1_300.0).abs() < 0.01,
        "30% preload widens the prepared window (offset {offset}, height {height})"
    );
    let (_, unpreloaded) = VirtualWindow::preparation(2_000.0, 1_000.0, 0.0);
    assert_eq!(
        unpreloaded, 1_000.0,
        "no preload prepares exactly one viewport"
    );

    window.record(0..0, 0..0, 0, size);
    assert!(
        !window.needs_refill(0..0, size),
        "an empty data set never refreshes"
    );
    let hidden = Size::new(200.0, 0.0);
    window.record(0..0, 0..0, 10, hidden);
    assert!(
        !window.needs_refill(0..0, hidden),
        "a zero-height viewport never refreshes"
    );
}

fn wheel_event(distance: f32) -> MouseWheelEvent {
    MouseWheelEvent {
        x: 0.0,
        y: -distance,
        mouse_x: Some(50.0),
        mouse_y: Some(50.0),
        input: ScrollInput {
            unit: ScrollUnit::Points,
            device: ScrollDevice::Continuous,
            ..Default::default()
        },
    }
}

fn buffered_scroll(stack: AnyView, counts: Rc<RefCell<Counts>>) {
    InteractiveScene::run(200.0, 1_000.0, |scene| {
        scene.install(stack.frame_size(200.0, 1_000.0));
        let scroll = scene
            .first(|node| node.is_scroll_viewport())
            .expect("the stack mounts a scroll viewport");
        let dispatcher = scene.dispatcher();

        let wheel = |distance: f32| {
            dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(distance)));
        };

        wheel(2_000.0);
        scene.settle(5, 1.0);
        let before = counts.borrow().rows.clone();
        let previous_offset = scroll.content_offset().y;
        let mounted = scene
            .all(|node| {
                support::debug_name(node).is_some_and(|name| name.starts_with("preload-row-"))
            })
            .len();
        assert!(
            (130..=136).contains(&mounted),
            "a 1_000pt viewport mounts the visible rows plus the preload buffer, got {mounted}"
        );

        wheel(1.0);
        scene.settle(5, 1.0);
        assert_eq!(
            scroll.content_offset().y,
            previous_offset + 1.0,
            "a 1pt wheel step advances the offset exactly"
        );
        assert!(
            counts.borrow().rows == before,
            "a 1pt step stays inside the buffer and rebuilds no row"
        );

        wheel(100.0);
        scene.settle(5, 1.0);
        let kept: Vec<usize> = before
            .keys()
            .copied()
            .filter(|index| (200..300).contains(index))
            .collect();
        assert!(!kept.is_empty(), "the original visible window is known");
        for index in kept {
            assert_eq!(
                counts.borrow().rows.get(&index),
                before.get(&index),
                "row {index} that stayed visible was not reconstructed"
            );
        }
        let previous_max = before.keys().max().copied().unwrap_or(0);
        assert!(
            counts
                .borrow()
                .rows
                .keys()
                .any(|index| *index > previous_max),
            "the refill constructed rows entering from below"
        );

        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(2_000.0)));
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(-2_000.0)));
        scene.settle(5, 1.0);
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(2_000.0)));
        scene.settle(5, 1.0);
        let first_visible = (scroll.content_offset().y / 10.0) as usize;
        assert!(
            scene
                .named(&format!("preload-row-{}", first_visible + 1))
                .is_some(),
            "returning to the old offset still refills its window"
        );
    });
}

#[test]
fn buffered_scroll_keeps_fixed_rows() {
    let counts = Rc::new(RefCell::new(Counts::default()));
    let rows = counts.clone();
    let stack = VirtualStack::<usize>::fixed(10.0)
        .overscan(0)
        .build((0..10_000).collect(), move |index| {
            row_view(*index, rows.clone())
        });
    buffered_scroll(stack, counts);
}

#[test]
fn buffered_scroll_keeps_measured_rows() {
    let counts = Rc::new(RefCell::new(Counts::default()));
    let rows = counts.clone();
    let stack = VirtualStack::<usize>::variable(|_, _, _| 10.0)
        .overscan(0)
        .build((0..10_000).collect(), move |index| {
            row_view(*index, rows.clone())
        });
    buffered_scroll(stack, counts);
}

#[test]
fn buffered_scroll_keeps_estimated_rows() {
    let counts = Rc::new(RefCell::new(Counts::default()));
    let rows = counts.clone();
    let stack = VirtualStack::<usize>::estimated(10.0)
        .overscan(0)
        .build((0..10_000).collect(), move |index| {
            row_view(*index, rows.clone())
        });
    buffered_scroll(stack, counts);
}
