//! Embedded native web views.
//!
//! One composed `WebView` drives a platform surface through [`WebViewHost`], so
//! the same code runs in an SDL window, an app host or a custom embedded
//! surface. Every desktop and mobile platform has a real host:
//!
//! | Platform            | Engine     | Module      |
//! |---------------------|------------|-------------|
//! | macOS / iOS         | WKWebView  | `apple`     |
//! | Windows             | WebView2   | `windows`   |
//! | Linux               | WebKitGTK  | `linux`     |
mod view;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

pub use view::{geometry_for, WebView, WebViewNodeResource};

pub use cove_core::Rect;

/// Platform cursor/geometry helpers shared with the surface implementations.
pub type Point = cove_core::Point;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use apple::{AppleWebKitHost, AppleWebKitSurface};
#[cfg(target_os = "linux")]
pub use linux::{LinuxWebKitHost, LinuxWebKitSurface};
#[cfg(target_os = "windows")]
pub use windows::{WindowsWebView2Host, WindowsWebView2Surface};

/// Content to display. A request is a value: comparing it is how `WebView`
/// decides whether the native surface needs another load.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WebViewRequest {
    Url(String),
    Html { html: String, base_url: Option<String> },
}

impl WebViewRequest {
    pub fn url(url: impl Into<String>) -> Self {
        Self::Url(url.into())
    }
    pub fn html(html: impl Into<String>) -> Self {
        Self::Html { html: html.into(), base_url: None }
    }
    pub fn html_with_base(html: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self::Html { html: html.into(), base_url: Some(base_url.into()) }
    }
}

/// Navigation phase of one surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WebViewPhase {
    #[default]
    Idle,
    Loading,
    Loaded,
    Failed,
}

/// Observable-facing snapshot of one native web surface. Platform hosts own the
/// navigation state and publish immutable copies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebViewState {
    pub phase: WebViewPhase,
    pub url: Option<String>,
    pub title: Option<String>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub error_description: Option<String>,
}

impl WebViewState {
    pub fn is_loading(&self) -> bool {
        self.phase == WebViewPhase::Loading
    }
    pub fn has_failed(&self) -> bool {
        self.phase == WebViewPhase::Failed
    }
    /// Title, then URL, then empty — the same precedence as Swift.
    pub fn display_title(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.url.clone())
            .unwrap_or_default()
    }
}

/// Navigation command issued by composed controls. Loading new content is a
/// separate operation because it carries a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebViewCommand {
    Back,
    Forward,
    Reload,
    Stop,
}

/// Rectangle reported to a native surface.
///
/// `frame` is the node's window-space rect and `visible_frame` is the part that
/// survives clipping ancestors. Both use the composed top-left coordinate
/// space; a platform host converts to its own view coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WebViewGeometry {
    pub frame: Rect,
    pub visible_frame: Rect,
    pub is_visible: bool,
}

impl WebViewGeometry {
    pub const HIDDEN: Self = Self {
        frame: Rect::new(0.0, 0.0, 0.0, 0.0),
        visible_frame: Rect::new(0.0, 0.0, 0.0, 0.0),
        is_visible: false,
    };

    pub fn new(frame: Rect, visible_frame: Rect, is_visible: bool) -> Self {
        Self { frame, visible_frame, is_visible }
    }
}

/// One platform-owned web surface.
///
/// A surface owns the native browser view. It must attach lazily on the first
/// non-empty geometry so a view created before its host window is ready still
/// works.
pub trait WebViewSurface {
    /// Receives every published navigation state.
    fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>);
    fn load(&mut self, request: &WebViewRequest);
    fn perform(&mut self, command: WebViewCommand);
    /// Window-space geometry of the composed node. `visible_frame` is already
    /// intersected with every clipping ancestor; the host clips the native view
    /// to that rectangle and hides it when nothing remains.
    fn update_geometry(&mut self, geometry: WebViewGeometry);
    /// Remove the native view and release browser resources. Must be idempotent
    /// because node teardown can reach it more than once.
    fn detach(&mut self);
}

/// Platform factory passed by the application. Keeping the window binding in
/// the application (instead of a global) is what lets the same composed view
/// run in a main window, an auxiliary window, or an embedded host.
pub trait WebViewHost {
    fn make_surface(&self) -> Box<dyn WebViewSurface>;
}

/// Imperative navigation handle shared by a `WebView` and its surrounding
/// controls. It is deliberately not observable: navigation state belongs to
/// `WebViewState`, so a handle can live in application state without owning a
/// second source of truth.
///
/// A handle binds to at most one surface. Creating a new `WebView` with the
/// same handle rebinds it, which makes swapping the composed page safe.
#[derive(Default)]
pub struct WebViewHandle {
    surface: Option<std::rc::Rc<std::cell::RefCell<Box<dyn WebViewSurface>>>>,
    pending: Option<WebViewCommand>,
}

