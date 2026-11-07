//! Translation of `Tests/CoveUIComposeTests/SectionedListTests.swift`.
//!
//! Behaviour checklist:
//! - headers, footers and one list-item row per item render, and only item rows
//!   carry the list-item role
//! - a row click selects it; Down moves the cursor and Return activates it
//! - sticky headers are opt-in: without the flag no pinned header ever mounts
//! - with the flag, scrolling the list is the only way to pin a header
//!
//! Named differences from the Swift suite:
//! - Rust has no `sectioned-list-sticky-band` node and no scrollbar-gutter
//!   carve-out on the pinned header; the pinned header is a single
//!   debug-named node, which is why no band width is asserted.
//! - Swift drives `ScrollView.contentOffset` and calls `layoutDidUpdate`;
//!   Rust exposes no content-offset setter, so `scroll` sends a point-unit
//!   mouse wheel over the list (the same `on_viewport_change` report path a
//!   real wheel uses).

use crate::support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::State;
use cove_core::scene::{AccessibilityRole, Node};
use cove_platform::{InputEvent, KeyModifiers, MouseWheelEvent, ScrollInput, ScrollUnit};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::RowState;
use cove_ui_compose::primitives::data::lists::Selection;
use cove_ui_compose::primitives::data::lists::{ListSection, SectionedList};
use cove_ui_compose::primitives::text::Text;

use support::InteractiveScene;

const LIST_WIDTH: f32 = 240.0;

fn rows(scene: &InteractiveScene) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
    })
}

fn sticky_title(node: &Node) -> Option<String> {
    node.layout_node()
        .and_then(|layout| layout.text_inputs())
        .map(|inputs| inputs.text)
        .or_else(|| node.children().iter().find_map(sticky_title))
}

fn sticky(scene: &InteractiveScene) -> Option<Node> {
    scene.named("sectioned-list-sticky-header")
}

fn scroll_port(scene: &InteractiveScene) -> Option<Node> {
    scene.first(|node| node.scroll_reveal().is_some())
}

/// A point-unit wheel is an exact offset placement in the Rust scroll view.
fn scroll(scene: &InteractiveScene, to: f32) {
    let list = scene.named("sectioned-list").expect("named list");
    let frame = list.absolute_frame();
    scene
        .dispatcher()
        .dispatch(&InputEvent::MouseWheel(MouseWheelEvent {
            x: 0.0,
            y: -to,
            mouse_x: Some(frame.x + frame.width / 2.0),
            mouse_y: Some(frame.y + frame.height / 2.0),
            input: ScrollInput {
                unit: ScrollUnit::Points,
                ..Default::default()
            },
        }));
    scene.settle(4, 1.0);
}

fn keyboard_host(scene: &InteractiveScene) -> Node {
    scene
        .first(|node| node.is_focusable() && scene.registry.handlers(node).key.is_some())
        .expect("focusable keyboard host")
}

#[test]
fn sections_and_keyboard() {
    InteractiveScene::run(280.0, 700.0, |scene| {
        let selected = State::new(Some(1));
        let activated = Rc::new(Cell::new(None::<i32>));
        let activated_handler = activated.clone();
        scene.install(
            SectionedList::new(
                vec![
                    ListSection::new("a", vec![1, 2])
                        .title("Alpha")
                        .footer("End of Alpha"),
                ],
                |item: &i32| *item,
                |item: &i32, _: RowState| AnyView::new(Text::new(format!("Item {item}"))),
            )
            .selection(Selection::single(selected.binding()))
            .on_activate(move |item: &i32| activated_handler.set(Some(*item)))
            .frame_size(LIST_WIDTH, 600.0),
        );

        assert!(
            scene
                .labelled("Alpha", Some(AccessibilityRole::StaticText))
                .is_some(),
            "the inline header mounts"
        );
        assert!(
            scene
                .labelled("End of Alpha", Some(AccessibilityRole::StaticText))
                .is_some(),
            "the footer mounts"
        );

        let items = rows(scene);
        assert_eq!(
            items.len(),
            2,
            "one list-item role per item: {}",
            items.len()
        );

        scene.click(&items[0], 1);
        assert_eq!(
            selected.get(),
            Some(1),
            "a click selects the first item: {:?}",
            selected.get()
        );

        let host = keyboard_host(scene);
        scene.focus.focus(Some(&host), true);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            selected.get(),
            Some(2),
            "Down selects the next item: {:?}",
            selected.get()
        );

        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            activated.get(),
            Some(2),
            "Return activates the cursor item: {:?}",
            activated.get()
        );
    });
}

