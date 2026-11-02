//! Translation of `Tests/CoveUIComposeTests/ButtonStyleAnimationTests.swift`.
//!
//! Drives a real `Button` through the interaction registry so the press
//! transition exercises the full production path: pointer handler →
//! node-local `ButtonHost` interaction state → built-in chrome node writes →
//! repaint.
//!
//! Behaviour checklist:
//! - `PrimaryButtonStyle` swaps to `accent_pressed` on a real pointer-down and
//!   schedules no tween
//! - `DestructiveButtonStyle` swaps to `error` composited over the pressed
//!   state layer, also unanimated
//! - a first render leaves no animation controller active

use crate::support;

use std::rc::Rc;

use cove_composition::AnyView;
use cove_composition::ViewGraph;
use cove_core::scene::{
    AnimatorScheduler, EventPhase, InteractionRegistry, InteractionRegistryHolder, Node, NodeTree,
    PointerPhase, Recomposer,
};
use cove_platform::MouseButtonEvent;
use cove_ui_compose::primitives::controls::button::Button;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::{ButtonStyleViewExt, DestructiveButtonStyle};
use cove_ui_compose::theme::{Theme, ThemeViewExt};

use support::GlobalTestLock;

/// The innermost node with a painted, non-transparent background — the button
/// face the chrome wrote.
fn find_filled(root: &Node) -> Option<Node> {
    if root.background_color().is_some_and(|color| color.a > 0.0) {
        return Some(root.clone());
    }
    for child in root.children() {
        if let Some(found) = find_filled(&child) {
            return Some(found);
        }
    }
    None
}

/// The inner hit-testable host, found children-first so the graph root anchor
/// does not shadow it.
fn find_hit_testable(root: &Node) -> Option<Node> {
    for child in root.children() {
        if let Some(found) = find_hit_testable(&child) {
            return Some(found);
        }
    }
    root.is_hit_testable().then(|| root.clone())
}

fn install(view: AnyView) -> (Rc<InteractionRegistry>, Rc<ViewGraph>) {
    let registry = Rc::new(InteractionRegistry::new());
    InteractionRegistryHolder::set_current(Some(registry.clone()));
    let graph = Rc::new(ViewGraph::new(
        Rc::new(std::cell::RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ));
    graph.install_view(view, Default::default());
    (registry, graph)
}

fn press(registry: &InteractionRegistry, root: &Node) {
    let host = find_hit_testable(root).expect("the button host is hit-testable");
    let event = MouseButtonEvent {
        button: cove_platform::MouseButton::Left,
        x: 0.0,
        y: 0.0,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    let handler = registry
        .handlers(&host)
        .pointer
        .expect("the button registers a pointer handler");
    handler(&event, PointerPhase::Down, EventPhase::Target);
}

#[test]
fn primary_style_swaps_its_face_on_pointer_down() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let theme = Theme::default_dark();
            let resting = theme.colors.accent;
            let pressed = theme.colors.accent_pressed;

            let (registry, graph) = install(AnyView::new(
                Button::new(Text::new("Hi"), || {}).theme(theme.clone()),
            ));

            let root = graph.tree.borrow().root().expect("root");
            assert_eq!(
                find_filled(&root).and_then(|node| node.background_color()),
                Some(resting),
                "the resting face is the accent colour"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "a resting button animates nothing"
            );

            press(&registry, &root);
            graph.recomposer.commit_all();

            let root = graph.tree.borrow().root().expect("root");
            assert_eq!(
                find_filled(&root).and_then(|node| node.background_color()),
                Some(pressed),
                "the face is already pressed on the frame of the event"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "built-in chrome writes are deliberately unanimated"
            );

            scheduler.tick(0.06);
            let root = graph.tree.borrow().root().expect("root");
            assert_eq!(
                find_filled(&root).and_then(|node| node.background_color()),
                Some(pressed),
                "a later tick does not move the face"
            );
            InteractionRegistryHolder::set_current(None);
        });
    });
}

#[test]
fn destructive_style_swaps_its_face_on_pointer_down() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let theme = Theme::default_dark();
            let resting = theme.colors.error;
            let pressed = theme
                .colors
                .error
                .composited(theme.colors.state_layer_pressed);

            let (registry, graph) = install(AnyView::new(
                Button::new(Text::new("X"), || {})
                    .button_style(DestructiveButtonStyle)
                    .theme(theme.clone()),
            ));

            let root = graph.tree.borrow().root().expect("root");
            assert_eq!(
                find_filled(&root).and_then(|node| node.background_color()),
                Some(resting),
                "the resting face is the error colour"
            );

            press(&registry, &root);
            graph.recomposer.commit_all();

            let root = graph.tree.borrow().root().expect("root");
            assert_eq!(
                find_filled(&root).and_then(|node| node.background_color()),
                Some(pressed),
                "pressing composites the pressed state layer over the error fill"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "the swap schedules no controller"
            );
            InteractionRegistryHolder::set_current(None);
        });
    });
}

#[test]
fn first_render_snaps_with_no_controllers_active() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let (_registry, _graph) = install(AnyView::new(Button::new(Text::new("Hi"), || {})));
            assert_eq!(
                scheduler.active_count(),
                0,
                "mounting a button must not start an animation"
            );
            InteractionRegistryHolder::set_current(None);
        });
    });
}
