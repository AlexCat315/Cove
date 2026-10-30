//! WebKitGTK host for Linux.
//!
//! `WebKitWebView` is a `gtk_widget` subclass. This module calls the GTK3/GTK4
//! and WebKit2GTK C API directly through FFI rather than pulling in a new
//! workspace dependency. The WebView is placed in a fixed container that is
//! positioned to the visible rect, which is how GTK expresses the rectangle the
//! composed tree assigned.
//!
//! Library loading is explicit and ordered: GTK must be initialized before
//! WebKit, and both must happen on the UI thread. When either library is absent
//! the surface reports `Failed` with the library name instead of pretending to
//! work.
#![cfg(target_os = "linux")]

use super::{WebViewCommand, WebViewGeometry, WebViewHost, WebViewRequest, WebViewState, WebViewSurface};
use crate::webview::WebViewPhase;
use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::rc::Rc;

/// GTK widget/container handle (`GtkWidget *`).
type GtkWidget = *mut c_void;
/// `WebKitWebView *`.
type WebKitWebView = *mut c_void;
/// `GtkWindow *`, supplied by the application.
type GtkWindow = *mut c_void;
type GCallback = Option<unsafe extern "C" fn()>;

#[derive(Default)]
struct GtkApi {
    gtk_init_check: Option<unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int>,
    gtk_window_new: Option<unsafe extern "C" fn(c_int) -> GtkWidget>,
    gtk_window_set_child: Option<unsafe extern "C" fn(GtkWindow, GtkWidget)>,
    gtk_container_add: Option<unsafe extern "C" fn(GtkWidget, GtkWidget)>,
    gtk_widget_add_css_class: Option<unsafe extern "C" fn(GtkWidget, *const c_char)>,
    gtk_fixed_new: Option<unsafe extern "C" fn() -> GtkWidget>,
    gtk_fixed_put: Option<unsafe extern "C" fn(GtkWidget, GtkWidget, f64, f64)>,
    gtk_fixed_move: Option<unsafe extern "C" fn(GtkWidget, GtkWidget, f64, f64)>,
    gtk_widget_set_size_request: Option<unsafe extern "C" fn(GtkWidget, c_int, c_int)>,
    gtk_widget_set_visible: Option<unsafe extern "C" fn(GtkWidget, c_int)>,
    gtk_widget_get_width: Option<unsafe extern "C" fn(GtkWidget) -> c_int>,
    gtk_widget_destroy: Option<unsafe extern "C" fn(GtkWidget)>,
    gtk_widget_unparent: Option<unsafe extern "C" fn(GtkWidget)>,
}

#[derive(Default)]
struct WebKitApi {
    webkit_web_view_new: Option<unsafe extern "C" fn() -> WebKitWebView>,
    webkit_web_view_load_uri: Option<unsafe extern "C" fn(WebKitWebView, *const c_char)>,
    webkit_web_view_load_html: Option<unsafe extern "C" fn(WebKitWebView, *const c_char, *const c_char)>,
    webkit_web_view_reload: Option<unsafe extern "C" fn(WebKitWebView)>,
    webkit_web_view_stop_loading: Option<unsafe extern "C" fn(WebKitWebView)>,
    webkit_web_view_go_back: Option<unsafe extern "C" fn(WebKitWebView)>,
    webkit_web_view_go_forward: Option<unsafe extern "C" fn(WebKitWebView)>,
    webkit_web_view_can_go_back: Option<unsafe extern "C" fn(WebKitWebView) -> c_int>,
    webkit_web_view_can_go_forward: Option<unsafe extern "C" fn(WebKitWebView) -> c_int>,
    webkit_web_view_get_uri: Option<unsafe extern "C" fn(WebKitWebView) -> *const c_char>,
    webkit_web_view_get_title: Option<unsafe extern "C" fn(WebKitWebView) -> *const c_char>,
    g_signal_connect_data: Option<unsafe extern "C" fn(*mut c_void, *const c_char, GCallback, *mut c_void, *mut c_void, c_int) -> u64>,
}

