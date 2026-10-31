//! Translation of `Tests/CoveUIComposeTests/SectionedListTests.swift`.
//!
//! Behaviour checklist:
//! - headers, footers and one row node per item render, and only item rows
//!   carry the list-item role
//! - a row click selects it; Down moves the cursor and Return activates it
//! - with sticky headers the active title pins to the viewport top, is pushed
//!   up by the next header, hands the slot over at the boundary, keeps a single
//!   accessibility label at the hand-off, and follows an End reveal
//! - without the opt-in no header is ever pinned
//!
//! Named differences from the Swift suite:
//! - Rust `SectionedList` has no `sectioned-list-sticky-band` node and no
//!   `Shadow`/`ScrollView` gutter carve-out on the pinned header: the pinned
//!   header is one debug-named node spanning the list's full width, so the
//!   band assertions pin that width instead of `list.width - gutter`.
//! - The Swift suite drives `ScrollView.contentOffset` and then
//!   `layoutDidUpdate`; Rust exposes no content-offset setter on the list, so
//!   `scroll` sends a point-unit mouse wheel over the list, which reaches the
//!   same `on_viewport_change` report path as a real wheel.
//! - Swift checks that the pinned header hides and hit-test-disables some
//!   descendant; in Rust those apply to the pinned header node itself, which is
//!   the node `sticky` debug-names.

mod support;

use std::cell::Cell;
use std::rc::Rc;

use cove_composition::{AnyView, LayoutDebugExt};
use cove_core::State;
use cove_core::scene::{AccessibilityRole, AccessibilityTree};
use cove_platform::{InputEvent, KeyModifiers, MouseWheelEvent, ScrollInput, ScrollUnit};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::data::lists::{ListSection, SectionedList};
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::Scancode;

use support::InteractiveScene;

const LIST_WIDTH: f32 = 240.0;

fn rows(scene: &InteractiveScene) -> Vec<cove_core::scene::Node> {
    scene.all(|node| {
        node.accessibility()
            .is_some_and(|semantics| semantics.role == AccessibilityRole::ListItem)
    })
}

fn sticky(scene: &InteractiveScene) -> Option<cove_core::scene::Node> {
    scene.named("sectioned-list-sticky-header")
}

fn labelled_inside(node: &cove_core::scene::Node, label: &str) -> bool {
    let mut pending = vec![node.clone()];
    while let Some(current) = pending.pop() {
        if current
            .accessibility()
            .is_some_and(|semantics| semantics.label == label)
        {
            return true;
        }
        pending.extend(current.children());
    }
    false
}

/// A point-unit wheel is an exact offset placement in the Rust scroll view.
fn scroll(scene: &InteractiveScene, to: f32) {
    let list = scene.named("sectioned-list").expect("named list");
    let frame = list.absolute_frame();
    let scroller = scene
        .first(|node| node.scroll_reveal().is_some())
        .expect("scroll view");
    let handler = scene.registry.handlers(&scroller).wheel.expect("wheel");
    handler(
        &MouseWheelEvent {
            x: 0.0,
            y: -to,
            mouse_x: Some(frame.x + frame.width / 2.0),
            mouse_y: Some(frame.y + frame.height / 2.0),
            input: ScrollInput {
                unit: ScrollUnit::Points,
                ..Default::default()
            },
        },
        cove_core::scene::EventPhase::Target,
    );
    eprintln!(
        "scroll to {to}: handler={:?} before={:?}",
        scene
            .first(|node| node.scroll_reveal().is_some())
            .map(|node| scene.registry.handlers(&node).wheel.is_some()),
        scene
            .first(|node| node.scroll_reveal().is_some())
            .map(|node| node.content_offset())
    );
    scene.settle(4, 1.0);
    eprintln!(
        "scroll to {to}: after={:?}",
        scene
            .first(|node| node.scroll_reveal().is_some())
            .map(|node| node.content_offset())
    );
}

fn keyboard_host(scene: &InteractiveScene) -> cove_core::scene::Node {
    scene
        .first(|node| node.is_focusable() && scene.registry.handlers(node).key.is_some())
        .expect("focusable keyboard host")
}

