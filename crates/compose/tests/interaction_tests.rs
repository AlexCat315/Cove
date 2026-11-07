//! Translation of `Tests/CoveUIComposeTests/Animation/InteractionTests.swift`.
//!
//! Behaviour checklist:
//! - a pointer drag moves the surface, takes capture, and releases with the
//!   measured velocity still reaching a spring
//! - the moved surface stays hittable where it is drawn
//! - re-grabbing mid-flight continues from the displayed offset without a jump
//! - disabling the drag releases capture and clears the spring
//! - reduced motion releases without any spring
//! - a pointer press applies the themed compression on the same frame
//! - the hovered face and scale are restored on release
//!
//! Model difference: the Swift suite pokes the scheduler's clock through
//! `session.now`. The Rust [`SpringDragSession`] samples the process clock, so
//! these tests drive `begin`/`move_to`/`release` directly instead of through
//! dispatched mouse events where the velocity sample window matters; the drag
//! entry points themselves are still exercised through the real registry
//! handlers.

use crate::support;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use cove_composition::AnyView;
use cove_core::scene::{
    Align, AnimatorScheduler, FlexDirection, HitTester, InteractionRegistryHolder, Justify, Node,
    PointerCaptureHolder, SpringSolution, Wrap,
};
use cove_core::{Binding, Point};
use cove_ui_compose::BoxView;
use cove_ui_compose::animation::interaction::spring_drag::{
    SpringDrag, SpringDragOptions, SpringDragSession,
};
use cove_ui_compose::modifiers::layout::{
    AbsolutePositionModifier, FrameDimension, LayoutFrameExt,
};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::theme::styles::buttons::builtin_button_chrome::BUTTON_HOST_MARKER_KEY;
use cove_ui_compose::theme::{Theme, ThemeViewExt};

use support::GlobalTestLock;

/// Installs the process-wide interaction holders for the scope, matching the
/// Swift `PlatformInputContext.withCurrent` wrapper.
struct Holders {
    registry: Rc<cove_core::scene::InteractionRegistry>,
    capture: Rc<cove_core::scene::PointerCapture>,
    focus: Rc<cove_core::scene::FocusChain>,
}

impl Holders {
    fn new() -> Self {
        let registry = Rc::new(cove_core::scene::InteractionRegistry::new());
        let capture = Rc::new(cove_core::scene::PointerCapture::new());
        let focus = Rc::new(cove_core::scene::FocusChain::new());
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        cove_core::scene::FocusChainHolder::set_current(Some(focus.clone()));
        Self {
            registry,
            capture,
            focus,
        }
    }
}

impl Drop for Holders {
    fn drop(&mut self) {
        InteractionRegistryHolder::set_current(None);
        PointerCaptureHolder::set_current(None);
        cove_core::scene::FocusChainHolder::set_current(None);
    }
}