#[derive(Default)]
struct Libraries {
    gtk: GtkApi,
    webkit: WebKitApi,
    gtk_major: u32,
    error: Option<String>,
}

thread_local! {
    static LIBS: RefCell<Option<Rc<Libraries>>> = const { RefCell::new(None) };
}

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flag: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

const RTLD_NOW: c_int = 2;

fn open(names: &[&str]) -> Option<*mut c_void> {
    for name in names {
        let Ok(c_name) = CString::new(*name) else { continue };
        let handle = unsafe { dlopen(c_name.as_ptr(), RTLD_NOW) };
        if !handle.is_null() {
            return Some(handle);
        }
    }
    None
}

unsafe fn resolve<T>(handle: *mut c_void, name: &str, target: &mut Option<T>) {
    let Ok(c_name) = CString::new(name) else { return };
    let symbol = unsafe { dlsym(handle, c_name.as_ptr()) };
    if !symbol.is_null() {
        *target = Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&symbol) });
    }
}

/// Load GTK and WebKit once per process. GTK3 and GTK4 expose different symbol
/// names for parenting a widget (`gtk_container_add` vs `gtk_window_set_child`),
/// so both are probed and the available pair decides the major version.
fn libraries() -> Rc<Libraries> {
    LIBS.with(|slot| {
        if let Some(libraries) = slot.borrow().as_ref() {
            return libraries.clone();
        }
        let mut libraries = Libraries::default();
        let Some(gtk_handle) = open(&["libgtk-4.so.1", "libgtk-4.so", "libgtk-3.so.0", "libgtk-3.so"]) else {
            libraries.error = Some("GTK is not installed (libgtk-4 or libgtk-3)".into());
            let libraries = Rc::new(libraries);
            *slot.borrow_mut() = Some(libraries.clone());
            return libraries;
        };
        let Some(webkit_handle) = open(&["libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.0.so.37", "libwebkit2gtk-4.0.so"]) else {
            libraries.error = Some("WebKitGTK is not installed (libwebkit2gtk-4.1 or 4.0)".into());
            let libraries = Rc::new(libraries);
            *slot.borrow_mut() = Some(libraries.clone());
            return libraries;
        };
        unsafe {
            let gtk = &mut libraries.gtk;
            resolve(gtk_handle, "gtk_init_check", &mut gtk.gtk_init_check);
            resolve(gtk_handle, "gtk_window_new", &mut gtk.gtk_window_new);
            resolve(gtk_handle, "gtk_window_set_child", &mut gtk.gtk_window_set_child);
            resolve(gtk_handle, "gtk_container_add", &mut gtk.gtk_container_add);
            resolve(gtk_handle, "gtk_widget_add_css_class", &mut gtk.gtk_widget_add_css_class);
            resolve(gtk_handle, "gtk_fixed_new", &mut gtk.gtk_fixed_new);
            resolve(gtk_handle, "gtk_fixed_put", &mut gtk.gtk_fixed_put);
            resolve(gtk_handle, "gtk_fixed_move", &mut gtk.gtk_fixed_move);
            resolve(gtk_handle, "gtk_widget_set_size_request", &mut gtk.gtk_widget_set_size_request);
            resolve(gtk_handle, "gtk_widget_set_visible", &mut gtk.gtk_widget_set_visible);
            resolve(gtk_handle, "gtk_widget_get_width", &mut gtk.gtk_widget_get_width);
            resolve(gtk_handle, "gtk_widget_destroy", &mut gtk.gtk_widget_destroy);
            resolve(gtk_handle, "gtk_widget_unparent", &mut gtk.gtk_widget_unparent);

            let webkit = &mut libraries.webkit;
            resolve(webkit_handle, "webkit_web_view_new", &mut webkit.webkit_web_view_new);
            resolve(webkit_handle, "webkit_web_view_load_uri", &mut webkit.webkit_web_view_load_uri);
            resolve(webkit_handle, "webkit_web_view_load_html", &mut webkit.webkit_web_view_load_html);
            resolve(webkit_handle, "webkit_web_view_reload", &mut webkit.webkit_web_view_reload);
            resolve(webkit_handle, "webkit_web_view_stop_loading", &mut webkit.webkit_web_view_stop_loading);
            resolve(webkit_handle, "webkit_web_view_go_back", &mut webkit.webkit_web_view_go_back);
            resolve(webkit_handle, "webkit_web_view_go_forward", &mut webkit.webkit_web_view_go_forward);
            resolve(webkit_handle, "webkit_web_view_can_go_back", &mut webkit.webkit_web_view_can_go_back);
            resolve(webkit_handle, "webkit_web_view_can_go_forward", &mut webkit.webkit_web_view_can_go_forward);
            resolve(webkit_handle, "webkit_web_view_get_uri", &mut webkit.webkit_web_view_get_uri);
            resolve(webkit_handle, "webkit_web_view_get_title", &mut webkit.webkit_web_view_get_title);
            // `g_signal_connect_data` lives in GObject, which GTK depends on.
            let gobject_handle = open(&["libgobject-2.0.so.0"]).unwrap_or(gtk_handle);
            resolve(gobject_handle, "g_signal_connect_data", &mut webkit.g_signal_connect_data);
        }
        libraries.gtk_major = if libraries.gtk.gtk_window_set_child.is_some() { 4 } else { 3 };
        if libraries.gtk.gtk_init_check.is_some() {
            // GTK parses argv; this surface passes none and tolerates a failure
            // because a display may legitimately be absent in a headless test.
            unsafe {
                if let Some(init) = libraries.gtk.gtk_init_check {
                    init(std::ptr::null_mut(), std::ptr::null_mut());
                }
            }
        }
        let libraries = Rc::new(libraries);
        *slot.borrow_mut() = Some(libraries.clone());
        libraries
    })
}

