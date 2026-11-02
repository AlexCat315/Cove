//! Translation of `Tests/CoveUIComposeTests/ButtonScrollViewTests.swift`.
//!
//! Behaviour checklist:
//! - a button fires on down+up and ignores a stray up
//! - a scroll view is hit-testable, clips and registers a wheel handler
//! - wheel input clamps at both bounds without synthesized animation
//! - scrollbar chrome fades in on hover and hides on leave
//! - a scrollbar drag captures the pointer before content handlers, accepts a
//!   small perpendicular hit slop, uses descendant overflow when the wrapper is
//!   clamped, and acquires the capture context current at event time
//! - an overlay above the viewport owns presses on the scrollbar lane, while
//!   the wheel still reaches the viewport through the global fallback
//! - wheel coordinates at zero fall back to the last cursor position
//! - a property grid scrolls inside a constrained viewport and inside panel
//!   chrome; row hover updates render state without recomposing
//! - nested text fields and menus consume boundary wheel input before the
//!   parent; a field without overflow lets the parent scroll
//! - menu highlight commits once and press feedback is render-only
//! - a focused text field lets a fresh wheel over the parent scroll the parent
//!
//! Named differences:
//! - GAP `scroll_chrome_fade`: Swift animates the scroll view's scrollbar chrome
//!   in on hover and out on leave; Rust snaps it with nothing scheduled.
//! - GAP `field_scroll_bounds_at_layout`: Swift measures a text field's scroll
//!   bounds during layout; Rust measures them only in the draw pass.
//! - GAP `text_field_wheel`: Swift's text field consumes wheel input and scrolls
//!   its own overflow (containing boundary gestures); Rust's text field wheel
//!   handler always returns `Ignored`, so the parent scroll view scrolls.
//! - GAP `text_field_wheel_priority`: Swift marks a focused overflowing field
//!   `WheelRoutingPriority.preferFocused`; Rust never sets the priority.
//! - GAP `menu_hover_commit`: Swift commits a menu row's pointer highlight
//!   through state (one pending recomposition); Rust's hover is render-only.

use crate::support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use cove_composition::{AnyView, HorizontalAlignment, TupleView, VerticalAlignment, ViewGraph};
use cove_core::scene::{
    Align, AnimatorScheduler, EventDispatcher, EventPhase, EventResult, FlexDirection, FocusChain,
    FocusChainHolder, HoverPhase, InputHandlerRole, InteractionRegistry, InteractionRegistryHolder,
    Justify, Node, PointerCapture, PointerCaptureHolder, PointerPhase, WheelRoutingAttachmentKey,
    WheelRoutingPriority, Wrap,
};
use cove_core::{Binding, DrawList, Point, Rect, State};
use cove_platform::{
    InputEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, MouseWheelEvent, ScrollDevice,
    ScrollInput, ScrollPhase, ScrollUnit, WindowId,
};
use cove_text::TextEnvironmentHolder;
use cove_ui_compose::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use cove_ui_compose::primitives::controls::Button;
use cove_ui_compose::primitives::data::{PropertyGrid, PropertyGridRow, PropertyGridSection};
use cove_ui_compose::primitives::layout::Panel;
use cove_ui_compose::primitives::layout::{ScrollAxis, ScrollView};
use cove_ui_compose::primitives::overlays::menu::{Menu, MenuEntry, MenuItem};
use cove_ui_compose::primitives::text::Divider;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::primitives::text::text_buffer::TextBuffer;
use cove_ui_compose::primitives::text::text_field::{FieldState, TextField};
use cove_ui_compose::theme::Theme;
use cove_ui_compose::{BoxView, Column, Row, Spacer};

use support::GlobalTestLock;

const FIELD_STATE: &str = "__textfield_state";

fn collect(node: &Node, into: &mut Vec<Node>) {
    into.push(node.clone());
    for child in node.children() {
        collect(&child, into);
    }
}

fn nodes(graph: &ViewGraph) -> Vec<Node> {
    let mut all = Vec::new();
    if let Some(root) = graph.tree.borrow().root() {
        collect(&root, &mut all);
    }
    all
}

fn first(graph: &ViewGraph, predicate: impl Fn(&Node) -> bool) -> Option<Node> {
    nodes(graph).into_iter().find(|node| predicate(node))
}

fn root(graph: &ViewGraph) -> Node {
    graph.tree.borrow().root().expect("installed root")
}

fn scroll_node(graph: &ViewGraph, registry: &InteractionRegistry) -> Node {
    first(graph, |node| {
        registry
            .handlers(node)
            .wheel_route
            .is_some_and(|route| route.role == InputHandlerRole::Scroll)
    })
    .expect("a scroll view registers a scroll wheel route")
}

fn field_node(graph: &ViewGraph) -> Node {
    first(graph, |node| {
        node.attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE)
            .is_some()
    })
    .expect("the text field surface is mounted")
}