impl WebViewHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn go_back(&mut self) {
        self.perform(WebViewCommand::Back);
    }
    pub fn go_forward(&mut self) {
        self.perform(WebViewCommand::Forward);
    }
    pub fn reload(&mut self) {
        self.perform(WebViewCommand::Reload);
    }
    pub fn stop_loading(&mut self) {
        self.perform(WebViewCommand::Stop);
    }

    /// Replace the page without changing the composed request. The next
    /// `WebView` materialisation still wins if it carries a different request.
    pub fn load(&mut self, request: &WebViewRequest) {
        if let Some(surface) = &self.surface {
            surface.borrow_mut().load(request);
        }
    }

    /// A command issued before the surface exists is held and replayed on
    /// attach, matching Swift's `pending`.
    fn perform(&mut self, command: WebViewCommand) {
        match &self.surface {
            Some(surface) => surface.borrow_mut().perform(command),
            None => self.pending = Some(command),
        }
    }

    pub(crate) fn attach(&mut self, surface: std::rc::Rc<std::cell::RefCell<Box<dyn WebViewSurface>>>) {
        self.surface = Some(surface.clone());
        if let Some(pending) = self.pending.take() {
            surface.borrow_mut().perform(pending);
        }
    }

    pub(crate) fn detach(&mut self, surface: &std::rc::Rc<std::cell::RefCell<Box<dyn WebViewSurface>>>) {
        if self
            .surface
            .as_ref()
            .is_some_and(|current| std::rc::Rc::ptr_eq(current, surface))
        {
            self.surface = None;
        }
    }

    pub fn is_attached(&self) -> bool {
        self.surface.is_some()
    }
}

/// Resolve a node against the clipping ancestors that are actually painted. A
/// hidden ancestor hides the native view as well; a native view is never
/// allowed to outlive the composed subtree that owns it.
pub fn resolve_geometry(node: &cove_core::scene::Node) -> WebViewGeometry {
    if !node.is_layout_visible() || node.opacity() <= 0.0 || node.subtree_opacity() <= 0.0 {
        return WebViewGeometry::HIDDEN;
    }
    let frame = node.absolute_frame();
    if frame.width <= 0.0 || frame.height <= 0.0 {
        return WebViewGeometry::HIDDEN;
    }
    let mut visible = frame;
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        if !current.is_layout_visible() || current.opacity() <= 0.0 || current.subtree_opacity() <= 0.0 {
            return WebViewGeometry::HIDDEN;
        }
        if current.clips_to_bounds() {
            let Some(intersected) = visible.intersection(current.absolute_frame()) else {
                return WebViewGeometry::HIDDEN;
            };
            visible = intersected;
        }
        ancestor = current.parent();
    }
    WebViewGeometry::new(frame, visible, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_title_prefers_title_then_url() {
        let state = WebViewState::default();
        assert_eq!(state.display_title(), "");
        let state = WebViewState { url: Some("https://example.com".into()), ..WebViewState::default() };
        assert_eq!(state.display_title(), "https://example.com");
        let state = WebViewState { title: Some("Example".into()), ..state };
        assert_eq!(state.display_title(), "Example");
    }

    #[test]
    fn pending_navigation_replays_on_attach() {
        use std::cell::Cell;
        use std::rc::Rc;
        let performed = Rc::new(Cell::new(0usize));
        struct Recorder {
            performed: Rc<Cell<usize>>,
        }
        impl WebViewSurface for Recorder {
            fn set_on_state_change(&mut self, _callback: Option<Box<dyn Fn(WebViewState)>>) {}
            fn load(&mut self, _request: &WebViewRequest) {}
            fn perform(&mut self, _command: WebViewCommand) {
                self.performed.set(self.performed.get() + 1);
            }
            fn update_geometry(&mut self, _geometry: WebViewGeometry) {}
            fn detach(&mut self) {}
        }
        let surface: Rc<std::cell::RefCell<Box<dyn WebViewSurface>>> =
            Rc::new(std::cell::RefCell::new(Box::new(Recorder { performed: performed.clone() })));
        let mut handle = WebViewHandle::new();
        handle.reload();
        assert!(!handle.is_attached());
        assert_eq!(performed.get(), 0);
        handle.attach(surface.clone());
        assert!(handle.is_attached());
        assert_eq!(performed.get(), 1, "the pending command replays on attach");
        handle.go_back();
        assert_eq!(performed.get(), 2);
        handle.detach(&surface);
        assert!(!handle.is_attached());
    }
}
