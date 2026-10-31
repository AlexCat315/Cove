//! Translation of `Tests/CoveUIRuntimeTests/EventDispatcherTests.swift`.
//!
//! Behaviour checklist:
//! - a pointer down captures root → mid, targets the leaf and bubbles back up
//! - `Handled` during capture stops the target and bubble phases
//! - `PointerCapture` overrides hit-testing for motion events
//! - releasing capture on pointer up refreshes hover at the current cursor
//! - a left click auto-focuses the deepest focusable node
//! - motion emits enter/leave only for changed path segments
//! - the wheel goes to the deepest target before ancestors
//! - the wheel can hit-test from its own cursor coordinates without motion
//! - high-priority chrome preempts a target control
//! - key-up uses key-up handlers and does not replay key-down handlers
//! - the global shortcut route receives a key the focus chain ignores
//! - a handled focused key wins before the global shortcut fallback
//! - an overlay handles keys before pointer capture and viewport focus

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::Rect;
use cove_core::scene::{
    EventDispatcher, EventPhase, EventResult, FocusChain, HoverPhase, InputDispatchTrace,
    InputHandlerRole, InputHandlerRoute, InteractionRegistry, Node, NodeTree, PointerCapture,
    PointerPhase,
};
use cove_platform::{
    InputEvent, KeyEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent,
    PointerKind, ScrollInput, WindowId,
};

#[derive(Default)]
struct Recorder {
    capture: Vec<String>,
    target: Vec<String>,
    bubble: Vec<String>,
    hover: Vec<String>,
}

struct Scene {
    recorder: Rc<RefCell<Recorder>>,
    leaf: Node,
    dispatcher: EventDispatcher,
}

fn framed(x: f32, y: f32, width: f32, height: f32) -> Node {
    let node = Node::new();
    node.set_frame(Rect::new(x, y, width, height));
    node
}

fn phase_name(phase: EventPhase) -> &'static str {
    match phase {
        EventPhase::Capture => "capture",
        EventPhase::Target => "target",
        EventPhase::Bubble => "bubble",
    }
}

fn hover_name(phase: HoverPhase) -> &'static str {
    match phase {
        HoverPhase::Enter => "enter",
        HoverPhase::Leave => "leave",
    }
}

fn register(
    interactions: &InteractionRegistry,
    recorder: &Rc<RefCell<Recorder>>,
    node: &Node,
    name: &str,
    handle: bool,
) {
    let label = name.to_string();
    let pointer = recorder.clone();
    let pointer_label = label.clone();
    interactions.set_pointer(
        node,
        InputHandlerRoute::control(),
        Rc::new(
            move |_: &MouseButtonEvent, _: PointerPhase, phase: EventPhase| {
                record(&pointer, &pointer_label, phase);
                result(handle)
            },
        ),
    );
    let hover = recorder.clone();
    let hover_label = label.clone();
    interactions.set_hover(
        node,
        Rc::new(move |phase: HoverPhase| {
            let entry = format!("{hover_label}:{}", hover_name(phase));
            hover.borrow_mut().hover.push(entry);
        }),
    );
    for slot in ["motion", "wheel", "key"] {
        let handler = recorder.clone();
        let handler_label = label.clone();
        match slot {
            "motion" => interactions.set_motion(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |_: &MouseMotionEvent, phase: EventPhase| {
                    record(&handler, &handler_label, phase);
                    result(handle)
                }),
            ),
            "wheel" => interactions.set_wheel(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |_: &MouseWheelEvent, phase: EventPhase| {
                    record(&handler, &handler_label, phase);
                    result(handle)
                }),
            ),
            _ => interactions.set_key(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |_: &KeyEvent, phase: EventPhase| {
                    record(&handler, &handler_label, phase);
                    result(handle)
                }),
            ),
        }
    }
}

fn record(recorder: &Rc<RefCell<Recorder>>, label: &str, phase: EventPhase) {
    let mut recorder = recorder.borrow_mut();
    let entry = format!("{label}:{}", phase_name(phase));
    match phase {
        EventPhase::Capture => recorder.capture.push(entry),
        EventPhase::Target => recorder.target.push(entry),
        EventPhase::Bubble => recorder.bubble.push(entry),
    }
}

fn result(handle: bool) -> EventResult {
    if handle {
        EventResult::Handled
    } else {
        EventResult::Ignored
    }
}