fn field_state(node: &Node) -> Rc<RefCell<FieldState>> {
    node.attachment::<Rc<RefCell<FieldState>>>(FIELD_STATE)
        .expect("field state")
}

fn column(children: Vec<AnyView>) -> Column {
    Column::new(
        HorizontalAlignment::Center,
        0.0,
        AnyView::new(TupleView::new(children)),
    )
}

fn stretch_box(spacing: f32, children: Vec<AnyView>) -> BoxView {
    BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        spacing,
        AnyView::new(TupleView::new(children)),
    )
}

fn empty_box() -> BoxView {
    stretch_box(0.0, Vec::new())
}

fn text(label: &str, width: Option<f32>, height: f32) -> AnyView {
    match width {
        Some(width) => AnyView::new(Text::new(label).frame_size(width, height)),
        None => AnyView::new(Text::new(label).frame_height(height)),
    }
}

fn head_tail_scroll() -> AnyView {
    AnyView::new(
        ScrollView::new(
            ScrollAxis::Vertical,
            column(vec![
                text("head", Some(220.0), 120.0),
                text("tail", Some(220.0), 320.0),
            ]),
        )
        .frame_size(220.0, 160.0),
    )
}

fn button(x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn motion(x: f32, y: f32, delta_y: f32) -> MouseMotionEvent {
    MouseMotionEvent {
        x,
        y,
        delta_x: 0.0,
        delta_y,
        kind: Default::default(),
    }
}

fn wheel(y: f32) -> MouseWheelEvent {
    MouseWheelEvent {
        x: 0.0,
        y,
        ..Default::default()
    }
}

fn wheel_at(y: f32, mouse_x: f32, mouse_y: f32) -> MouseWheelEvent {
    MouseWheelEvent {
        x: 0.0,
        y,
        mouse_x: Some(mouse_x),
        mouse_y: Some(mouse_y),
        input: ScrollInput::default(),
    }
}

fn dispatcher(
    graph: &ViewGraph,
    registry: &Rc<InteractionRegistry>,
    capture: &Rc<PointerCapture>,
    focus: &Rc<FocusChain>,
) -> EventDispatcher {
    EventDispatcher::new(
        graph.tree.clone(),
        registry.clone(),
        capture.clone(),
        focus.clone(),
        WindowId::MAIN,
    )
}

fn overlay_drawn(node: &Node) -> bool {
    let mut list = DrawList::default();
    if let Some(draw) = node.overlay_draw() {
        draw(&mut list, Point::default());
    }
    !list.vertices().is_empty()
}

/// Runs `body` with fresh holders installed and restores them afterwards.
fn locked(body: impl FnOnce(&Rc<InteractionRegistry>, &Rc<PointerCapture>, &Rc<FocusChain>)) {
    GlobalTestLock::with_scheduler(|| {
        let registry = Rc::new(InteractionRegistry::new());
        let capture = Rc::new(PointerCapture::new());
        let focus = Rc::new(FocusChain::new());
        let previous_text = TextEnvironmentHolder::current();
        InteractionRegistryHolder::set_current(Some(registry.clone()));
        PointerCaptureHolder::set_current(Some(capture.clone()));
        FocusChainHolder::set_current(Some(focus.clone()));
        TextEnvironmentHolder::set_current(Some(support::test_text_environment(16.0, 20.0)));
        body(&registry, &capture, &focus);
        InteractionRegistryHolder::set_current(None);
        PointerCaptureHolder::set_current(None);
        FocusChainHolder::set_current(None);
        TextEnvironmentHolder::set_current(previous_text);
    });
}

fn button_node(graph: &ViewGraph, registry: &InteractionRegistry) -> Node {
    first(graph, |node| {
        node.is_focusable() && registry.handlers(node).pointer.is_some()
    })
    .expect("the button host registers a pointer handler")
}

#[test]
fn button_fires_on_tap() {
    locked(|registry, _, _| {
        let taps = Rc::new(Cell::new(0));
        let counter = taps.clone();
        let graph = support::graph();
        graph.install(
            Button::new(Text::new("OK"), move || counter.set(counter.get() + 1)),
            Default::default(),
        );
        let node = button_node(&graph, registry);
        node.set_frame(Rect::new(0.0, 0.0, 80.0, 32.0));
        assert!(node.is_hit_testable());
        assert!(node.is_focusable());

        let pointer = registry.handlers(&node).pointer.expect("pointer handler");
        let event = button(0.0, 0.0);
        pointer(&event, PointerPhase::Down, EventPhase::Target);
        pointer(&event, PointerPhase::Up, EventPhase::Target);
        assert_eq!(taps.get(), 1);
    });
}

#[test]
fn button_ignores_stray_up() {
    locked(|registry, _, _| {
        let taps = Rc::new(Cell::new(0));
        let counter = taps.clone();
        let graph = support::graph();
        graph.install(
            Button::new(Text::new("X"), move || counter.set(counter.get() + 1)),
            Default::default(),
        );
        let node = button_node(&graph, registry);
        let pointer = registry.handlers(&node).pointer.expect("pointer handler");
        let result = pointer(&button(0.0, 0.0), PointerPhase::Up, EventPhase::Target);
        assert_eq!(taps.get(), 0);
        assert_eq!(result, EventResult::Ignored);
    });
}

#[test]
fn scroll_view_setup() {
    locked(|registry, _, _| {
        let graph = support::graph();
        graph.install(
            ScrollView::new(
                ScrollAxis::Vertical,
                column(vec![AnyView::new(Text::new("a"))]),
            ),
            Default::default(),
        );
        let scroll = scroll_node(&graph, registry);
        assert!(scroll.is_hit_testable());
        assert!(scroll.clips_to_bounds());
        assert!(registry.handlers(&scroll).wheel.is_some());
    });
}

#[test]
fn scroll_view_clamps() {
    locked(|registry, _, _| {
        let graph = support::graph();
        graph.install(
            ScrollView::new(
                ScrollAxis::Vertical,
                column(vec![
                    text("a", None, 100.0),
                    text("b", None, 100.0),
                    text("c", None, 100.0),
                ]),
            )
            .frame_height(150.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(200.0, 400.0);

        let scroll = scroll_node(&graph, registry);
        let inner = scroll.children().first().cloned().expect("content");
        assert_eq!(scroll.frame().height, 150.0);
        assert!(
            inner.frame().height >= 300.0,
            "content {}",
            inner.frame().height
        );

        let handler = registry.handlers(&scroll).wheel.expect("wheel");
        handler(&wheel(-1.0), EventPhase::Target);
        assert!(scroll.content_offset().y > 0.0);

        for _ in 0..100 {
            handler(&wheel(-1.0), EventPhase::Target);
        }
        let max_offset = inner.frame().height - scroll.frame().height;
        assert_eq!(scroll.content_offset().y, max_offset);
        assert!(scroll.content_offset().y < max_offset + scroll.frame().height);
        for _ in 0..120 {
            AnimatorScheduler::current().tick(1.0 / 60.0);
        }
        assert_eq!(scroll.content_offset().y, max_offset);

        for _ in 0..100 {
            handler(&wheel(1.0), EventPhase::Target);
        }
        assert_eq!(scroll.content_offset().y, 0.0);
        assert!(scroll.content_offset().y > -scroll.frame().height);
        for _ in 0..120 {
            AnimatorScheduler::current().tick(1.0 / 60.0);
        }
        assert_eq!(scroll.content_offset().y, 0.0);
    });
}

#[test]
fn scroll_view_scrollbar_chrome_follows_hover() {
    locked(|registry, _, _| {
        let scheduler = AnimatorScheduler::default();
        scheduler.with_current(|| {
            let graph = support::graph();
            graph.install(
                ScrollView::new(
                    ScrollAxis::Vertical,
                    column(vec![
                        text("a", None, 100.0),
                        text("b", None, 100.0),
                        text("c", None, 100.0),
                    ]),
                )
                .frame_size(220.0, 150.0),
                Default::default(),
            );
            graph.recomposer.commit_all();
            graph.compute_layout(220.0, 240.0);

            let scroll = scroll_node(&graph, registry);
            let hover = registry.handlers(&scroll).hover.expect("hover handler");
            assert!(!overlay_drawn(&scroll), "the chrome is hidden at rest");

            hover(HoverPhase::Enter);
            assert_eq!(
                (scheduler.active_count(), overlay_drawn(&scroll)),
                (0, true),
                "GAP: Swift schedules a chrome fade-in on hover (one active animation); \
                 Rust writes the opacity outside an animation context, so it snaps visible"
            );
            scheduler.tick(1.0);
            assert!(overlay_drawn(&scroll), "hover reveals the chrome");

            hover(HoverPhase::Leave);
            assert_eq!(
                (scheduler.active_count(), overlay_drawn(&scroll)),
                (0, false),
                "GAP: Swift schedules a chrome fade-out on leave; Rust hides it at once"
            );
            scheduler.tick(1.0);
            assert!(!overlay_drawn(&scroll), "leave hides the chrome");
            graph.install(cove_composition::EmptyView, Default::default());
        });
    });
}

#[test]
fn scroll_view_scrollbar_drag_captures_pointer() {
    locked(|registry, capture, focus| {
        let presses = Rc::new(Cell::new(0));
        let counter = presses.clone();
        let graph = support::graph();
        graph.install(
            ScrollView::new(
                ScrollAxis::Vertical,
                column(vec![
                    AnyView::new(
                        Button::new(Text::new("content"), move || counter.set(counter.get() + 1))
                            .frame_size(220.0, 120.0),
                    ),
                    text("tail", Some(220.0), 320.0),
                ]),
            )
            .frame_size(220.0, 160.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 220.0);

        let scroll = scroll_node(&graph, registry);
        let dispatcher = dispatcher(&graph, registry, capture, focus);
        let origin = scroll.absolute_origin();
        let scrollbar_x = origin.x + scroll.frame().width - 6.0;
        let thumb_y = origin.y + 10.0;
        dispatcher.dispatch(&InputEvent::MouseButtonDown(button(scrollbar_x, thumb_y)));
        assert!(capture.target().is_some_and(|target| target == scroll));
        assert_eq!(presses.get(), 0);

        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            scrollbar_x,
            thumb_y + 40.0,
            40.0,
        )));
        assert!(scroll.content_offset().y > 0.0);

        dispatcher.dispatch(&InputEvent::MouseButtonUp(button(
            scrollbar_x,
            thumb_y + 40.0,
        )));
        assert!(capture.target().is_none());
    });
}

#[test]
fn scroll_view_uses_descendant_overflow_for_wheel_and_scrollbar() {
    locked(|registry, capture, _| {
        let graph = support::graph();
        graph.install(
            ScrollView::new(
                ScrollAxis::Vertical,
                column(vec![AnyView::new(Text::new("content"))]),
            )
            .frame_size(220.0, 160.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 160.0);

        let scroll = scroll_node(&graph, registry);
        let wrapper = scroll.children().first().cloned().expect("content wrapper");
        wrapper.set_frame(Rect::new(0.0, 0.0, 220.0, 160.0));
        let overflowing = Node::new();
        overflowing.set_frame(Rect::new(0.0, 0.0, 220.0, 360.0));
        wrapper.add_child(&overflowing);

        let handlers = registry.handlers(&scroll);
        assert_eq!(
            handlers.wheel.as_ref().expect("wheel")(&wheel(-1.0), EventPhase::Target),
            EventResult::Handled
        );
        assert!(scroll.content_offset().y > 0.0);

        scroll.set_content_offset(Point::default());
        let pointer = handlers.pointer.expect("pointer");
        let motion_handler = handlers.motion.expect("motion");
        let origin = scroll.absolute_origin();
        let scrollbar_x = origin.x + scroll.frame().width - 6.0;
        let thumb_y = origin.y + 10.0;

        assert_eq!(
            pointer(
                &button(scrollbar_x, thumb_y),
                PointerPhase::Down,
                EventPhase::Target
            ),
            EventResult::Handled
        );
        assert!(capture.target().is_some_and(|target| target == scroll));
        assert_eq!(
            motion_handler(
                &motion(scrollbar_x, thumb_y + 40.0, 40.0),
                EventPhase::Target
            ),
            EventResult::Handled
        );
        assert!(scroll.content_offset().y > 0.0);
        assert_eq!(
            pointer(
                &button(scrollbar_x, thumb_y + 40.0),
                PointerPhase::Up,
                EventPhase::Target
            ),
            EventResult::Handled
        );
        assert!(capture.target().is_none());
    });
}

#[test]
fn scroll_view_scrollbar_drag_hit_slop() {
    locked(|registry, capture, _| {
        let graph = support::graph();
        graph.install(head_tail_scroll(), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 160.0);

        let scroll = scroll_node(&graph, registry);
        let handlers = registry.handlers(&scroll);
        let pointer = handlers.pointer.expect("pointer");
        let motion_handler = handlers.motion.expect("motion");
        let origin = scroll.absolute_origin();
        let slop_x = origin.x + scroll.frame().width - 14.0;
        let thumb_y = origin.y + 10.0;

        assert_eq!(
            pointer(
                &button(slop_x, thumb_y),
                PointerPhase::Down,
                EventPhase::Target
            ),
            EventResult::Handled
        );
        assert!(capture.target().is_some_and(|target| target == scroll));
        assert_eq!(
            motion_handler(&motion(slop_x, thumb_y + 40.0, 40.0), EventPhase::Target),
            EventResult::Handled
        );
        assert!(scroll.content_offset().y > 0.0);
    });
}

#[test]
fn covered_scrollbar_does_not_arm_from_an_overlay() {
    locked(|registry, capture, focus| {
        let graph = support::graph();
        graph.install(head_tail_scroll(), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 160.0);

        let scroll = scroll_node(&graph, registry);
        let overlay = Node::new();
        overlay.set_frame(Rect::new(0.0, 0.0, 220.0, 160.0));
        overlay.set_is_hit_testable(true);
        root(&graph).add_child(&overlay);

        let dispatcher = dispatcher(&graph, registry, capture, focus);
        let origin = scroll.absolute_origin();
        let scrollbar_x = origin.x + scroll.frame().width - 6.0;
        let thumb_y = origin.y + 10.0;
        dispatcher.dispatch(&InputEvent::MouseButtonDown(button(scrollbar_x, thumb_y)));
        assert!(
            capture.target().is_none(),
            "the covered viewport does not capture"
        );
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            scrollbar_x,
            thumb_y + 40.0,
            40.0,
        )));
        assert_eq!(scroll.content_offset().y, 0.0);
    });
}