pub struct LinuxWebKitHost {
    window_provider: Rc<dyn Fn() -> Option<GtkWindow>>,
}

impl LinuxWebKitHost {
    /// `window_provider` returns the `GtkWindow*` the web view should follow.
    pub fn new(window_provider: impl Fn() -> Option<GtkWindow> + 'static) -> Self {
        Self { window_provider: Rc::new(window_provider) }
    }
}

impl WebViewHost for LinuxWebKitHost {
    fn make_surface(&self) -> Box<dyn WebViewSurface> {
        let provider = self.window_provider.clone();
        Box::new(LinuxWebKitSurface {
            state: Rc::new(RefCell::new(LinuxWebKitState {
                window_provider: Box::new(move || provider()),
                container: std::ptr::null_mut(),
                web_view: std::ptr::null_mut(),
                state: WebViewState::default(),
                on_state_change: None,
                loads_html: false,
                detached: false,
            })),
        })
    }
}

pub struct LinuxWebKitState {
    window_provider: Box<dyn Fn() -> Option<GtkWindow>>,
    container: GtkWidget,
    web_view: WebKitWebView,
    state: WebViewState,
    on_state_change: Option<Box<dyn Fn(WebViewState)>>,
    loads_html: bool,
    detached: bool,
}

pub struct LinuxWebKitSurface {
    state: Rc<RefCell<LinuxWebKitState>>,
}