impl Scene {
    /// The Swift `setup()` tree: root 200², mid at (10,10) 100², leaf at (5,5) 30².
    fn new(handle_root: bool) -> Self {
        let root = framed(0.0, 0.0, 200.0, 200.0);
        let mid = framed(10.0, 10.0, 100.0, 100.0);
        let leaf = framed(5.0, 5.0, 30.0, 30.0);
        root.add_child(&mid);
        mid.add_child(&leaf);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        tree.borrow_mut().set_root(Some(root.clone()));
        let interactions = Rc::new(InteractionRegistry::new());
        let recorder = Rc::new(RefCell::new(Recorder::default()));
        register(&interactions, &recorder, &root, "root", handle_root);
        register(&interactions, &recorder, &mid, "mid", false);
        register(&interactions, &recorder, &leaf, "leaf", false);
        let capture = Rc::new(PointerCapture::new());
        let focus = Rc::new(FocusChain::new());
        let dispatcher = EventDispatcher::new(
            tree.clone(),
            interactions.clone(),
            capture.clone(),
            focus.clone(),
            WindowId::MAIN,
        );
        Self {
            recorder,
            leaf,
            dispatcher,
        }
    }

    fn recorder(&self) -> std::cell::Ref<'_, Recorder> {
        self.recorder.borrow()
    }
}

fn left_down(x: f32, y: f32) -> InputEvent {
    InputEvent::MouseButtonDown(MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: PointerKind::default(),
    })
}

fn left_up(x: f32, y: f32) -> InputEvent {
    InputEvent::MouseButtonUp(MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: PointerKind::default(),
    })
}

fn motion(x: f32, y: f32, delta_x: f32, delta_y: f32) -> InputEvent {
    InputEvent::MouseMotion(MouseMotionEvent {
        x,
        y,
        delta_x,
        delta_y,
        kind: PointerKind::default(),
    })
}

fn wheel(x: f32, y: f32, mouse_x: Option<f32>, mouse_y: Option<f32>) -> InputEvent {
    InputEvent::MouseWheel(MouseWheelEvent {
        x,
        y,
        mouse_x,
        mouse_y,
        input: ScrollInput::default(),
    })
}

fn key(scancode: u32) -> KeyEvent {
    KeyEvent {
        scancode,
        keycode: 0,
        modifiers: Default::default(),
        is_repeat: false,
    }
}

fn assert_matches(actual: &[String], expected: &[String], message: &str) {
    assert_eq!(actual, expected, "{message}");
}

#[test]
fn capture_target_bubble() {
    let scene = Scene::new(false);

    // Point lands inside leaf (root 0,0 + mid 10,10 + leaf 5,5 = 15,15;
    // leaf is 30x30 so a point at 20,20 is well inside).
    scene.dispatcher.dispatch(&left_down(20.0, 20.0));

    let recorder = scene.recorder();
    assert_matches(
        &recorder.capture,
        &["root:capture".into(), "mid:capture".into()],
        "capture root→mid",
    );
    assert_matches(&recorder.target, &["leaf:target".into()], "target leaf");
    assert_matches(
        &recorder.bubble,
        &["mid:bubble".into(), "root:bubble".into()],
        "bubble mid→root",
    );
}

#[test]
fn capture_handled() {
    let recorder = Rc::new(RefCell::new(Recorder::default()));
    let interactions = Rc::new(InteractionRegistry::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let leaf = framed(10.0, 10.0, 30.0, 30.0);
    root.add_child(&leaf);
    tree.borrow_mut().set_root(Some(root.clone()));
    register(&interactions, &recorder, &root, "root", true);
    register(&interactions, &recorder, &leaf, "leaf", false);

    let dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        Rc::new(FocusChain::new()),
        WindowId::MAIN,
    );
    dispatcher.dispatch(&left_down(20.0, 20.0));

    let recorder = recorder.borrow();
    assert_matches(
        &recorder.capture,
        &["root:capture".into()],
        "only the handled capture ran",
    );
    assert!(
        recorder.target.is_empty(),
        "capture handling stops the target phase"
    );
    assert!(
        recorder.bubble.is_empty(),
        "capture handling stops the bubble phase"
    );
}

#[test]
fn pointer_capture_motion() {
    let scene = Scene::new(false);
    scene.dispatcher.capture.acquire(&scene.leaf, false);

    // Point far outside leaf — would normally miss or land elsewhere.
    scene.dispatcher.dispatch(&motion(500.0, 500.0, 1.0, 1.0));

    assert_matches(
        &scene.recorder().target,
        &["leaf:target".into()],
        "capture target receives motion",
    );
}