fn sized_box(width: f32, height: f32) -> AnyView {
    AnyView::new(
        BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(cove_composition::EmptyView),
        )
        .frame(
            Some(FrameDimension::Points(width)),
            Some(FrameDimension::Points(height)),
            None,
            None,
            None,
            None,
        ),
    )
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn all_nodes(graph: &cove_composition::ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

/// A shareable cell standing in for a Swift `@State` binding, matching the
/// `Binding` requirement that both closures are `Send + Sync`.
#[derive(Clone)]
struct CellValue<T>(Arc<Mutex<T>>);

impl<T: Clone + Send + 'static> CellValue<T> {
    fn new(value: T) -> Self {
        Self(Arc::new(Mutex::new(value)))
    }
    fn get(&self) -> T {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
    fn set(&self, value: T) {
        *self.0.lock().unwrap_or_else(|error| error.into_inner()) = value;
    }
    fn binding(&self) -> Binding<T> {
        let get = self.clone();
        let set = self.clone();
        Binding::new(move || get.get(), move |value| set.set(value))
    }
}

fn drag_node(graph: &cove_composition::ViewGraph) -> Node {
    all_nodes(graph)
        .into_iter()
        .find(|node| node.first_resource::<SpringDragSession>().is_some())
        .expect("the spring drag surface is mounted")
}

#[test]
fn drag_release_springs_to_rest_and_stays_hittable() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let _holders = Holders::new();
            let offset = CellValue::new(Point::default());
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        AnyView::new(SpringDrag::new(offset.binding(), sized_box(100.0, 60.0)))
                            .frame(
                                Some(FrameDimension::Points(100.0)),
                                Some(FrameDimension::Points(60.0)),
                                None,
                                None,
                                None,
                                None,
                            ),
                    )
                    .with_modifier(AbsolutePositionModifier::new(
                        Some(40.0),
                        Some(30.0),
                        None,
                        None,
                    )),
                ),
                Default::default(),
            );
            graph.compute_layout(400.0, 300.0);

            let node = drag_node(&graph);
            let session = node
                .first_resource::<SpringDragSession>()
                .expect("the session resource is installed");

            session.begin(Point::new(70.0, 50.0));
            assert!(
                PointerCaptureHolder::current()
                    .expect("the harness installs a capture holder")
                    .target()
                    .is_some_and(|target| target.id() == node.id()),
                "pressing the surface takes pointer capture"
            );

            session.move_to(Point::new(100.0, 60.0));
            assert_eq!(
                offset.get(),
                Point::new(30.0, 10.0),
                "the offset follows the pointer delta"
            );
            assert_eq!(
                node.absolute_frame().x,
                70.0,
                "the drawn surface moves with the offset"
            );
            assert_eq!(node.absolute_frame().y, 40.0);

            session.move_to(Point::new(130.0, 70.0));
            session.release();
            assert!(
                PointerCaptureHolder::current()
                    .expect("capture holder")
                    .target()
                    .is_none(),
                "release drops pointer capture"
            );
            assert_eq!(
                scheduler.active_count(),
                1,
                "a released drag registers one spring controller"
            );

            let released = offset.get();
            assert!(!session.is_finished(), "the spring is still running");
            scheduler.tick(0.0001);
            let moving = offset.get();
            assert!(
                moving.x > released.x && moving.y > released.y,
                "the spring carries the release velocity outward ({moving:?} vs {released:?})"
            );

            let root = graph.tree.borrow().root().expect("root");
            let center = Point::new(
                node.absolute_frame().x + node.absolute_frame().width / 2.0,
                node.absolute_frame().y + node.absolute_frame().height / 2.0,
            );
            let hit = HitTester::hit_test(&root, center).expect("the moving surface is hittable");
            assert_eq!(
                hit.node.id(),
                node.id(),
                "the surface is hittable at its drawn frame"
            );

            scheduler.tick(5.0);
            assert_eq!(offset.get(), Point::default(), "the spring settles at rest");
            assert_eq!(scheduler.active_count(), 0, "the spring releases its entry");
        });
    });
}

#[test]
fn regrab_does_not_jump_and_disabling_clears_the_spring() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let _holders = Holders::new();
            let offset = CellValue::new(Point::default());
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(SpringDrag::new(offset.binding(), sized_box(100.0, 60.0))).frame(
                        Some(FrameDimension::Points(100.0)),
                        Some(FrameDimension::Points(60.0)),
                        None,
                        None,
                        None,
                        None,
                    ),
                ),
                Default::default(),
            );
            graph.compute_layout(400.0, 300.0);

            let node = drag_node(&graph);
            let session = node
                .first_resource::<SpringDragSession>()
                .expect("the session resource is installed");

            session.begin(Point::new(70.0, 50.0));
            session.move_to(Point::new(130.0, 70.0));
            session.release();
            scheduler.tick(0.04);
            let displayed = offset.get();
            assert!(
                session.is_finished() || !session.is_dragging(),
                "release ended the drag"
            );

            session.cancel();
            assert_eq!(
                scheduler.active_count(),
                0,
                "cancelling unregisters the spring"
            );

            let frame = node.absolute_frame();
            session.begin(Point::new(
                frame.x + frame.width / 2.0,
                frame.y + frame.height / 2.0,
            ));
            assert_eq!(
                offset.get(),
                displayed,
                "re-grabbing keeps the displayed offset instead of jumping"
            );
            session.cancel();

            // Disabling releases capture and clears the spring.
            session.configure(
                offset.binding(),
                SpringDragOptions {
                    target: Point::default(),
                    spring: SpringSolution::new(0.4, 0.82),
                    is_enabled: false,
                },
                false,
            );
            assert!(!session.is_dragging(), "a disabled drag is not dragging");
            assert_eq!(
                offset.get(),
                Point::default(),
                "disabling clears the offset"
            );
            assert_eq!(scheduler.active_count(), 0, "disabling clears the spring");

            // Reduced motion releases without a spring.
            session.configure(
                offset.binding(),
                SpringDragOptions {
                    target: Point::default(),
                    spring: SpringSolution::new(0.4, 0.82),
                    is_enabled: true,
                },
                true,
            );
            session.begin(Point::new(70.0, 50.0));
            session.move_to(Point::new(130.0, 70.0));
            session.release();
            assert_eq!(
                scheduler.active_count(),
                0,
                "reduced motion never registers a spring"
            );
        });
    });
}

