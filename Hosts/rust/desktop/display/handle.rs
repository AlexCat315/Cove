//! Translation of Desktop/Sources/CoveUI/Display/Handle.swift.
//!
//! `AppDisplayHandle` is the public control surface application content uses to
//! drive its window: presentation overrides, window actions, native dialogs,
//! auxiliary windows and the display-request signal. Every callback is installed
//! by the host during `Application::start`; an uninstalled callback behaves like
//! the Swift `nil` optional (request goes nowhere / returns the documented
//! default).

use super::controls::{MainWindowControls, PresentationControls, WindowControls};
use cove_platform::WindowId;
use cove_runtime::PlatformFrameRateMode;
use cove_shell::{FileDialogFilter, WindowChromeHitTest};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// A request to open one auxiliary window.
#[derive(Clone)]
pub struct AppAuxiliaryWindowRequest {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub root_view: cove_composition::AnyView,
    pub rendering: crate::desktop::application::configuration::AppWindowRendering,
}

/// Coalescing display-request signal. A burst of requests collapses into one
/// wake of the event loop; `drain` reports whether a wake is owed.
#[derive(Default)]
pub struct DisplaySignal {
    pending: Cell<bool>,
}

impl DisplaySignal {
    pub fn request(&self) -> bool {
        if self.pending.get() {
            return false;
        }
        self.pending.set(true);
        true
    }

    pub fn drain(&self) -> bool {
        let was_pending = self.pending.get();
        self.pending.set(false);
        was_pending
    }
}

/// Host-installed callbacks behind the public display handle.
#[derive(Default)]
pub struct DisplayBindings {
    pub presentation: PresentationControls,
    pub main_window: MainWindowControls,
    pub windows: WindowControls,
    pub open_auxiliary_window: Option<Rc<dyn Fn(AppAuxiliaryWindowRequest) -> Option<WindowId>>>,
    pub close_auxiliary_window: Option<Rc<dyn Fn(WindowId)>>,
    pub auxiliary_window_is_open: Option<Rc<dyn Fn(WindowId) -> bool>>,
    pub install_native_menu_bar: Option<Rc<dyn Fn(&super::super::window::native_menu::NativeMenuBar)>>,
    pub set_close_interceptor: Option<Rc<dyn Fn(Option<Rc<dyn Fn(Option<WindowId>) -> bool>>)>>,
    pub quit: Option<Rc<dyn Fn()>>,
    pub main_window_id_query: Option<Rc<dyn Fn() -> Option<WindowId>>>,
    pub open_folder_dialog:
        Option<Rc<dyn Fn(Option<&str>, Box<dyn FnOnce(Option<String>)>)>>,
    pub open_file_dialog: Option<
        Rc<dyn Fn(Option<&str>, &[FileDialogFilter], bool, Box<dyn FnOnce(Vec<String>)>)>,
    >,
    pub memory_query: Option<Rc<dyn Fn() -> Option<BTreeMap<String, u64>>>>,
    pub trim_memory: Option<Rc<dyn Fn()>>,
}

/// Public control surface for the running application window.
///
/// Cloning shares the same bindings, matching how application content held the
/// Swift reference type.
#[derive(Clone)]
pub struct AppDisplayHandle {
    bindings: Rc<RefCell<DisplayBindings>>,
    signal: Rc<DisplaySignal>,
    wake_event_loop: Rc<dyn Fn()>,
}

impl Default for AppDisplayHandle {
    fn default() -> Self {
        Self::new(|| {})
    }
}

impl AppDisplayHandle {
    pub fn new(wake_event_loop: impl Fn() + 'static) -> Self {
        Self {
            bindings: Rc::new(RefCell::new(DisplayBindings::default())),
            signal: Rc::new(DisplaySignal::default()),
            wake_event_loop: Rc::new(wake_event_loop),
        }
    }

    /// Replaces the host bindings. Called once during `Application::start`.
    pub fn install(&self, bindings: DisplayBindings) {
        *self.bindings.borrow_mut() = bindings;
    }