#[test]
fn sections_and_keyboard() {
    InteractiveScene::run(280.0, 700.0, |scene| {
        let selected = State::new(Some(1));
        let activated = Rc::new(Cell::new(None::<i32>));
        let selection = selected.clone();
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
            scene.labelled("Alpha", Some(AccessibilityRole::StaticText)).is_some(),
            "GAP: header label Alpha missing from {:?}",
            scene
                .nodes()
                .iter()
                .filter_map(|node| node.accessibility().map(|s| s.label))
                .collect::<Vec<_>>()
        );
        assert!(
            scene
                .labelled("End of Alpha", Some(AccessibilityRole::StaticText))
                .is_some(),
            "GAP: footer label missing"
        );

        let items = rows(scene);
        assert_eq!(items.len(), 2, "GAP: list-item rows = {}", items.len());

        scene.click(&items[0], 1);
        assert_eq!(
            selection.get(),
            Some(1),
            "GAP: click left selection at {:?}",
            selection.get()
        );

        let host = keyboard_host(scene);
        scene.focus.focus(Some(&host), true);
        scene.key(Scancode::ARROW_DOWN, KeyModifiers::default());
        assert_eq!(
            selection.get(),
            Some(2),
            "GAP: Down left selection at {:?}",
            selection.get()
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
            "GAP: a header pinned before scrolling: {:?}",
            sticky(scene).map(|node| node.absolute_frame())
        );

        let host = keyboard_host(scene);
        scene.focus.focus(Some(&host), true);
        scene.key(Scancode::END, KeyModifiers::default());
        scene.settle(4, 1.0);
        for node in scene.nodes() {
            if let Some(semantics) = node.accessibility()
                && semantics.role == AccessibilityRole::ScrollArea
            {
                eprintln!(
                    "END scroll area frame={:?} offset={:?}",
                    node.absolute_frame(),
                    node.content_offset()
                );
            }
        }
        eprintln!(
            "END selected={:?} focus={:?} sticky={:?} rows={}",
            selected.get(),
            scene.focus.focused().map(|node| support::debug_name(&node)),
            sticky(scene).map(|node| node.accessibility().map(|s| s.label)),
            rows(scene).len()
        );
        let list = scene.named("sectioned-list").expect("named list");
        let list_frame = list.absolute_frame();

        scroll(scene, 30.0);
        let first = sticky(scene).expect("pinned header after scroll 30");
        let first_frame = first.absolute_frame();
        assert!(
            (first_frame.y - list_frame.y).abs() < 0.5,
            "GAP: pinned y {} vs list top {}",
            first_frame.y,
            list_frame.y
        );
        assert!(
            (first_frame.x - list_frame.x).abs() < 0.5,
            "GAP: pinned x {} vs list left {}",
            first_frame.x,
            list_frame.x
        );
        assert!(
            (first_frame.width - list_frame.width).abs() < 0.5,
            "GAP: pinned width {} vs list width {} (no gutter carve-out)",
            first_frame.width,
            list_frame.width
        );
        assert!(
            labelled_inside(&first, "Alpha"),
            "GAP: pinned header has no Alpha label"
        );
        assert!(
            first
                .accessibility()
                .is_some_and(|semantics| semantics.is_hidden),
            "GAP: pinned header is not hidden from accessibility"
        );
        assert!(
            !first.is_hit_testable(),
            "GAP: pinned header still accepts hit testing"
        );

        scroll(scene, 1.0);
        let snapshot = scene
            .graph
            .tree
            .borrow()
            .root()
            .map(|root| AccessibilityTree::snapshot(&root))
            .expect("mounted root");
        fn flatten(elements: Vec<cove_core::scene::AccessibilityElement>) -> usize {
            elements
                .into_iter()
                .map(|element| {
                    let own = usize::from(element.semantics.label == "Alpha");
                    own + flatten(element.children)
                })
                .sum()
        }
        assert_eq!(
            flatten(snapshot),
            1,
            "GAP: Alpha appears more than once at the hand-off boundary"
        );

        scroll(scene, 255.0);
        let pushed = sticky(scene).expect("pinned header after scroll 255");
        let pushed_frame = pushed.absolute_frame();
        assert!(
            (pushed_frame.y - (list_frame.y - 15.0)).abs() < 0.5,
            "GAP: pushed y {} vs expected {}",
            pushed_frame.y,
            list_frame.y - 15.0
        );
        assert!(
            labelled_inside(&pushed, "Alpha"),
            "GAP: pushed header lost its Alpha label"
        );

        scroll(scene, 280.0);
        let second = sticky(scene).expect("pinned header after scroll 280");
        assert!(
            labelled_inside(&second, "Scene"),
            "GAP: second section did not take the pinned slot"
        );
        assert!(
            (second.absolute_frame().y - list_frame.y).abs() < 0.5,
            "GAP: handed-off header y {} vs list top {}",
            second.absolute_frame().y,
            list_frame.y
        );

        scroll(scene, 0.0);
        let host = keyboard_host(scene);
        scene.focus.focus(Some(&host), true);
        scene.key(Scancode::END, KeyModifiers::default());
        scene.settle(4, 1.0);
        let last = sticky(scene).expect("pinned header after End");
        assert!(
            labelled_inside(&last, "Hidden"),
            "GAP: End reveal did not pin the last section"
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
        for node in scene.nodes() {
            if let Some(semantics) = node.accessibility()
                && semantics.role == AccessibilityRole::ScrollArea
            {
                eprintln!(
                    "scroll area frame={:?} offset={:?}",
                    node.absolute_frame(),
                    node.content_offset()
                );
            }
        }
        eprintln!(
            "rows={} list={:?}",
            rows(scene).len(),
            scene.named("sectioned-list").map(|node| node.frame())
        );
        assert!(
            sticky(scene).is_none(),
            "GAP: opt-out list pinned a header"
        );
        assert!(
            scene
                .labelled("Scene", Some(AccessibilityRole::StaticText))
                .is_some(),
            "GAP: inline Scene header disappeared"
        );
    });
}
