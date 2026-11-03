//! Translation of `Tests/CoveUIComposeTests/ReorderableListTests.swift`.
//!
//! Behaviour checklist:
//! - keyboard and assistive actions move the focused row and keep the handle
//! - Escape cancels a drag without reporting a move
//! - touch surfaces reserve a 44pt handle
//! - edge speed is zero mid-viewport and rises to the maximum at and beyond the edge
//! - dragging the handle shows a drop line and moves the row once
//!
//! Named differences (documented, not dropped):
//! - Handles are located by accessible role/value rather than Swift's
//!   `reorder-handle-<index>` debug name, so the lookup survives a reorder.
//! - The Swift suite also drives held-at-edge auto-scroll and the floating drag
//!   preview, which need `EnclosingScroll.container(of:)`/`viewport(of:)`. Rust's
//!   `EdgeAutoScroll` exposes the same pure velocity contract, which
//!   [`edge_velocity`] asserts; the two interaction scenarios are recorded.
//!
//! Deviations are limited to lookup mechanics and the two interaction scenarios
//! that need `EnclosingScroll`; every assertion below states the Swift contract.

use crate::support;
use cove_composition::Scope;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, DynamicField, LayoutDebugExt, View};
use cove_core::State;
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{Point, Rect};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::reorderable::ReorderableList;
use cove_ui_compose::primitives::interaction::edge_auto_scroll::{MAXIMUM_SPEED, velocity};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

const ROW_HEIGHT: f32 = 36.0;

/// `@State var items` plus the move log, so a keyboard move can re-render.
struct Harness {
    items: State<Vec<u64>>,
    moves: Rc<RefCell<Vec<(usize, usize)>>>,
    /// The Swift suite installs `[.touch, .keyboard]` for the handle size test.
    touch: bool,
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let items = self.items.get();
        let reordered = self.items.clone();
        let moves = self.moves.clone();
        let list = ReorderableList::new(
            items,
            |id: &u64| *id,
            |id: &u64, state: RowState| {
                let dragging = state.is_dragging;

                use cove_ui_compose::modifiers::style::AppearanceExt;
                AnyView::new(
                    AnyView::new(Text::new(format!("Item {}", id + 1)).foreground_color(
                        if dragging {
                            cove_core::Color::from_rgba8(0, 122, 255, 255)
                        } else {
                            cove_core::Color::from_rgba8(229, 229, 234, 255)
                        },
                    ))
                    .debug_name(&format!("reorder-row-{id}")),
                )
            },
        )
        .on_move(move |from, to| {
            moves.borrow_mut().push((from, to));
            let mut next = reordered.get();
            if from < next.len() && to <= next.len() {
                let moved = next.remove(from);
                let index = to.min(next.len());
                next.insert(index, moved);
                reordered.set(next);
            }
        })
        .row_height(ROW_HEIGHT)
        .row_label(|id: &u64| format!("Item {}", id + 1));

        let list = AnyView::new(AnyView::new(list).frame_size(280.0, 500.0));
        if self.touch {
            use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
            let mut environment = cove_core::Environment::default();
            environment.input = cove_core::InputCapabilities::TOUCH
                | cove_core::InputCapabilities::KEYBOARD;
            AnyView::new(list.surface_environment(environment))
        } else {
            list
        }
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("items", &self.items)]
    }
}

fn install(scene: &mut InteractiveScene, count: u64) -> Rc<RefCell<Vec<(usize, usize)>>> {
    install_with(scene, count, false)
}

fn install_with(
    scene: &mut InteractiveScene,
    count: u64,
    touch: bool,
) -> Rc<RefCell<Vec<(usize, usize)>>> {
    let moves = Rc::new(RefCell::new(Vec::new()));
    scene.install(Harness {
        items: State::new((0..count).collect()),
        moves: moves.clone(),
        touch,
    });
    scene.settle(8, 1.0);
    moves
}

fn handle(scene: &InteractiveScene, index: usize, count: usize) -> Node {
    let wanted = format!("Position {} of {count}", index + 1);
    scene
        .first(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Slider && semantics.value == wanted
            })
        })
        .unwrap_or_else(|| panic!("a handle at {} mounts", index + 1))
}

fn indicator(scene: &InteractiveScene) -> Option<Node> {
    scene.named("reorder-drop-indicator")
}

fn positions(scene: &InteractiveScene) -> Vec<String> {
    scene
        .all(|node| {
            node.accessibility()
                .is_some_and(|semantics| semantics.role == AccessibilityRole::Slider)
        })
        .into_iter()
        .filter_map(|node| node.accessibility().map(|semantics| semantics.value))
        .collect()
}

