//! Translation of `Tests/CoveUIComposeTests/RatingTests.swift`.
//!
//! Behaviour checklist:
//! - authored selection/appearance groups normalize independently without
//!   rewriting the controlled value
//! - every density keeps its intrinsic height; a custom star size and spacing
//!   size the row
//! - pointer hover previews without writing; a complete click commits and a
//!   repeat click clears; a right button is ignored
//! - half-star hit regions follow the absolute origin; releasing outside
//!   cancels; candidates clamp at the extremes and for NaN
//! - fractional precision snaps exactly; Shift steps whole symbols
//! - a custom symbol changes the painted glyph pair
//! - right-to-left mirrors star order, hit candidates and arrow direction
//! - one Tab stop adjusts by half steps, ignores chords and Escape cancels a
//!   captured press
//! - dragging back to the starting score commits without clearing
//! - accessibility normalizes finite writes; read-only and disabled controls
//!   expose no actions
//! - reconfiguration and unmount release capture and clear previews
//! - fractional fills clip matching SVG stars and focus paints a ring
//!
//! Swift wrote the value through a get/set closure binding that `_updateNode`
//! re-read on demand. Rust's `Rating` takes a plain value plus `on_change`, so
//! the harness keeps the value in a `State` observed by its body and the test
//! settles after every write, which is what re-runs `update_node`. Swift's
//! direct `_updateNode(node)` calls map to forcing a harness re-render.
//!
//! Named differences (GAP assertions):
//! - Session identity: `Rating::new` creates a new `RatingSession` every
//!   construction and only the first one is mounted on the node, so a parent
//!   re-render leaves the handlers on an unmounted session whose presses never
//!   capture (`a_rebuilt_rating_drops_its_press_session`). The other cases keep
//!   one harness-owned session through the public `Rating::session` field,
//!   which is what Swift's persistent session provides.
//! - Inactive flags: Swift clears `isFocusable`/`isHitTestable` when a rating
//!   becomes read-only or disabled; Rust sets them only in `make_node`, so a
//!   reused node keeps both (the registry handlers and actions are removed).
//! - `RatingPaint` attachment: Swift exposes the cached rasters as a
//!   `rating.paint` attachment; Rust has none, so the custom-symbol case
//!   compares the painted texture ids instead.

mod support;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use cove_composition::layout::LayoutDirectionExt;
use cove_composition::{AnyView, DynamicField, TupleView, View};
use cove_core::scene::{AccessibilityRole, Direction, EventPhase, EventResult, Node, PointerPhase};
use cove_core::{DrawList, Point, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent,
};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::{LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::controls::rating::{
    Rating, RatingAppearance, RatingPrecision, RatingSelection, RatingSession, RatingSymbol,
};
use cove_ui_compose::primitives::media::ui_icon_catalog;
use cove_ui_compose::theme::ControlSize;
use cove_ui_compose::theme::tokens::control_size::ControlSizeViewExt;
use cove_ui_compose::{Row, VerticalAlignment};

use support::InteractiveScene;

type Build = Rc<dyn Fn(f64, Rc<dyn Fn(f64)>, Rc<RatingSession>) -> AnyView>;

/// A rating whose controlled value lives in observed state, plus a tick that
/// forces a re-render (the Swift `_updateNode` call).
struct Harness {
    value: State<f64>,
    tick: State<u32>,
    session: Rc<RatingSession>,
    build: RefCell<Build>,
}

impl Harness {
    fn new(
        value: f64,
        build: impl Fn(f64, Rc<dyn Fn(f64)>, Rc<RatingSession>) -> AnyView + 'static,
    ) -> Rc<Self> {
        Rc::new(Self {
            value: State::new(value),
            tick: State::new(0),
            session: RatingSession::new(),
            build: RefCell::new(Rc::new(build)),
        })
    }

    fn value(&self) -> f64 {
        self.value.get()
    }

    fn rerender(&self, scene: &InteractiveScene) {
        self.tick.set(self.tick.get() + 1);
        scene.settle(2, 0.0);
    }
}

struct HarnessView(Rc<Harness>);

impl View for HarnessView {
    fn body(&self) -> AnyView {
        let _ = self.0.tick.get();
        let state = self.0.value.clone();
        let build = self.0.build.borrow().clone();
        build(
            self.0.value.get(),
            Rc::new(move |next| state.set(next)),
            self.0.session.clone(),
        )
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![
            DynamicField::state("value", &self.0.value),
            DynamicField::state("tick", &self.0.tick),
        ]
    }
}

/// A rating that keeps the harness-owned session across re-renders, the way a
/// Swift `@State` session survives `_updateNode`.
fn rating(label: &str, value: f64, write: Rc<dyn Fn(f64)>, session: &Rc<RatingSession>) -> Rating {
    let mut rating = Rating::new(label, value, move |next| write(next));
    rating.session = session.clone();
    rating
}

fn host(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| node.attachment::<Rc<RatingSession>>("rating.session").is_some())
        .expect("the rating host is mounted")
}