impl WebViewSurface for LinuxWebKitSurface {
    fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>) {
        self.state.borrow_mut().on_state_change = callback;
    }

    fn load(&mut self, request: &WebViewRequest) {
        let mut state = self.state.borrow_mut();
        if state.detached {
            return;
        }
        let libraries = libraries();
        if let Some(error) = &libraries.error {
            state.state.phase = WebViewPhase::Failed;
            state.state.error_description = Some(error.clone());
            state.publish();
            return;
        }
        if state.web_view.is_null() {
            ensure_views(&mut state, &libraries);
        }
        if state.web_view.is_null() {
            state.state.phase = WebViewPhase::Failed;
            state.state.error_description = Some("WebKitGTK could not create a web view".into());
            state.publish();
            return;
        }
        state.state = WebViewState { phase: WebViewPhase::Loading, ..WebViewState::default() };
        match request {
            WebViewRequest::Url(url) => {
                state.loads_html = false;
                state.state.url = Some(url.clone());
                if let (Some(load), Ok(c_url)) = (libraries.webkit.webkit_web_view_load_uri, CString::new(url.as_str())) {
                    unsafe { load(state.web_view, c_url.as_ptr()) };
                }
            }
            WebViewRequest::Html { html, base_url } => {
                state.loads_html = true;
                state.state.url = base_url.clone();
                if let Some(load) = libraries.webkit.webkit_web_view_load_html {
                    let body = CString::new(html.as_str()).unwrap_or_default();
                    let base = base_url
                        .as_ref()
                        .and_then(|url| CString::new(url.as_str()).ok());
                    unsafe {
                        load(
                            state.web_view,
                            body.as_ptr(),
                            base.as_ref().map_or(std::ptr::null(), |value| value.as_ptr()),
                        )
                    };
                }
            }
        }
        state.publish();
    }

    fn perform(&mut self, command: WebViewCommand) {
        let mut state = self.state.borrow_mut();
        if state.web_view.is_null() {
            return;
        }
        let libraries = libraries();
        let webkit = &libraries.webkit;
        match command {
            WebViewCommand::Back => {
                if let Some(can) = webkit.webkit_web_view_can_go_back
                    && unsafe { can(state.web_view) } != 0
                    && let Some(go) = webkit.webkit_web_view_go_back
                {
                    unsafe { go(state.web_view) };
                }
            }
            WebViewCommand::Forward => {
                if let Some(can) = webkit.webkit_web_view_can_go_forward
                    && unsafe { can(state.web_view) } != 0
                    && let Some(go) = webkit.webkit_web_view_go_forward
                {
                    unsafe { go(state.web_view) };
                }
            }
            WebViewCommand::Reload => {
                state.state.phase = WebViewPhase::Loading;
                state.state.error_description = None;
                if let Some(reload) = webkit.webkit_web_view_reload {
                    unsafe { reload(state.web_view) };
                }
            }
            WebViewCommand::Stop => {
                if let Some(stop) = webkit.webkit_web_view_stop_loading {
                    unsafe { stop(state.web_view) };
                }
                state.state.phase = WebViewPhase::Loaded;
            }
        }
        state.publish();
    }

    fn update_geometry(&mut self, geometry: WebViewGeometry) {
        let mut state = self.state.borrow_mut();
        if state.detached {
            return;
        }
        let libraries = libraries();
        if libraries.error.is_some() {
            return;
        }
        if state.container.is_null() {
            if !geometry.is_visible {
                return;
            }
            ensure_views(&mut state, &libraries);
        }
        if state.container.is_null() {
            return;
        }
        // GTK positions children in container coordinates; `gtk_fixed` is what
        // turns the composed absolute frame into a child allocation.
        if let Some(put) = libraries.gtk.gtk_fixed_put
            && state.web_view != std::ptr::null_mut()
        {
            let visible = geometry.visible_frame;
            let x = visible.x as f64;
            let y = visible.y as f64;
            unsafe { put(state.container, state.web_view, x, y) };
            if let Some(request_size) = libraries.gtk.gtk_widget_set_size_request {
                unsafe {
                    request_size(
                        state.web_view,
                        visible.width.round() as c_int,
                        visible.height.round() as c_int,
                    )
                };
            }
        }
        if let Some(set_visible) = libraries.gtk.gtk_widget_set_visible {
            unsafe { set_visible(state.container, c_int::from(geometry.is_visible)) };
        }
        state.refresh(&libraries);
    }

    fn detach(&mut self) {
        let mut state = self.state.borrow_mut();
        state.detached = true;
        let libraries = libraries();
        if let Some(stop) = libraries.webkit.webkit_web_view_stop_loading
            && state.web_view != std::ptr::null_mut()
        {
            unsafe { stop(state.web_view) };
        }
        if state.web_view != std::ptr::null_mut() {
            if libraries.gtk_major >= 4 {
                if let Some(unparent) = libraries.gtk.gtk_widget_unparent {
                    unsafe { unparent(state.web_view) };
                }
            }
            state.web_view = std::ptr::null_mut();
        }
        if state.container != std::ptr::null_mut() {
            if libraries.gtk_major >= 4 {
                if let Some(unparent) = libraries.gtk.gtk_widget_unparent {
                    unsafe { unparent(state.container) };
                }
            } else if let Some(destroy) = libraries.gtk.gtk_widget_destroy {
                unsafe { destroy(state.container) };
            }
            state.container = std::ptr::null_mut();
        }
        state.on_state_change = None;
    }
}