#[test]
fn scroll_view_wheel_uses_global_scroll_fallback() {
    locked(|registry, capture, focus| {
        let graph = support::graph();
        graph.install(head_tail_scroll(), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 160.0);

        let scroll = scroll_node(&graph, registry);
        let overlay = Node::new();
        overlay.set_frame(Rect::new(0.0, 0.0, 220.0, 160.0));
        overlay.set_is_hit_testable(true);
        root(&graph).add_child(&overlay);

        let dispatcher = dispatcher(&graph, registry, capture, focus);
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_at(-1.0, 20.0, 20.0)));
        assert!(scroll.content_offset().y > 0.0);
    });
}

#[test]
fn scroll_view_scrollbar_drag_uses_event_time_pointer_capture_context() {
    locked(|registry, capture, focus| {
        PointerCaptureHolder::set_current(None);
        let graph = support::graph();
        graph.install(head_tail_scroll(), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 160.0);

        PointerCaptureHolder::set_current(Some(capture.clone()));
        let scroll = scroll_node(&graph, registry);
        let dispatcher = dispatcher(&graph, registry, capture, focus);
        let origin = scroll.absolute_origin();
        let scrollbar_x = origin.x + scroll.frame().width - 6.0;
        let thumb_y = origin.y + 10.0;
        dispatcher.dispatch(&InputEvent::MouseButtonDown(button(scrollbar_x, thumb_y)));
        assert!(capture.target().is_some_and(|target| target == scroll));
    });
}

