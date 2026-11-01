//! `WebView` as a composed view plus the node resource that owns its surface.
use super::{
    WebViewGeometry, WebViewHandle, WebViewHost, WebViewRequest, WebViewState, WebViewSurface,
    resolve_geometry,
};
use cove_composition::{AnyView, PrimitiveView, impl_primitive_view};
use cove_core::scene::{Align, FlexDirection, LayoutNode, Node, NodeResource};
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

/// Owns the platform surface for one node. The surface is created lazily so a
/// `WebView` can be built before its host window exists; the first geometry
/// update attaches it.
pub struct WebViewNodeResource {
    host: RefCell<Option<Rc<dyn WebViewHost>>>,
    surface: RefCell<Option<Rc<RefCell<Box<dyn WebViewSurface>>>>>,
    request: RefCell<Option<WebViewRequest>>,
    on_state_change: RefCell<Option<Rc<dyn Fn(WebViewState)>>>,
    handle: RefCell<Option<Rc<RefCell<WebViewHandle>>>>,
    /// Set once `unmount` ran so a later geometry callback cannot resurrect a
    /// detached native view.
    detached: RefCell<bool>,
}

impl Default for WebViewNodeResource {
    fn default() -> Self {
        Self {
            host: RefCell::new(None),
            surface: RefCell::new(None),
            request: RefCell::new(None),
            on_state_change: RefCell::new(None),
            handle: RefCell::new(None),
            detached: RefCell::new(false),
        }
    }
}

impl WebViewNodeResource {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Apply a new configuration. A different host tears the surface down so
    /// the native view cannot outlive the host that owns it; a different
    /// request reloads the existing surface.
    pub fn configure(
        &self,
        host: Rc<dyn WebViewHost>,
        request: WebViewRequest,
        handle: Option<Rc<RefCell<WebViewHandle>>>,
        on_state_change: Option<Box<dyn Fn(WebViewState)>>,
    ) {
        let host_changed = self
            .host
            .borrow()
            .as_ref()
            .is_none_or(|current| !Rc::ptr_eq(current, &host));
        let request_changed = self.request.borrow().as_ref() != Some(&request);

        // Rebind the handle, detaching the old surface from it first so a stale
        // handle cannot navigate a surface it no longer owns.
        let current_handle = self.handle.borrow().clone();
        let new_handle_matches = match (&current_handle, &handle) {
            (None, None) => true,
            (Some(current), Some(next)) => Rc::ptr_eq(current, next),
            _ => false,
        };
        if !new_handle_matches {
            if let (Some(current), Some(surface)) =
                (&current_handle, self.surface.borrow().as_ref())
            {
                current.borrow_mut().detach(surface);
            }
            *self.handle.borrow_mut() = handle.clone();
        }

        *self.host.borrow_mut() = Some(host.clone());
        *self.request.borrow_mut() = Some(request.clone());
        *self.on_state_change.borrow_mut() =
            on_state_change.map(|callback| Rc::from(callback) as Rc<dyn Fn(WebViewState)>);

        if host_changed
            && let Some(surface) = self.surface.borrow_mut().take() {
                surface.borrow_mut().detach();
            }

        let surface = self.ensure_surface(&host);
        // Clone the `Rc` out of the `RefCell` first: a surface that publishes
        // synchronously from `set_on_state_change` must not see a live borrow.
        let current = self.on_state_change.borrow().clone();
        let forward: Option<Box<dyn Fn(WebViewState)>> = current.map(|callback| {
            Box::new(move |state: WebViewState| callback(state)) as Box<dyn Fn(WebViewState)>
        });
        surface.borrow_mut().set_on_state_change(forward);
        if let Some(handle) = &handle {
            handle.borrow_mut().attach(surface.clone());
        }
        if host_changed || request_changed {
            surface.borrow_mut().load(&request);
        }
    }

    fn ensure_surface(&self, host: &Rc<dyn WebViewHost>) -> Rc<RefCell<Box<dyn WebViewSurface>>> {
        if let Some(surface) = self.surface.borrow().as_ref() {
            return surface.clone();
        }
        let created: Rc<RefCell<Box<dyn WebViewSurface>>> =
            Rc::new(RefCell::new(host.make_surface()));
        *self.surface.borrow_mut() = Some(created.clone());
        created
    }

