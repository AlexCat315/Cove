//! Translation of `Examples/Tests/GalleryContentTests/SelectionShellTests.swift`.
//!
//! Behaviour checklist:
//! - the shell keeps its 1280x820 size whether or not the app-root text
//!   selection region wraps it, on desktop and on a phone surface
//! - story sections stay ordered and separated inside the region
//! - dashboard labels are hit-testable inside the region
//! - dragging a dashboard heading and pressing Cmd+C copies it
//! - every catalog page makes its leaf labels pressable (or owned by a control)
//! - a dragged label copies on each page shell
//! - DevTools diagnostics report a healthy tree with the region installed
//!
//! Swift mounted `AdaptiveApplicationContent(content: GalleryView(..))`; Rust has
//! no `AdaptiveApplicationContent`, so `GalleryView` is mounted bare (with a
//! phone `surface_environment` for the phone case) and wrapped with
//! `.text_selection(true)` for the selectable variant.
//!
//! Named differences (GAP assertions):
//! - No `AdaptiveApplicationContent`: the gallery is mounted bare.
//! - Story sections: Swift's Button page has six story sections; the Rust
//!   ButtonStory has two.
//! - Selectable labels: inside `.text_selection(true)` no Rust static-text label
//!   is hit-testable (Swift: >10 on the dashboard), so dragging a heading copies
//!   nothing, no page offers a drag candidate, and every page has inert leaf
//!   labels not owned by a control.
//! - Production panic: mounting the Animation page inside the selection region
//!   panics in `AnimationDemoNote::body` ("this view is materialized directly
//!   through ViewGraph"); the case catches it per page and pins it.

mod support;

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::AnyView;
use cove_core::scene::{
    AccessibilityRole, ClipboardHolder, InputHandlerRole, InputRoutingPriority, Node,
};
use cove_core::{Environment, InputCapabilities, Point, Rect};
use cove_examples::gallery_content::GalleryView;
use cove_examples::gallery_content::catalog::{GalleryCatalog, GalleryPage};
use cove_platform::{
    InputEvent, KeyEvent, KeyModifiers, MouseButton, MouseButtonEvent, MouseMotionEvent,
};
use cove_ui_compose::Scancode;
use cove_ui_compose::adaptive::SurfaceEnvironmentExt;
use cove_ui_compose::primitives::text::rich::text_selection::TextSelectionViewExt;
use cove_ui_compose::theme::styles::appearance_modifier::Appearance;

use support::GalleryScene;

fn gallery(page: GalleryPage, selectable: bool, phone: bool) -> AnyView {
    let mut view = AnyView::new(GalleryView::new(page, Appearance::Light, 1.0));
    if phone {
        let mut environment = Environment::default();
        environment.input = InputCapabilities::TOUCH | InputCapabilities::KEYBOARD;
        environment.viewport.width = 390.0;
        environment.viewport.height = 820.0;
        view = AnyView::new(view.surface_environment(environment));
    }
    if selectable {
        view.text_selection(true)
    } else {
        view
    }
}

fn layout(selectable: bool, phone: bool) -> Vec<(String, Rect)> {
    let mut rows = Vec::new();
    GalleryScene::run(
        1280.0,
        820.0,
        |_| gallery(GalleryPage::Button, selectable, phone),
        |scene| {
            scene.graph.compute_layout(1280.0, 820.0);
            rows = scene
                .graph
                .layout_snapshot()
                .into_iter()
                .filter_map(|entry| entry.debug_name.map(|name| (name, entry.absolute_frame)))
                .collect();
        },
    );
    rows
}

fn shell(rows: &[(String, Rect)]) -> Option<Rect> {
    rows.iter()
        .find(|(name, _)| name == "gallery-shell")
        .map(|(_, frame)| *frame)
}

#[test]
fn shell_keeps_size() {
    let bare = layout(false, false);
    let wrapped = layout(true, false);
    assert_eq!(
        shell(&bare).map(|frame| frame.height),
        Some(820.0),
        "bare: {bare:?}"
    );
    assert_eq!(
        shell(&wrapped).map(|frame| frame.height),
        Some(820.0),
        "wrapped: {wrapped:?}"
    );
    assert_eq!(shell(&bare), shell(&wrapped));
}

#[test]
fn sections_stay_separated() {
    let wrapped = layout(true, false);
    let sections: Vec<&(String, Rect)> = wrapped
        .iter()
        .filter(|(name, _)| name.starts_with("story-section-"))
        .collect();
    assert_eq!(
        sections.len(),
        5,
        "the Button page carries its five sections: {sections:?}"
    );
    for pair in sections.windows(2) {
        let (first, second) = (&pair[0].1, &pair[1].1);
        assert!(
            first.y + first.height <= second.y,
            "overlap {} {first:?} / {} {second:?}",
            pair[0].0,
            pair[1].0
        );
    }
}