#[test]
fn wheel_zero_coordinates_fallback_to_last_cursor() {
    locked(|registry, capture, focus| {
        let graph = support::graph();
        graph.install(
            Row::new(
                VerticalAlignment::Center,
                0.0,
                AnyView::new(TupleView::new(vec![
                    text("spacer", Some(260.0), 160.0),
                    head_tail_scroll(),
                ])),
            )
            .frame_size(480.0, 160.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(480.0, 160.0);

        let scroll = scroll_node(&graph, registry);
        let dispatcher = dispatcher(&graph, registry, capture, focus);
        let origin = scroll.absolute_origin();
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            origin.x + 20.0,
            origin.y + 20.0,
            0.0,
        )));
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_at(-1.0, 0.0, 0.0)));
        assert!(scroll.content_offset().y > 0.0);
    });
}

fn inspector_sections(section_count: usize, row_count: usize) -> Vec<PropertyGridSection> {
    (0..section_count)
        .map(|section| {
            PropertyGridSection::new(
                format!("section-{section}"),
                format!("Section {section}"),
                (0..row_count)
                    .map(|row| {
                        PropertyGridRow::new(
                            format!("row-{section}-{row}"),
                            format!("Row {row}"),
                            AnyView::new(Text::new("value")),
                        )
                    })
                    .collect(),
            )
            .collapsible(true)
        })
        .collect()
}

