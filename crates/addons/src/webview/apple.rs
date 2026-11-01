//! WKWebView-backed host for macOS and iOS.
//!
//! The provider is supplied by the application so the same composed `WebView`
//! can follow the main window, an auxiliary window, or a window restored after
//! the surface was created. WebKit stays an implementation detail of this
//! module; callers only see `WebViewHost`.
//!
//! A rectangular `WKWebView` is clipped to the composed node's visible
//! rectangle. The native view cannot be composited by the renderer, so it lives
//! above the content view. `WebViewGeometry` supplies both the full frame and
//! the part that survives ancestor clipping; a container view turns the latter
//! into AppKit's clip. Rounded corners, opacity and transforms are not
//! representable on a child `NSView` and are intentionally not faked.
#![cfg(target_os = "macos")]

use super::{
    WebViewCommand, WebViewGeometry, WebViewHost, WebViewRequest, WebViewState, WebViewSurface,
};
use crate::webview::WebViewPhase;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSView, NSWindow};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSInteger, NSURL, NSURLRequest};
use objc2_web_kit::{WKNavigation, WKNavigationDelegate, WKWebView, WKWebViewConfiguration};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// `NSURLErrorCancelled`. A superseded load reports cancellation; it is not a
/// visible error, so it must not flip the phase to `failed`.
const NS_URL_ERROR_CANCELLED: NSInteger = -999;

/// Mutable state shared between the surface and its navigation delegate.
pub struct AppleSurfaceState {
    state: WebViewState,
    /// Loaded via `loadHTMLString:`, so a subsequent real navigation means the
    /// document left the local body and its URL should be reported.
    loads_html: bool,
    on_state_change: Option<Box<dyn Fn(WebViewState)>>,
    web_view: Option<Retained<WKWebView>>,
    detached: bool,
}

impl AppleSurfaceState {
    fn publish(&mut self) {
        if let Some(web_view) = &self.web_view {
            self.state.can_go_back = unsafe { web_view.canGoBack() };
            self.state.can_go_forward = unsafe { web_view.canGoForward() };
        }
        if let Some(callback) = &self.on_state_change {
            callback(self.state.clone());
        }
    }

    fn navigation_started(&mut self) {
        self.state.phase = WebViewPhase::Loading;
        self.state.error_description = None;
        // A link inside a local document leaves HTML mode. Keep reporting the
        // real URL from that point so Back/Forward and status text stay honest.
        let current_url = self.web_view.as_ref().and_then(|web_view| {
            unsafe { web_view.URL() }
                .and_then(|url| url.absoluteString())
                .map(|value| value.to_string())
        });
        if self.loads_html
            && let Some(url) = &current_url
            && url != "about:blank"
        {
            self.loads_html = false;
            self.state.url = Some(url.clone());
        }
        if !self.loads_html && self.state.url.is_none() {
            self.state.url = current_url;
        }
        self.publish();
    }

    fn navigation_finished(&mut self) {
        self.state.phase = WebViewPhase::Loaded;
        self.state.error_description = None;
        if let Some(web_view) = &self.web_view {
            if !self.loads_html
                && let Some(url) = unsafe { web_view.URL() }.and_then(|url| url.absoluteString()) {
                    self.state.url = Some(url.to_string());
                }
            if let Some(title) = unsafe { web_view.title() } {
                let title = title.to_string();
                if !title.is_empty() {
                    self.state.title = Some(title);
                }
            }
        }
        self.publish();
    }

    fn navigation_failed(&mut self, cancelled: bool, description: &str) {
        if cancelled {
            return;
        }
        self.state.phase = WebViewPhase::Failed;
        self.state.error_description = Some(description.to_string());
        if !self.loads_html && self.state.url.is_none() {
            self.state.url = self.web_view.as_ref().and_then(|web_view| {
                unsafe { web_view.URL() }
                    .and_then(|url| url.absoluteString())
                    .map(|value| value.to_string())
            });
        }
        self.publish();
    }

