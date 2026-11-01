//! Translation of `Tests/CoveUIWebViewTests/WebViewGeometryTests.swift`.
//!
//! Behaviour checklist:
//! - a node without clipping ancestors reports its full window frame
//! - a clipping ancestor trims the visible rectangle but keeps the full frame
//! - a scrolled-out node reports no visible rectangle
//! - a hidden ancestor (opacity or subtree opacity) hides native content
//! - a zero-sized node has no visible rectangle
//! - requests with the same content compare equal
//! - state derives loading/failed/displayTitle without a second source of truth

use cove_core::Rect;
use cove_core::scene::Node;
use cove_ui_addons::webview::{
    WebViewGeometry, WebViewPhase, WebViewRequest, WebViewState, resolve_geometry,
};

fn sized_node(frame: Rect) -> Node {
    let node = Node::new();
    node.set_frame(frame);
    node
}

#[test]
fn unclipped_frame() {
    let root = sized_node(Rect::new(0.0, 0.0, 800.0, 600.0));
    let web = sized_node(Rect::new(40.0, 60.0, 320.0, 240.0));
    root.add_child(&web);

    let geometry = resolve_geometry(&web);
    assert!(geometry.is_visible, "an unclipped node is visible");
    assert_eq!(geometry.frame, Rect::new(40.0, 60.0, 320.0, 240.0));
    assert_eq!(geometry.visible_frame, geometry.frame);
}

#[test]
fn clipped_frame() {
    let root = sized_node(Rect::new(0.0, 0.0, 800.0, 600.0));
    let scroll = sized_node(Rect::new(0.0, 100.0, 400.0, 200.0));
    scroll.set_clips_to_bounds(true);
    root.add_child(&scroll);
    let web = sized_node(Rect::new(20.0, 150.0, 360.0, 400.0));
    scroll.add_child(&web);

    let geometry = resolve_geometry(&web);
    assert!(geometry.is_visible);
    assert_eq!(
        geometry.frame,
        Rect::new(20.0, 250.0, 360.0, 400.0),
        "the full frame keeps the clipped-away part"
    );
    assert_eq!(geometry.visible_frame, Rect::new(20.0, 250.0, 360.0, 50.0));
}

#[test]
fn scrolled_out() {
    let root = sized_node(Rect::new(0.0, 0.0, 800.0, 600.0));
    let scroll = sized_node(Rect::new(0.0, 100.0, 400.0, 200.0));
    scroll.set_clips_to_bounds(true);
    root.add_child(&scroll);
    let web = sized_node(Rect::new(0.0, -300.0, 400.0, 200.0));
    scroll.add_child(&web);

    let geometry = resolve_geometry(&web);
    assert!(!geometry.is_visible, "no overlap leaves nothing to show");
    assert_eq!(geometry, WebViewGeometry::HIDDEN);
}

#[test]
fn hidden_ancestor() {
    let root = sized_node(Rect::new(0.0, 0.0, 800.0, 600.0));
    let panel = sized_node(Rect::new(0.0, 0.0, 400.0, 300.0));
    root.add_child(&panel);
    let web = sized_node(Rect::new(0.0, 0.0, 400.0, 300.0));
    panel.add_child(&web);

    panel.set_opacity(0.0);
    assert_eq!(resolve_geometry(&web), WebViewGeometry::HIDDEN);

    panel.set_opacity(1.0);
    panel.set_subtree_opacity(0.0);
    assert_eq!(resolve_geometry(&web), WebViewGeometry::HIDDEN);
}

#[test]
fn zero_sized() {
    let root = sized_node(Rect::new(0.0, 0.0, 800.0, 600.0));
    let web = sized_node(Rect::new(20.0, 20.0, 0.0, 100.0));
    root.add_child(&web);

    assert_eq!(resolve_geometry(&web), WebViewGeometry::HIDDEN);
}

#[test]
fn request_equality() {
    let url = "https://example.com";
    assert_eq!(
        WebViewRequest::url(url),
        WebViewRequest::Url(url.to_string())
    );
    assert_eq!(
        WebViewRequest::html("<p>Hi</p>"),
        WebViewRequest::Html {
            html: "<p>Hi</p>".to_string(),
            base_url: None,
        }
    );
    assert_ne!(
        WebViewRequest::html("<p>Hi</p>"),
        WebViewRequest::html("<p>Bye</p>")
    );
    assert_ne!(WebViewRequest::url(url), WebViewRequest::html("<p>Hi</p>"));
}

#[test]
fn state_derivation() {
    let mut state = WebViewState::default();
    assert!(!state.is_loading());
    assert!(!state.has_failed());
    assert_eq!(state.display_title(), "");

    state.phase = WebViewPhase::Loading;
    state.url = Some("https://example.com/docs".to_string());
    assert!(state.is_loading());
    assert_eq!(state.display_title(), "https://example.com/docs");

    state.title = Some("Docs".to_string());
    assert_eq!(state.display_title(), "Docs");

    state.phase = WebViewPhase::Failed;
    state.error_description = Some("Offline".to_string());
    assert!(state.has_failed());
}