#[test]
fn pointer_up_after_capture_release_refreshes_hover() {
    let recorder = Rc::new(RefCell::new(Recorder::default()));
    let interactions = Rc::new(InteractionRegistry::new());
    let capture = Rc::new(PointerCapture::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 240.0, 120.0);
    let left = framed(0.0, 0.0, 80.0, 80.0);
    let right = framed(120.0, 0.0, 80.0, 80.0);
    root.add_child(&left);
    root.add_child(&right);
    tree.borrow_mut().set_root(Some(root.clone()));

    for (node, label) in [(&root, "root"), (&left, "left"), (&right, "right")] {
        let hover = recorder.clone();
        let hover_label = label.to_string();
        interactions.set_hover(
            node,
            Rc::new(move |phase: HoverPhase| {
                let entry = format!("{hover_label}:{}", hover_name(phase));
                hover.borrow_mut().hover.push(entry);
            }),
        );
    }
    let down_capture = capture.clone();
    let down_node = left.clone();
    let up_capture = capture.clone();
    interactions.set_pointer(
        &left,
        InputHandlerRoute::control(),
        Rc::new(
            move |_: &MouseButtonEvent, phase: PointerPhase, _: EventPhase| match phase {
                PointerPhase::Down => {
                    down_capture.acquire(&down_node, false);
                    EventResult::Handled
                }
                PointerPhase::Up => {
                    up_capture.release();
                    EventResult::Handled
                }
            },
        ),
    );

    let dispatcher = EventDispatcher::new(
        tree,
        interactions,
        capture,
        Rc::new(FocusChain::new()),
        WindowId::MAIN,
    );

    dispatcher.dispatch(&motion(20.0, 20.0, 0.0, 0.0));
    recorder.borrow_mut().hover.clear();
    dispatcher.dispatch(&left_down(20.0, 20.0));
    dispatcher.dispatch(&motion(140.0, 20.0, 120.0, 0.0));
    dispatcher.dispatch(&left_up(140.0, 20.0));

    assert_matches(
        &recorder.borrow().hover,
        &["left:leave".into(), "right:enter".into()],
        "hover refreshes at the release cursor",
    );
}

#[test]
fn auto_focus() {
    let scene = Scene::new(false);
    scene.leaf.set_is_focusable(true);

    scene.dispatcher.dispatch(&left_down(20.0, 20.0));

    let focused = scene
        .dispatcher
        .focus_chain
        .focused()
        .expect("a focused node");
    assert_eq!(
        focused.id(),
        scene.leaf.id(),
        "the clicked leaf takes focus"
    );
}

#[test]
fn hover_path_diff() {
    let scene = Scene::new(false);

    scene.dispatcher.dispatch(&motion(20.0, 20.0, 1.0, 1.0));
    assert_matches(
        &scene.recorder().hover,
        &["root:enter".into(), "mid:enter".into(), "leaf:enter".into()],
        "entering the deep path reports every segment",
    );

    scene.recorder.borrow_mut().hover.clear();
    scene.dispatcher.dispatch(&motion(90.0, 90.0, 1.0, 1.0));
    assert_matches(
        &scene.recorder().hover,
        &["leaf:leave".into()],
        "only the leaf leaves",
    );

    scene.recorder.borrow_mut().hover.clear();
    scene.dispatcher.dispatch(&motion(240.0, 240.0, 1.0, 1.0));
    assert_matches(
        &scene.recorder().hover,
        &["mid:leave".into(), "root:leave".into()],
        "leaving the window unwinds leaf-first",
    );
}

#[test]
fn wheel_prefers_deepest_target() {
    let recorder = Rc::new(RefCell::new(Recorder::default()));
    let interactions = Rc::new(InteractionRegistry::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let mid = framed(10.0, 10.0, 100.0, 100.0);
    let leaf = framed(5.0, 5.0, 30.0, 30.0);
    root.add_child(&mid);
    mid.add_child(&leaf);
    tree.borrow_mut().set_root(Some(root.clone()));
    register(&interactions, &recorder, &root, "root", false);
    register(&interactions, &recorder, &mid, "mid", true);
    register(&interactions, &recorder, &leaf, "leaf", true);

    let dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        Rc::new(FocusChain::new()),
        WindowId::MAIN,
    );

    dispatcher.dispatch(&motion(20.0, 20.0, 0.0, 0.0));
    *recorder.borrow_mut() = Recorder::default();
    dispatcher.dispatch(&wheel(0.0, -1.0, None, None));

    let recorder = recorder.borrow();
    assert!(
        recorder.capture.is_empty(),
        "the wheel takes no capture pass"
    );
    assert_matches(
        &recorder.target,
        &["leaf:target".into()],
        "the deepest target consumes the wheel",
    );
    assert!(
        recorder.bubble.is_empty(),
        "a handled wheel target stops the bubble"
    );
}