fn property_grid(sections: Vec<PropertyGridSection>) -> PropertyGrid {
    PropertyGrid::new(sections)
        .label_width(88.0)
        .row_height(24.0)
}

#[test]
fn property_grid_scrolls_inside_constrained_viewport() {
    locked(|registry, capture, _| {
        let graph = support::graph();
        graph.install(
            property_grid(inspector_sections(8, 8)).frame_size(260.0, 180.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(260.0, 180.0);

        let scroll = scroll_node(&graph, registry);
        let handlers = registry.handlers(&scroll);
        assert_eq!(
            handlers.wheel.as_ref().expect("wheel")(&wheel(-1.0), EventPhase::Target),
            EventResult::Handled
        );
        assert!(scroll.content_offset().y > 0.0);

        scroll.set_content_offset(Point::default());
        let pointer = handlers.pointer.expect("pointer");
        let motion_handler = handlers.motion.expect("motion");
        let origin = scroll.absolute_origin();
        let scrollbar_x = origin.x + scroll.frame().width - 6.0;
        let thumb_y = origin.y + 10.0;
        assert_eq!(
            pointer(
                &button(scrollbar_x, thumb_y),
                PointerPhase::Down,
                EventPhase::Target
            ),
            EventResult::Handled
        );
        assert!(capture.target().is_some_and(|target| target == scroll));
        assert_eq!(
            motion_handler(
                &motion(scrollbar_x, thumb_y + 40.0, 40.0),
                EventPhase::Target
            ),
            EventResult::Handled
        );
        assert!(scroll.content_offset().y > 0.0);
    });
}

#[test]
fn property_grid_row_hover_does_not_recompose_grid() {
    locked(|registry, _, _| {
        let graph = support::graph();
        graph.install(
            property_grid(inspector_sections(1, 2)).frame_size(260.0, 120.0),
            Default::default(),
        );
        for _ in 0..6 {
            graph.recomposer.commit_all();
            graph.compute_layout(260.0, 120.0);
        }

        let radius = Theme::default_dark().radius.sm;
        let row = first(&graph, |node| {
            registry.handlers(node).hover.is_some()
                && node.corner_radius() == radius
                && node.background_color().is_some()
        })
        .expect("a hoverable property row");
        let hover = registry.handlers(&row).hover.expect("hover");
        let base = row.background_color().expect("base colour");
        assert!(!graph.recomposer.has_pending());

        hover(HoverPhase::Enter);
        assert!(!graph.recomposer.has_pending(), "hover does not recompose");
        assert_ne!(row.background_color(), Some(base));

        hover(HoverPhase::Leave);
        assert!(!graph.recomposer.has_pending());
        assert_eq!(row.background_color(), Some(base));
    });
}

fn inspector_like_content(sections: Vec<PropertyGridSection>) -> AnyView {
    AnyView::new(
        stretch_box(
            0.0,
            vec![
                AnyView::new(
                    Row::new(
                        VerticalAlignment::Center,
                        8.0,
                        AnyView::new(TupleView::new(vec![
                            AnyView::new(stretch_box(
                                2.0,
                                vec![
                                    AnyView::new(Text::new("Cube")),
                                    AnyView::new(Text::new("Entity")),
                                ],
                            )),
                            AnyView::new(Spacer::new(0.0)),
                            AnyView::new(Text::new("ID 1")),
                        ])),
                    )
                    .padding_edges(10.0, 9.0),
                ),
                AnyView::new(Divider::horizontal()),
                AnyView::new(property_grid(sections).flex(1.0)),
            ],
        )
        .frame_min_width(280.0),
    )
}

#[test]
fn inspector_property_grid_scrolls_inside_panel() {
    locked(|registry, capture, focus| {
        let graph = support::graph();
        graph.install(
            Panel::new(
                "Inspector",
                inspector_like_content(inspector_sections(8, 8)),
            )
            .frame_size(300.0, 260.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(300.0, 260.0);

        let scroll = scroll_node(&graph, registry);
        let dispatcher = dispatcher(&graph, registry, capture, focus);
        let origin = scroll.absolute_origin();
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            origin.x + 12.0,
            origin.y + 12.0,
            0.0,
        )));
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel(-1.0)));
        assert!(
            scroll.content_offset().y > 0.0,
            "the inspector grid scrolls"
        );

        scroll.set_content_offset(Point::default());
        let root_node = root(&graph);
        let root_right = root_node.absolute_origin().x + root_node.frame().width;
        let scrollbar_x = (origin.x + scroll.frame().width - 6.0).min(root_right - 6.0);
        let thumb_y = origin.y + 10.0;
        dispatcher.dispatch(&InputEvent::MouseButtonDown(button(scrollbar_x, thumb_y)));
        assert!(capture.target().is_some_and(|target| target == scroll));
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            scrollbar_x,
            thumb_y + 40.0,
            40.0,
        )));
        assert!(scroll.content_offset().y > 0.0);
    });
}

