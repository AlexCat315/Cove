//! Translation of `Tests/CoveUIComposeTests/ButtonPressRecomposeTests.swift`.
//!
//! Behaviour checklist:
//! - a resting button paints the accent colour, a pointer down paints the pressed
//!   colour, and release returns to the accent
//! - hover enter paints the hover colour and leave returns to the accent
//! - dragging outside a pressed button clears press and hover, moves pointer
//!   capture, and cancels the action on release
//! - dragging back inside restores the pressed paint and commits the action once
//!
//! The pointer transitions are driven through the registry's own handlers, and the
//! implicit press animation is settled by ticking the scheduler to its terminal
//! value, exactly as the Swift suite did.
//!
//! Named difference: the Swift drag cases assigned `host.frame` directly
//! (`CGRect(x: 0, y: 0, width: 100, height: 32)`); the Rust host computes its frame
//! from layout, so the scene is laid out at 100x32 instead. The pointer
//! coordinates and the resulting paint are unchanged.

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{
    AnimatorScheduler, EventPhase, EventResult, HoverPhase, InteractionRegistry,
    InteractionRegistryHolder, Node, PointerCapture, PointerCaptureHolder, PointerPhase,
};
use cove_platform::{MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::text::Text;

use support::GlobalTestLock;

/// The first node with a non-transparent background — the button chrome.
fn find_filled(root: &Node) -> Option<Node> {
    if root.background_color().is_some_and(|color| color.a > 0.0) {
        return Some(root.clone());
    }
    root.children().iter().find_map(find_filled)
}

fn button_host(root: &Node) -> Node {
    fn walk(node: &Node) -> Option<Node> {
        if node
            .accessibility()
            .is_some_and(|semantics| semantics.role == cove_core::scene::AccessibilityRole::Button)
        {
            return Some(node.clone());
        }
        node.children().iter().find_map(walk)
    }
    walk(root).expect("the button host is mounted")
}

fn down() -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x: 10.0,
        y: 10.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn up(x: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y: 10.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn motion(x: f32, delta_x: f32) -> MouseMotionEvent {
    MouseMotionEvent {
        x,
        y: 10.0,
        delta_x,
        delta_y: 0.0,
        kind: Default::default(),
    }
}

/// Installs a button and returns the graph, host, and settled chrome colour.
fn scene(label: &str, action: impl Fn() + 'static) -> (Rc<cove_composition::ViewGraph>, Node) {
    if InteractionRegistryHolder::current().is_none() {
        InteractionRegistryHolder::set_current(Some(Rc::new(InteractionRegistry::new())));
    }
    let graph = support::graph();
    graph.install_view(
        AnyView::new(Button::new(Text::new(label), action)),
        Default::default(),
    );
    // The drag cases compare pointer coordinates against the host's frame, so the
    // graph must have been laid out. Swift set the frame by hand.
    graph.compute_layout(100.0, 32.0);
    let root = graph.tree.borrow().root().expect("root");
    let host = button_host(&root);
    (graph, host)
}

fn filled_colour(graph: &Rc<cove_composition::ViewGraph>) -> cove_core::Color {
    let root = graph.tree.borrow().root().expect("root");
    find_filled(&root)
        .and_then(|node| node.background_color())
        .expect("the button paints a background")
}

#[test]
fn pointer_down_paints_the_pressed_chrome() {
    GlobalTestLock::with_scheduler(|| {
        let theme = cove_ui_compose::theme::Theme::default_dark();
        let resting = theme.colors.accent;
        let pressed = theme.colors.accent_pressed;
        let (_graph, host) = scene("Tap", || {});
        let registry = InteractionRegistryHolder::current().expect("registry installed");
        let handler = registry
            .handlers(&host)
            .pointer
            .expect("the button registers a pointer handler");

        assert_eq!(
            filled_colour(&_graph),
            resting,
            "a resting button paints the accent colour"
        );

        let _ = handler(&down(), PointerPhase::Down, EventPhase::Target);
        _graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&_graph),
            pressed,
            "a pointer down paints the pressed colour"
        );

        let _ = handler(&down(), PointerPhase::Up, EventPhase::Target);
        _graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&_graph),
            resting,
            "release returns to the accent colour"
        );
    });
}