#[test]
fn wheel_uses_event_cursor_coordinates() {
    let recorder = Rc::new(RefCell::new(Recorder::default()));
    let interactions = Rc::new(InteractionRegistry::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let leaf = framed(40.0, 40.0, 80.0, 80.0);
    root.add_child(&leaf);
    tree.borrow_mut().set_root(Some(root));
    register(&interactions, &recorder, &leaf, "leaf", true);

    let dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        Rc::new(FocusChain::new()),
        WindowId::MAIN,
    );
    dispatcher.dispatch(&wheel(0.0, -1.0, Some(60.0), Some(60.0)));

    let recorder = recorder.borrow();
    assert_matches(
        &recorder.target,
        &["leaf:target".into()],
        "the wheel hit-tests its own cursor",
    );
    assert!(recorder.bubble.is_empty());
}

#[test]
fn chrome_route_preempts_target_control() {
    let recorder = Rc::new(RefCell::new(Vec::new()));
    let interactions = Rc::new(InteractionRegistry::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let leaf = framed(20.0, 20.0, 80.0, 80.0);
    root.add_child(&leaf);
    tree.borrow_mut().set_root(Some(root.clone()));

    let chrome = recorder.clone();
    interactions.set_pointer(
        &root,
        InputHandlerRoute::scroll_chrome(),
        Rc::new(
            move |_: &MouseButtonEvent, _: PointerPhase, phase: EventPhase| {
                chrome
                    .borrow_mut()
                    .push(format!("chrome:{}", phase_name(phase)));
                EventResult::Handled
            },
        ),
    );
    let control = recorder.clone();
    interactions.set_pointer(
        &leaf,
        InputHandlerRoute::control(),
        Rc::new(
            move |_: &MouseButtonEvent, _: PointerPhase, phase: EventPhase| {
                control
                    .borrow_mut()
                    .push(format!("leaf:{}", phase_name(phase)));
                EventResult::Handled
            },
        ),
    );

    let mut dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        Rc::new(FocusChain::new()),
        WindowId::MAIN,
    );
    let traces: Rc<RefCell<Vec<InputDispatchTrace>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = traces.clone();
    dispatcher.trace_sink = Some(Rc::new(move |trace: &InputDispatchTrace| {
        sink.borrow_mut().push(trace.clone());
    }));

    dispatcher.dispatch(&left_down(40.0, 40.0));

    assert_matches(
        recorder.borrow().as_slice(),
        &["chrome:capture".into()],
        "chrome preempts the control",
    );
    let traces = traces.borrow();
    let first = traces.first().expect("a dispatch trace was emitted");
    assert_eq!(
        first.route.as_ref().map(|route| route.role.clone()),
        Some(InputHandlerRole::ScrollChrome),
        "the trace names the chrome route",
    );
    assert_eq!(first.result, EventResult::Handled);
}

#[test]
fn key_up_has_distinct_delivery() {
    let interactions = Rc::new(InteractionRegistry::new());
    let focus = Rc::new(FocusChain::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let leaf = framed(10.0, 10.0, 40.0, 40.0);
    leaf.set_is_focusable(true);
    root.add_child(&leaf);
    tree.borrow_mut().set_root(Some(root));
    focus.focus(Some(&leaf), false);

    let downs = Rc::new(RefCell::new(0));
    let ups = Rc::new(RefCell::new(0));
    let down_counter = downs.clone();
    interactions.set_key(
        &leaf,
        InputHandlerRoute::control(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            *down_counter.borrow_mut() += 1;
            EventResult::Handled
        }),
    );
    let up_counter = ups.clone();
    interactions.set_key_up(
        &leaf,
        InputHandlerRoute::control(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            *up_counter.borrow_mut() += 1;
            EventResult::Handled
        }),
    );

    let dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        focus,
        WindowId::MAIN,
    );
    let event = key(44);

    dispatcher.dispatch(&InputEvent::KeyUp(event.clone()));
    assert_eq!(*downs.borrow(), 0, "key-up does not replay the key handler");
    assert_eq!(*ups.borrow(), 1);

    dispatcher.dispatch(&InputEvent::KeyDown(event));
    assert_eq!(*downs.borrow(), 1);
    assert_eq!(*ups.borrow(), 1);
}