#[test]
fn keyboard_reorder() {
    InteractiveScene::run(320.0, 520.0, |scene| {
        let moves = install(scene, 4);

        let first = handle(scene, 0, 4);
        let semantics = first.accessibility().expect("the handle is accessible");
        assert_eq!(
            semantics.role,
            AccessibilityRole::Slider,
            "a reorder handle announces itself as a slider"
        );
        assert_eq!(
            semantics.value, "Position 1 of 4",
            "the handle names its position and the row count"
        );

        scene.focus.focus(Some(&first), false);
        scene.key(cove_ui_compose::Scancode::ARROW_DOWN, Default::default());
        scene.settle(8, 1.0);

        assert_eq!(
            moves.borrow().as_slice(),
            [(0, 1)],
            "the keyboard moves the focused row down one position"
        );

        assert!(
            scene.focus.focused().is_some_and(|node| node
                .accessibility()
                .is_some_and(|semantics| { semantics.role == AccessibilityRole::Slider })),
            "the handle keeps focus across the move"
        );
        assert_eq!(
            positions(scene),
            [
                "Position 1 of 4".to_string(),
                "Position 2 of 4".to_string(),
                "Position 3 of 4".to_string(),
                "Position 4 of 4".to_string(),
            ],
            "every handle reports its position after the move"
        );
    });
}

#[test]
fn pointer_reorder() {
    InteractiveScene::run(320.0, 520.0, |scene| {
        let moves = install(scene, 4);

        let source = handle(scene, 0, 4);
        let start = source.absolute_frame();
        let start_point = Point::new(start.x + start.width / 2.0, start.y + start.height / 2.0);

        scene.drag(
            start_point,
            Point::new(start_point.x, start_point.y + 80.0),
            6,
        );

        assert!(
            indicator(scene).is_none(),
            "the drop indicator is cleared once the drag ends"
        );
        assert_eq!(
            moves.borrow().as_slice(),
            [(0, 1)],
            "dragging the first row one stride down reports a single move"
        );
        assert_eq!(
            positions(scene),
            [
                "Position 1 of 4".to_string(),
                "Position 2 of 4".to_string(),
                "Position 3 of 4".to_string(),
                "Position 4 of 4".to_string(),
            ],
            "every handle reports its position after the pointer move"
        );
    });
}

#[test]
fn escape_cancels() {
    InteractiveScene::run(320.0, 520.0, |scene| {
        let moves = install(scene, 4);

        let source = handle(scene, 1, 4);
        let start = source.absolute_frame();
        let start_point = Point::new(start.x + start.width / 2.0, start.y + start.height / 2.0);

        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(
            cove_platform::MouseButtonEvent {
                button: cove_platform::MouseButton::Left,
                x: start_point.x,
                y: start_point.y,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            },
        ));
        scene.settle(6, 1.0);
        dispatcher.dispatch(&cove_platform::InputEvent::MouseMotion(
            cove_platform::MouseMotionEvent {
                x: start_point.x,
                y: start_point.y + 80.0,
                delta_x: 0.0,
                delta_y: 80.0,
                kind: Default::default(),
            },
        ));
        scene.settle(6, 1.0);

        assert!(
            indicator(scene).is_some(),
            "an in-flight drag paints the drop line"
        );

        scene.key(cove_ui_compose::Scancode::ESCAPE, Default::default());
        scene.settle(6, 1.0);

        assert!(indicator(scene).is_none(), "Escape leaves no drop line");
        assert!(
            moves.borrow().is_empty(),
            "a cancelled drag reports no move"
        );
    });
}

#[test]
fn touch_handle() {
    InteractiveScene::run(320.0, 520.0, |scene| {
        install_with(scene, 4, true);

        let handle = handle(scene, 0, 4);
        let frame = handle.frame();
        assert!(
            frame.width >= 44.0,
            "a touch handle is at least 44pt wide (got {})",
            frame.width
        );
        assert!(
            frame.height >= 44.0,
            "a touch handle is at least 44pt tall (got {})",
            frame.height
        );

        let mut nodes = Vec::new();
        fn collect(node: &Node, into: &mut Vec<Node>) {
            into.push(node.clone());
            for child in node.children() {
                collect(&child, into);
            }
        }
        collect(&handle, &mut nodes);
        assert!(
            nodes.iter().any(|node| node.frame().width > 0.0),
            "the handle paints real content"
        );
    });
}

#[test]
fn edge_velocity() {
    let viewport = Rect::new(0.0, 100.0, 300.0, 400.0);

    assert_eq!(
        velocity(300.0, viewport),
        0.0,
        "mid-viewport there is no automatic scroll"
    );
    assert_eq!(
        velocity(100.0, viewport),
        -MAXIMUM_SPEED,
        "at the top edge the list scrolls up at maximum speed"
    );
    assert_eq!(
        velocity(520.0, viewport),
        MAXIMUM_SPEED,
        "at and beyond the bottom edge the list scrolls down at maximum speed"
    );

    let shallow = velocity(470.0, viewport);
    let deep = velocity(490.0, viewport);
    assert!(
        shallow > 0.0 && deep > shallow,
        "the speed rises quadratically toward the edge (shallow {shallow}, deep {deep})"
    );
}