    pub fn bindings(&self) -> std::cell::Ref<'_, DisplayBindings> {
        self.bindings.borrow()
    }

    // MARK: - Display signal

    /// Coalesced "draw the next frame" request; safe to call from any callback.
    pub fn request_display(&self) {
        if self.signal.request() {
            (self.wake_event_loop)();
        }
    }

    pub fn drain_display_request(&self) -> bool {
        self.signal.drain()
    }

    // MARK: - Memory

    pub fn memory_snapshot(&self) -> Option<BTreeMap<String, u64>> {
        self.bindings.borrow().memory_query.as_ref().and_then(|query| query())
    }

    pub fn trim_memory(&self) {
        if let Some(trim) = self.bindings.borrow().trim_memory.clone() {
            trim();
        }
    }

    // MARK: - Presentation

    pub fn set_target_frame_rate(&self, frames_per_second: Option<f64>) {
        if let Some(set) = self.bindings.borrow().presentation.set_runtime_target_frame_rate.clone() {
            set(frames_per_second);
        }
        self.request_display();
    }

    pub fn set_frame_rate_mode(&self, mode: PlatformFrameRateMode) {
        if let Some(set) = self.bindings.borrow().presentation.set_runtime_frame_rate_mode.clone() {
            set(mode);
        }
        self.request_display();
    }

    pub fn set_vsync_enabled(&self, enabled: bool) {
        if let Some(set) = self.bindings.borrow().presentation.set_runtime_vsync_enabled.clone() {
            set(enabled);
        }
        self.request_display();
    }

    pub fn current_display_refresh_rate(&self) -> Option<f64> {
        let query = self
            .bindings
            .borrow()
            .presentation
            .current_runtime_display_refresh_rate
            .clone()?;
        query()
    }

    // MARK: - Menus

    pub fn install_native_menu_bar(&self, menu_bar: &super::super::window::native_menu::NativeMenuBar) {
        if let Some(install) = self.bindings.borrow().install_native_menu_bar.clone() {
            install(menu_bar);
        }
    }

    // MARK: - Main window

    pub fn minimize_window(&self) {
        let action = self.bindings.borrow().main_window.minimize_main_window_action.clone();
        if let Some(action) = action {
            action();
        }
    }

    pub fn maximize_window(&self) {
        let action = self.bindings.borrow().main_window.maximize_main_window_action.clone();
        if let Some(action) = action {
            action();
        }
    }

    pub fn restore_window(&self) {
        let action = self.bindings.borrow().main_window.restore_main_window_action.clone();
        if let Some(action) = action {
            action();
        }
    }

    /// Hides the main window completely — not a minimize.
    ///
    /// A minimize leaves a Dock entry; hiding does not. Resident applications
    /// driven from a status-bar/tray icon should use this when the window is
    /// closed, otherwise the window lingers in the Dock.
    pub fn hide_window(&self) {
        let action = self.bindings.borrow().main_window.hide_main_window_action.clone();
        if let Some(action) = action {
            action();
        }
    }

    /// Re-shows a main window previously hidden by [`Self::hide_window`].
    pub fn show_window(&self) {
        let action = self.bindings.borrow().main_window.show_main_window_action.clone();
        if let Some(action) = action {
            action();
        }
    }

    pub fn toggle_maximize_window(&self) {
        if self.is_window_maximized() { self.restore_window() } else { self.maximize_window() }
    }

    pub fn toggle_maximize_window_for(&self, window_id: WindowId) {
        if self.is_window_maximized_for(window_id) {
            self.restore_window_for(window_id);
        } else {
            self.maximize_window_for(window_id);
        }
    }

    pub fn close_main_window(&self) {
        let action = self.bindings.borrow().main_window.close_main_window_action.clone();
        if let Some(action) = action {
            action();
        }
    }

    pub fn is_window_maximized(&self) -> bool {
        let query = self.bindings.borrow().main_window.main_window_maximized_query.clone();
        query.is_some_and(|query| query())
    }

    pub fn set_window_chrome_hit_test(&self, hit_test: Option<&WindowChromeHitTest>) {
        let action =
            self.bindings.borrow().main_window.set_main_window_chrome_hit_test_action.clone();
        if let Some(action) = action {
            action(hit_test);
        }
    }

    // MARK: - Any window

    pub fn minimize_window_for(&self, window_id: WindowId) {
        let action = self.bindings.borrow().windows.minimize_window_by_id_action.clone();
        if let Some(action) = action {
            action(window_id);
        }
    }

    pub fn maximize_window_for(&self, window_id: WindowId) {
        let action = self.bindings.borrow().windows.maximize_window_by_id_action.clone();
        if let Some(action) = action {
            action(window_id);
        }
    }

    pub fn restore_window_for(&self, window_id: WindowId) {
        let action = self.bindings.borrow().windows.restore_window_by_id_action.clone();
        if let Some(action) = action {
            action(window_id);
        }
    }

    pub fn is_window_maximized_for(&self, window_id: WindowId) -> bool {
        let query = self.bindings.borrow().windows.window_maximized_by_id_query.clone();
        query.is_some_and(|query| query(window_id))
    }

    pub fn show_window_system_menu(&self, window_id: WindowId, x: f32, y: f32) {
        let action = self.bindings.borrow().windows.show_window_system_menu_action.clone();
        if let Some(action) = action {
            action(window_id, x, y);
        }
    }

    // MARK: - Dialogs

    /// Presents the native "choose a folder" dialog. `accept` receives the
    /// chosen directory path, or `None` if the user cancelled (or no platform
    /// dialog is available).
    pub fn request_open_folder(
        &self,
        default_path: Option<&str>,
        accept: Box<dyn FnOnce(Option<String>)>,
    ) {
        let action = self.bindings.borrow().open_folder_dialog.clone();
        match action {
            Some(action) => action(default_path, accept),
            None => accept(None),
        }
    }

    /// Presents the native "choose file(s)" dialog; `accept` receives the chosen
    /// absolute paths (empty when cancelled or unavailable).
    pub fn request_open_file(
        &self,
        filters: &[FileDialogFilter],
        allows_multiple: bool,
        default_path: Option<&str>,
        accept: Box<dyn FnOnce(Vec<String>)>,
    ) {
        let action = self.bindings.borrow().open_file_dialog.clone();
        match action {
            Some(action) => action(default_path, filters, allows_multiple, accept),
            None => accept(Vec::new()),
        }
    }

    // MARK: - Auxiliary windows

    pub fn open_window(&self, request: AppAuxiliaryWindowRequest) -> Option<WindowId> {
        let open = self.bindings.borrow().open_auxiliary_window.clone();
        open.and_then(|open| open(request))
    }

    pub fn close_window(&self, window_id: WindowId) {
        let bindings = self.bindings.borrow();
        if let Some(close) = bindings.windows.close_window_by_id_action.clone() {
            drop(bindings);
            close(window_id);
            return;
        }
        if let Some(close) = bindings.close_auxiliary_window.clone() {
            drop(bindings);
            close(window_id);
        }
    }

    pub fn is_window_open(&self, window_id: WindowId) -> bool {
        let query = self.bindings.borrow().auxiliary_window_is_open.clone();
        query.is_some_and(|query| query(window_id))
    }

    // MARK: - Lifecycle

    /// Vetoes OS-initiated window closes / application quit (`None` = quit).
    /// Programmatic [`Self::close_window`] and [`Self::quit`] bypass the
    /// interceptor, so a confirmation flow can re-issue a close it vetoed.
    pub fn set_window_close_interceptor(
        &self,
        handler: Option<Rc<dyn Fn(Option<WindowId>) -> bool>>,
    ) {
        let action = self.bindings.borrow().set_close_interceptor.clone();
        if let Some(action) = action {
            action(handler);
        }
    }

    pub fn quit(&self) {
        let action = self.bindings.borrow().quit.clone();
        if let Some(action) = action {
            action();
        }
    }

    pub fn main_window_id(&self) -> Option<WindowId> {
        let query = self.bindings.borrow().main_window_id_query.clone();
        query.and_then(|query| query())
    }
}
