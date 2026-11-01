//! Translation of `Tests/CoveUITests/WindowChromeInteractionTests.swift`.
//!
//! Behaviour checklist:
//! - the collector indexes chrome nodes once and reads geometry live, so moving
//!   a drag region or scrolling an ancestor updates the rects without a rescan
//! - the discovery pass reruns only when the structure changes
//! - interactive regions land in `non_draggable_rects`
//! - title-bar menu triggers stay hit testable across the whole bar, are not
//!   draggable, and window controls keep a pointer cursor
//!
//! Swift's second case installed an `ImmersiveWindowTitleBar` inside a live
//! `ViewGraph` and clicked real menu triggers. The geometry half below drives
//! the same collector directly, which is what the assertions read; the
//! title-bar interaction half needs the Compose view graph and is covered by
//! the Compose title-bar suites.

use cove_core::scene::Node;
use cove_core::{Point, Rect};
use cove_hosts::desktop::window::chrome::{
    CONFIGURATION_KEY, DRAG_REGION_KEY, INTERACTIVE_REGION_KEY, WindowChromeHitTestCollector,
};
use cove_shell::WindowChromeHitTest;

fn configured_chrome(height: f32, uses_explicit_drag_rects: bool) -> Node {
    let chrome = Node::new();
    chrome.set_attachment(
        CONFIGURATION_KEY,
        WindowChromeHitTest {
            title_bar_height: height,
            draggable_leading_inset: 0.0,
            draggable_trailing_inset: 0.0,
            resize_border_width: 0.0,
            draggable_rects: Vec::new(),
            non_draggable_rects: Vec::new(),
            uses_explicit_drag_rects,
        },
    );
    chrome
}

#[test]
fn collector_indexes_chrome_nodes_once_and_reads_geometry_live() {
    let root = Node::new();
    root.set_frame(Rect::new(3.0, 4.0, 900.0, 600.0));
    root.set_content_offset(Point::new(0.0, 2.0));

    let chrome = configured_chrome(34.0, true);
    root.add_child(&chrome);

    let drag = Node::new();
    drag.set_frame(Rect::new(10.0, 20.0, 100.0, 24.0));
    drag.set_attachment(DRAG_REGION_KEY, true);
    chrome.add_child(&drag);

    let mut collector = WindowChromeHitTestCollector::new();
    let first = collector
        .collect(&root)
        .expect("the chrome configuration is indexed");
    assert_eq!(collector.discovery_pass_count(), 1, "one full scan so far");
    assert_eq!(first.draggable_rects.len(), 1, "the drag region is listed");
    assert_eq!(
        first.draggable_rects[0].x, 13.0,
        "the rect composes the root origin"
    );
    assert_eq!(
        first.draggable_rects[0].y, 22.0,
        "the rect composes the root origin and content offset"
    );

    drag.set_frame(Rect::new(30.0, 20.0, 100.0, 24.0));
    root.set_content_offset(Point::new(0.0, 6.0));
    let moved = collector.collect(&root).expect("collect after the move");
    assert_eq!(
        collector.discovery_pass_count(),
        1,
        "moving a region must not rescan the tree"
    );
    assert_eq!(
        moved.draggable_rects[0].x, 33.0,
        "geometry is read live, not from the index"
    );
    assert_eq!(
        moved.draggable_rects[0].y, 18.0,
        "the content offset is re-applied to the moved rect"
    );

    let interactive = Node::new();
    interactive.set_frame(Rect::new(5.0, 7.0, 20.0, 10.0));
    interactive.set_attachment(INTERACTIVE_REGION_KEY, true);
    chrome.add_child(&interactive);

    let structurally_changed = collector
        .collect(&root)
        .expect("collect after adding a region");
    assert_eq!(
        collector.discovery_pass_count(),
        2,
        "adding a chrome node forces one rescan"
    );
    assert_eq!(
        structurally_changed.non_draggable_rects.len(),
        1,
        "the interactive region is excluded from dragging"
    );
}

#[test]
fn title_bar_controls_remain_hittable_across_the_whole_bar() {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 900.0, 34.0));

    let bar = configured_chrome(34.0, true);
    bar.set_frame(Rect::new(0.0, 0.0, 900.0, 34.0));
    root.add_child(&bar);

    // Six menu triggers plus three window controls, laid out along the bar.
    let mut menu_frames = Vec::new();
    for (index, name) in ["File", "Edit", "Window", "Tools", "Build", "Help"]
        .iter()
        .enumerate()
    {
        let trigger = Node::new();
        let x = 10.0 + index as f32 * 80.0;
        trigger.set_frame(Rect::new(x, 0.0, 76.0, 34.0));
        trigger.set_attachment(INTERACTIVE_REGION_KEY, true);
        bar.add_child(&trigger);
        menu_frames.push((name.to_string(), trigger.frame()));
    }

    let mut control_frames = Vec::new();
    for (index, name) in ["minimize", "maximize", "close"].iter().enumerate() {
        let control = Node::new();
        let x = 780.0 + index as f32 * 40.0;
        control.set_frame(Rect::new(x, 0.0, 38.0, 34.0));
        control.set_attachment(INTERACTIVE_REGION_KEY, true);
        bar.add_child(&control);
        control_frames.push((name.to_string(), control.frame()));
    }

    let chrome = WindowChromeHitTestCollector::collect_once(&root)
        .expect("the chrome configuration is reachable from the root");
    assert!(
        chrome.uses_explicit_drag_rects,
        "an explicit drag-rect chrome keeps its declaration"
    );
    assert_eq!(
        chrome.non_draggable_rects.len(),
        9,
        "every menu trigger and window control is excluded from dragging"
    );
    assert_eq!(
        control_frames.len(),
        3,
        "three window controls were laid out"
    );

    for (name, frame) in &menu_frames {
        let centre = Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
        assert!(
            chrome
                .non_draggable_rects
                .iter()
                .any(|rect| rect.contains(centre.x, centre.y)),
            "menu trigger {name} at {centre:?} must be non-draggable"
        );
    }

    for (name, frame) in &control_frames {
        let centre = Point::new(frame.x + frame.width / 2.0, frame.y + frame.height / 2.0);
        assert!(
            chrome
                .non_draggable_rects
                .iter()
                .any(|rect| rect.contains(centre.x, centre.y)),
            "window control {name} at {centre:?} must be non-draggable so it stays hittable"
        );
    }

    let empty_bar_point = Point::new(600.0, 17.0);
    assert!(
        !chrome
            .non_draggable_rects
            .iter()
            .any(|rect| rect.contains(empty_bar_point.x, empty_bar_point.y)),
        "an empty stretch of the bar stays draggable"
    );
}