    /// Push the node's clipped window-space rect into the native surface.
    pub fn update_geometry(&self, node: &Node) {
        if *self.detached.borrow() {
            return;
        }
        if let Some(surface) = self.surface.borrow().as_ref() {
            surface.borrow_mut().update_geometry(resolve_geometry(node));
        }
    }

    pub fn is_attached(&self) -> bool {
        self.surface.borrow().is_some()
    }

    fn teardown(&self) {
        *self.detached.borrow_mut() = true;
        if let Some(surface) = self.surface.borrow_mut().take() {
            surface.borrow_mut().detach();
            if let Some(handle) = self.handle.borrow().as_ref() {
                handle.borrow_mut().detach(&surface);
            }
        }
        *self.handle.borrow_mut() = None;
        *self.host.borrow_mut() = None;
        *self.request.borrow_mut() = None;
        *self.on_state_change.borrow_mut() = None;
    }
}

impl NodeResource for WebViewNodeResource {
    fn mount(&self, _node: &Node) {}
    fn unmount(&self, _node: &Node) {
        self.teardown();
    }
    fn as_any_rc(self: Rc<Self>) -> Rc<dyn Any> {
        self
    }
}

/// Native web content inside the composed tree.
///
/// The view owns layout, clipping and lifetime. The actual browser view comes
/// from a [`WebViewHost`], so the same code runs in an SDL window, an app host
/// or a custom embedded surface.
pub struct WebView {
    request: WebViewRequest,
    host: Rc<dyn WebViewHost>,
    handle: Option<Rc<RefCell<WebViewHandle>>>,
    on_state_change: Option<Rc<dyn Fn(WebViewState)>>,
}

impl WebView {
    pub fn new(request: WebViewRequest, host: Rc<dyn WebViewHost>) -> Self {
        Self {
            request,
            host,
            handle: None,
            on_state_change: None,
        }
    }
    pub fn handle(mut self, handle: Rc<RefCell<WebViewHandle>>) -> Self {
        self.handle = Some(handle);
        self
    }
    pub fn on_state_change(mut self, callback: impl Fn(WebViewState) + 'static) -> Self {
        self.on_state_change = Some(Rc::new(callback));
        self
    }
    pub fn boxed(self) -> AnyView {
        AnyView::new(self)
    }
}

impl PrimitiveView for WebView {
    fn make_node(&self) -> Node {
        let node = Node::new();
        // A native view owns its own hit testing; the composed node must not
        // block pointer input from reaching the browser.
        node.set_is_hit_testable(false);
        node.add_resource(WebViewNodeResource::new());
        node
    }

    fn update_node(&self, node: &Node) {
        let Some(resource) = node.first_resource::<WebViewNodeResource>() else {
            return;
        };
        let callback = self.on_state_change.clone().map(|callback| {
            Box::new(move |state: WebViewState| callback(state)) as Box<dyn Fn(WebViewState)>
        });
        resource.configure(
            self.host.clone(),
            self.request.clone(),
            self.handle.clone(),
            callback,
        );

        // Geometry must follow layout, and also follow every draw so a clip
        // change from an ancestor resize is picked up even when this node's own
        // frame did not move.
        let layout_resource = resource.clone();
        node.set_layout_did_update(Some(Rc::new(move |node: &Node| {
            layout_resource.update_geometry(node);
        })));
        // The draw hook takes no node, so hold a weak handle to read geometry
        // from. `NodeWeak` upgrades only while the node is alive, which keeps a
        // detached node from resurrecting its surface.
        let draw_node = node.downgrade();
        let draw_resource = resource.clone();
        node.update_draw(
            WebViewDrawIdentity {
                request: self.request.clone(),
                host: Rc::as_ptr(&self.host).cast::<()>() as usize,
            },
            move |_list: &mut cove_core::DrawList, _offset: cove_core::Point| {
                if let Some(node) = Node::upgrade(&draw_node) {
                    draw_resource.update_geometry(&node);
                }
            },
        );
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
        Some(layout)
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
    }
}
impl_primitive_view!(WebView);

/// Identity for the draw callback: a new request or host means the geometry
/// hook has to be re-registered.
#[derive(Clone, PartialEq, Eq, Hash)]
struct WebViewDrawIdentity {
    request: WebViewRequest,
    host: usize,
}

/// Convenience: `WebViewGeometry` for a node without needing the resolver.
pub fn geometry_for(node: &Node) -> WebViewGeometry {
    resolve_geometry(node)
}
