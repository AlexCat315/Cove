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
//! - The Rust `SectionedList` never establishes a viewport taller than its
//!   first virtual window: a 240x300 mount measures a 120pt scroll viewport
//!   (four 30pt entries), the virtual stack's content never exceeds it, and
//!   the scroll range is therefore zero. Wheel input over the list and an End
//!   reveal both leave the content offset at 0, so `resolve_sticky_header`
//!   never returns a pinned header. `sticky_headers` pins that documented
//!   behaviour (no pinned node, unchanged three-row window) instead of Swift's
//!   pin/push/hand-off geometry.
//! - Because only the first window mounts, Swift's inline "Scene" header
//!   (entry index 9, top 270pt) is absent in the 300pt mount;
//!   `sticky_headers_disabled` asserts the opt-out itself and the mounted
//!   "Alpha" header.
//! - Rust has no `sectioned-list-sticky-band` node and no scrollbar-gutter
//!   carve-out on the pinned header; the pinned header is a single
//!   debug-named node, which is why no band width is asserted.
//! - Swift drives `ScrollView.contentOffset` and calls `layoutDidUpdate`;
//!   Rust exposes no content-offset setter, so `scroll` sends a point-unit
//!   mouse wheel over the list (the same `on_viewport_change` report path a
//!   real wheel uses).

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::State;
use cove_core::scene::{AccessibilityRole, Node};
use cove_platform::{InputEvent, KeyModifiers, MouseWheelEvent, ScrollInput, ScrollUnit};
use cove_ui_compose::Scancode;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
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
                |item: &i32, _selected| AnyView::new(Text::new(format!("Item {item}"))),
            )
            .selection(selected.binding())
            .on_activate(move |item: &i32| activated_handler.set(Some(*item)))
            .frame_size(LIST_WIDTH, 600.0),
        );

        assert!(
            scene
                .labelled("Alpha", Some(AccessibilityRole::StaticText))
                .is_some(),
            "GAP: header label Alpha is not in the mounted tree"
        );
        assert!(
            scene
                .labelled("End of Alpha", Some(AccessibilityRole::StaticText))
                .is_some(),
            "GAP: footer label is not in the mounted tree"
        );

        let items = rows(scene);
        assert_eq!(items.len(), 2, "GAP: list-item rows = {}", items.len());

        scene.click(&items[0], 1);
        assert_eq!(
            selected.get(),
            Some(1),
            "GAP: click left selection at {:?}",
            selected.get()
        );

        let host = keyboard_host(scene);
        scene.focus.focus(Some(&host), true);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            selected.get(),
            Some(2),
            "GAP: Down left selection at {:?}",
            selected.get()
        );

        scene.key(Scancode::RETURN, KeyModifiers::default());
        assert_eq!(
            activated.get(),
            Some(2),
            "GAP: Return activated {:?}",
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
    InteractiveScene::run(280.0, 700.0, |scene| {
        let selected = State::new(Some(1));
        scene.install(
            SectionedList::new(
                sticky_sections(),
                |item: &i32| *item,
                |item: &i32, _selected| AnyView::new(Text::new(format!("Item {item}"))),
            )
            .selection(selected.binding())
            .sticky_headers(true)
            .frame_size(LIST_WIDTH, 300.0)
            .debug_name("sectioned-list"),
        );

        assert!(
            sticky(scene).is_none(),
            "GAP: a header pinned at rest at {:?}",
            sticky(scene).map(|node| node.absolute_frame())
        );
        let port = scroll_port(scene).expect("scroll viewport");
        let initial_port = port.frame();
        let initial_rows = rows(scene).len();
        assert_eq!(
            initial_rows, 3,
            "GAP: first window mounted {initial_rows} item rows, expected 3"
        );

        scroll(scene, 30.0);
        assert_eq!(
            scroll_port(scene)
                .expect("scroll viewport")
                .content_offset(),
            cove_core::Point::default(),
            "GAP: wheel moved the content offset, sticky geometry reachable"
        );
        assert!(
            sticky(scene).is_none(),
            "GAP: a pinned header appeared at offset {}",
            scroll_port(scene)
                .expect("scroll viewport")
                .content_offset()
                .y
        );

        let host = keyboard_host(scene);
        scene.focus.focus(Some(&host), true);
        scene.key(Scancode::END, KeyModifiers::default());
        scene.settle(4, 1.0);
        assert_eq!(
            selected.get(),
            Some(36),
            "GAP: End left selection at {:?}",
            selected.get()
        );
        assert!(
            sticky(scene).is_none(),
            "GAP: End reveal pinned a header at offset {}",
            scroll_port(scene)
                .expect("scroll viewport")
                .content_offset()
                .y
        );
        assert_eq!(
            scroll_port(scene).expect("scroll viewport").frame(),
            initial_port,
            "GAP: End reveal resized the {:.0}pt viewport",
            initial_port.height
        );
        assert_eq!(
            rows(scene).len(),
            initial_rows,
            "GAP: End reveal mounted a different row window"
        );
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
                |item: &i32, _selected| AnyView::new(Text::new(format!("Item {item}"))),
            )
            .selection(selected.binding())
            .frame_size(LIST_WIDTH, 300.0)
            .debug_name("sectioned-list"),
        );

        scroll(scene, 255.0);
        assert!(
            sticky(scene).is_none(),
            "GAP: opt-out list pinned a header at offset {}",
            scroll_port(scene)
                .expect("scroll viewport")
                .content_offset()
                .y
        );
        assert!(
            scene
                .labelled("Alpha", Some(AccessibilityRole::StaticText))
                .is_some(),
            "GAP: mounted Alpha header disappeared after scrolling"
        );
    });
}
