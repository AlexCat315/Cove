//! Translation of `Tests/CoveUIRuntimeTests/WheelInputTests.swift`.
//!
//! A phased scroll stream must stay with the consumer that claimed it. The
//! interesting failures are not "the wrong view scrolled once" — they are the
//! ones where a gesture ends, the OS keeps sending momentum frames, and the
//! pointer moves elsewhere: a latched stream must not hand its tail to a
//! neighbouring viewport, and a click or a new gesture must break the latch.
//!
//! Behaviour checklist:
//! - gesture and OS momentum stay with their consumer; a click catches momentum
//!   and a new gesture restarts
//! - cancellation and phaseless line wheels release the previous viewport
//! - focused fallback is retained without a pointer, and a fresh known pointer
//!   selects its own viewport
//! - a global scroll fallback latches its consumer when an overlay intercepted
//!   hit testing
//! - a removed consumer cannot transfer its remaining momentum to a neighbour
//! - a new modal scope prevents a latched background viewport from scrolling
//! - resting fingers catch old momentum at `mayBegin`, before a new pan starts

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_core::Rect;
use cove_core::scene::{
    EventDispatcher, EventPhase, EventResult, FocusChain, InputHandlerRoute, InteractionRegistry,
    Node, NodeTree, PointerCapture, WheelRoutingPriority,
};
use cove_platform::{
    InputEvent, MouseButton, MouseButtonEvent, MouseWheelEvent, ScrollDevice, ScrollInput,
    ScrollPhase, ScrollUnit, WindowId,
};

struct Fixture {
    tree: Rc<RefCell<NodeTree>>,
    registry: Rc<InteractionRegistry>,
    focus: Rc<FocusChain>,
    left: Node,
    right: Node,
    dispatcher: EventDispatcher,
}

impl Fixture {
    fn new() -> Self {
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let registry = Rc::new(InteractionRegistry::new());
        let focus = Rc::new(FocusChain::new());
        let dispatcher = EventDispatcher::new(
            tree.clone(),
            registry.clone(),
            Rc::new(PointerCapture::new()),
            focus.clone(),
            WindowId::MAIN,
        );

        let root = Node::new();
        root.set_frame(Rect::new(0.0, 0.0, 400.0, 200.0));
        let left = Node::new();
        left.set_frame(Rect::new(0.0, 0.0, 180.0, 200.0));
        let right = Node::new();
        right.set_frame(Rect::new(200.0, 0.0, 180.0, 200.0));
        root.add_child(&left);
        root.add_child(&right);
        tree.borrow_mut().set_root(Some(root));

        Self {
            tree,
            registry,
            focus,
            left,
            right,
            dispatcher,
        }
    }

    fn wheel_with(&self, x: f32, phase: ScrollPhase, momentum: ScrollPhase) {
        self.dispatcher
            .dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
                x: 0.0,
                y: -5.0,
                mouse_x: Some(x),
                mouse_y: Some(50.0),
                input: ScrollInput {
                    unit: ScrollUnit::Points,
                    device: ScrollDevice::Continuous,
                    phase,
                    momentum_phase: momentum,
                    ..ScrollInput::default()
                },
            }));
    }

    fn wheel(&self, x: f32) {
        self.wheel_with(x, ScrollPhase::None, ScrollPhase::None);
    }

    fn click(&self, x: f32) {
        self.dispatcher
            .dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
                button: MouseButton::Left,
                x,
                y: 50.0,
                clicks: 1,
                ..MouseButtonEvent::default()
            }));
    }

    fn counter(&self, node: &Node, route: InputHandlerRoute) -> Rc<Cell<usize>> {
        let count = Rc::new(Cell::new(0));
        let tracked = count.clone();
        self.registry.set_wheel(
            node,
            route,
            Rc::new(move |_: &MouseWheelEvent, _: EventPhase| {
                tracked.set(tracked.get() + 1);
                EventResult::Handled
            }),
        );
        count
    }
}

#[test]
fn gesture_and_os_momentum_stay_with_their_consumer_and_a_click_catches_momentum() {
    let f = Fixture::new();
    let left = f.counter(&f.left, InputHandlerRoute::default());
    let right = f.counter(&f.right, InputHandlerRoute::default());

    f.wheel_with(50.0, ScrollPhase::Began, ScrollPhase::None);
    f.wheel_with(250.0, ScrollPhase::Changed, ScrollPhase::None);
    f.wheel_with(250.0, ScrollPhase::Ended, ScrollPhase::None);
    f.wheel_with(250.0, ScrollPhase::None, ScrollPhase::Began);
    assert_eq!(
        (left.get(), right.get()),
        (4, 0),
        "the phased stream stays with the viewport that claimed it",
    );

    f.click(250.0);
    f.wheel_with(250.0, ScrollPhase::None, ScrollPhase::Changed);
    f.wheel_with(250.0, ScrollPhase::None, ScrollPhase::Ended);
    assert_eq!(
        (left.get(), right.get()),
        (4, 0),
        "a click catches the remaining momentum instead of releasing it",
    );

    f.wheel_with(250.0, ScrollPhase::Began, ScrollPhase::None);
    f.wheel_with(250.0, ScrollPhase::Changed, ScrollPhase::None);
    assert_eq!(
        (left.get(), right.get()),
        (4, 2),
        "a new gesture restarts routing from the pointer",
    );
}