    fn web_content_process_terminated(&mut self) {
        self.state.phase = WebViewPhase::Failed;
        self.state.error_description = Some("Web content process terminated".into());
        self.publish();
    }
}

struct DelegateIvars {
    surface: Weak<RefCell<AppleSurfaceState>>,
}

define_class!(
    // SAFETY: `NSObject` has no subclassing requirements and the delegate
    // stores no `Drop` type.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = DelegateIvars]
    #[name = "CoveAppleNavigationDelegate"]
    struct AppleNavigationDelegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for AppleNavigationDelegate {}

    // SAFETY: every selector below matches `WKNavigationDelegate`.
    unsafe impl WKNavigationDelegate for AppleNavigationDelegate {
        #[unsafe(method(webView:didStartProvisionalNavigation:))]
        fn did_start_provisional_navigation(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
        ) {
            if let Some(surface) = self.ivars().surface.upgrade() {
                surface.borrow_mut().navigation_started();
            }
        }

        #[unsafe(method(webView:didCommitNavigation:))]
        fn did_commit_navigation(&self, _web_view: &WKWebView, _navigation: Option<&WKNavigation>) {
            if let Some(surface) = self.ivars().surface.upgrade() {
                surface.borrow_mut().navigation_started();
            }
        }

        #[unsafe(method(webView:didFinishNavigation:))]
        fn did_finish_navigation(&self, _web_view: &WKWebView, _navigation: Option<&WKNavigation>) {
            if let Some(surface) = self.ivars().surface.upgrade() {
                surface.borrow_mut().navigation_finished();
            }
        }

        #[unsafe(method(webView:didFailNavigation:withError:))]
        fn did_fail_navigation(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &objc2_foundation::NSError,
        ) {
            let cancelled = error.code() == NS_URL_ERROR_CANCELLED;
            let description = error.localizedDescription().to_string();
            if let Some(surface) = self.ivars().surface.upgrade() {
                surface
                    .borrow_mut()
                    .navigation_failed(cancelled, &description);
            }
        }

        #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
        fn did_fail_provisional_navigation(
            &self,
            _web_view: &WKWebView,
            _navigation: Option<&WKNavigation>,
            error: &objc2_foundation::NSError,
        ) {
            let cancelled = error.code() == NS_URL_ERROR_CANCELLED;
            let description = error.localizedDescription().to_string();
            if let Some(surface) = self.ivars().surface.upgrade() {
                surface
                    .borrow_mut()
                    .navigation_failed(cancelled, &description);
            }
        }

        #[unsafe(method(webViewWebContentProcessDidTerminate:))]
        fn web_content_process_did_terminate(&self, _web_view: &WKWebView) {
            if let Some(surface) = self.ivars().surface.upgrade() {
                surface.borrow_mut().web_content_process_terminated();
            }
        }
    }
);

impl AppleNavigationDelegate {
    fn new(mtm: MainThreadMarker, surface: Weak<RefCell<AppleSurfaceState>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars { surface });
        unsafe { msg_send![super(this), init] }
    }
}

/// Converts a composed top-left rect into a container view's coordinate space,
/// honouring whether that view is flipped.
fn container_rect(rect: super::Rect, container: &NSView) -> CGRect {
    let bounds = container.bounds();
    let (x, y, width, height) = (
        rect.x as f64,
        rect.y as f64,
        rect.width as f64,
        rect.height as f64,
    );
    let top = if container.isFlipped() {
        y - bounds.origin.y
    } else {
        // AppKit is bottom-left based unless the content view opted into flipping.
        bounds.origin.y + bounds.size.height - (y + height)
    };
    CGRect {
        origin: CGPoint {
            x: x - bounds.origin.x,
            y: top,
        },
        size: CGSize { width, height },
    }
}

