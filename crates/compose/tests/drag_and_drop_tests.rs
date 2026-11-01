//! Translation of `Tests/CoveUIComposeTests/DragAndDropTests.swift`.
//!
//! Behaviour checklist:
//! - a typed payload drops once on a matching target, clears pointer capture and
//!   reports the target as highlighted and then unhighlighted
//! - releasing outside every target cancels without a drop
//! - the drag preview occupies the portal only while a drag is in flight
//!
//! Note: the preview only mounts because `DragScope` exposes its `revision`
//! counter through `dynamic_properties`. Swift keeps that counter in `@State`
//! (`DragAndDrop.swift:15,22`), so `session.on_change` bumping it recomposes the
//! scope and `DragPreviewPortal` re-runs. That write is what makes the preview
//! appear at all, so the third test exercises the recomposition path as well as
//! the portal lifecycle.
//!
//! Model difference: Swift's payload is generic (`Draggable(42, ...)` with a
//! `DropTarget` over `Int`), while the Rust `Draggable`/`DropTarget` pair carries
//! a `u64` payload token (`primitives/interaction/drag_and_drop.rs`). The tests
//! use `42u64` and an `accepts` predicate over it, which preserves the typed
//! matching behaviour the Swift suite asserted: the int target accepts 42, and a
//! second target that rejects it never fires.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{Node, PointerCaptureHolder};
use cove_platform::{InputEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::interaction::drag_and_drop::{DragScope, Draggable, DropTarget};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::{Column, HorizontalAlignment};

use support::InteractiveScene;

/// The Swift `Model`: what the targets reported.
#[derive(Default)]
struct Model {
    dropped: RefCell<Vec<u64>>,
    rejected: RefCell<Vec<u64>>,
    highlights: RefCell<Vec<bool>>,
}

fn harness(model: Rc<Model>) -> AnyView {
    let targeted = model.clone();
    let dropped_int = model.clone();
    let dropped_string = model.clone();
    AnyView::new(LayerRoot::new(AnyView::new(AnyView::new(
        DragScope::new(AnyView::new(Column::new(
            HorizontalAlignment::Leading,
            16.0,
            AnyView::new(cove_composition::TupleView::new(vec![
                AnyView::new(Draggable::new(
                    42,
                    AnyView::new(Text::new("42")),
                    AnyView::new(AnyView::new(Text::new("Drag me")).frame_size(120.0, 36.0)),
                )),
                AnyView::new(
                    DropTarget::new(
                        |payload| *payload == 42,
                        move |payload| dropped_int.dropped.borrow_mut().push(payload),
                        AnyView::new(AnyView::new(Text::new("Drop here")).frame_size(160.0, 80.0)),
                    )
                    .on_targeted(move |is_targeted| {
                        targeted.highlights.borrow_mut().push(is_targeted)
                    }),
                ),
                AnyView::new(DropTarget::new(
                    |_| false,
                    move |payload| dropped_string.rejected.borrow_mut().push(payload),
                    AnyView::new(AnyView::new(Text::new("Text only")).frame_size(160.0, 40.0)),
                )),
            ])),
        )))
        .frame_size(320.0, 320.0),
    ))))
}

fn nodes(scene: &InteractiveScene) -> Vec<Node> {
    scene.nodes()
}

fn labelled(scene: &InteractiveScene, label: &str) -> Node {
    scene
        .labelled(label, None)
        .unwrap_or_else(|| panic!("node labelled {label} materialises"))
}

fn drag(scene: &InteractiveScene, from: cove_core::Point, to: cove_core::Point) {
    let dispatcher = scene.dispatcher();
    dispatcher.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
        button: MouseButton::Left,
        x: from.x,
        y: from.y,
        clicks: 1,
        modifiers: KeyModifiers::default(),
        kind: Default::default(),
    }));
    // Cross the six-point threshold while still inside the source, then jump to
    // the target as a captured drag — the Swift helper's motion sequence.
    for (index, step) in [4.0f32, 8.0f32].into_iter().enumerate() {
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: from.x + step,
            y: from.y + step,
            delta_x: 4.0,
            delta_y: 4.0,
            kind: Default::default(),
        }));
        if index == 0 {
            scene.settle(1, 0.05);
        }
    }
    dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
        x: to.x,
        y: to.y,
        delta_x: to.x - from.x,
        delta_y: to.y - from.y,
        kind: Default::default(),
    }));
    dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
        button: MouseButton::Left,
        x: to.x,
        y: to.y,
        clicks: 1,
        modifiers: KeyModifiers::default(),
        kind: Default::default(),
    }));
    scene.settle(6, 0.05);
}