#[test]
fn cancellation_and_phaseless_line_wheels_release_the_previous_viewport() {
    let f = Fixture::new();
    let left = f.counter(&f.left, InputHandlerRoute::default());
    let right = f.counter(&f.right, InputHandlerRoute::default());

    f.wheel_with(50.0, ScrollPhase::Began, ScrollPhase::None);
    f.wheel_with(250.0, ScrollPhase::Cancelled, ScrollPhase::None);
    f.wheel_with(250.0, ScrollPhase::None, ScrollPhase::Began);
    assert_eq!(
        (left.get(), right.get()),
        (2, 0),
        "cancellation arrives at the latch owner before the momentum does",
    );

    f.wheel(250.0);
    assert_eq!(
        (left.get(), right.get()),
        (2, 1),
        "a phaseless wheel re-routes to the viewport under the pointer",
    );
}

#[test]
fn focused_fallback_is_retained_without_a_pointer_and_a_known_pointer_selects_its_own_viewport() {
    let f = Fixture::new();
    f.left.set_is_focusable(true);
    f.left
        .set_wheel_routing_priority(Some(WheelRoutingPriority::PreferFocused));
    f.focus.focus(Some(&f.left), false);
    let left = f.counter(&f.left, InputHandlerRoute::text_input());
    let right = f.counter(&f.right, InputHandlerRoute::scroll());

    f.dispatcher
        .dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
            x: 0.0,
            y: -1.0,
            ..MouseWheelEvent::default()
        }));
    assert_eq!(
        (left.get(), right.get()),
        (1, 0),
        "with no pointer the focused viewport is the fallback",
    );

    f.wheel(250.0);
    assert_eq!(
        (left.get(), right.get()),
        (1, 1),
        "a fresh known pointer selects the viewport under it",
    );
}

#[test]
fn a_global_scroll_fallback_latches_its_consumer_when_an_overlay_intercepted_hit_testing() {
    let f = Fixture::new();
    let left = f.counter(&f.left, InputHandlerRoute::scroll());
    let right = f.counter(&f.right, InputHandlerRoute::control());

    let root = f.tree.borrow().root().expect("root");
    let overlay = Node::new();
    overlay.set_frame(root.frame());
    overlay.set_is_hit_testable(true);
    root.add_child(&overlay);

    f.wheel_with(50.0, ScrollPhase::Began, ScrollPhase::None);
    assert_eq!(
        (left.get(), right.get()),
        (1, 0),
        "the global scroll fallback takes a wheel the overlay swallowed",
    );

    root.remove_child(&overlay);
    f.wheel_with(250.0, ScrollPhase::None, ScrollPhase::Changed);
    assert_eq!(
        (left.get(), right.get()),
        (2, 0),
        "momentum keeps the latch even after the overlay leaves",
    );
}

#[test]
fn a_removed_consumer_cannot_transfer_its_remaining_momentum_to_a_neighbouring_viewport() {
    let f = Fixture::new();
    let _left = f.counter(&f.left, InputHandlerRoute::default());
    let right = f.counter(&f.right, InputHandlerRoute::default());

    f.wheel_with(50.0, ScrollPhase::Began, ScrollPhase::None);
    f.tree.borrow().root().expect("root").remove_child(&f.left);
    f.wheel_with(250.0, ScrollPhase::None, ScrollPhase::Changed);
    assert_eq!(
        right.get(),
        0,
        "momentum from a removed consumer is dropped, not handed to a neighbour",
    );

    f.wheel_with(250.0, ScrollPhase::Began, ScrollPhase::None);
    assert_eq!(
        right.get(),
        1,
        "a new gesture over the neighbour starts fresh",
    );
}

#[test]
fn a_new_modal_scope_prevents_a_latched_background_viewport_from_scrolling() {
    let f = Fixture::new();
    let left = f.counter(&f.left, InputHandlerRoute::default());

    f.wheel_with(50.0, ScrollPhase::Began, ScrollPhase::None);
    f.focus.begin_modal(&f.right);
    f.wheel_with(50.0, ScrollPhase::None, ScrollPhase::Changed);
    assert_eq!(
        left.get(),
        1,
        "the modal scope blocks the latched background viewport",
    );
}

#[test]
fn resting_fingers_catch_old_momentum_at_may_begin_before_a_new_pan_starts() {
    let f = Fixture::new();
    let left = f.counter(&f.left, InputHandlerRoute::default());

    f.wheel_with(50.0, ScrollPhase::None, ScrollPhase::Began);
    f.wheel_with(50.0, ScrollPhase::MayBegin, ScrollPhase::None);
    f.wheel_with(50.0, ScrollPhase::None, ScrollPhase::Changed);
    assert_eq!(left.get(), 2, "mayBegin is delivered to the momentum owner",);

    f.wheel_with(50.0, ScrollPhase::Began, ScrollPhase::None);
    assert_eq!(left.get(), 3, "the new pan restarts the stream");
}