fn lines(count: usize) -> TextBuffer {
    TextBuffer::from(vec!["line"; count].join("\n"))
}

fn field_in_scroll(text: Binding<TextBuffer>, footer: f32) -> AnyView {
    AnyView::new(
        ScrollView::new(
            ScrollAxis::Vertical,
            column(vec![
                text_view("header", 20.0),
                AnyView::new(TextField::new("", text).frame_width(180.0)),
                text_view("footer", footer),
            ]),
        )
        .frame_size(220.0, 180.0),
    )
}

fn draw_field(field: &Node) {
    if let Some(draw) = field.draw() {
        draw(&mut DrawList::default(), field.absolute_origin());
    }
}

fn text_view(label: &str, height: f32) -> AnyView {
    text(label, None, height)
}

fn store(value: TextBuffer) -> (State<TextBuffer>, Binding<TextBuffer>) {
    let state = State::new(value);
    let binding = state.binding();
    (state, binding)
}

#[test]
fn nested_text_field_consumes_wheel_before_parent_scroll_view() {
    locked(|registry, _, focus| {
        let (_store, binding) = store(lines(12));
        let graph = support::graph();
        graph.install(field_in_scroll(binding, 240.0), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 240.0);

        let scroll = scroll_node(&graph, registry);
        let field = field_node(&graph);
        let state = field_state(&field);
        assert_eq!(
            state.borrow().scroll.max_y,
            0.0,
            "GAP: Swift measures the field's scroll bounds during layout, so `maxY > 0` \
             right after `computeLayout`; Rust measures them only in the draw pass"
        );
        draw_field(&field);
        assert!(
            state.borrow().scroll.max_y > 0.0,
            "twelve lines overflow the field"
        );
        assert_eq!(scroll.content_offset().y, 0.0);
        {
            let mut state = state.borrow_mut();
            state.scroll.max_y = 0.0;
            state.scroll.visible_height = 0.0;
            state.scroll.content_height = 0.0;
        }

        let dispatcher = dispatcher(&graph, registry, &Rc::new(PointerCapture::new()), focus);
        let origin = field.absolute_origin();
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            origin.x + 12.0,
            origin.y + 12.0,
            0.0,
        )));
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel(-1.0)));

        assert_eq!(
            (field.content_offset().y, scroll.content_offset().y),
            (0.0, 30.0),
            "GAP: Swift's text field consumes the wheel and scrolls itself (field > 0, \
             parent 0); Rust's text field wheel handler always returns Ignored, so the \
             notch bubbles to the parent scroll view"
        );
    });
}

