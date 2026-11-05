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
//!
//! Deviations are limited to lookup mechanics; every assertion below states the
//! Swift contract, including held-at-edge auto-scroll and the floating preview.

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
use cove_ui_compose::primitives::data::lists::reorder_model::drop_indicator_y;
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
    list_height: f32,
    /// Pushes the list down so pointer coordinates and the list's local
    /// coordinates differ, like the gallery's story section.
    top_padding: f32,
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

        let list = AnyView::new(AnyView::new(list).frame_size(280.0, self.list_height));
        // Offset the list from the window origin like a padded parent would.
        let list = if self.top_padding > 0.0 {
            use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
            use cove_ui_compose::{BoxView, Spacer, TupleView};
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Spacer::new(self.top_padding)),
                    list,
                ])),
            ))
        } else {
            list
        };
        if self.touch {
            use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
            let mut environment = cove_core::Environment::default();
            environment.input =
                cove_core::InputCapabilities::TOUCH | cove_core::InputCapabilities::KEYBOARD;
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
    install_in(scene, count, touch, 500.0, 0.0)
}

fn install_in(
    scene: &mut InteractiveScene,
    count: u64,
    touch: bool,
    list_height: f32,
    top_padding: f32,
) -> Rc<RefCell<Vec<(usize, usize)>>> {
    let moves = Rc::new(RefCell::new(Vec::new()));
    scene.install(Harness {
        items: State::new((0..count).collect()),
        moves: moves.clone(),
        touch,
        list_height,
        top_padding,
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

fn window_point(node: &Node) -> Point {
    let frame = node.absolute_frame();
    Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0)
}

fn press(point: Point) -> cove_platform::InputEvent {
    cove_platform::InputEvent::MouseButtonDown(cove_platform::MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x: point.x,
        y: point.y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    })
}

fn motion(point: Point) -> cove_platform::InputEvent {
    cove_platform::InputEvent::MouseMotion(cove_platform::MouseMotionEvent {
        x: point.x,
        y: point.y,
        delta_x: 0.0,
        delta_y: 0.0,
        kind: Default::default(),
    })
}

fn release(point: Point) -> cove_platform::InputEvent {
    cove_platform::InputEvent::MouseButtonUp(cove_platform::MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x: point.x,
        y: point.y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    })
}

fn scroll_viewport(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| node.is_scroll_viewport())
        .expect("the list owns a scroll viewport")
}

/// Pointer events are window coordinates; the auto-scroll edge test must
/// compare them with the viewport in the scroll's own space. A list pushed
/// down the scene used to scroll itself to the end on any drag.
#[test]
fn dragging_inside_the_viewport_does_not_auto_scroll() {
    InteractiveScene::run(320.0, 520.0, |scene| {
        let moves = install_in(scene, 20, false, 220.0, 160.0);
        let scroll = scroll_viewport(scene);

        let mounted_before = positions(scene);
        assert!(
            mounted_before.len() > 1,
            "the virtualized window mounts several rows before the drag"
        );

        let source = handle(scene, 4, 20);
        let start = window_point(&source);
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&press(start));
        scene.settle(4, 0.1);

        assert_eq!(
            scroll.content_offset().y,
            0.0,
            "the pointer is mid-viewport, so a drag must not scroll the list"
        );

        dispatcher.dispatch(&motion(Point::new(start.x, start.y + 10.0)));
        scene.settle(4, 0.1);

        assert_eq!(
            scroll.content_offset().y,
            0.0,
            "holding inside the viewport keeps the content still"
        );
        assert_eq!(
            positions(scene),
            mounted_before,
            "the mounted rows stay exactly as they were during the drag"
        );

        dispatcher.dispatch(&release(start));
        scene.settle(4, 1.0);
        assert!(moves.borrow().is_empty(), "releasing on the row moves nothing");
    });
}

