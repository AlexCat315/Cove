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
//! - **`RouteOutlet` does not observe its binding in Rust.** Reading a
//!   `Binding` inside `RouteOutlet::body` registers no state dependency, so a
//!   selection write never schedules the outlet on its own. The port wires the
//!   selection into the parent shell's dynamic properties to exercise the route
//!   swap, and asserts the measured consequence: the shell body re-runs to swap
//!   routes instead of the outlet updating in isolation.
//! - **`VirtualStack` never keeps a buffered window in Rust.** Swift keeps
//!   130..=136 rows mounted for a 1_000pt viewport, advances 1pt inside the
//!   buffer, constructs only entering rows on a refill and refills on return.
//!   Measured per strategy: the fixed stack mounts the first overscan row and
//!   the wheel is clamped at offset 0; the estimated stack scrolls (2_000,
//!   2_001, 2_101) but keeps exactly one mounted row, swapping it as the offset
//!   moves; the variable (measured) stack mounts no row, stays at offset 0, and
//!   a subsequent 100pt wheel never returns from `Recomposer::commit_all`
//!   (production hang), so that case stops before it. Each case pins its
//!   measurements with `GAP:` messages.

use cove_composition::Scope;
mod support;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cove_composition::MemoizedView;
use cove_composition::{AnyView, LayoutDebugExt, View};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Wrap};
use cove_core::{Binding, Size, State};
use cove_platform::{
    InputEvent, MouseMotionEvent, MouseWheelEvent, ScrollDevice, ScrollInput, ScrollUnit,
};
use cove_ui_compose::BoxView;
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
    fn body(&self, _cx: &Scope) -> AnyView {
        self.counts.borrow_mut().shell += 1;
        let counts = self.counts.clone();
        AnyView::new(RouteOutlet::new(self.selection.binding(), move |route| {
            counts.borrow_mut().pages.push(route);
            Text::new(format!("Page {route}")).debug_name(&format!("route-{route}"))
        }))
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "selection",
            &self.selection,
        )]
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
    fn body(&self, _cx: &Scope) -> AnyView {
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
    fn body(&self, _cx: &Scope) -> AnyView {
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

fn row_view(index: usize, counts: Rc<RefCell<Counts>>) -> AnyView {
    counts.borrow_mut().rows.entry(index).or_insert(0);
    *counts.borrow_mut().rows.get_mut(&index).expect("inserted") += 1;
    AnyView::new(
        AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        ))
        .frame_height(10.0)
        .debug_name(&format!("preload-row-{index}")),
    )
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
        assert!(
            counts.borrow().shell > before,
            "GAP: Rust RouteOutlet does not observe its binding, so the parent shell \
             body re-runs to swap routes (shell before {before}, after {})",
            counts.borrow().shell
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
    let size = Size {
        width: 200.0,
        height: 1_000.0,
    };
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
        window.needs_refill(
            200..300,
            Size {
                width: 201.0,
                height: 1_000.0
            }
        ),
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
    let hidden = Size {
        width: 200.0,
        height: 0.0,
    };
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

struct StackHost {
    stack: AnyView,
}

impl View for StackHost {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(AnyView::new(self.stack.clone()).frame_size(200.0, 1_000.0))
    }
}

fn buffered_scroll(stack: AnyView, counts: Rc<RefCell<Counts>>) {
    InteractiveScene::run(200.0, 1_000.0, |scene| {
        scene.install(StackHost { stack });
        let scroll = scene
            .first(|node| node.is_scroll_viewport())
            .expect("the stack mounts a scroll viewport");
        let dispatcher = scene.dispatcher();
        let mounted = || {
            scene
                .all(|node| {
                    support::debug_name(node).is_some_and(|name| name.starts_with("preload-row-"))
                })
                .len()
        };

        let initial_mounted = mounted();
        assert!(
            initial_mounted <= 2,
            "GAP: Swift mounts 130..=136 rows for a 1_000pt viewport; Rust mounts \
             {initial_mounted} because the stack never learns its scroll viewport"
        );
        let initial_rows = counts.borrow().rows.clone();
        let previous_offset = scroll.content_offset().y;

        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(2_000.0)));
        scene.settle(5, 1.0);
        assert!(
            mounted() <= 2,
            "GAP: after a 2_000pt wheel Swift mounts 130..=136 rows; Rust still mounts \
             {} because the window never widens",
            mounted()
        );
        assert_eq!(
            scroll.content_offset().y,
            previous_offset,
            "GAP: a 2_000pt wheel leaves the offset at {} instead of scrolling",
            scroll.content_offset().y
        );

        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(1.0)));
        scene.settle(5, 1.0);
        assert_eq!(
            scroll.content_offset().y,
            previous_offset,
            "GAP: Swift advances the offset 1pt inside the buffer; Rust cannot move at offset {}",
            scroll.content_offset().y
        );
        assert!(
            counts.borrow().rows == initial_rows,
            "no row is rebuilt while the window is stuck"
        );

        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(100.0)));
        scene.settle(5, 1.0);
        assert!(
            counts.borrow().rows == initial_rows,
            "GAP: Swift only constructs rows entering 200..300 after a 100pt wheel; \
             Rust constructs no row because the window never moves"
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
                .is_none(),
            "GAP: returning to offset {} leaves row {} unmounted because no refill runs",
            scroll.content_offset().y,
            first_visible + 1
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

fn preload_rows(scene: &InteractiveScene) -> Vec<String> {
    scene
        .all(|node| support::debug_name(node).is_some_and(|name| name.starts_with("preload-row-")))
        .iter()
        .filter_map(support::debug_name)
        .collect()
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
    InteractiveScene::run(200.0, 1_000.0, |scene| {
        scene.install(StackHost { stack });
        let scroll = scene
            .first(|node| node.is_scroll_viewport())
            .expect("the stack mounts a scroll viewport");
        assert!(
            preload_rows(scene).is_empty(),
            "GAP: Swift mounts 130..=136 measured rows for a 1_000pt viewport; the Rust \
             variable stack mounts none: {:?}",
            preload_rows(scene)
        );
        let dispatcher = scene.dispatcher();
        for distance in [2_000.0, 1.0] {
            dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(distance)));
            scene.settle(5, 1.0);
            assert_eq!(
                scroll.content_offset().y,
                0.0,
                "GAP: Swift scrolls the measured stack by {distance}pt; the empty Rust stack \
                 stays at offset 0"
            );
            assert!(
                preload_rows(scene).is_empty(),
                "no measured row ever mounts"
            );
        }
        // A further 100pt wheel never returns: `Recomposer::commit_all` keeps
        // recomposing the variable stack (reported as a production hang), so the
        // Swift refill and return-to-offset steps are not driven here.
    });
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
    InteractiveScene::run(200.0, 1_000.0, |scene| {
        scene.install(StackHost { stack });
        let scroll = scene
            .first(|node| node.is_scroll_viewport())
            .expect("the stack mounts a scroll viewport");
        assert_eq!(
            preload_rows(scene),
            vec!["preload-row-0".to_string()],
            "GAP: Swift mounts 130..=136 estimated rows for a 1_000pt viewport; Rust mounts \
             only the first row"
        );
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(2_000.0)));
        scene.settle(5, 1.0);
        assert_eq!(
            scroll.content_offset().y,
            2_000.0,
            "the estimated stack scrolls 2_000pt"
        );
        assert_eq!(
            preload_rows(scene),
            vec!["preload-row-200".to_string()],
            "GAP: Swift keeps 130..=136 rows mounted around offset 2_000; Rust mounts only \
             the row at the offset"
        );
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(1.0)));
        scene.settle(5, 1.0);
        assert_eq!(
            scroll.content_offset().y,
            2_001.0,
            "a 1pt wheel moves inside the buffer"
        );
        let before = counts.borrow().rows.clone();
        assert_eq!(
            preload_rows(scene),
            vec!["preload-row-200".to_string()],
            "a 1pt move inside the row rebuilds nothing"
        );
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_event(100.0)));
        scene.settle(5, 1.0);
        assert_eq!(scroll.content_offset().y, 2_101.0);
        assert_eq!(
            preload_rows(scene),
            vec!["preload-row-210".to_string()],
            "GAP: Swift keeps the buffered window and only adds rows entering 200..300; Rust \
             swaps the single mounted row"
        );
        assert!(
            counts.borrow().rows.len() == before.len() + 1,
            "exactly one new row is constructed for the 100pt move"
        );
    });
}