#[test]
fn hover_enter_paints_the_hover_chrome() {
    GlobalTestLock::with_scheduler(|| {
        let theme = cove_ui_compose::theme::Theme::default_dark();
        let resting = theme.colors.accent;
        let hovered = theme.colors.accent_hover;
        let (graph, host) = scene("Hover", || {});
        let registry = InteractionRegistryHolder::current().expect("registry installed");
        let hover = registry
            .handlers(&host)
            .hover
            .expect("the button registers a hover handler");

        assert_eq!(filled_colour(&graph), resting);

        hover(HoverPhase::Enter);
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&graph),
            hovered,
            "hover enter paints the hover colour"
        );

        hover(HoverPhase::Leave);
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&graph),
            resting,
            "hover leave returns to the accent colour"
        );
    });
}

#[test]
fn dragging_outside_cancels_the_press_and_the_action() {
    GlobalTestLock::with_scheduler(|| {
        let capture = Rc::new(PointerCapture::new());
        PointerCaptureHolder::set_current(Some(capture.clone()));
        let theme = cove_ui_compose::theme::Theme::default_dark();
        let resting = theme.colors.accent;
        let pressed = theme.colors.accent_pressed;

        let fired = Rc::new(Cell::new(0));
        let counter = fired.clone();
        let (graph, host) = scene("Drag", move || counter.set(counter.get() + 1));
        let registry = InteractionRegistryHolder::current().expect("registry installed");
        let handlers = registry.handlers(&host);
        let pointer = handlers.pointer.expect("pointer handler");
        let motion_handler = handlers.motion.expect("motion handler");

        assert_eq!(
            pointer(&down(), PointerPhase::Down, EventPhase::Target),
            EventResult::Handled,
            "the press is handled"
        );
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&graph),
            pressed,
            "the press paints the pressed colour"
        );
        assert_eq!(
            capture.target().map(|node| node.id()),
            Some(host.id()),
            "the press takes pointer capture"
        );

        assert_eq!(
            motion_handler(&motion(140.0, 130.0), EventPhase::Target),
            EventResult::Handled,
            "dragging outside is handled"
        );
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&graph),
            resting,
            "dragging outside clears the pressed paint"
        );

        assert_eq!(
            pointer(&up(140.0), PointerPhase::Up, EventPhase::Target),
            EventResult::Handled,
            "the release is handled"
        );
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&graph),
            resting,
            "the release leaves the resting paint"
        );
        assert_eq!(fired.get(), 0, "a release outside does not run the action");
        assert!(
            capture.target().is_none(),
            "the release drops pointer capture"
        );
        PointerCaptureHolder::set_current(None);
    });
}

#[test]
fn dragging_back_inside_commits_the_press_once() {
    GlobalTestLock::with_scheduler(|| {
        let capture = Rc::new(PointerCapture::new());
        PointerCaptureHolder::set_current(Some(capture.clone()));
        let theme = cove_ui_compose::theme::Theme::default_dark();
        let pressed = theme.colors.accent_pressed;

        let fired = Rc::new(Cell::new(0));
        let counter = fired.clone();
        let (graph, host) = scene("Drag", move || counter.set(counter.get() + 1));
        let registry = InteractionRegistryHolder::current().expect("registry installed");
        let handlers = registry.handlers(&host);
        let pointer = handlers.pointer.expect("pointer handler");
        let motion_handler = handlers.motion.expect("motion handler");

        assert_eq!(
            pointer(&down(), PointerPhase::Down, EventPhase::Target),
            EventResult::Handled
        );
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);

        assert_eq!(
            motion_handler(&motion(140.0, 130.0), EventPhase::Target),
            EventResult::Handled,
            "dragging outside is handled"
        );
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);

        assert_eq!(
            motion_handler(&motion(20.0, -120.0), EventPhase::Target),
            EventResult::Handled,
            "dragging back inside is handled"
        );
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            filled_colour(&graph),
            pressed,
            "dragging back inside restores the pressed paint"
        );

        assert_eq!(
            pointer(&up(20.0), PointerPhase::Up, EventPhase::Target),
            EventResult::Handled
        );
        graph.recomposer.commit_all();
        AnimatorScheduler::current().tick(1.0);
        assert_eq!(
            fired.get(),
            1,
            "the release commits the action exactly once"
        );
        assert!(
            capture.target().is_none(),
            "the release drops pointer capture"
        );
        PointerCaptureHolder::set_current(None);
    });
}