fn sticky_sections() -> Vec<ListSection<&'static str, i32>> {
    vec![
        ListSection::new("a", (1..=8).collect()).title("Alpha"),
        ListSection::new("b", (9..=16).collect()).title("Scene"),
        ListSection::new("c", (17..=36).collect()).title("Hidden"),
    ]
}

#[test]
fn sticky_headers() {
    InteractiveScene::run(280.0, 340.0, |scene| {
        let selected = State::new(Some(1));
        scene.install(
            SectionedList::new(
                sticky_sections(),
                |item: &i32| *item,
                |item: &i32, _: RowState| AnyView::new(Text::new(format!("Item {item}"))),
            )
            .selection(Selection::single(selected.binding()))
            .sticky_headers(true)
            .frame_size(LIST_WIDTH, 300.0)
            .debug_name("sectioned-list"),
        );

        assert!(
            sticky(scene).is_none(),
            "no header pins before scrolling: {:?}",
            sticky(scene).map(|node| node.absolute_frame())
        );
        let port = scroll_port(scene).expect("scroll viewport");
        let initial_port = port.frame();
        let initial_rows = rows(scene).len();
        assert!(
            (10..36).contains(&initial_rows),
            "the first window fills the viewport with a bounded buffer: {initial_rows}"
        );
        assert_eq!(
            initial_port.height,
            scene
                .named("sectioned-list")
                .expect("named list")
                .frame()
                .height,
            "the viewport follows its allocation"
        );

        scroll(scene, 30.0);
        assert_eq!(
            scroll_port(scene).expect("viewport").content_offset().y,
            30.0
        );
        let pinned = sticky(scene).expect("scrolling pins Alpha");
        assert_eq!(sticky_title(&pinned).as_deref(), Some("Alpha"));
        assert_eq!(pinned.absolute_frame().y, port.absolute_frame().y);
        // The inset list box keeps 4pt on each side of its rows.
        assert_eq!(pinned.frame().width, LIST_WIDTH - 8.0);
        assert!(
            !pinned.allows_hit_testing(),
            "the pinned decoration never intercepts rows"
        );

        scroll(scene, 255.0);
        assert_eq!(
            sticky_title(&sticky(scene).expect("next pinned header")).as_deref(),
            Some("Scene")
        );
        let host = keyboard_host(scene);
        scene.focus.focus(Some(&host), true);
        scene.key(Scancode::END, KeyModifiers::default());
        scene.settle(4, 1.0);
        assert_eq!(selected.get(), Some(36), "End selects the last item");
        assert_eq!(
            sticky_title(&sticky(scene).expect("last pinned header")).as_deref(),
            Some("Hidden")
        );
        let viewport = scroll_port(scene).expect("viewport");
        assert_eq!(
            viewport.frame(),
            initial_port,
            "revealing a row does not resize the viewport"
        );
        let last = scene
            .labelled("Item 36", Some(AccessibilityRole::StaticText))
            .expect("End mounts the last item");
        assert!(
            last.absolute_frame().max_y() <= viewport.absolute_frame().max_y() + 0.5,
            "End reveals the last item"
        );
        assert!(rows(scene).len() < 36, "the end window remains bounded");
    });
}

#[test]
fn sticky_headers_disabled() {
    InteractiveScene::run(280.0, 700.0, |scene| {
        let selected = State::new(Some(1));
        scene.install(
            SectionedList::new(
                vec![
                    ListSection::new("a", (1..=8).collect()).title("Alpha"),
                    ListSection::new("b", (9..=16).collect()).title("Scene"),
                ],
                |item: &i32| *item,
                |item: &i32, _: RowState| AnyView::new(Text::new(format!("Item {item}"))),
            )
            .selection(Selection::single(selected.binding()))
            .frame_size(LIST_WIDTH, 300.0)
            .debug_name("sectioned-list"),
        );

        scroll(scene, 255.0);
        assert!(
            sticky(scene).is_none(),
            "an opted-out list never pins at offset {}",
            scroll_port(scene)
                .expect("scroll viewport")
                .content_offset()
                .y
        );
        assert!(
            scene
                .labelled("Scene", Some(AccessibilityRole::StaticText))
                .is_some(),
            "the inline Scene header stays available after scrolling"
        );
    });
}