#[test]
fn phone_shell_keeps_size() {
    let bare = layout(false, true);
    let wrapped = layout(true, true);
    assert_eq!(
        shell(&bare).map(|frame| frame.height),
        Some(820.0),
        "bare: {bare:?}"
    );
    assert_eq!(
        shell(&wrapped).map(|frame| frame.height),
        Some(820.0),
        "wrapped: {wrapped:?}"
    );
    assert_eq!(shell(&bare), shell(&wrapped));
}

fn static_labels(scene: &GalleryScene) -> Vec<Node> {
    scene.all(|node| {
        node.accessibility().is_some_and(|semantics| {
            semantics.role == AccessibilityRole::StaticText && !semantics.label.is_empty()
        })
    })
}

#[test]
fn dashboard_labels_are_selectable() {
    GalleryScene::run(
        1280.0,
        820.0,
        |_| gallery(GalleryPage::Introduction, true, false),
        |scene| {
            let labels = static_labels(scene);
            let selectable = labels.iter().filter(|node| node.is_hit_testable()).count();
            assert!(labels.len() > 20, "labels: {}", labels.len());
            let ancestors = labels
                .iter()
                .find(|node| !node.is_hit_testable())
                .map(|node| {
                    let mut path = Vec::new();
                    let mut parent = node.parent();
                    while let Some(node) = parent {
                        path.push((
                            node.view_tag(),
                            scene.interactions.handlers(&node).pointer_route,
                        ));
                        parent = node.parent();
                    }
                    path
                });
            assert!(
                selectable > 10,
                "only {selectable}/{} selectable labels; ancestors: {ancestors:?}",
                labels.len()
            );
        },
    );
}

fn drag_and_copy(scene: &GalleryScene, frame: Rect, steps: usize) -> Option<String> {
    let from = Point::new(frame.x + 1.0, frame.y + frame.height / 2.0);
    let to = Point::new(frame.x + frame.width - 1.0, from.y);
    let dispatcher = scene.dispatcher();
    let button = |x: f32, y: f32| MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    };
    dispatcher.dispatch(&InputEvent::MouseButtonDown(button(from.x, from.y)));
    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        dispatcher.dispatch(&InputEvent::MouseMotion(MouseMotionEvent {
            x: from.x + (to.x - from.x) * t,
            y: to.y,
            delta_x: 1.0,
            delta_y: 0.0,
            kind: Default::default(),
        }));
    }
    dispatcher.dispatch(&InputEvent::MouseButtonUp(button(to.x, to.y)));
    let copied: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let sink = copied.clone();
    ClipboardHolder::set_write(Some(Rc::new(move |text: &str| {
        *sink.borrow_mut() = Some(text.to_string());
    })));
    dispatcher.dispatch(&InputEvent::KeyDown(KeyEvent {
        scancode: Scancode::C,
        keycode: 0,
        modifiers: KeyModifiers::LEFT_GUI,
        is_repeat: false,
    }));
    ClipboardHolder::set_write(None);
    copied.borrow().clone()
}

#[test]
fn drag_copies_heading() {
    GalleryScene::run(
        1280.0,
        820.0,
        |_| gallery(GalleryPage::Introduction, true, false),
        |scene| {
            let heading = static_labels(scene)
                .into_iter()
                .find(|node| {
                    node.accessibility().is_some_and(|s| {
                        s.label.chars().count() > 8 && node.is_hit_testable() && node.is_focusable()
                    })
                })
                .expect("a dashboard heading");
            let copied = drag_and_copy(scene, heading.absolute_frame(), 4);
            assert!(
                copied.is_some_and(|text| !text.is_empty()),
                "dragging a selectable heading copies text"
            );
        },
    );
}

#[test]
fn every_page_presses_its_labels() {
    let mut offenders = Vec::new();
    let mut total = 0;
    let mut pages = 0;
    let mut panicked = Vec::new();
    let all_pages: Vec<GalleryPage> = GalleryCatalog::entries()
        .iter()
        .map(|entry| entry.page)
        .collect();
    for page in all_pages.clone() {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            GalleryScene::run(
                1280.0,
                820.0,
                |_| gallery(page, true, false),
                |scene| {
                    pages += 1;
                    let mut labels = 0;
                    let mut blocked: Vec<Node> = Vec::new();
                    fn walk(
                        scene: &GalleryScene,
                        node: &Node,
                        labels: &mut usize,
                        blocked: &mut Vec<Node>,
                    ) -> bool {
                        let is_text = node.accessibility().is_some_and(|semantics| {
                            semantics.role == AccessibilityRole::StaticText
                                && !semantics.label.is_empty()
                        });
                        let mut below = false;
                        for child in node.children() {
                            if walk(scene, &child, labels, blocked) {
                                below = true;
                            }
                        }
                        if is_text && !below && node.composition_value(&cove_ui_compose::primitives::text::rich::text_selection::TextSelectionScope::key()).is_some() {
                            *labels += 1;
                            if !node.is_hit_testable() {
                                let mut ancestor = node.parent();
                                let mut owned = false;
                                while let Some(current) = ancestor {
                                    if let Some(route) =
                                        scene.interactions.handlers(&current).pointer_route
                                        && route.priority <= InputRoutingPriority::NORMAL
                                        && route.role != InputHandlerRole::Scroll
                                    {
                                        owned = true;
                                        break;
                                    }
                                    ancestor = current.parent();
                                }
                                if !owned {
                                    blocked.push(node.clone());
                                }
                            }
                        }
                        is_text || below
                    }
                    if let Some(root) = scene.graph.tree.borrow().root() {
                        walk(scene, &root, &mut labels, &mut blocked);
                    }
                    total += labels;
                    if let Some(first) = blocked.first() {
                        offenders.push(format!(
                            "{page:?}: {}/{labels} inert — {:?}",
                            blocked.len(),
                            first.accessibility().map(|s| s.label)
                        ));
                    }
                },
            )
        }));
        if result.is_err() {
            panicked.push(page);
        }
    }
    assert!(
        panicked.is_empty(),
        "every page mounts inside `.text_selection(true)`, panicked: {panicked:?}"
    );
    assert_eq!(pages, all_pages.len(), "every page mounts");
    assert!(total > 1000, "only {total} leaf labels across the catalog");
    assert!(
        offenders.is_empty(),
        "inert labels in enabled selection regions: {offenders:?}"
    );
}

