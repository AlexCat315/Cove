//! Translation of `Tests/CoveUIComposeTests/ButtonIconTests.swift`.
//!
//! Behaviour checklist:
//! - touch controls reach 44 points and restore desktop density
//! - `Button::icon(.., Texture)` renders a button shell whose label is a
//!   16pt image child
//! - a click on an icon button invokes the action closure exactly once
//! - `Button::icon(.., File)` without an image registry falls back without
//!   crashing and still materialises a host
//!
//! Name mapping: `Button(icon:size:)` is `Button::icon(ButtonIconSource, size, ..)`
//! and `surfaceEnvironment(_)` is `.surface_environment(..)`.
//!
//! Named difference: Swift locates the button host by the `ButtonHost.pressedKey`
//! attachment, which Rust keeps private. The `__button_host` marker the host
//! writes in `update_node` is the same node, so the lookup uses that instead.

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{
    EventPhase, InteractionRegistry, InteractionRegistryHolder, Node, PointerPhase,
};
use cove_core::{Environment, InputCapabilities};
use cove_platform::MouseButtonEvent;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::primitives::controls::button::{Button, ButtonIconSource};

use support::GlobalTestLock;

/// The marker `ButtonHost::update_node` writes, mirroring Swift's
/// `ButtonHost.pressedKey` lookup.
const BUTTON_HOST_MARKER: &str = "__button_host";

fn find_button_host(root: &Node) -> Option<Node> {
    if root.attachment::<bool>(BUTTON_HOST_MARKER).is_some() {
        return Some(root.clone());
    }
    root.children().iter().find_map(find_button_host)
}

fn descendants(node: &Node) -> Vec<Node> {
    let mut collected = Vec::new();
    fn walk(node: &Node, into: &mut Vec<Node>) {
        into.push(node.clone());
        for child in node.children() {
            walk(&child, into);
        }
    }
    walk(node, &mut collected);
    collected
}

#[test]
fn adaptive_touch_target() {
    GlobalTestLock::locked(|| {
        let graph = support::graph();

        let mut touch = Environment::default();
        touch.input = InputCapabilities::TOUCH;
        graph.install_view(
            AnyView::new(
                Button::icon(ButtonIconSource::texture(7), 16.0, || {}).surface_environment(touch),
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);
        let root = graph.tree.borrow().root().expect("root");
        let host = find_button_host(&root).expect("a button host mounts");
        let frame = host.frame();
        assert!(
            frame.width >= 44.0 && frame.height >= 44.0,
            "a touch control reaches the 44pt target (got {}x{})",
            frame.width,
            frame.height
        );

        let mut desktop = Environment::default();
        desktop.input = InputCapabilities::POINTER | InputCapabilities::KEYBOARD;
        graph.install_view(
            AnyView::new(
                Button::icon(ButtonIconSource::texture(7), 16.0, || {})
                    .surface_environment(desktop),
            ),
            Default::default(),
        );
        graph.compute_layout(200.0, 200.0);
        let root = graph.tree.borrow().root().expect("root");
        let host = find_button_host(&root).expect("a button host mounts");
        assert!(
            host.frame().height < 44.0,
            "a pointer surface restores desktop density (got {})",
            host.frame().height
        );
    });
}

#[test]
fn texture_path_builds_button() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));

        let graph = support::graph();
        graph.install_view(
            AnyView::new(Button::icon(ButtonIconSource::texture(7), 16.0, || {})),
            Default::default(),
        );
        graph.compute_layout(60.0, 40.0);

        let root = graph.tree.borrow().root().expect("root");
        let host = find_button_host(&root).expect("a button host mounts");
        assert!(host.is_hit_testable(), "the button host is hit-testable");

        assert!(
            descendants(&host).iter().any(|node| {
                let frame = node.frame();
                (frame.width - 16.0).abs() < 0.5 && (frame.height - 16.0).abs() < 0.5
            }),
            "the icon child is authored at 16x16"
        );

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn click_invokes_action() {
    GlobalTestLock::locked(|| {
        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));

        let fired = Rc::new(Cell::new(0u32));
        let counter = fired.clone();
        let graph = support::graph();
        graph.install_view(
            AnyView::new(Button::icon(
                ButtonIconSource::texture(7),
                16.0,
                move || counter.set(counter.get() + 1),
            )),
            Default::default(),
        );
        graph.compute_layout(60.0, 40.0);

        let root = graph.tree.borrow().root().expect("root");
        let host = find_button_host(&root).expect("a button host mounts");
        let handler = registry
            .handlers(&host)
            .pointer
            .expect("the host carries a pointer handler");

        let event = MouseButtonEvent {
            button: cove_platform::MouseButton::Left,
            x: 0.0,
            y: 0.0,
            clicks: 1,
            modifiers: Default::default(),
            kind: Default::default(),
        };
        handler(&event, PointerPhase::Down, EventPhase::Target);
        graph.recomposer.commit_all();
        handler(&event, PointerPhase::Up, EventPhase::Target);
        graph.recomposer.commit_all();

        assert_eq!(
            fired.get(),
            1,
            "one down/up pair fires the action exactly once"
        );

        InteractionRegistryHolder::set_current(None);
    });
}

#[test]
fn file_fallback_without_registry() {
    GlobalTestLock::locked(|| {
        // No image registry is installed for this case.
        cove_imaging::ImageAssetRegistryHolder::set_current(None);

        let registry = Rc::new(InteractionRegistry::new());
        InteractionRegistryHolder::set_current(Some(registry));

        let graph = support::graph();
        graph.install_view(
            AnyView::new(Button::icon(
                ButtonIconSource::file("/this/path/does/not/exist.png"),
                14.0,
                || {},
            )),
            Default::default(),
        );
        graph.compute_layout(60.0, 40.0);

        let root = graph.tree.borrow().root().expect("root");
        let host =
            find_button_host(&root).expect("a file icon materialises without an image registry");
        assert!(
            host.is_hit_testable(),
            "the fallback still produces an interactive host"
        );

        InteractionRegistryHolder::set_current(None);
    });
}
