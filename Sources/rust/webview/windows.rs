//! WebView2 host for Windows.
//!
//! The WebView2 runtime is a COM API. Rather than take a wrapper-crate
//! dependency (which would need a new workspace dependency), this module
//! declares the exact vtable slices it uses. Every interface is queried by the
//! documented IID, so the implementation fails loudly if the headers drift
//! rather than silently calling the wrong slot.
//!
//! Creation is genuinely asynchronous in WebView2: both
//! `CreateCoreWebView2EnvironmentWithOptions` and
//! `CreateCoreWebView2Controller` complete through a handler. This module
//! implements those handlers with hand-written vtables and stores the result
//! into shared state, which the next `update_geometry` picks up. Until then the
//! surface reports `Loading`, exactly like a slow page load.
#![cfg(target_os = "windows")]

use super::{WebViewCommand, WebViewGeometry, WebViewHost, WebViewRequest, WebViewState, WebViewSurface};
use crate::webview::WebViewPhase;
use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::rc::Rc;

type HRESULT = i32;
type HWND = isize;
type LPCWSTR = *const u16;
type PCWSTR = *const u16;

const S_OK: HRESULT = 0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const fn guid(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Guid {
    Guid { data1, data2, data3, data4 }
}

/// IID_ICoreWebView2Environment.
const IID_ENVIRONMENT: Guid = guid(0xB96D755E, 0x0319, 0x4E92, [0xA2, 0x96, 0x34, 0x49, 0xF2, 0x37, 0x4A, 0x2B]);
/// IID_ICoreWebView2Controller.
const IID_CONTROLLER: Guid = guid(0x4D00C0D1, 0x9434, 0x4EB6, [0x80, 0x78, 0x86, 0x97, 0xA5, 0x60, 0x33, 0x70]);
/// IID_ICoreWebView2.
const IID_WEBVIEW: Guid = guid(0x76ECEACB, 0x0462, 0x4D94, [0x9A, 0xC7, 0x43, 0x49, 0xA0, 0xCA, 0x0B, 0x31]);
/// IID_ICoreWebView2CreateCoreWebView2EnvironmentCompletedHandler.
const IID_ENVIRONMENT_HANDLER: Guid = guid(0x4E8A3389, 0xC9D8, 0x4BD2, [0xB6, 0xB5, 0x12, 0x4F, 0xEE, 0x6C, 0xC1, 0x4D]);
/// IID_ICoreWebView2CreateCoreWebView2ControllerCompletedHandler.
const IID_CONTROLLER_HANDLER: Guid = guid(0x6C4819F3, 0xC9B7, 0x4260, [0x81, 0x27, 0xC9, 0xF5, 0xBD, 0xE7, 0xF6, 0x8C]);

// Interface slot indices. All interfaces below derive from IUnknown, whose
// three methods occupy slots 0..=2.
const SLOT_QUERY_INTERFACE: usize = 0;
const SLOT_ADD_REF: usize = 1;
const SLOT_RELEASE: usize = 2;

const ENVIRONMENT_CREATE_CONTROLLER: usize = 3;
const ENVIRONMENT_RELEASE: usize = 2;

const CONTROLLER_PUT_BOUNDS: usize = 3;
const CONTROLLER_PUT_IS_VISIBLE: usize = 4;
const CONTROLLER_GET_WEBVIEW: usize = 8;
const CONTROLLER_CLOSE: usize = 11;

const WEBVIEW_NAVIGATE: usize = 5;
const WEBVIEW_NAVIGATE_TO_STRING: usize = 6;
const WEBVIEW_STOP: usize = 9;
const WEBVIEW_RELOAD: usize = 11;
const WEBVIEW_GET_SOURCE: usize = 12;
const WEBVIEW_GO_BACK: usize = 14;
const WEBVIEW_GO_FORWARD: usize = 16;
const WEBVIEW_GET_DOCUMENT_TITLE: usize = 20;
const WEBVIEW_GET_CAN_GO_BACK: usize = 21;
const WEBVIEW_GET_CAN_GO_FORWARD: usize = 22;

/// Read a vtable entry as an `extern "system"` function pointer.
///
/// # Safety
/// `this` must be a live COM interface pointer whose vtable has at least
/// `slot + 1` entries, and the entry must have the signature `F`.
unsafe fn com_slot<F>(this: *mut c_void, slot: usize) -> F {
    let entry = unsafe { (*(this as *mut *const usize)).add(slot) };
    unsafe { std::ptr::read(entry as *const F) }
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
}

unsafe extern "system" {
    fn LoadLibraryW(name: LPCWSTR) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    /// Copies a UTF-16 string into a COM task allocation WebView2 takes
    /// ownership of.
    fn CoTaskMemFree(pointer: *mut c_void);
    fn GetActiveWindow() -> HWND;
}

fn release(pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    let release: unsafe extern "system" fn(*mut c_void) -> u32 = unsafe { com_slot(pointer, SLOT_RELEASE) };
    unsafe { release(pointer) };
}

fn query_interface(pointer: *mut c_void, iid: &Guid, out: *mut *mut c_void) -> HRESULT {
    let query: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT =
        unsafe { com_slot(pointer, SLOT_QUERY_INTERFACE) };
    unsafe { query(pointer, iid, out) }
}

fn webview_string_to_string(pointer: *mut c_void) -> Option<String> {
    // ICoreWebView2 exposes `get_Source`/`get_DocumentTitle` as
    // `(this, LPWSTR *value)`. The returned buffer is CoTaskMem-owned.
    if pointer.is_null() {
        return None;
    }
    let raw = pointer as *const u16;
    let mut length = 0usize;
    unsafe {
        while *raw.add(length) != 0 {
            length += 1;
        }
    }
    if length == 0 {
        unsafe { CoTaskMemFree(pointer as *mut c_void) };
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(raw, length) };
    let value = String::from_utf16_lossy(slice);
    unsafe { CoTaskMemFree(pointer as *mut c_void) };
    Some(value)
}

// MARK: - Shared state

pub struct WindowsWebView2State {
    parent_provider: Box<dyn Fn() -> Option<HWND>>,
    user_data_folder: String,
    environment: *mut c_void,
    controller: *mut c_void,
    web_view: *mut c_void,
    state: WebViewState,
    on_state_change: Option<Box<dyn Fn(WebViewState)>>,
    loads_html: bool,
    pending_html: Option<String>,
    detached: bool,
    creation_failed: Option<String>,
}

impl WindowsWebView2State {
    fn publish(&mut self) {
        if !self.web_view.is_null() {
            self.state.can_go_back = self.read_can_go_back().unwrap_or(false);
            self.state.can_go_forward = self.read_can_go_forward().unwrap_or(false);
        }
        if let Some(callback) = &self.on_state_change {
            callback(self.state.clone());
        }
    }

    fn read_can_go_back(&self) -> Option<bool> {
        if self.web_view.is_null() {
            return None;
        }
        let get: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT =
            unsafe { com_slot(self.web_view, WEBVIEW_GET_CAN_GO_BACK) };
        let mut value = 0i32;
        (unsafe { get(self.web_view, &mut value) } == S_OK).then_some(value != 0)
    }

    fn read_can_go_forward(&self) -> Option<bool> {
        if self.web_view.is_null() {
            return None;
        }
        let get: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT =
            unsafe { com_slot(self.web_view, WEBVIEW_GET_CAN_GO_FORWARD) };
        let mut value = 0i32;
        (unsafe { get(self.web_view, &mut value) } == S_OK).then_some(value != 0)
    }

    fn read_url(&self) -> Option<String> {
        if self.web_view.is_null() {
            return None;
        }
        let get: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
            unsafe { com_slot(self.web_view, WEBVIEW_GET_SOURCE) };
        let mut value: *mut c_void = std::ptr::null_mut();
        if unsafe { get(self.web_view, &mut value) } != S_OK {
            return None;
        }
        webview_string_to_string(value)
    }

    fn read_title(&self) -> Option<String> {
        if self.web_view.is_null() {
            return None;
        }
        let get: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
            unsafe { com_slot(self.web_view, WEBVIEW_GET_DOCUMENT_TITLE) };
        let mut value: *mut c_void = std::ptr::null_mut();
        if unsafe { get(self.web_view, &mut value) } != S_OK {
            return None;
        }
        webview_string_to_string(value)
    }
}

// MARK: - Asynchronous COM handlers

#[repr(C)]
struct EnvironmentHandler {
    vtable: *const EnvironmentHandlerVtable,
    ref_count: Cell<u32>,
    state: Rc<RefCell<WindowsWebView2State>>,
}

#[repr(C)]
struct EnvironmentHandlerVtable {
    query_interface: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    invoke: unsafe extern "system" fn(*mut c_void, HRESULT, *mut c_void) -> HRESULT,
}

static ENVIRONMENT_VTABLE: EnvironmentHandlerVtable = EnvironmentHandlerVtable {
    query_interface: environment_query_interface,
    add_ref: environment_add_ref,
    release: environment_release,
    invoke: environment_invoke,
};

unsafe extern "system" fn environment_query_interface(this: *mut c_void, iid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    let handler = this as *mut EnvironmentHandler;
    if iid.is_null() || out.is_null() {
        return -2147467261; // E_POINTER
    }
    let requested = unsafe { *iid };
    if guid_matches(&requested, &IID_ENVIRONMENT_HANDLER) {
        unsafe { *out = this };
        unsafe { environment_add_ref(this) };
        return S_OK;
    }
    // IUnknown is the identity interface.
    if guid_matches(&requested, &guid(0x00000000, 0x0000, 0x0000, [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46])) {
        unsafe { *out = this };
        unsafe { environment_add_ref(this) };
        let _ = handler;
        return S_OK;
    }
    unsafe { *out = std::ptr::null_mut() };
    -2147467262 // E_NOINTERFACE
}

unsafe extern "system" fn environment_add_ref(this: *mut c_void) -> u32 {
    let handler = unsafe { &*(this as *const EnvironmentHandler) };
    let next = handler.ref_count.get().saturating_add(1);
    handler.ref_count.set(next);
    next
}

unsafe extern "system" fn environment_release(this: *mut c_void) -> u32 {
    let handler = unsafe { &*(this as *const EnvironmentHandler) };
    let next = handler.ref_count.get().saturating_sub(1);
    handler.ref_count.set(next);
    if next == 0 {
        // Reclaim the box allocated in `create_environment`.
        drop(unsafe { Box::from_raw(this as *mut EnvironmentHandler) });
    }
    next
}

unsafe extern "system" fn environment_invoke(this: *mut c_void, result: HRESULT, environment: *mut c_void) -> HRESULT {
    let handler = unsafe { &*(this as *const EnvironmentHandler) };
    if result != S_OK || environment.is_null() {
        let mut state = handler.state.borrow_mut();
        state.creation_failed = Some(format!("WebView2 environment creation failed (0x{result:08X})"));
        state.state.phase = WebViewPhase::Failed;
        state.state.error_description = state.creation_failed.clone();
        state.publish();
        return S_OK;
    }
    let state_handle = handler.state.clone();
    {
        let mut state = state_handle.borrow_mut();
        state.environment = environment;
    }
    // Continue into controller creation on the same UI thread. The borrow is
    // dropped first: `create_controller` re-enters the same `RefCell`.
    create_controller(&state_handle);
    S_OK
}

#[repr(C)]
struct ControllerHandler {
    vtable: *const ControllerHandlerVtable,
    ref_count: Cell<u32>,
    state: Rc<RefCell<WindowsWebView2State>>,
}

#[repr(C)]
struct ControllerHandlerVtable {
    query_interface: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    invoke: unsafe extern "system" fn(*mut c_void, HRESULT, *mut c_void) -> HRESULT,
}

static CONTROLLER_VTABLE: ControllerHandlerVtable = ControllerHandlerVtable {
    query_interface: controller_query_interface,
    add_ref: controller_add_ref,
    release: controller_release,
    invoke: controller_invoke,
};

unsafe extern "system" fn controller_query_interface(this: *mut c_void, iid: *const Guid, out: *mut *mut c_void) -> HRESULT {
    if iid.is_null() || out.is_null() {
        return -2147467261;
    }
    let requested = unsafe { *iid };
    if guid_matches(&requested, &IID_CONTROLLER_HANDLER)
        || guid_matches(&requested, &guid(0x00000000, 0x0000, 0x0000, [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46]))
    {
        unsafe { *out = this };
        unsafe { controller_add_ref(this) };
        return S_OK;
    }
    unsafe { *out = std::ptr::null_mut() };
    -2147467262
}

unsafe extern "system" fn controller_add_ref(this: *mut c_void) -> u32 {
    let handler = unsafe { &*(this as *const ControllerHandler) };
    let next = handler.ref_count.get().saturating_add(1);
    handler.ref_count.set(next);
    next
}

unsafe extern "system" fn controller_release(this: *mut c_void) -> u32 {
    let handler = unsafe { &*(this as *const ControllerHandler) };
    let next = handler.ref_count.get().saturating_sub(1);
    handler.ref_count.set(next);
    if next == 0 {
        drop(unsafe { Box::from_raw(this as *mut ControllerHandler) });
    }
    next
}

unsafe extern "system" fn controller_invoke(this: *mut c_void, result: HRESULT, controller: *mut c_void) -> HRESULT {
    let handler = unsafe { &*(this as *const ControllerHandler) };
    if result != S_OK || controller.is_null() {
        let mut state = handler.state.borrow_mut();
        state.creation_failed = Some(format!("WebView2 controller creation failed (0x{result:08X})"));
        state.state.phase = WebViewPhase::Failed;
        state.state.error_description = state.creation_failed.clone();
        state.publish();
        return S_OK;
    }
    let get_web_view: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
        unsafe { com_slot(controller, CONTROLLER_GET_WEBVIEW) };
    let mut web_view: *mut c_void = std::ptr::null_mut();
    if unsafe { get_web_view(controller, &mut web_view) } != S_OK || web_view.is_null() {
        let mut state = handler.state.borrow_mut();
        state.creation_failed = Some("WebView2 controller exposed no web view".into());
        state.state.phase = WebViewPhase::Failed;
        state.state.error_description = state.creation_failed.clone();
        state.publish();
        release(controller);
        return S_OK;
    }
    let mut state = handler.state.borrow_mut();
    // Attach our parent window now that the controller exists.
    if let Some(parent) = (state.parent_provider)() {
        let put_parent: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT =
            unsafe { com_slot(controller, 6) };
        unsafe { put_parent(controller, parent) };
    }
    state.controller = controller;
    state.web_view = web_view;
    state.state.phase = WebViewPhase::Loaded;
    state.state.error_description = None;
    // A load requested before the controller existed runs now.
    if let Some(html) = state.pending_html.take() {
        let body = wide(&html);
        let navigate: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT =
            unsafe { com_slot(web_view, WEBVIEW_NAVIGATE_TO_STRING) };
        unsafe { navigate(web_view, body.as_ptr()) };
    }
    state.publish();
    S_OK
}

fn guid_matches(left: &Guid, right: &Guid) -> bool {
    left.data1 == right.data1 && left.data2 == right.data2 && left.data3 == right.data3 && left.data4 == right.data4
}

/// `CreateCoreWebView2EnvironmentWithOptions` resolution.
type CreateEnvironmentFn = unsafe extern "system" fn(PCWSTR, PCWSTR, *mut c_void, *mut c_void, *mut c_void) -> HRESULT;

fn create_environment(state: &Rc<RefCell<WindowsWebView2State>>) {
    let loaded = unsafe {
        let name = wide("WebView2Loader.dll");
        let module = LoadLibraryW(name.as_ptr());
        if module.is_null() {
            None
        } else {
            let symbol = GetProcAddress(module, c"CreateCoreWebView2EnvironmentWithOptions".as_ptr());
            (!symbol.is_null()).then_some(std::mem::transmute::<*mut c_void, CreateEnvironmentFn>(symbol))
        }
    };
    let Some(create) = loaded else {
        let mut state = state.borrow_mut();
        state.creation_failed = Some(
            "WebView2Loader.dll is unavailable. Install the Evergreen WebView2 Runtime.".into(),
        );
        state.state.phase = WebViewPhase::Failed;
        state.state.error_description = state.creation_failed.clone();
        state.publish();
        return;
    };
    let folder = wide(&state.borrow().user_data_folder);
    // The environment handler owns a strong reference to the surface state; the
    // COM ref count governs its lifetime, so it is boxed and never leaked.
    let handler = Box::new(EnvironmentHandler {
        vtable: &ENVIRONMENT_VTABLE as *const _,
        ref_count: Cell::new(1),
        state: state.clone(),
    });
    let handler_pointer = Box::into_raw(handler) as *mut c_void;
    let result = unsafe {
        create(
            std::ptr::null(),
            folder.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            handler_pointer,
        )
    };
    if result != S_OK {
        // A failed call never invokes the handler, so release our reference.
        unsafe { environment_release(handler_pointer) };
        let mut state = state.borrow_mut();
        state.creation_failed = Some(format!("WebView2 environment request failed (0x{result:08X})"));
        state.state.phase = WebViewPhase::Failed;
        state.state.error_description = state.creation_failed.clone();
        state.publish();
    }
}

fn create_controller(state: &Rc<RefCell<WindowsWebView2State>>) {
    let (environment, parent) = {
        let state = state.borrow();
        (state.environment, (state.parent_provider)())
    };
    if environment.is_null() {
        return;
    }
    let Some(parent) = parent.or_else(|| Some(unsafe { GetActiveWindow() })) else {
        return;
    };
    let handler = Box::new(ControllerHandler {
        vtable: &CONTROLLER_VTABLE as *const _,
        ref_count: Cell::new(1),
        state: state.clone(),
    });
    let handler_pointer = Box::into_raw(handler) as *mut c_void;
    let create: unsafe extern "system" fn(*mut c_void, HWND, *mut c_void) -> HRESULT =
        unsafe { com_slot(environment, ENVIRONMENT_CREATE_CONTROLLER) };
    let result = unsafe { create(environment, parent, handler_pointer) };
    if result != S_OK {
        unsafe { controller_release(handler_pointer) };
        let mut state = state.borrow_mut();
        state.creation_failed = Some(format!("WebView2 controller request failed (0x{result:08X})"));
        state.state.phase = WebViewPhase::Failed;
        state.state.error_description = state.creation_failed.clone();
        state.publish();
    }
}

// MARK: - Public host and surface

pub struct WindowsWebView2Host {
    parent_provider: Rc<dyn Fn() -> Option<HWND>>,
    user_data_folder: String,
}

impl WindowsWebView2Host {
    /// `parent_provider` returns the HWND the web view parents itself to;
    /// `None` falls back to the active window.
    pub fn new(parent_provider: impl Fn() -> Option<HWND> + 'static, user_data_folder: impl Into<String>) -> Self {
        Self { parent_provider: Rc::new(parent_provider), user_data_folder: user_data_folder.into() }
    }
}

impl WebViewHost for WindowsWebView2Host {
    fn make_surface(&self) -> Box<dyn WebViewSurface> {
        let provider = self.parent_provider.clone();
        Box::new(WindowsWebView2Surface {
            state: Rc::new(RefCell::new(WindowsWebView2State {
                parent_provider: Box::new(move || provider()),
                user_data_folder: self.user_data_folder.clone(),
                environment: std::ptr::null_mut(),
                controller: std::ptr::null_mut(),
                web_view: std::ptr::null_mut(),
                state: WebViewState::default(),
                on_state_change: None,
                loads_html: false,
                pending_html: None,
                detached: false,
                creation_failed: None,
            })),
        })
    }
}

pub struct WindowsWebView2Surface {
    state: Rc<RefCell<WindowsWebView2State>>,
}

impl WebViewSurface for WindowsWebView2Surface {
    fn set_on_state_change(&mut self, callback: Option<Box<dyn Fn(WebViewState)>>) {
        self.state.borrow_mut().on_state_change = callback;
    }

    fn load(&mut self, request: &WebViewRequest) {
        let state_handle = self.state.clone();
        {
            let mut state = state_handle.borrow_mut();
            if state.detached {
                return;
            }
            state.state = WebViewState { phase: WebViewPhase::Loading, ..WebViewState::default() };
        }
        if state_handle.borrow().environment.is_null() {
            // `create_environment` re-enters this `RefCell`, so the borrow must
            // be released first.
            create_environment(&state_handle);
        }
        let mut state = state_handle.borrow_mut();
        if state.web_view.is_null() {
            // Creation is asynchronous; remember what to load once it finishes.
            match request {
                WebViewRequest::Html { html, base_url } => {
                    state.loads_html = true;
                    state.state.url = base_url.clone();
                    state.pending_html = Some(html.clone());
                }
                WebViewRequest::Url(url) => {
                    state.state.url = Some(url.clone());
                    state.pending_html = Some(String::new());
                }
            }
            state.publish();
            return;
        }
        apply_load(&mut state, request);
        state.publish();
    }

    fn perform(&mut self, command: WebViewCommand) {
        let mut state = self.state.borrow_mut();
        let web_view = state.web_view;
        if web_view.is_null() {
            return;
        }
        match command {
            WebViewCommand::Back => {
                if state.read_can_go_back().unwrap_or(false) {
                    let go: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                        unsafe { com_slot(web_view, WEBVIEW_GO_BACK) };
                    unsafe { go(web_view) };
                }
            }
            WebViewCommand::Forward => {
                if state.read_can_go_forward().unwrap_or(false) {
                    let go: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                        unsafe { com_slot(web_view, WEBVIEW_GO_FORWARD) };
                    unsafe { go(web_view) };
                }
            }
            WebViewCommand::Reload => {
                state.state.phase = WebViewPhase::Loading;
                state.state.error_description = None;
                let reload: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                    unsafe { com_slot(web_view, WEBVIEW_RELOAD) };
                unsafe { reload(web_view) };
            }
            WebViewCommand::Stop => {
                let stop: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                    unsafe { com_slot(web_view, WEBVIEW_STOP) };
                unsafe { stop(web_view) };
                state.state.phase = WebViewPhase::Loaded;
            }
        }
        state.publish();
    }

    fn update_geometry(&mut self, geometry: WebViewGeometry) {
        let state_handle = self.state.clone();
        if state_handle.borrow().detached {
            return;
        }
        let needs_environment = {
            let state = state_handle.borrow();
            state.environment.is_null() && geometry.is_visible
        };
        if needs_environment {
            create_environment(&state_handle);
        }
        let mut state = state_handle.borrow_mut();
        let controller = state.controller;
        if controller.is_null() {
            return;
        }
        let visible = geometry.visible_frame;
        let bounds = if geometry.is_visible && visible.width > 0.0 && visible.height > 0.0 {
            Rect {
                left: visible.x.round() as i32,
                top: visible.y.round() as i32,
                right: (visible.x + visible.width).round() as i32,
                bottom: (visible.y + visible.height).round() as i32,
            }
        } else {
            Rect { left: 0, top: 0, right: 0, bottom: 0 }
        };
        let put_bounds: unsafe extern "system" fn(*mut c_void, Rect) -> HRESULT =
            unsafe { com_slot(controller, CONTROLLER_PUT_BOUNDS) };
        unsafe { put_bounds(controller, bounds) };
        let put_visible: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT =
            unsafe { com_slot(controller, CONTROLLER_PUT_IS_VISIBLE) };
        unsafe { put_visible(controller, i32::from(geometry.is_visible)) };
        // Refresh navigation state while no navigation event is wired: this is
        // the only point the surface is guaranteed to run.
        state.state.can_go_back = state.read_can_go_back().unwrap_or(false);
        state.state.can_go_forward = state.read_can_go_forward().unwrap_or(false);
        if !state.loads_html {
            if let Some(url) = state.read_url() {
                state.state.url = Some(url);
            }
        }
        if let Some(title) = state.read_title()
            && !title.is_empty()
        {
            state.state.title = Some(title);
        }
    }

    fn detach(&mut self) {
        let mut state = self.state.borrow_mut();
        state.detached = true;
        if !state.controller.is_null() {
            let close: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                unsafe { com_slot(state.controller, CONTROLLER_CLOSE) };
            unsafe { close(state.controller) };
            release(state.controller);
            state.controller = std::ptr::null_mut();
        }
        if !state.web_view.is_null() {
            release(state.web_view);
            state.web_view = std::ptr::null_mut();
        }
        if !state.environment.is_null() {
            release(state.environment);
            state.environment = std::ptr::null_mut();
        }
        state.pending_html = None;
        state.on_state_change = None;
    }
}

fn apply_load(state: &mut WindowsWebView2State, request: &WebViewRequest) {
    let web_view = state.web_view;
    match request {
        WebViewRequest::Url(url) => {
            state.loads_html = false;
            state.state.url = Some(url.clone());
            let uri = wide(url);
            let navigate: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT =
                unsafe { com_slot(web_view, WEBVIEW_NAVIGATE) };
            unsafe { navigate(web_view, uri.as_ptr()) };
        }
        WebViewRequest::Html { html, base_url } => {
            state.loads_html = true;
            state.state.url = base_url.clone();
            let body = wide(html);
            let navigate: unsafe extern "system" fn(*mut c_void, PCWSTR) -> HRESULT =
                unsafe { com_slot(web_view, WEBVIEW_NAVIGATE_TO_STRING) };
            unsafe { navigate(web_view, body.as_ptr()) };
        }
    }
}