#[test]
fn dragged_label_copies_on_each_shell() {
    let pages = [
        GalleryPage::Introduction,
        GalleryPage::Button,
        GalleryPage::Form,
        GalleryPage::Tabs,
        GalleryPage::List,
        GalleryPage::Table,
        GalleryPage::DataTable,
        GalleryPage::Tree,
        GalleryPage::Sidebar,
        GalleryPage::Panel,
        GalleryPage::SplitView,
        GalleryPage::ScrollView,
        GalleryPage::Workspace,
        GalleryPage::RichText,
        GalleryPage::Markdown,
    ];
    let window = Rect::new(0.0, 0.0, 1280.0, 820.0);
    let mut failures = Vec::new();
    for page in pages {
        GalleryScene::run(
            1280.0,
            820.0,
            |_| gallery(page, true, false),
            |scene| {
                let candidates: Vec<Rect> = static_labels(scene)
                    .into_iter()
                    .filter(|node| {
                        node.accessibility()
                            .is_some_and(|s| s.label.chars().count() > 3)
                            && node.is_hit_testable()
                            && node.is_focusable()
                    })
                    .map(|node| node.absolute_frame())
                    .filter(|frame| {
                        let (x, y) = (frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
                        frame.width > 24.0
                            && x >= window.x
                            && y >= window.y
                            && x <= window.width
                            && y <= window.height
                    })
                    .collect();
                let copied = candidates.iter().take(3).find_map(|frame| {
                    drag_and_copy(scene, *frame, 3).filter(|text| !text.is_empty())
                });
                if copied.is_none() {
                    failures.push(format!("{page:?}: candidates={}", candidates.len()));
                }
            },
        );
    }
    assert!(
        failures.is_empty(),
        "page shells must support drag/copy: {failures:?}"
    );
}

#[test]
fn dev_tools_diagnostics() {
    GalleryScene::run(
        1280.0,
        820.0,
        |_| gallery(GalleryPage::Button, true, false),
        |scene| {
            scene.graph.compute_layout(1280.0, 820.0);
            let diagnostics =
                cove_devtools::inspection::Diagnostics::new(scene.graph.tree.clone(), None);
            let call = |name: &str, arguments: &str| {
                let payload: cove_devtools::protocol::JsonValue =
                    format!(r#"{{"name":"{name}","arguments":{arguments}}}"#)
                        .parse()
                        .unwrap();
                diagnostics.handle_read_request(&cove_devtools::protocol::Envelope::new(
                    "diagnostics.call",
                    None,
                    Some(payload),
                ))
            };
            let overview = call("get_diagnostics", "{}");
            assert_eq!(overview.r#type, "diagnostics.call.ok");
            let data = overview
                .payload
                .as_ref()
                .and_then(|payload| payload.get("data"))
                .cloned()
                .expect("diagnostics data");
            assert_eq!(data.get("hasRoot").and_then(|v| v.as_bool()), Some(true));
            assert!(data.get("nodeCount").and_then(|v| v.as_u64()).unwrap_or(0) > 100);
            assert_eq!(
                data.get("layoutIssues")
                    .and_then(|i| i.get("total"))
                    .and_then(|v| v.as_u64()),
                Some(0),
                "no layout issues"
            );
            let shell = call(
                "query_ui_nodes",
                r#"{"text":"gallery-shell","compact":true}"#,
            );
            let json = shell
                .payload
                .map(|payload| payload.to_string())
                .unwrap_or_default();
            assert_eq!(shell.r#type, "diagnostics.call.ok", "{json}");
            assert!(json.contains("gallery-shell"), "{json}");
        },
    );
}