/// Offset of the full frame inside the visible rectangle. The web view keeps
/// its full frame inside the smaller clipping container, so the part that is
/// clipped away must be expressed as a negative origin.
fn inner_rect(full: super::Rect, visible: super::Rect, container: &NSView) -> CGRect {
    let (fx, fy, fw, fh) = (
        full.x as f64,
        full.y as f64,
        full.width as f64,
        full.height as f64,
    );
    let (vx, vy, vh) = (visible.x as f64, visible.y as f64, visible.height as f64);
    // The web view keeps the full frame inside a smaller clipping container.
    let y = if container.isFlipped() {
        fy - vy
    } else {
        vh - fh - (fy - vy)
    };
    CGRect {
        origin: CGPoint { x: fx - vx, y },
        size: CGSize {
            width: fw,
            height: fh,
        },
    }
}

pub struct AppleWebKitHost {
    window_provider: Rc<dyn Fn() -> Option<Retained<NSWindow>>>,
}

impl AppleWebKitHost {
    /// `window_provider` runs on every geometry update, so the surface can
    /// follow the window it belongs to even when that window changes.
    pub fn new(window_provider: impl Fn() -> Option<Retained<NSWindow>> + 'static) -> Self {
        Self {
            window_provider: Rc::new(window_provider),
        }
    }
}

impl WebViewHost for AppleWebKitHost {
    fn make_surface(&self) -> Box<dyn WebViewSurface> {
        let provider = self.window_provider.clone();
        Box::new(AppleWebKitSurface {
            state: Rc::new(RefCell::new(AppleSurfaceState {
                state: WebViewState::default(),
                loads_html: false,
                on_state_change: None,
                web_view: None,
                detached: false,
            })),
            window_provider: Box::new(move || provider()),
            container: None,
            delegate: None,
        })
    }
}

/// A rectangular WKWebView clipped to the composed node's visible rectangle.
pub struct AppleWebKitSurface {
    state: Rc<RefCell<AppleSurfaceState>>,
    window_provider: Box<dyn Fn() -> Option<Retained<NSWindow>>>,
    container: Option<Retained<NSView>>,
    delegate: Option<Retained<AppleNavigationDelegate>>,
}

impl AppleWebKitSurface {
    /// Create the native view tree. Only called on the main thread, and only
    /// once, so a `WebView` can be built before its host window exists.
    fn ensure_views(&mut self) {
        if self.state.borrow().web_view.is_some() {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            let mut state = self.state.borrow_mut();
            state.state.phase = WebViewPhase::Failed;
            state.state.error_description =
                Some("WebKit surface must be created on the main thread".into());
            state.publish();
            return;
        };
        let container = NSView::initWithFrame(
            NSView::alloc(mtm),
            CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize {
                    width: 0.0,
                    height: 0.0,
                },
            },
        );
        container.setWantsLayer(true);
        container.setHidden(true);

        let configuration = unsafe { WKWebViewConfiguration::new(mtm) };
        let web_view = unsafe {
            WKWebView::initWithFrame_configuration(
                WKWebView::alloc(mtm),
                CGRect {
                    origin: CGPoint { x: 0.0, y: 0.0 },
                    size: CGSize {
                        width: 0.0,
                        height: 0.0,
                    },
                },
                &configuration,
            )
        };
        unsafe { web_view.setAllowsBackForwardNavigationGestures(true) };
        container.addSubview(&web_view);

        let delegate = AppleNavigationDelegate::new(mtm, Rc::downgrade(&self.state));
        // WebKit holds the delegate weakly; the surface must retain it.
        let delegate_object = objc2::runtime::ProtocolObject::from_ref(&*delegate);
        unsafe { web_view.setNavigationDelegate(Some(delegate_object)) };

        self.container = Some(container);
        self.delegate = Some(delegate);
        self.state.borrow_mut().web_view = Some(web_view);
    }
}