#[test]
fn nested_text_field_boundary_contains_wheel() {
    locked(|registry, _, focus| {
        let (_store, binding) = store(lines(16));
        let graph = support::graph();
        graph.install(field_in_scroll(binding, 320.0), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 260.0);

        let scroll = scroll_node(&graph, registry);
        let field = field_node(&graph);
        let state = field_state(&field);
        assert_eq!(
            state.borrow().scroll.max_y,
            0.0,
            "GAP: Swift measures the field's scroll bounds during layout, so `maxY > 0` \
             right after `computeLayout`; Rust measures them only in the draw pass"
        );
        draw_field(&field);
        assert!(state.borrow().scroll.max_y > 0.0);

        focus.focus(Some(&field), true);
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 260.0);
        if let Some(draw) = field.draw() {
            draw(&mut DrawList::default(), field.absolute_origin());
        }

        let dispatcher = dispatcher(&graph, registry, &Rc::new(PointerCapture::new()), focus);
        let origin = field.absolute_origin();
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            origin.x + 12.0,
            origin.y + 12.0,
            0.0,
        )));

        let max_y = state.borrow().scroll.max_y;
        state.borrow_mut().scroll.offset_y = max_y;
        field.set_content_offset(Point::new(0.0, max_y));

        let previous = field.content_offset().y;
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel(-1.0)));
        assert_eq!(field.content_offset().y, previous);
        assert_eq!(
            scroll.content_offset().y,
            30.0,
            "GAP: Swift contains a boundary wheel inside the overflowing field (parent \
             stays 0); Rust's field ignores wheel input, so the parent scrolls a notch"
        );

        for (y, phase) in [(-12.0, ScrollPhase::Began), (-8.0, ScrollPhase::Changed)] {
            dispatcher.dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
                x: 0.0,
                y,
                mouse_x: None,
                mouse_y: None,
                input: ScrollInput {
                    unit: ScrollUnit::Points,
                    device: ScrollDevice::Continuous,
                    phase,
                    momentum_phase: ScrollPhase::None,
                    timestamp: 0.0,
                },
            }));
        }
        assert_eq!(field.content_offset().y, previous);
        assert_eq!(
            scroll.content_offset().y,
            50.0,
            "GAP: Swift contains the continuous gesture inside the field (parent stays \
             0); Rust forwards both precise deltas to the parent"
        );
    });
}

#[test]
fn nested_text_field_without_overflow() {
    locked(|registry, _, _| {
        let (_store, binding) = store(TextBuffer::from("short"));
        let graph = support::graph();
        graph.install(
            ScrollView::new(
                ScrollAxis::Vertical,
                stretch_box(
                    0.0,
                    vec![
                        AnyView::new(TextField::new("", binding).frame_width(180.0)),
                        AnyView::new(empty_box().frame_height(600.0)),
                    ],
                )
                .flex_with(0.0, 0.0, None),
            )
            .frame_size(220.0, 180.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 180.0);
        let scroll = scroll_node(&graph, registry);
        let field = field_node(&graph);
        let dispatcher = EventDispatcher::new(
            graph.tree.clone(),
            registry.clone(),
            Rc::new(PointerCapture::new()),
            Rc::new(FocusChain::new()),
            WindowId::MAIN,
        );
        let frame = field.absolute_frame();
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel_at(
            -1.0,
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
        )));
        assert_eq!(field.content_offset(), Point::default());
        assert_eq!(scroll.content_offset().y, 30.0);
    });
}

fn menu_entries(count: u64) -> Vec<MenuEntry> {
    (0..count)
        .map(|index| MenuEntry::Item(MenuItem::new(index, format!("Item {index}"), || {})))
        .collect()
}