#[test]
fn a_typed_payload_drops_once_on_a_matching_target() {
    let model = Rc::new(Model::default());
    let harness_model = model.clone();
    InteractiveScene::run(360.0, 360.0, |scene| {
        scene.install(harness(harness_model));
        let source = labelled(scene, "Drag me").absolute_frame();
        let target = labelled(scene, "Drop here").absolute_frame();

        drag(
            scene,
            cove_core::Point::new(
                source.x + source.width / 2.0,
                source.y + source.height / 2.0,
            ),
            cove_core::Point::new(
                target.x + target.width / 2.0,
                target.y + target.height / 2.0,
            ),
        );

        assert_eq!(
            *model.dropped.borrow(),
            vec![42],
            "the matching target receives the payload exactly once"
        );
        assert!(
            model.rejected.borrow().is_empty(),
            "the target that rejects the payload never fires"
        );
        assert!(
            PointerCaptureHolder::current().is_none_or(|capture| capture.target().is_none()),
            "releasing the pointer clears capture"
        );
        let highlights = model.highlights.borrow().clone();
        assert!(
            highlights.contains(&true),
            "the target reports itself highlighted during the drag, saw {highlights:?}"
        );
        assert_eq!(
            highlights.last().copied(),
            Some(false),
            "the target is unhighlighted once the drag ends, saw {highlights:?}"
        );
    });
}

#[test]
fn releasing_outside_every_target_cancels_without_a_drop() {
    let model = Rc::new(Model::default());
    let harness_model = model.clone();
    InteractiveScene::run(360.0, 360.0, |scene| {
        scene.install(harness(harness_model));
        let source = labelled(scene, "Drag me").absolute_frame();

        drag(
            scene,
            cove_core::Point::new(
                source.x + source.width / 2.0,
                source.y + source.height / 2.0,
            ),
            cove_core::Point::new(350.0, 350.0),
        );

        assert!(
            model.dropped.borrow().is_empty(),
            "no target accepts a payload released outside it"
        );
        let highlights = model.highlights.borrow().clone();
        assert!(
            highlights.is_empty() || highlights == vec![false],
            "an untouched target reports no highlight, saw {highlights:?}"
        );
    });
}

#[test]
fn the_preview_uses_the_portal_only_while_dragging() {
    InteractiveScene::run(360.0, 360.0, |scene| {
        scene.install(harness(Rc::new(Model::default())));
        let source = labelled(scene, "Drag me").absolute_frame();
        let mid = cove_core::Point::new(
            source.x + source.width / 2.0,
            source.y + source.height / 2.0,
        );

        let dispatcher = scene.dispatcher();
        dispatcher.dispatch(&InputEvent::MouseButtonDown(MouseButtonEvent {
            button: MouseButton::Left,
            x: mid.x,
            y: mid.y,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        }));
        for step in [4.0f32, 8.0f32] {
            dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
                x: mid.x + step,
                y: mid.y + step,
                delta_x: 4.0,
                delta_y: 4.0,
                kind: Default::default(),
            }));
        }
        scene.settle(6, 0.05);
        assert_eq!(
            scene.portal.entries().len(),
            1,
            "an in-flight drag publishes exactly one preview entry"
        );

        dispatcher.dispatch(&InputEvent::MouseButtonUp(MouseButtonEvent {
            button: MouseButton::Left,
            x: mid.x + 40.0,
            y: mid.y + 40.0,
            clicks: 1,
            modifiers: KeyModifiers::default(),
            kind: Default::default(),
        }));
        scene.settle(6, 0.05);
        assert!(
            scene.portal.entries().is_empty(),
            "no preview lingers after the drag ends, still had {}",
            scene.portal.entries().len()
        );
    });
}
