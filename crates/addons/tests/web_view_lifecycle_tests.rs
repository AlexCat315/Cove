//! Translation of `Tests/CoveUIWebViewTests/WebViewLifecycleTests.swift`.
//!
//! Behaviour checklist:
//! - mounting loads once, layout reports geometry, and unmounting detaches
//! - a second install with the same host creates a fresh surface
//! - an unchanged request reuses the surface without a second load
//! - changing the request loads the new content exactly once
//! - a navigation handle binds to the surface and detaches with it

use std::cell::RefCell;
use std::rc::Rc;

use cove_composition::{AnyView, EmptyView, ViewGraph};
use cove_core::Size;
use cove_core::scene::{
    Align, FlexDirection, Justify, Node, NodeResource, NodeTree, Recomposer, Wrap,
};
use cove_ui_addons::webview::{
    WebView, WebViewCommand, WebViewGeometry, WebViewHandle, WebViewHost, WebViewNodeResource,
    WebViewPhase, WebViewRequest, WebViewState, WebViewSurface,
};
use cove_ui_compose::BoxView;
use cove_ui_compose::modifiers::layout::LayoutFrameExt;

type SharedSurface = Rc<RefCell<RecordingSurface>>;

#[derive(Default)]
struct RecordingSurface {
    loads: Vec<WebViewRequest>,
    geometries: Vec<WebViewGeometry>,
    commands: Vec<WebViewCommand>,
    detach_count: usize,
    state_change: Option<Rc<dyn Fn(WebViewState)>>,
}

impl WebViewSurface for RecordingSurface {
    fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>) {
        self.state_change = callback.map(Rc::from);
    }

    fn load(&mut self, request: &WebViewRequest) {
        self.loads.push(request.clone());
        let state = WebViewState {
            phase: WebViewPhase::Loading,
            ..WebViewState::default()
        };
        let callback = self.state_change.clone();
        if let Some(callback) = callback {
            callback(state);
        }
    }

    fn perform(&mut self, command: WebViewCommand) {
        self.commands.push(command);
    }

    fn update_geometry(&mut self, geometry: WebViewGeometry) {
        self.geometries.push(geometry);
    }

    fn detach(&mut self) {
        self.detach_count += 1;
    }
}

#[derive(Default)]
struct RecordingHost {
    surfaces: RefCell<Vec<SharedSurface>>,
}

impl RecordingHost {
    /// The most recently requested surface, mirroring the Swift `fatalError`
    /// when nothing has been asked for yet.
    fn surface(&self) -> SharedSurface {
        self.surfaces
            .borrow()
            .last()
            .cloned()
            .expect("no surface was requested")
    }

    fn surface_count(&self) -> usize {
        self.surfaces.borrow().len()
    }
}

impl WebViewHost for RecordingHost {
    fn make_surface(&self) -> Box<dyn WebViewSurface> {
        let shared: SharedSurface = Rc::new(RefCell::new(RecordingSurface::default()));
        self.surfaces.borrow_mut().push(shared.clone());
        Box::new(ForwardingSurface { shared })
    }
}

/// Forwards to the record the host kept, so the test can read what the surface
/// saw without owning the boxed trait object.
struct ForwardingSurface {
    shared: SharedSurface,
}

impl WebViewSurface for ForwardingSurface {
    fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>) {
        self.shared.borrow_mut().set_on_state_change(callback);
    }

    fn load(&mut self, request: &WebViewRequest) {
        self.shared.borrow_mut().load(request);
    }

    fn perform(&mut self, command: WebViewCommand) {
        self.shared.borrow_mut().perform(command);
    }

    fn update_geometry(&mut self, geometry: WebViewGeometry) {
        self.shared.borrow_mut().update_geometry(geometry);
    }

    fn detach(&mut self) {
        self.shared.borrow_mut().detach();
    }
}

fn boxed(content: impl cove_composition::View + 'static) -> AnyView {
    AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        0.0,
        AnyView::new(content),
    ))
}