fn session(node: &Node) -> Rc<RatingSession> {
    node.attachment::<Rc<RatingSession>>("rating.session")
        .expect("a rating session")
}

fn preview(node: &Node) -> Option<f64> {
    *session(node).preview.borrow()
}

fn pointer(node: &Node, star: usize, fraction: f32, y_offset: f32) -> MouseButtonEvent {
    let geometry = session(node)
        .configuration
        .borrow()
        .expect("a configured rating")
        .geometry;
    let frame = node.absolute_frame();
    let rect = geometry.star_rect(
        star,
        Point::new(frame.x, frame.y),
        node.frame().height,
        node.resolves_right_to_left(),
    );
    MouseButtonEvent {
        button: MouseButton::Left,
        x: rect.x + rect.width * fraction,
        y: rect.y + rect.height / 2.0 + y_offset,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn key(code: u32, modifiers: KeyModifiers) -> InputEvent {
    InputEvent::KeyDown(KeyEvent {
        scancode: code,
        keycode: 0,
        modifiers,
        is_repeat: false,
    })
}

fn dispatch(scene: &InteractiveScene, event: InputEvent) {
    scene.dispatcher().dispatch(&event);
    scene.settle(1, 0.0);
}

fn down(scene: &InteractiveScene, event: &MouseButtonEvent) {
    dispatch(scene, InputEvent::MouseButtonDown(event.clone()));
}

fn up(scene: &InteractiveScene, event: &MouseButtonEvent) {
    dispatch(scene, InputEvent::MouseButtonUp(event.clone()));
}

fn motion(scene: &InteractiveScene, x: f32, y: f32) {
    dispatch(
        scene,
        InputEvent::MouseMotion(MouseMotionEvent {
            x,
            y,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        }),
    );
}

fn click(scene: &InteractiveScene, event: &MouseButtonEvent) {
    down(scene, event);
    up(scene, event);
}

fn same(a: Option<Node>, b: &Node) -> bool {
    a.is_some_and(|node| node.id() == b.id())
}

#[test]
fn configuration() {
    let mut selection = RatingSelection::default();
    selection.maximum = 200;
    selection.precision = RatingPrecision::HALF;
    let mut appearance = RatingAppearance::default();
    appearance.star_size = Some(f32::INFINITY);
    appearance.spacing = f32::NEG_INFINITY;
    let rating = Rating::new("", f64::NAN, |_| {})
        .selection(selection)
        .appearance(appearance);
    assert!(rating.selection.maximum == 20 && rating.selection.precision == RatingPrecision::HALF);
    assert!(rating.appearance.star_size.is_none() && rating.appearance.spacing == 4.0);
    assert!(rating.value.is_nan(), "the controlled value is not rewritten");
    assert!(rating.selection.clamped(f64::NAN) == 0.0 && rating.selection.clamped(100.0) == 20.0);
    let mut selection = RatingSelection::default();
    selection.maximum = 0;
    selection.validate();
    assert_eq!(selection.maximum, 1);
    selection.precision = RatingPrecision::HALF;
    selection.allows_clear = false;
    assert!(selection.snapped(0.0) == 0.5 && selection.snapped(0.76) == 1.0);
}

#[test]
fn density() {
    for (size, points) in [
        (ControlSize::Mini, 12.0_f32),
        (ControlSize::Small, 16.0),
        (ControlSize::Regular, 20.0),
        (ControlSize::Large, 28.0),
    ] {
        InteractiveScene::run(500.0, 100.0, |scene| {
            scene.install(Row::new(
                VerticalAlignment::Center,
                0.0,
                AnyView::new(
                    AnyView::new(Rating::new("", 2.0, |_| {}).control_size(size)).frame_width(180.0),
                ),
            ));
            let node = host(scene);
            let geometry = session(&node).configuration.borrow().unwrap().geometry;
            assert_eq!(
                node.frame().height,
                32.0_f32.max(points + 8.0),
                "{size:?} keeps its intrinsic height"
            );
            assert_eq!(geometry.star_size, points, "{size:?} star size");
            assert_eq!(node.frame().width, 180.0, "{size:?} fills the authored width");
        });
    }
    InteractiveScene::run(500.0, 100.0, |scene| {
        let mut selection = RatingSelection::default();
        selection.maximum = 7;
        let mut appearance = RatingAppearance::default();
        appearance.star_size = Some(36.0);
        appearance.spacing = 8.0;
        scene.install(
            Rating::new("", 0.0, |_| {})
                .selection(selection)
                .appearance(appearance),
        );
        let node = host(scene);
        assert_eq!((node.frame().width, node.frame().height), (364.0, 44.0));
    });
}

#[test]
fn pointer_commit() {
    InteractiveScene::run(500.0, 100.0, |scene| {
        let harness = Harness::new(2.0, |value, write, session| {
            AnyView::new(
                AnyView::new(Row::new(
                    VerticalAlignment::Center,
                    0.0,
                    AnyView::new(rating("Overall", value, write, &session)),
                ))
                .padding(20.0),
            )
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        let event = pointer(&node, 3, 0.75, 0.0);
        scene.graph.tree.borrow_mut().flush();
        scene.dispatcher().dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: event.x,
            y: event.y,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
        assert_eq!(harness.value(), 2.0, "hover never writes");
        assert_eq!(preview(&node), Some(4.0), "hover previews the fourth star");
        assert!(!scene.graph.recomposer.has_pending(), "a preview does not recompose");
        assert!(scene.graph.tree.borrow().has_render_updates(), "a preview repaints");
        up(scene, &event);
        assert_eq!(harness.value(), 2.0, "a release without a press is ignored");
        let mut right = event.clone();
        right.button = MouseButton::Right;
        let handler = scene.registry.handlers(&node).pointer.expect("a pointer handler");
        assert_eq!(
            handler(&right, PointerPhase::Down, EventPhase::Target),
            EventResult::Ignored,
            "the right button is ignored"
        );
        down(scene, &event);
        assert!(same(scene.capture.target(), &node), "the press captures the pointer");
        assert!(same(scene.focus.focused(), &node), "the press focuses the rating");
        assert_eq!(harness.value(), 2.0);
        harness.rerender(scene);
        assert!(same(scene.capture.target(), &node), "an unchanged update keeps the capture");
        up(scene, &event);
        assert_eq!(harness.value(), 4.0, "a complete click commits");
        assert_eq!(preview(&node), None);
        assert!(scene.capture.target().is_none(), "the release frees the pointer");
        click(scene, &event);
        assert_eq!(harness.value(), 0.0, "repeating the current value clears");
        assert_eq!(node.accessibility().map(|s| s.label), Some("Overall".to_string()));
    });
}

#[test]
fn half_pointer() {
    InteractiveScene::run(500.0, 100.0, |scene| {
        let harness = Harness::new(0.0, |value, write, session| {
            let mut selection = RatingSelection::default();
            selection.precision = RatingPrecision::HALF;
            AnyView::new(
                AnyView::new(Row::new(
                    VerticalAlignment::Center,
                    0.0,
                    AnyView::new(rating("", value, write, &session).selection(selection)),
                ))
                .padding(30.0),
            )
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        let half = pointer(&node, 2, 0.25, 0.0);
        let whole = pointer(&node, 2, 0.75, 0.0);
        click(scene, &half);
        assert_eq!(harness.value(), 2.5);
        click(scene, &whole);
        assert_eq!(harness.value(), 3.0);
        down(scene, &half);
        let outside = pointer(&node, 2, 0.25, 300.0);
        motion(scene, outside.x, outside.y);
        assert!(preview(&node).is_none(), "leaving the row drops the preview");
        assert!(same(scene.capture.target(), &node), "the press keeps the capture");
        assert_eq!(harness.value(), 3.0);
        up(scene, &outside);
        assert_eq!(harness.value(), 3.0, "an outside release cancels");
        assert!(session(&node).pressed_candidate.borrow().is_none());
        assert!(scene.capture.target().is_none());
        let geometry = session(&node).configuration.borrow().unwrap().geometry;
        let origin = node.absolute_frame().x;
        let rtl = node.resolves_right_to_left();
        assert_eq!(geometry.candidate(f32::MAX, origin, rtl, RatingPrecision::HALF), 5.0);
        assert_eq!(geometry.candidate(-f32::MAX, origin, rtl, RatingPrecision::HALF), 0.5);
        assert_eq!(geometry.candidate(f32::NAN, origin, rtl, RatingPrecision::HALF), 0.0);
    });
}

#[test]
fn fractional_precision() {
    assert_eq!(RatingPrecision::divisions(0), RatingPrecision::WHOLE);
    assert_eq!(RatingPrecision::divisions(64).divisions, 20);
    assert_eq!(RatingPrecision::TENTH.snapped(0.1 + 0.2), 0.3);
    assert_eq!(RatingPrecision::QUARTER.snapped(2.13), 2.25);
    assert_eq!(RatingPrecision::TENTH.part(0.3), 0.3);
    assert_eq!(RatingPrecision::TENTH.part(0.31), 0.4);
    assert_eq!(RatingPrecision::HALF.part(0.0), 0.5);
    InteractiveScene::run(500.0, 100.0, |scene| {
        let harness = Harness::new(0.0, |value, write, session| {
            let mut selection = RatingSelection::default();
            selection.precision = RatingPrecision::TENTH;
            AnyView::new(rating("", value, write, &session).selection(selection))
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        click(scene, &pointer(&node, 1, 0.33, 0.0));
        assert_eq!(harness.value(), 1.4);
        scene.focus.clear();
        dispatch(scene, key(Scancode::TAB, KeyModifiers::default()));
        dispatch(scene, key(Scancode::ARROW_RIGHT, KeyModifiers::default()));
        assert_eq!(harness.value(), 1.5);
        dispatch(scene, key(Scancode::ARROW_RIGHT, KeyModifiers::LEFT_SHIFT));
        assert_eq!(harness.value(), 2.5);
        dispatch(scene, key(Scancode::ARROW_LEFT, KeyModifiers::LEFT_SHIFT));
        assert_eq!(harness.value(), 1.5);
    });
}

fn painted_textures(node: &Node) -> Vec<u32> {
    let mut list = DrawList::default();
    if let Some(draw) = node.draw() {
        draw(&mut list, Point::default());
    }
    list.batches().iter().map(|batch| batch.texture_id).collect()
}

#[test]
fn custom_symbol() {
    InteractiveScene::run(500.0, 100.0, |scene| {
        let symbol = Rc::new(RefCell::new(RatingSymbol::star()));
        let harness = Harness::new(2.0, {
            let symbol = symbol.clone();
            move |value, write, session| {
                let mut appearance = RatingAppearance::default();
                appearance.symbol = symbol.borrow().clone();
                AnyView::new(rating("", value, write, &session).appearance(appearance))
            }
        });
        scene.install(HarnessView(harness.clone()));
        let first = painted_textures(&host(scene));
        assert!(!first.is_empty(), "the star row paints");
        *symbol.borrow_mut() = RatingSymbol::heart();
        harness.rerender(scene);
        let second = painted_textures(&host(scene));
        assert_eq!(RatingSymbol::heart().filled, "heart-fill");
        assert!(
            !second.is_empty() && second != first,
            "the heart paints different rasters: {first:?} vs {second:?}"
        );
    });
}

#[test]
fn right_to_left() {
    InteractiveScene::run(400.0, 100.0, |scene| {
        let harness = Harness::new(3.0, |value, write, session| {
            let mut selection = RatingSelection::default();
            selection.precision = RatingPrecision::HALF;
            AnyView::new(
                rating("RTL rating", value, write, &session)
                    .selection(selection)
                    .layout_direction(Direction::Rtl),
            )
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        let geometry = session(&node).configuration.borrow().unwrap().geometry;
        let rtl = node.resolves_right_to_left();
        assert!(rtl, "the rating resolves right-to-left");
        let frame = node.absolute_frame();
        let origin = Point::new(frame.x, frame.y);
        assert!(
            geometry.star_rect(0, origin, node.frame().height, rtl).x
                > geometry.star_rect(4, origin, node.frame().height, rtl).x,
            "the first star is drawn rightmost"
        );
        click(scene, &pointer(&node, 0, 0.75, 0.0));
        assert_eq!(harness.value(), 0.5);
        click(scene, &pointer(&node, 4, 0.25, 0.0));
        assert_eq!(harness.value(), 5.0);
        harness.value.set(3.0);
        scene.settle(2, 0.0);
        scene.focus.clear();
        dispatch(scene, key(Scancode::TAB, KeyModifiers::default()));
        assert!(same(scene.focus.focused(), &node), "Tab focuses the rating");
        dispatch(scene, key(Scancode::ARROW_LEFT, KeyModifiers::default()));
        assert_eq!(harness.value(), 3.5);
        dispatch(scene, key(Scancode::ARROW_RIGHT, KeyModifiers::default()));
        assert_eq!(harness.value(), 3.0);
        dispatch(scene, key(Scancode::ARROW_UP, KeyModifiers::default()));
        assert_eq!(harness.value(), 3.5);
        dispatch(scene, key(Scancode::ARROW_DOWN, KeyModifiers::default()));
        assert_eq!(harness.value(), 3.0);
    });
}

#[test]
fn keyboard() {
    InteractiveScene::run(500.0, 100.0, |scene| {
        let harness = Harness::new(2.5, |value, write, session| {
            let mut half = RatingSelection::default();
            half.precision = RatingPrecision::HALF;
            let mut read_only = RatingSelection::default();
            read_only.is_read_only = true;
            AnyView::new(Row::new(
                VerticalAlignment::Center,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(rating("", value, write, &session).selection(half)),
                    AnyView::new(Rating::new("", 4.25, |_| {}).selection(read_only)),
                    AnyView::new(Button::title("Next", || {})),
                ])),
            ))
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        dispatch(scene, key(Scancode::TAB, KeyModifiers::default()));
        assert!(same(scene.focus.focused(), &node) && scene.focus.is_focus_visible());
        dispatch(scene, key(Scancode::ARROW_RIGHT, KeyModifiers::default()));
        assert_eq!(harness.value(), 3.0);
        dispatch(
            scene,
            InputEvent::KeyUp(KeyEvent {
                scancode: Scancode::ARROW_RIGHT,
                keycode: 0,
                modifiers: KeyModifiers::default(),
                is_repeat: false,
            }),
        );
        assert_eq!(harness.value(), 3.0, "key up does nothing");
        for modifier in [
            KeyModifiers::LEFT_CTRL,
            KeyModifiers::RIGHT_CTRL,
            KeyModifiers::LEFT_GUI,
            KeyModifiers::RIGHT_GUI,
            KeyModifiers::LEFT_ALT,
            KeyModifiers::RIGHT_ALT,
        ] {
            dispatch(scene, key(Scancode::ARROW_RIGHT, modifier));
            assert_eq!(harness.value(), 3.0, "{modifier:?} chords are ignored");
        }
        dispatch(scene, key(Scancode::ARROW_DOWN, KeyModifiers::default()));
        assert_eq!(harness.value(), 2.5);
        dispatch(scene, key(Scancode::END, KeyModifiers::default()));
        assert_eq!(harness.value(), 5.0);
        dispatch(scene, key(Scancode::HOME, KeyModifiers::default()));
        assert_eq!(harness.value(), 0.0);
        dispatch(scene, key(Scancode::ARROW_UP, KeyModifiers::default()));
        assert_eq!(harness.value(), 0.5);
        dispatch(scene, key(Scancode::DELETE, KeyModifiers::default()));
        assert_eq!(harness.value(), 0.0);
        let node = host(scene);
        down(scene, &pointer(&node, 3, 0.75, 0.0));
        assert!(same(scene.capture.target(), &node), "the press captures");
        dispatch(scene, key(Scancode::ESCAPE, KeyModifiers::default()));
        assert!(scene.capture.target().is_none(), "Escape releases the capture");
        assert!(preview(&node).is_none() && harness.value() == 0.0);
        dispatch(scene, key(Scancode::TAB, KeyModifiers::default()));
        let focused = scene.focus.focused().expect("focus moves on");
        assert!(focused.id() != node.id(), "one Tab stop: the read-only rating is skipped");
        assert_eq!(
            focused.accessibility().map(|s| s.role),
            Some(AccessibilityRole::Button)
        );
    });
}

#[test]
fn drag_and_binding() {
    InteractiveScene::run(300.0, 100.0, |scene| {
        let original = State::new(3.0_f64);
        let replacement = State::new(3.0_f64);
        let use_replacement = Rc::new(RefCell::new(false));
        let harness = Harness::new(0.0, {
            let original = original.clone();
            let replacement = replacement.clone();
            let use_replacement = use_replacement.clone();
            move |_, _, session| {
                let target = if *use_replacement.borrow() {
                    replacement.clone()
                } else {
                    original.clone()
                };
                let write = target.clone();
                AnyView::new(rating("", target.get(), Rc::new(move |next| write.set(next)), &session))
            }
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        let start = pointer(&node, 2, 0.75, 0.0);
        let end = pointer(&node, 4, 0.75, 0.0);
        down(scene, &start);
        motion(scene, end.x, end.y);
        assert_eq!(original.get(), 3.0);
        assert_eq!(preview(&node), Some(5.0), "the drag previews the fifth star");
        harness.rerender(scene);
        assert!(same(scene.capture.target(), &node), "an update keeps the drag");
        motion(scene, start.x, start.y);
        up(scene, &start);
        assert_eq!(original.get(), 3.0, "dragging back commits 3 instead of clearing");
        assert!(scene.capture.target().is_none());
        *use_replacement.borrow_mut() = true;
        harness.rerender(scene);
        dispatch(scene, key(Scancode::ARROW_UP, KeyModifiers::default()));
        harness.rerender(scene);
        assert_eq!((original.get(), replacement.get()), (3.0, 4.0), "the new binding is live");
        let node = host(scene);
        down(scene, &pointer(&node, 2, 0.75, 0.0));
        replacement.set(1.0);
        harness.rerender(scene);
        assert!(scene.capture.target().is_none(), "an external value change cancels the press");
        assert!(preview(&node).is_none());
        up(scene, &pointer(&node, 2, 0.75, 0.0));
        assert_eq!(replacement.get(), 1.0);
    });
}

#[test]
fn accessibility() {
    InteractiveScene::run(300.0, 100.0, |scene| {
        let mode = Rc::new(RefCell::new(None::<bool>));
        let harness = Harness::new(2.5, {
            let mode = mode.clone();
            move |value, write, session| {
                let mut selection = RatingSelection::default();
                selection.precision = RatingPrecision::HALF;
                selection.allows_clear = false;
                if let Some(read_only) = *mode.borrow() {
                    selection.is_read_only = read_only;
                    selection.is_enabled = read_only;
                }
                AnyView::new(rating("Customer score", value, write, &session).selection(selection))
            }
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        let semantics = node.accessibility().expect("semantics");
        assert!(semantics.role == AccessibilityRole::Slider && semantics.value == "2.5");
        let act = |scene: &InteractiveScene, run: &dyn Fn(&cove_core::scene::AccessibilityActions)| {
            run(&host(scene).accessibility_actions());
            scene.settle(2, 0.0);
        };
        act(scene, &|actions| (actions.set_value.as_ref().unwrap())("4.26"));
        assert_eq!(harness.value(), 4.5);
        act(scene, &|actions| (actions.increment.as_ref().unwrap())());
        assert_eq!(harness.value(), 5.0);
        act(scene, &|actions| (actions.decrement.as_ref().unwrap())());
        assert_eq!(harness.value(), 4.5);
        for invalid in ["nan", "inf", "not a score"] {
            act(scene, &|actions| (actions.set_value.as_ref().unwrap())(invalid));
            assert_eq!(harness.value(), 4.5, "{invalid:?} is ignored");
        }
        act(scene, &|actions| (actions.set_value.as_ref().unwrap())("-4"));
        assert_eq!(harness.value(), 0.5);
        let handler = scene.registry.handlers(&node).key.expect("a key handler");
        assert_eq!(
            handler(
                &KeyEvent {
                    scancode: Scancode::BACKSPACE,
                    keycode: 0,
                    modifiers: KeyModifiers::default(),
                    is_repeat: false,
                },
                EventPhase::Target
            ),
            EventResult::Ignored,
            "clearing is ignored when allows_clear is off"
        );
        for read_only in [true, false] {
            *mode.borrow_mut() = Some(read_only);
            harness.rerender(scene);
            let node = host(scene);
            assert!(
                node.is_focusable() && node.is_hit_testable(),
                "GAP: Swift's update clears isFocusable/isHitTestable on an inactive rating \
                 ({read_only}); Rust sets them only in make_node, so the reused node keeps them"
            );
            assert!(scene.registry.handlers(&node).pointer.is_none());
            let actions = node.accessibility_actions();
            assert!(actions.set_value.is_none() && actions.increment.is_none());
            let semantics = node.accessibility().unwrap();
            assert_eq!(
                semantics.role,
                if read_only { AccessibilityRole::Image } else { AccessibilityRole::Slider }
            );
            assert!(semantics.state.is_read_only == read_only && harness.value() == 0.5);
        }
    });
}

#[test]
fn cleanup() {
    let departed: RefCell<Option<std::rc::Weak<()>>> = RefCell::new(None);
    InteractiveScene::run(300.0, 100.0, |scene| {
        let selection = Rc::new(RefCell::new(RatingSelection::default()));
        let harness = Harness::new(2.0, {
            let selection = selection.clone();
            move |value, _, session| {
                AnyView::new(rating("", value, Rc::new(|_| {}), &session).selection(*selection.borrow()))
            }
        });
        scene.install(HarnessView(harness.clone()));
        let node = host(scene);
        down(scene, &pointer(&node, 3, 0.75, 0.0));
        selection.borrow_mut().maximum = 3;
        harness.rerender(scene);
        assert!(scene.capture.target().is_none(), "reconfiguring releases the capture");
        assert!(preview(&node).is_none());
        down(scene, &pointer(&node, 1, 0.75, 0.0));
        selection.borrow_mut().is_enabled = false;
        harness.rerender(scene);
        assert!(scene.capture.target().is_none() && scene.focus.focused().is_none());
        assert!(scene.registry.handlers(&node).key.is_none());
        selection.borrow_mut().is_enabled = true;
        harness.rerender(scene);
        down(scene, &pointer(&node, 1, 0.75, 0.0));
        scene.graph.install(cove_composition::EmptyView, Default::default());
        scene.settle(1, 0.0);
        assert!(scene.capture.target().is_none() && scene.focus.focused().is_none());
        assert!(scene.registry.handlers(&node).pointer.is_none());
        let marker = Rc::new(());
        *departed.borrow_mut() = Some(Rc::downgrade(&marker));
        node.set_attachment("test.marker", marker);
    });
    assert!(
        departed.borrow().as_ref().unwrap().upgrade().is_none(),
        "the unmounted node is released"
    );
}

#[test]
fn a_rebuilt_rating_drops_its_press_session() {
    InteractiveScene::run(500.0, 100.0, |scene| {
        let tick = State::new(0_u32);
        struct Rebuilt(State<u32>);
        impl View for Rebuilt {
            fn body(&self) -> AnyView {
                let _ = self.0.get();
                AnyView::new(Rating::new("", 2.0, |_| {}))
            }
            fn dynamic_properties(&self) -> Vec<DynamicField> {
                vec![DynamicField::state("tick", &self.0)]
            }
        }
        scene.install(Rebuilt(tick.clone()));
        let node = host(scene);
        down(scene, &pointer(&node, 3, 0.75, 0.0));
        assert!(same(scene.capture.target(), &node), "the first session captures");
        scene.dispatcher().dispatch(&InputEvent::MouseButtonUp(pointer(&node, 3, 0.75, 0.0)));
        tick.set(1);
        scene.settle(2, 0.0);
        down(scene, &pointer(&node, 3, 0.75, 0.0));
        assert!(
            scene.capture.target().is_none(),
            "GAP: Swift's rating session survives `_updateNode`; `Rating::new` builds a fresh \
             RatingSession that is never mounted on the reused node, so after a parent \
             re-render a press no longer captures the pointer"
        );
    });
}

#[test]
fn paint() {
    for resource in [ui_icon_catalog::status::star(), ui_icon_catalog::status::star_fill()] {
        let path = resource.path().expect("the icon resolves");
        let bitmap = cove_imaging::ImageDecoder::decode(
            &path,
            Some(cove_imaging::PixelSize { width: 40, height: 40 }),
        )
        .expect("the SVG decodes");
        assert!(bitmap.width() == 40 && bitmap.height() == 40);
        assert!(bitmap.pixels().iter().skip(3).step_by(4).any(|alpha| *alpha > 0));
    }
    InteractiveScene::run(300.0, 100.0, |scene| {
        scene.install(Rating::new("", 2.5, |_| {}));
        let node = host(scene);
        let mut list = DrawList::default();
        (node.draw().expect("a painter"))(&mut list, Point::default());
        assert_eq!(list.vertices().len(), 32, "five outlines and three fills");
        let textures: HashSet<u32> = list.batches().iter().map(|batch| batch.texture_id).collect();
        assert_eq!(textures.len(), 2, "one outline and one fill raster");
        let fractional: Vec<_> = list
            .batches()
            .iter()
            .filter_map(|batch| batch.scissor)
            .filter(|scissor| scissor.width == 10.0)
            .collect();
        assert!(
            fractional.len() == 1 && fractional[0].x == 72.0 && fractional[0].height == 20.0,
            "the half star clips to its filled half: {fractional:?}"
        );
        assert!(list.current_clip().is_none());
        let unfocused = list.vertices().len();
        scene.focus.focus(Some(&node), true);
        list.reset();
        (node.draw().expect("a painter"))(&mut list, Point::default());
        assert!(list.vertices().len() > unfocused && list.current_clip().is_none());
    });
}