impl WebViewSurface for AppleWebKitSurface {
    fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>) {
        self.state.borrow_mut().on_state_change = callback;
    }

    fn load(&mut self, request: &WebViewRequest) {
        if self.state.borrow().detached {
            return;
        }
        self.ensure_views();
        let web_view = self.state.borrow().web_view.clone();
        let Some(web_view) = web_view else { return };
        {
            let mut state = self.state.borrow_mut();
            state.state = WebViewState {
                phase: WebViewPhase::Loading,
                ..WebViewState::default()
            };
            match request {
                WebViewRequest::Url(url) => {
                    state.loads_html = false;
                    state.state.url = Some(url.clone());
                }
                WebViewRequest::Html { html: _, base_url } => {
                    state.loads_html = true;
                    state.state.url = base_url.clone();
                }
            }
        }
        match request {
            WebViewRequest::Url(url) => {
                if let Some(ns_url) =
                    NSURL::URLWithString(&objc2_foundation::NSString::from_str(url))
                {
                    let request = NSURLRequest::requestWithURL(&ns_url);
                    let _ = unsafe { web_view.loadRequest(&request) };
                }
            }
            WebViewRequest::Html { html, base_url } => {
                let body = objc2_foundation::NSString::from_str(html);
                let base = base_url.as_ref().and_then(|url| {
                    NSURL::URLWithString(&objc2_foundation::NSString::from_str(url))
                });
                let _ = unsafe { web_view.loadHTMLString_baseURL(&body, base.as_deref()) };
            }
        }
        self.state.borrow_mut().publish();
    }

    fn perform(&mut self, command: WebViewCommand) {
        let web_view = self.state.borrow().web_view.clone();
        let Some(web_view) = web_view else { return };
        match command {
            WebViewCommand::Back => {
                if unsafe { web_view.canGoBack() } {
                    let _ = unsafe { web_view.goBack() };
                }
            }
            WebViewCommand::Forward => {
                if unsafe { web_view.canGoForward() } {
                    let _ = unsafe { web_view.goForward() };
                }
            }
            WebViewCommand::Reload => {
                {
                    let mut state = self.state.borrow_mut();
                    state.state.phase = WebViewPhase::Loading;
                    state.state.error_description = None;
                }
                let _ = unsafe { web_view.reload() };
            }
            WebViewCommand::Stop => {
                unsafe { web_view.stopLoading() };
                self.state.borrow_mut().state.phase = WebViewPhase::Loaded;
            }
        }
        self.state.borrow_mut().publish();
    }

    fn update_geometry(&mut self, geometry: WebViewGeometry) {
        if self.state.borrow().detached {
            return;
        }
        // Attach lazily: a surface created before its window exists still works.
        if self.container.is_none() {
            if !geometry.is_visible {
                return;
            }
            self.ensure_views();
        }
        let (Some(container), Some(web_view)) =
            (self.container.clone(), self.state.borrow().web_view.clone())
        else {
            return;
        };
        let current_window = (self.window_provider)();
        let visible = geometry.visible_frame;
        let hidden = || container.setHidden(true);
        if !geometry.is_visible
            || geometry.frame.width <= 0.0
            || geometry.frame.height <= 0.0
            || visible.width <= 0.0
            || visible.height <= 0.0
        {
            hidden();
            return;
        }
        let Some(content_view) = current_window
            .as_ref()
            .and_then(|window| window.contentView())
        else {
            hidden();
            return;
        };
        // Re-parent when the window changed.
        if unsafe { container.superview() }.as_deref() != Some(&*content_view) {
            container.removeFromSuperview();
            content_view.addSubview(&container);
        }
        container.setFrame(container_rect(visible, &content_view));
        web_view.setFrame(inner_rect(geometry.frame, visible, &content_view));
        container.setHidden(false);
    }

    fn detach(&mut self) {
        {
            let mut state = self.state.borrow_mut();
            state.detached = true;
            if let Some(web_view) = state.web_view.take() {
                unsafe { web_view.stopLoading() };
                unsafe { web_view.setNavigationDelegate(None) };
                web_view.removeFromSuperview();
            }
            state.on_state_change = None;
        }
        if let Some(container) = self.container.take() {
            container.removeFromSuperview();
        }
        self.delegate = None;
    }
}
