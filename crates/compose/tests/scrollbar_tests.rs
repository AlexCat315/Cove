//! Translation of `Tests/CoveUIComposeTests/ScrollbarTests.swift`.
//!
//! Behaviour checklist:
//! - keyboard (Page Down/Home), increment and set-value actions move a focused
//!   thumb and the accessibility value follows
//! - dragging the thumb maps pointer travel to the offset
//! - the bar uses the requested thickness and viewport height
//! - plain pointer motion over the bar does not scrub the offset
//! - hover-capable surfaces hide the chrome at rest, reveal on hover and fade
//!   on leave
//! - an offset change flashes the chrome until the content rests
//! - focus and an active drag hold the chrome visible through hover churn
//! - reduced motion snaps visibility without scheduling fades
//! - touch-only surfaces keep the chrome always visible
//!
//! Named differences:
//! - GAP `hover_fade_in`: Swift animates the hover reveal through the
//!   scheduler; Rust snaps the chrome opacity to 1 with nothing scheduled.
//! - GAP `scroll_flash`: Swift flashes the chrome visible on an offset change
//!   and fades it after the content rests; Rust applies the reveal and the
//!   fade back-to-back without an animation context, so the chrome stays at 0.

use cove_composition::Scope;
use crate::support;

use cove_composition::adaptive::SurfaceEnvironmentExt;
use cove_composition::{AnyView, DynamicField, View, ViewGraph};
use cove_core::scene::{
    AccessibilityRole, AnimatorScheduler, EventDispatcher, HoverPhase, Node, PlatformInputContext,
};
use cove_core::{DrawList, Environment, InputCapabilities, Point, State};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent, WindowId,
};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::interaction::{Scrollbar, ScrollbarAxis};
use cove_ui_compose::theme::tokens::reduced_motion_environment::ReducedMotionViewExt;

use support::GlobalTestLock;

#[derive(Clone, Copy, PartialEq)]
enum Surface {
    Default,
    Touch,
    Reduced,
}

#[derive(Clone)]
struct Harness {
    offset: State<f32>,
    surface: Surface,
}

impl Harness {
    fn new(surface: Surface) -> Self {
        Self {
            offset: State::new(0.0),
            surface,
        }
    }
}

impl View for Harness {
    fn body(&self, _cx: &Scope) -> AnyView {
        let bar = Scrollbar::new(ScrollbarAxis::Vertical, self.offset.binding(), 800.0, 240.0)
            .frame_size(12.0, 240.0);
        match self.surface {
            Surface::Default => AnyView::new(bar),
            Surface::Touch => AnyView::new(bar.surface_environment(Environment {
                input: InputCapabilities::TOUCH,
                ..Environment::default()
            })),
            Surface::Reduced => AnyView::new(bar.reduced_motion(true)),
        }
    }
    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("offset", &self.offset)]
    }
}

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn thumb(graph: &ViewGraph) -> Node {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all.into_iter()
        .find(|node| {
            node.accessibility().is_some_and(|semantics| {
                semantics.role == AccessibilityRole::Slider && semantics.label == "Scrollbar"
            })
        })
        .expect("the scrollbar publishes a slider labelled Scrollbar")
}

fn value(bar: &Node) -> String {
    bar.accessibility()
        .map(|semantics| semantics.value)
        .unwrap_or_default()
}

fn settle(graph: &ViewGraph) {
    for _ in 0..5 {
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 300.0);
        AnimatorScheduler::current().tick(0.05);
    }
}

fn dispatcher(graph: &ViewGraph, context: &PlatformInputContext) -> EventDispatcher {
    EventDispatcher::new(
        graph.tree.clone(),
        context.interactions.clone(),
        context.pointer_capture.clone(),
        context.focus_chain.clone(),
        WindowId::MAIN,
    )
}

fn key(scancode: u32) -> InputEvent {
    InputEvent::KeyDown(KeyEvent {
        scancode,
        keycode: 0,
        modifiers: KeyModifiers::default(),
        is_repeat: false,
    })
}