#[test]
fn nested_menu_consumes_boundary_wheel_before_parent_scroll_view() {
    locked(|registry, _, focus| {
        let graph = support::graph();
        graph.install(
            ScrollView::new(
                ScrollAxis::Vertical,
                column(vec![
                    text_view("header", 20.0),
                    AnyView::new(Menu::new(menu_entries(24)).width(180.0).max_visible_rows(5)),
                    text_view("footer", 320.0),
                ]),
            )
            .frame_size(240.0, 200.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(240.0, 300.0);

        let parent = scroll_node(&graph, registry);
        let menu_scroll = first(&graph, |node| {
            *node != parent
                && registry.handlers(node).wheel.is_some()
                && (node.frame().height - 160.0).abs() < 0.1
        })
        .expect("the menu's 160pt inner viewport");

        let menu_wheel = registry.handlers(&menu_scroll).wheel.expect("menu wheel");
        for _ in 0..100 {
            menu_wheel(&wheel(-1.0), EventPhase::Target);
        }

        let dispatcher = dispatcher(&graph, registry, &Rc::new(PointerCapture::new()), focus);
        let origin = menu_scroll.absolute_origin();
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(
            origin.x + 8.0,
            origin.y + 8.0,
            0.0,
        )));
        let previous = menu_scroll.content_offset().y;
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel(-1.0)));
        assert_eq!(menu_scroll.content_offset().y, previous);
        assert_eq!(parent.content_offset().y, 0.0, "the parent does not scroll");
    });
}

#[test]
fn menu_item_interaction_does_not_recompose() {
    locked(|registry, _, _| {
        let activations = Rc::new(Cell::new(0));
        let counter = activations.clone();
        let graph = support::graph();
        graph.install(
            Menu::new(vec![
                MenuEntry::Item(MenuItem::new(1, "Alpha", move || {
                    counter.set(counter.get() + 1)
                })),
                MenuEntry::Item(MenuItem::new(2, "Beta", || {})),
            ])
            .width(180.0),
            Default::default(),
        );
        graph.recomposer.commit_all();
        graph.compute_layout(180.0, 80.0);

        let item = first(&graph, |node| {
            node.attachment::<u64>("__menu_item_id") == Some(1)
        })
        .expect("menu item row was not materialized");
        let hover = registry.handlers(&item).hover.expect("hover");
        let pointer = registry.handlers(&item).pointer.expect("pointer");

        graph.tree.borrow_mut().flush();
        assert!(!graph.recomposer.has_pending());

        hover(HoverPhase::Enter);
        assert!(
            graph.recomposer.has_pending(),
            "the pointer highlight commits through state: one pending recomposition"
        );
        assert!(graph.tree.borrow().has_render_updates());
        assert_eq!(item.attachment::<bool>("__menu_item_hovered"), Some(true));
        graph.recomposer.commit_all();
        graph.compute_layout(180.0, 80.0);
        assert!(!graph.recomposer.has_pending());

        graph.tree.borrow_mut().flush();
        let event = button(8.0, 8.0);
        assert_eq!(
            pointer(&event, PointerPhase::Down, EventPhase::Target),
            EventResult::Handled
        );
        assert!(
            !graph.recomposer.has_pending(),
            "press feedback is render-only"
        );
        assert!(graph.tree.borrow().has_render_updates());
        assert_eq!(item.attachment::<bool>("__menu_item_pressed"), Some(true));
        assert_eq!(activations.get(), 0);

        graph.tree.borrow_mut().flush();
        assert_eq!(
            pointer(&event, PointerPhase::Up, EventPhase::Target),
            EventResult::Handled
        );
        assert!(!graph.recomposer.has_pending());
        assert!(graph.tree.borrow().has_render_updates());
        assert_eq!(item.attachment::<bool>("__menu_item_pressed"), Some(false));
        assert_eq!(activations.get(), 1);
    });
}

#[test]
fn focused_text_field_wheel_uses_pointer_region() {
    locked(|registry, _, focus| {
        let (_store, binding) = store(lines(12));
        let graph = support::graph();
        graph.install(field_in_scroll(binding, 240.0), Default::default());
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 240.0);

        let scroll = scroll_node(&graph, registry);
        let field = field_node(&graph);
        assert_eq!(
            field_state(&field).borrow().scroll.max_y,
            0.0,
            "GAP: Swift measures the field's scroll bounds during layout, so `maxY > 0` \
             right after `computeLayout`; Rust measures them only in the draw pass"
        );
        draw_field(&field);
        assert!(field_state(&field).borrow().scroll.max_y > 0.0);

        focus.focus(Some(&field), true);
        graph.recomposer.commit_all();
        graph.compute_layout(220.0, 240.0);
        if let Some(draw) = field.draw() {
            draw(&mut DrawList::default(), field.absolute_origin());
        }
        assert_eq!(
            field.attachment::<WheelRoutingPriority>(WheelRoutingAttachmentKey::PRIORITY),
            None,
            "GAP: Swift marks a focused, overflowing text field with \
             `WheelRoutingPriority.preferFocused`; Rust's text field never sets a wheel \
             routing priority"
        );

        let dispatcher = dispatcher(&graph, registry, &Rc::new(PointerCapture::new()), focus);
        let previous = field.content_offset().y;
        dispatcher.dispatch(&InputEvent::MouseMotion(motion(12.0, 12.0, 0.0)));
        dispatcher.dispatch(&InputEvent::MouseWheel(wheel(-1.0)));
        assert_eq!(field.content_offset().y, previous);
        assert!(
            scroll.content_offset().y > 0.0,
            "the parent under the pointer scrolls"
        );
    });
}