fn graph() -> Rc<ViewGraph> {
    Rc::new(ViewGraph::new(
        Rc::new(RefCell::new(NodeTree::new())),
        Rc::new(Recomposer::new()),
    ))
}

fn web_root(
    host: Rc<RecordingHost>,
    request: WebViewRequest,
    handle: Option<Rc<RefCell<WebViewHandle>>>,
) -> AnyView {
    let web = match handle {
        Some(handle) => WebView::new(request, host).handle(handle),
        None => WebView::new(request, host),
    };
    boxed(web.frame_size(600.0, 400.0))
}

#[test]
fn lifecycle() {
    let host = Rc::new(RecordingHost::default());
    let handle = Rc::new(RefCell::new(WebViewHandle::new()));
    let request = WebViewRequest::url("https://example.com");
    let graph = graph();

    graph.install(
        web_root(host.clone(), request.clone(), Some(handle.clone())),
        Default::default(),
    );
    graph.compute_layout(600.0, 400.0);

    assert_eq!(host.surface_count(), 1, "one install makes one surface");
    assert_eq!(host.surface().borrow().loads, vec![request.clone()]);
    let geometry = host
        .surface()
        .borrow()
        .geometries
        .last()
        .cloned()
        .expect("layout reported geometry");
    assert!(geometry.is_visible, "the mounted web view is visible");
    assert_eq!(
        geometry.frame.size(),
        Size {
            width: 600.0,
            height: 400.0
        }
    );
    assert_eq!(
        geometry.visible_frame.size(),
        Size {
            width: 600.0,
            height: 400.0
        }
    );

    let first = host.surface();
    graph.install(EmptyView, Default::default());
    assert_eq!(
        first.borrow().detach_count,
        1,
        "unmounting detaches the native surface exactly once"
    );

    // A second install with the same host must create a fresh surface; a
    // detached surface is never silently reused.
    graph.install(
        web_root(host.clone(), request.clone(), Some(handle)),
        Default::default(),
    );
    assert_eq!(host.surface_count(), 2);
    assert_eq!(
        host.surface().borrow().loads,
        vec![request],
        "the fresh surface loads the request once"
    );
}

#[test]
fn unchanged_request_does_not_reload() {
    let host = Rc::new(RecordingHost::default());
    let host_trait: Rc<dyn WebViewHost> = host.clone();
    let request = WebViewRequest::html("<p>hello</p>");
    let resource = WebViewNodeResource::new();

    resource.configure(host_trait.clone(), request.clone(), None, None);
    resource.configure(host_trait, request.clone(), None, None);

    assert_eq!(host.surface_count(), 1);
    assert_eq!(host.surface().borrow().loads, vec![request]);
}

#[test]
fn request_change_reloads() {
    let host = Rc::new(RecordingHost::default());
    let host_trait: Rc<dyn WebViewHost> = host.clone();
    let first = WebViewRequest::url("https://example.com");
    let second = WebViewRequest::html("<p>next</p>");
    let resource = WebViewNodeResource::new();

    resource.configure(host_trait.clone(), first.clone(), None, None);
    resource.configure(host_trait, second.clone(), None, None);

    assert_eq!(host.surface_count(), 1);
    assert_eq!(host.surface().borrow().loads, vec![first, second]);
}

#[test]
fn handle_binding() {
    let host = Rc::new(RecordingHost::default());
    let host_trait: Rc<dyn WebViewHost> = host.clone();
    let handle = Rc::new(RefCell::new(WebViewHandle::new()));
    let resource = WebViewNodeResource::new();

    resource.configure(
        host_trait,
        WebViewRequest::url("https://example.com"),
        Some(handle.clone()),
        None,
    );
    handle.borrow_mut().reload();
    assert_eq!(
        host.surface().borrow().commands,
        vec![WebViewCommand::Reload],
        "the handle navigates the bound surface"
    );

    resource.unmount(&Node::new());
    handle.borrow_mut().reload();
    assert_eq!(
        host.surface().borrow().commands,
        vec![WebViewCommand::Reload],
        "a detached handle no longer reaches the surface"
    );
}