#[test]
fn global_shortcut_fallback_receives_key() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let interactions = Rc::new(InteractionRegistry::new());
    let focus = Rc::new(FocusChain::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let focused = framed(0.0, 0.0, 40.0, 40.0);
    focused.set_is_focusable(true);
    let global = framed(50.0, 0.0, 40.0, 40.0);
    root.add_child(&focused);
    root.add_child(&global);
    tree.borrow_mut().set_root(Some(root));
    focus.focus(Some(&focused), false);

    let focused_calls = calls.clone();
    interactions.set_key(
        &focused,
        InputHandlerRoute::control(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            focused_calls.borrow_mut().push("focused".into());
            EventResult::Ignored
        }),
    );
    let shortcut_calls = calls.clone();
    interactions.set_key(
        &global,
        InputHandlerRoute::shortcut(),
        Rc::new(move |_: &KeyEvent, phase: EventPhase| {
            shortcut_calls
                .borrow_mut()
                .push(format!("shortcut:{}", phase_name(phase)));
            EventResult::Handled
        }),
    );

    let dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        focus,
        WindowId::MAIN,
    );
    dispatcher.dispatch(&InputEvent::KeyDown(key(42)));

    assert_matches(
        calls.borrow().as_slice(),
        &["focused".into(), "shortcut:capture".into()],
        "an ignored focused key falls through to the shortcut route",
    );
}

#[test]
fn focused_key_handler_wins_before_global_shortcut() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let interactions = Rc::new(InteractionRegistry::new());
    let focus = Rc::new(FocusChain::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let focused = framed(0.0, 0.0, 40.0, 40.0);
    focused.set_is_focusable(true);
    let global = framed(50.0, 0.0, 40.0, 40.0);
    root.add_child(&focused);
    root.add_child(&global);
    tree.borrow_mut().set_root(Some(root));
    focus.focus(Some(&focused), false);

    let focused_calls = calls.clone();
    interactions.set_key(
        &focused,
        InputHandlerRoute::control(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            focused_calls.borrow_mut().push("focused".into());
            EventResult::Handled
        }),
    );
    let shortcut_calls = calls.clone();
    interactions.set_key(
        &global,
        InputHandlerRoute::shortcut(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            shortcut_calls.borrow_mut().push("shortcut".into());
            EventResult::Handled
        }),
    );

    let dispatcher = EventDispatcher::new(
        tree,
        interactions,
        Rc::new(PointerCapture::new()),
        focus,
        WindowId::MAIN,
    );
    dispatcher.dispatch(&InputEvent::KeyDown(key(42)));

    assert_matches(
        calls.borrow().as_slice(),
        &["focused".into()],
        "the focused handler wins alone",
    );
}

#[test]
fn overlay_key_route_has_first_refusal() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let interactions = Rc::new(InteractionRegistry::new());
    let focus = Rc::new(FocusChain::new());
    let capture = Rc::new(PointerCapture::new());
    let tree = Rc::new(RefCell::new(NodeTree::new()));

    let root = framed(0.0, 0.0, 200.0, 200.0);
    let viewport = framed(0.0, 0.0, 100.0, 100.0);
    viewport.set_is_focusable(true);
    let dragged = framed(0.0, 0.0, 100.0, 100.0);
    let overlay = framed(20.0, 20.0, 80.0, 80.0);
    root.add_child(&viewport);
    root.add_child(&dragged);
    root.add_child(&overlay);
    tree.borrow_mut().set_root(Some(root));
    focus.focus(Some(&viewport), false);
    capture.acquire(&dragged, false);

    let viewport_calls = calls.clone();
    interactions.set_key(
        &viewport,
        InputHandlerRoute::viewport(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            viewport_calls.borrow_mut().push("viewport".into());
            EventResult::Handled
        }),
    );
    let drag_calls = calls.clone();
    interactions.set_key(
        &dragged,
        InputHandlerRoute::workspace_drag(),
        Rc::new(move |_: &KeyEvent, _: EventPhase| {
            drag_calls.borrow_mut().push("capture".into());
            EventResult::Handled
        }),
    );
    let overlay_calls = calls.clone();
    interactions.set_key(
        &overlay,
        InputHandlerRoute::overlay(),
        Rc::new(move |_: &KeyEvent, phase: EventPhase| {
            overlay_calls
                .borrow_mut()
                .push(format!("overlay:{}", phase_name(phase)));
            EventResult::Handled
        }),
    );

    let dispatcher = EventDispatcher::new(tree, interactions, capture, focus, WindowId::MAIN);
    dispatcher.dispatch(&InputEvent::KeyDown(key(41)));

    assert_matches(
        calls.borrow().as_slice(),
        &["overlay:target".into()],
        "the mounted overlay refuses the key first",
    );
}