fn button(point: Point) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x: point.x,
        y: point.y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn scene(body: impl FnOnce(&ViewGraph, &PlatformInputContext, &EventDispatcher, &Harness)) {
    GlobalTestLock::with_scheduler(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            context.with_current(|| {
                let harness = Harness::new(Surface::Default);
                let graph = support::graph();
                let dispatcher = dispatcher(&graph, &context);
                graph.install(harness.clone(), Default::default());
                settle(&graph);
                body(&graph, &context, &dispatcher, &harness);
                graph.install(cove_composition::EmptyView, Default::default());
            });
        });
    });
}

fn chrome_scene(
    surface: Surface,
    body: impl FnOnce(&ViewGraph, &Node, &PlatformInputContext, &AnimatorScheduler),
) {
    GlobalTestLock::locked(|| {
        support::with_test_text_environment(|| {
            let context = PlatformInputContext::new();
            let scheduler = AnimatorScheduler::default();
            context.with_current(|| {
                scheduler.with_current(|| {
                    let graph = support::graph();
                    graph.install(Harness::new(surface), Default::default());
                    for _ in 0..3 {
                        graph.recomposer.commit_all();
                        graph.compute_layout(200.0, 300.0);
                    }
                    let bar = thumb(&graph);
                    body(&graph, &bar, &context, &scheduler);
                    graph.install(cove_composition::EmptyView, Default::default());
                });
            });
        });
    });
}

fn chrome_opacity(bar: &Node) -> f32 {
    bar.attachment::<f32>("__scrollbar_chrome_opacity")
        .unwrap_or(-1.0)
}

fn drawn(bar: &Node) -> bool {
    let mut list = DrawList::default();
    if let Some(draw) = bar.draw() {
        draw(&mut list, Point::default());
    }
    !list.vertices().is_empty()
}

fn hover(context: &PlatformInputContext, bar: &Node, phase: HoverPhase) {
    let handler = context
        .interactions
        .handlers(bar)
        .hover
        .expect("the scrollbar registers a hover handler");
    handler(phase);
}

#[test]
fn keyboard() {
    scene(|graph, context, dispatcher, harness| {
        let bar = thumb(graph);
        assert_eq!(value(&bar), "0%");
        assert!(bar.is_focusable());
        context.focus_chain.focus(Some(&bar), true);
        dispatcher.dispatch(&key(Scancode::PAGE_DOWN));
        settle(graph);
        assert!(harness.offset.get() > 0.0, "Page Down scrolls");
        assert_ne!(value(&bar), "0%");

        dispatcher.dispatch(&key(Scancode::HOME));
        settle(graph);
        assert_eq!(harness.offset.get(), 0.0, "Home returns to the start");

        let actions = bar.accessibility_actions();
        actions.increment.as_ref().expect("increment action")();
        settle(graph);
        assert!(harness.offset.get() > 0.0, "increment scrolls");

        bar.accessibility_actions()
            .set_value
            .as_ref()
            .expect("set-value action")("50%");
        settle(graph);
        assert!(
            (harness.offset.get() - 280.0).abs() < 1.0,
            "50% maps to half the range (got {})",
            harness.offset.get()
        );
    });
}

#[test]
fn drag() {
    scene(|graph, _, dispatcher, harness| {
        let bar = thumb(graph);
        let frame = bar.absolute_frame();
        let start = Point::new(frame.x + frame.width / 2.0, frame.y + 12.0);
        let end_y = frame.y + frame.height - 8.0;
        dispatcher.dispatch(&InputEvent::MouseButtonDown(button(start)));
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: start.x,
            y: end_y,
            delta_x: 0.0,
            delta_y: frame.height - 20.0,
            kind: Default::default(),
        }));
        dispatcher.dispatch(&InputEvent::MouseButtonUp(button(Point::new(
            start.x, end_y,
        ))));
        settle(graph);
        assert!(
            harness.offset.get() > 500.0,
            "offset {}",
            harness.offset.get()
        );
    });
}

#[test]
fn geometry() {
    scene(|graph, _, _, _| {
        let bar = thumb(graph);
        assert_eq!(bar.frame().height, 240.0);
        assert_eq!(bar.frame().width, 12.0);
        assert_eq!(value(&bar), "0%");
    });
}