fn ensure_views(state: &mut LinuxWebKitState, libraries: &Libraries) {
    if state.web_view != std::ptr::null_mut() {
        return;
    }
    let Some(fixed_new) = libraries.gtk.gtk_fixed_new else { return };
    let Some(web_view_new) = libraries.webkit.webkit_web_view_new else { return };
    let container = unsafe { fixed_new() };
    let web_view = unsafe { web_view_new() };
    if container.is_null() || web_view.is_null() {
        return;
    }
    let Some(window) = (state.window_provider)() else {
        return;
    };
    // GTK3 parents through the container; GTK4 through the window itself.
    if libraries.gtk_major >= 4 {
        if let Some(set_child) = libraries.gtk.gtk_window_set_child {
            unsafe { set_child(window, container) };
        }
    } else if let Some(add) = libraries.gtk.gtk_container_add {
        unsafe { add(window, container) };
    }
    if let Some(put) = libraries.gtk.gtk_fixed_put {
        unsafe { put(container, web_view, 0.0, 0.0) };
    }
    state.container = container;
    state.web_view = web_view;
}

impl LinuxWebKitState {
    fn publish(&mut self) {
        if let Some(callback) = &self.on_state_change {
            callback(self.state.clone());
        }
    }

    /// WebKitGTK reports navigation through signals. The surface refreshes the
    /// observable fields here because GTK signals are delivered on the same
    /// main loop iteration that calls into this surface.
    fn refresh(&mut self, libraries: &Libraries) {
        if self.web_view.is_null() {
            return;
        }
        let webkit = &libraries.webkit;
        if let Some(can) = webkit.webkit_web_view_can_go_back {
            self.state.can_go_back = unsafe { can(self.web_view) } != 0;
        }
        if let Some(can) = webkit.webkit_web_view_can_go_forward {
            self.state.can_go_forward = unsafe { can(self.web_view) } != 0;
        }
        if !self.loads_html
            && let Some(get_uri) = webkit.webkit_web_view_get_uri
        {
            let uri = unsafe { get_uri(self.web_view) };
            if !uri.is_null() {
                let value = unsafe { CStr::from_ptr(uri) }.to_string_lossy().to_string();
                if !value.is_empty() {
                    self.state.url = Some(value);
                }
            }
        }
        if let Some(get_title) = webkit.webkit_web_view_get_title {
            let title = unsafe { get_title(self.web_view) };
            if !title.is_null() {
                let value = unsafe { CStr::from_ptr(title) }.to_string_lossy().to_string();
                if !value.is_empty() {
                    self.state.title = Some(value);
                }
            }
        }
    }
}