#[test]
fn pointer_press_compresses_immediately_and_springs_back_on_release() {
    GlobalTestLock::locked(|| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let holders = Holders::new();
            let mut theme = Theme::default_light();
            theme.motion.press_scale = 0.9;
            let expected_hover = theme.colors.accent_hover;
            let expected_press_scale = 0.9f32;
            let activations = Rc::new(Cell::new(0));
            let counter = activations.clone();
            let graph = support::graph();
            graph.install_view(
                AnyView::new(
                    AnyView::new(
                        cove_ui_compose::theme::styles::buttons::ButtonStyleViewExt::button_style(
                            Button::title("Press", move || counter.set(counter.get() + 1)),
                            cove_ui_compose::theme::styles::buttons::PrimaryButtonStyle,
                        ),
                    )
                    .frame(
                        Some(FrameDimension::Points(200.0)),
                        Some(FrameDimension::Points(44.0)),
                        None,
                        None,
                        None,
                        None,
                    )
                    .theme(theme.clone()),
                ),
                Default::default(),
            );
            graph.compute_layout(400.0, 300.0);

            let chrome = all_nodes(&graph)
                .into_iter()
                .find(|node| {
                    node.attachment::<String>("__builtin_button_chrome_state")
                        .is_none()
                        && node.corner_radius() > 0.0
                })
                .expect("the button chrome is mounted");
            let host = all_nodes(&graph)
                .into_iter()
                .find(|node| {
                    node.attachment::<bool>(BUTTON_HOST_MARKER_KEY)
                        .unwrap_or(false)
                })
                .expect("the button host is mounted");

            let dispatcher = cove_core::scene::EventDispatcher::new(
                graph.tree.clone(),
                holders.registry.clone(),
                holders.capture.clone(),
                holders.focus.clone(),
                cove_platform::WindowId::MAIN,
            );

            // Hover the host so the resting face is the hovered one.
            dispatcher.dispatch(&cove_platform::InputEvent::MouseMotion(
                cove_platform::MouseMotionEvent {
                    x: 1.0,
                    y: 22.0,
                    delta_x: 0.0,
                    delta_y: 0.0,
                    kind: Default::default(),
                },
            ));
            graph.recomposer.commit_all();
            graph.compute_layout(400.0, 300.0);
            let resting = chrome.background_color();
            assert_eq!(
                resting,
                Some(expected_hover),
                "hovering paints the accent-hover face"
            );

            let event = cove_platform::MouseButtonEvent {
                button: cove_platform::MouseButton::Left,
                x: 1.0,
                y: 22.0,
                clicks: 1,
                modifiers: Default::default(),
                kind: Default::default(),
            };
            dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonDown(event.clone()));
            graph.recomposer.commit_all();
            graph.compute_layout(400.0, 300.0);
            assert_ne!(
                chrome.background_color(),
                resting,
                "the pressed face changes on the same frame"
            );
            assert_eq!(
                chrome.transform().scale,
                expected_press_scale,
                "the press applies the themed compression without animating"
            );
            assert_eq!(
                scheduler.active_count(),
                0,
                "pointer feedback is immediate, never queued as an animation"
            );

            dispatcher.dispatch(&cove_platform::InputEvent::MouseButtonUp(event));
            graph.recomposer.commit_all();
            graph.compute_layout(400.0, 300.0);
            assert_eq!(activations.get(), 1, "release activates the button once");
            assert!(
                scheduler.active_count() > 0,
                "release springs back instead of snapping"
            );
            for _ in 0..240 {
                scheduler.tick(1.0 / 120.0);
            }
            assert_eq!(
                chrome.background_color(),
                resting,
                "release settles on the hovered face"
            );
            assert!(
                (chrome.transform().scale - 1.0).abs() < 0.001,
                "release settles at the resting scale"
            );
            assert_eq!(scheduler.active_count(), 0, "the release spring settles");
            let _ = host;
        });
    });
}