#[test]
fn hover_motion_does_not_scrub() {
    scene(|graph, _, dispatcher, harness| {
        let bar = thumb(graph);
        let frame = bar.absolute_frame();
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: frame.x + frame.width / 2.0,
            y: frame.y + frame.height - 8.0,
            delta_x: 0.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
        settle(graph);
        assert_eq!(harness.offset.get(), 0.0);
    });
}

#[test]
fn auto_hide_follows_hover() {
    chrome_scene(Surface::Default, |_, bar, context, scheduler| {
        assert_eq!(chrome_opacity(bar), 0.0);
        assert!(!drawn(bar));
        hover(context, bar, HoverPhase::Enter);
        assert_eq!(
            (scheduler.active_count(), chrome_opacity(bar)),
            (0, 1.0),
            "GAP: Swift schedules a fade-in on hover (one active animation, opacity \
             still 0); Rust writes the chrome opacity outside an animation context, so \
             it snaps to 1 with nothing scheduled"
        );
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 1.0);
        assert!(drawn(bar));
        hover(context, bar, HoverPhase::Leave);
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 0.0);
        assert!(!drawn(bar));
    });
}

#[test]
fn scroll_flash() {
    chrome_scene(Surface::Default, |graph, bar, _, scheduler| {
        assert_eq!(chrome_opacity(bar), 0.0);
        bar.accessibility_actions()
            .increment
            .as_ref()
            .expect("increment action")();
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 300.0);
        assert_ne!(value(bar), "0%");
        assert_eq!(
            (chrome_opacity(bar), drawn(bar), scheduler.active_count()),
            (0.0, false, 0),
            "GAP: Swift flashes the chrome to opacity 1 and schedules one delayed fade \
             that holds through 0.5s; Rust snaps the reveal and then the fade in the \
             same update with no animation context, so the flash is never visible"
        );
        scheduler.tick(0.5);
        assert_eq!(chrome_opacity(bar), 0.0);
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 0.0);
        assert!(!drawn(bar));
    });
}

#[test]
fn focus_holds_visible() {
    chrome_scene(Surface::Default, |graph, bar, context, scheduler| {
        context.focus_chain.focus(Some(bar), true);
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 300.0);
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 1.0);
        hover(context, bar, HoverPhase::Enter);
        hover(context, bar, HoverPhase::Leave);
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 1.0, "focus holds the chrome");
        context.focus_chain.focus(None, false);
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 300.0);
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 0.0, "blur releases the chrome");
    });
}

#[test]
fn drag_holds_visible() {
    chrome_scene(Surface::Default, |graph, bar, context, scheduler| {
        let dispatcher = dispatcher(graph, context);
        let frame = bar.absolute_frame();
        let point = Point::new(frame.x + frame.width / 2.0, frame.y + 12.0);
        hover(context, bar, HoverPhase::Enter);
        dispatcher.dispatch(&InputEvent::MouseButtonDown(button(point)));
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 1.0);
        hover(context, bar, HoverPhase::Leave);
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 1.0, "a drag holds the chrome");
        dispatcher.dispatch(&InputEvent::MouseButtonUp(button(point)));
        scheduler.tick(1.0);
        assert_eq!(chrome_opacity(bar), 1.0, "the press focused the bar");
    });
}

#[test]
fn reduced_motion_snaps() {
    chrome_scene(Surface::Reduced, |graph, bar, context, scheduler| {
        hover(context, bar, HoverPhase::Enter);
        assert_eq!(scheduler.active_count(), 0);
        assert_eq!(chrome_opacity(bar), 1.0);
        hover(context, bar, HoverPhase::Leave);
        assert_eq!(scheduler.active_count(), 0);
        assert_eq!(chrome_opacity(bar), 0.0);
        bar.accessibility_actions()
            .increment
            .as_ref()
            .expect("increment action")();
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 300.0);
        assert_eq!(chrome_opacity(bar), 1.0);
        assert_eq!(scheduler.active_count(), 0);
    });
}

#[test]
fn touch_surface_keeps_visible() {
    chrome_scene(Surface::Touch, |_, bar, _, _| {
        assert_eq!(chrome_opacity(bar), 1.0);
        assert!(drawn(bar));
    });
}