/// Holding the handle at the bottom edge scrolls, but the offset stops at the
/// content end and the drop lands below the initial window (Swift
/// `edgeAutoScroll`).
#[test]
fn edge_auto_scroll_stops_at_the_content_end() {
    InteractiveScene::run(320.0, 520.0, |scene| {
        let moves = install_in(scene, 30, false, 200.0, 160.0);
        let scroll = scroll_viewport(scene);

        let viewport = scroll.absolute_frame();
        let source = handle(scene, 0, 30);
        let start = window_point(&source);
        let edge = Point::new(start.x, viewport.max_y() - 2.0);
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&press(start));
        scene.settle(2, 0.1);

        dispatcher.dispatch(&motion(edge));
        scene.settle(12, 0.1);

        let maximum = 30.0 * ROW_HEIGHT - viewport.height;
        assert_eq!(
            scroll.content_offset().y,
            maximum,
            "auto-scroll clamps to the last content offset instead of running past it"
        );
        let line = indicator(scene).expect("a drop line");
        let boundary = ((edge.y - start.y + maximum) / ROW_HEIGHT)
            .round()
            .clamp(0.0, 30.0) as usize;
        let expected_line_y = viewport.y
            + drop_indicator_y(boundary, 30, ROW_HEIGHT, 30.0 * ROW_HEIGHT)
            - scroll.content_offset().y;
        assert!(
            (line.absolute_frame().y - expected_line_y).abs() < 1.0,
            "the drop line follows the insertion boundary (got {}, expected {expected_line_y})",
            line.absolute_frame().y
        );

        let preview = scene.named("reorder-preview").expect("a lifted preview");
        assert!(
            (preview.absolute_frame().max_y() - scroll.absolute_frame().max_y()).abs() < 1.0,
            "the lifted row rests at the bottom edge of the viewport (preview {:?}, viewport {:?})",
            preview.absolute_frame(),
            scroll.absolute_frame()
        );

        dispatcher.dispatch(&release(edge));
        scene.settle(4, 1.0);

        // The drop lands where the pointer is in content coordinates.
        let expected = ((edge.y - start.y + maximum) / ROW_HEIGHT).round() as usize;
        let expected = expected - 1;
        let expected: Vec<(usize, usize)> = if expected == 0 {
            Vec::new()
        } else {
            vec![(0, expected)]
        };
        assert_eq!(
            moves.borrow().as_slice(),
            expected.as_slice(),
            "the drop lands where the pointer rests in content coordinates"
        );
        assert!(
            expected.first().is_some_and(|(_, to)| *to > 6),
            "the drop lands below the initially visible rows"
        );
        assert_eq!(
            scroll.content_offset().y,
            maximum,
            "releasing the drag stops auto-scrolling"
        );
    });
}

/// The lifted copy follows the pointer and the source row stays as a dimmed
/// placeholder (Swift `floatingPreview`).
#[test]
fn floating_preview_follows_the_pointer() {
    InteractiveScene::run(320.0, 520.0, |scene| {
        install_in(scene, 10, false, 300.0, 0.0);

        let source = handle(scene, 1, 10);
        let start = source.absolute_frame();
        let point = window_point(&source);
        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&press(point));
        scene.settle(2, 1.0);

        let preview = scene.named("reorder-preview").expect("a lifted preview");
        assert!(
            (preview.absolute_frame().y - start.y).abs() < 1.0,
            "the lifted row starts over its source row"
        );
        assert!(
            (preview.absolute_frame().height - start.height).abs() < 1.0,
            "the lifted row keeps the row height"
        );

        dispatcher.dispatch(&motion(Point::new(point.x, point.y + 50.0)));
        scene.settle(2, 0.1);
        let preview = scene.named("reorder-preview").expect("the preview stays mounted");
        assert!(
            (preview.absolute_frame().y - (start.y + 50.0)).abs() < 1.0,
            "the lifted row follows pointer travel without a relayout"
        );

        let row = scene.named("reorder-row-1").expect("the source row stays mounted");
        assert!(
            std::iter::successors(Some(row), |node| node.parent())
                .any(|node| (node.opacity() - 0.35).abs() < 0.001),
            "the source row remains as a dimmed placeholder"
        );

        dispatcher.dispatch(&release(Point::new(point.x, point.y + 50.0)));
        scene.settle(2, 1.0);
        assert!(
            scene.named("reorder-preview").is_none(),
            "the lifted row disappears when the drag ends"
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
