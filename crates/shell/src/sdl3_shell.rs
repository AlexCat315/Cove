use crate::{DisplayRefreshTick, native, platform_shell::*};
use cove_platform::*;
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_void},
    rc::Rc,
    sync::{
        Arc,
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};
thread_local! { static SDL_LEASED: Cell<bool> = const { Cell::new(false) }; }
struct Session;
impl Session {
    fn acquire() -> Result<Rc<Self>, ShellError> {
        if SDL_LEASED.with(|slot| slot.replace(true)) {
            return Err(ShellError::InitializationFailed(
                "only one SDL shell can own the event queue on an application thread".into(),
            ));
        }
        // SAFETY: called on the application thread before any windows exist; the bridge checks SDL main-thread affinity.
        if !unsafe { native::cove_shell_init() } {
            SDL_LEASED.with(|slot| slot.set(false));
            return Err(native::error());
        }
        Ok(Rc::new(Self))
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: Rc confines the final lease to its application thread, after every retained window is closed.
        unsafe { native::cove_shell_quit() };
        SDL_LEASED.with(|slot| slot.set(false));
    }
}
pub struct Sdl3WindowHandle {
    id: WindowId,
    pointer: Cell<*mut native::Window>,
    _session: Rc<Session>,
    last_area: Cell<Option<TextInputArea>>,
    /// Refreshed once per event poll and after every command, so reads never
    /// reach the platform.
    state: Cell<native::WindowState>,
    backdrop: Cell<WindowBackdrop>,
}
impl Sdl3WindowHandle {
    fn state(&self) -> native::WindowState {
        self.state.get()
    }
    fn refresh(&self) {
        let mut state = native::WindowState::default();
        let pointer = self.pointer.get();
        if !pointer.is_null() {
            // SAFETY: live handle and exclusively borrowed output are valid; Rc confines all access to this thread.
            unsafe { native::cove_shell_state(pointer, &mut state) };
        }
        self.state.set(state);
    }
    fn close(&self) {
        let pointer = self.pointer.replace(std::ptr::null_mut());
        if !pointer.is_null() {
            // SAFETY: take-and-null prevents double destruction even if clients retain the Rust handle.
            unsafe { native::cove_shell_destroy(pointer) };
        }
        self.state.set(native::WindowState::default());
    }
    fn command(&self, command: i32, a: i32, b: i32) {
        // SAFETY: bridge accepts a closed/null window and calls SDL only for a live pointer.
        unsafe { native::cove_shell_command(self.pointer.get(), command, a, b) };
        self.refresh();
    }
}
impl Drop for Sdl3WindowHandle {
    fn drop(&mut self) {
        self.close();
    }
}
impl WindowHandle for Sdl3WindowHandle {
    fn id(&self) -> WindowId {
        self.id
    }
    fn native_window_reference(&self) -> Option<NativeWindowReference> {
        let state = self.state();
        if state.native_window == 0 {
            return None;
        }
        #[cfg(target_os = "macos")]
        {
            Some(NativeWindowReference::AppKit(state.native_window))
        }
        #[cfg(target_os = "ios")]
        {
            Some(NativeWindowReference::UIKit(state.native_window))
        }
        #[cfg(target_os = "windows")]
        {
            Some(NativeWindowReference::Win32(state.native_window))
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "windows")))]
        {
            None
        }
    }
    fn render_surface(&self) -> Option<NativeRenderSurface> {
        let s = self.state();
        match s.surface_kind {
            1 if s.surface != 0 => Some(NativeRenderSurface::MetalLayer(s.surface)),
            2 if s.surface != 0 => Some(NativeRenderSurface::Win32Window {
                hwnd: s.surface,
                hinstance: s.instance,
            }),
            3 => Some(NativeRenderSurface::XlibWindow {
                display: s.display,
                window: s.native_number,
            }),
            4 => Some(NativeRenderSurface::WaylandSurface {
                display: s.display,
                surface: s.surface,
            }),
            5 if s.surface != 0 => Some(NativeRenderSurface::AndroidWindow(s.surface)),
            _ => None,
        }
    }
    fn backdrop(&self) -> WindowBackdrop {
        self.backdrop.get()
    }
    fn safe_area(&self) -> WindowSafeArea {
        let s = self.state();
        WindowSafeArea {
            top: s.top,
            bottom: s.bottom,
            leading: s.leading,
            trailing: s.trailing,
        }
    }
    fn drawable_size(&self) -> (u32, u32) {
        let s = self.state();
        (s.pixel_width.max(1) as u32, s.pixel_height.max(1) as u32)
    }
    fn logical_size(&self) -> (u32, u32) {
        let s = self.state();
        let scale = s.scale.max(1.0);
        (
            (s.pixel_width as f32 / scale).round().max(1.0) as u32,
            (s.pixel_height as f32 / scale).round().max(1.0) as u32,
        )
    }
    fn content_scale_factor(&self) -> f32 {
        let scale = self.state().scale;
        if scale.is_finite() && scale > 0.0 {
            (scale * 4.0).round().max(4.0) / 4.0
        } else {
            1.0
        }
    }
    fn is_focused(&self) -> bool {
        self.state().focused
    }
    fn is_visible(&self) -> bool {
        self.state().visible
    }
    fn is_minimized(&self) -> bool {
        self.state().minimized
    }
    fn is_occluded(&self) -> bool {
        self.state().occluded
    }
}
struct DialogResult {
    id: u64,
    files: Vec<String>,
}
struct DialogNativeContext {
    id: u64,
    sender: Sender<DialogResult>,
}
struct Dialogs {
    sender: Sender<DialogResult>,
    receiver: Receiver<DialogResult>,
    next: u64,
    callbacks: HashMap<u64, DialogCompletion>,
}
pub struct Sdl3Shell {
    session: Option<Rc<Session>>,
    windows: HashMap<WindowId, Rc<Sdl3WindowHandle>>,
    order: Vec<WindowId>,
    quitting: bool,
    pub close_interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>,
    dialogs: RefCell<Dialogs>,
    sampler: crate::color_sampler::ScreenColorSampler,
    default_options: WindowOptions,
}
impl Sdl3Shell {
    pub fn new() -> Result<Self, ShellError> {
        crate::diagnostics::install_sdl_logging();
        let (sender, receiver) = mpsc::channel();
        Ok(Self {
            session: None,
            windows: HashMap::new(),
            order: Vec::new(),
            quitting: false,
            close_interceptor: None,
            dialogs: RefCell::new(Dialogs {
                sender,
                receiver,
                next: 1,
                callbacks: HashMap::new(),
            }),
            sampler: crate::color_sampler::ScreenColorSampler::new(Arc::new(Self::wake)),
            default_options: WindowOptions::default(),
        })
    }
    pub fn with_size(width: i32, height: i32) -> Result<Self, ShellError> {
        let mut shell = Self::new()?;
        shell.default_options.width = width;
        shell.default_options.height = height;
        Ok(shell)
    }
    pub fn wake() {
        // SAFETY: bridge wake is thread-safe, coalesced, and ignores an uninitialized event loop.
        unsafe { native::cove_shell_wake() };
    }
    fn command(&self, id: WindowId, command: i32, a: i32, b: i32) {
        if let Some(window) = self.windows.get(&id) {
            window.command(command, a, b);
        }
    }
    fn parent(&self, id: Option<WindowId>) -> *mut native::Window {
        id.or_else(|| self.main_window_id())
            .and_then(|id| self.windows.get(&id))
            .map_or(std::ptr::null_mut(), |w| w.pointer.get())
    }
    fn dialog(
        &self,
        id: Option<WindowId>,
        folder: bool,
        path: Option<&str>,
        filters: &[FileDialogFilter],
        multiple: bool,
        accept: DialogCompletion,
    ) {
        let path = path.and_then(|s| CString::new(s).ok());
        let mut names = Vec::new();
        let mut patterns = Vec::new();
        for filter in filters {
            if let (Ok(name), Ok(pattern)) = (
                CString::new(filter.name.as_str()),
                CString::new(filter.pattern()),
            ) {
                names.push(name);
                patterns.push(pattern);
            }
        }
        let name_ptrs: Vec<_> = names.iter().map(|s| s.as_ptr()).collect();
        let pattern_ptrs: Vec<_> = patterns.iter().map(|s| s.as_ptr()).collect();
        let context = {
            let mut dialogs = self.dialogs.borrow_mut();
            let id = dialogs.next;
            dialogs.next = dialogs.next.wrapping_add(1);
            dialogs.callbacks.insert(id, accept);
            Box::new(DialogNativeContext {
                id,
                sender: dialogs.sender.clone(),
            })
        };
        unsafe extern "C" fn done(context: *mut c_void, files: *const *const c_char) {
            // SAFETY: native dialog calls exactly once with this Box transferred from dialog().
            let context = unsafe { Box::from_raw(context.cast::<DialogNativeContext>()) };
            let mut chosen = Vec::new();
            if !files.is_null() {
                let mut index = 0;
                loop {
                    // SAFETY: SDL provides a NULL terminated list for the duration of the callback.
                    let entry = unsafe { *files.add(index) };
                    if entry.is_null() {
                        break;
                    } // SAFETY: each list entry is a NUL terminated filename.
                    chosen.push(
                        unsafe { CStr::from_ptr(entry) }
                            .to_string_lossy()
                            .into_owned(),
                    );
                    index += 1;
                }
            }
            let _ = context.sender.send(DialogResult {
                id: context.id,
                files: chosen,
            });
            Sdl3Shell::wake();
        }
        // SAFETY: the bridge copies all filter/path strings synchronously, and releases context via done exactly once.
        unsafe {
            native::cove_shell_dialog(
                self.parent(id),
                folder,
                path.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                name_ptrs.as_ptr(),
                pattern_ptrs.as_ptr(),
                names.len(),
                multiple,
                done,
                Box::into_raw(context).cast(),
            )
        };
    }
    fn complete_dialogs(&self) -> Vec<Box<dyn FnOnce()>> {
        let mut work: Vec<Box<dyn FnOnce()>> = Vec::new();
        loop {
            let next = {
                let mut dialogs = self.dialogs.borrow_mut();
                match dialogs.receiver.try_recv() {
                    Ok(result) => dialogs
                        .callbacks
                        .remove(&result.id)
                        .map(|callback| (callback, result.files)),
                    Err(_) => None,
                }
            };
            if let Some((callback, files)) = next {
                work.push(Box::new(move || callback(files)));
            } else {
                break;
            }
        }
        work
    }
}
impl Shell for Sdl3Shell {
    fn main_window_id(&self) -> Option<WindowId> {
        self.order.first().copied()
    }
    fn window_ids(&self) -> Vec<WindowId> {
        self.order.clone()
    }
    fn is_running(&self) -> bool {
        !self.quitting && !self.windows.is_empty()
    }
    fn set_close_interceptor(
        &mut self,
        interceptor: Option<Box<dyn FnMut(Option<WindowId>) -> bool>>,
    ) {
        self.close_interceptor = interceptor;
    }
    fn create_window(
        &mut self,
        title: &str,
        options: WindowOptions,
    ) -> Result<Rc<dyn WindowHandle>, ShellError> {
        if options.width <= 0 || options.height <= 0 {
            return Err(ShellError::InitializationFailed(
                "invalid window dimensions".into(),
            ));
        }
        if self.session.is_none() {
            self.session = Some(Session::acquire()?);
        }
        let title = CString::new(title)
            .map_err(|_| ShellError::InitializationFailed("window title contains NUL".into()))?;
        // SAFETY: valid C title and dimensions, session owns the SDL main-thread event queue.
        let pointer = unsafe {
            native::cove_shell_create(
                title.as_ptr(),
                options.width,
                options.height,
                options.title_bar_style == WindowTitleBarStyle::HiddenInset,
                options.shows_dock_icon,
            )
        };
        if pointer.is_null() {
            return Err(native::error());
        }
        let mut state = native::WindowState::default(); // SAFETY: pointer is live and state is writable.
        unsafe { native::cove_shell_state(pointer, &mut state) };
        if state.id == 0 {
            // SAFETY: failed handle has not escaped to any owner.
            unsafe { native::cove_shell_destroy(pointer) };
            return Err(native::error());
        }
        let id = WindowId(state.id);
        let window = Rc::new(Sdl3WindowHandle {
            id,
            pointer: Cell::new(pointer),
            _session: self.session.as_ref().unwrap().clone(),
            last_area: Cell::new(None),
            state: Cell::new(state),
            backdrop: Cell::new(WindowBackdrop::Opaque),
        });
        let applied = match options.backdrop {
            WindowBackdrop::Opaque => false,
            // SAFETY: the window pointer is live and owned by `window`.
            WindowBackdrop::Sidebar => unsafe { native::cove_shell_backdrop(pointer, 1) },
        };
        if applied {
            window.backdrop.set(options.backdrop);
            // The backdrop moves content under the title bar; the first frame
            // must already see the new size and top inset.
            let mut state = native::WindowState::default();
            // SAFETY: pointer is live and state is writable.
            if unsafe { native::cove_shell_state(pointer, &mut state) } {
                window.state.set(state);
            }
        }
        self.windows.insert(id, window.clone());
        self.order.retain(|entry| *entry != id);
        self.order.push(id);
        self.quitting = false;
        Ok(window)
    }
    fn window(&self, id: WindowId) -> Option<Rc<dyn WindowHandle>> {
        self.windows
            .get(&id)
            .cloned()
            .map(|w| w as Rc<dyn WindowHandle>)
    }
    fn destroy_window(&mut self, id: WindowId) {
        if let Some(window) = self.windows.remove(&id) {
            window.close();
        }
        self.order.retain(|entry| *entry != id);
    }
    fn initialize_window(&mut self, title: &str) -> Result<(), ShellError> {
        if self.main_window_id().is_none() {
            self.create_window(title, self.default_options.clone())?;
        }
        Ok(())
    }
    fn poll_window_events(&mut self) -> Vec<WindowInputEvent> {
        if self.session.is_none() {
            return Vec::new();
        }
        let mut collected = Vec::new();
        loop {
            let mut event = native::Event::default(); // SAFETY: event owns any text copied by the bridge and releases it on Drop.
            if !unsafe { native::cove_shell_poll(&mut event) } {
                break;
            }
            let mut id = WindowId(event.window);
            match event.kind {
                1 => {
                    if self.close_interceptor.as_mut().is_none_or(|f| f(None)) {
                        self.quitting = true;
                    }
                    continue;
                }
                2 => {
                    if self.close_interceptor.as_mut().is_none_or(|f| f(Some(id))) {
                        self.destroy_window(id);
                    }
                    continue;
                }
                3 => {
                    self.destroy_window(id);
                    continue;
                }
                20..=24 => {
                    id = self.main_window_id().unwrap_or(WindowId::MAIN);
                }
                // A drop may name no window at all: iOS reports dropped
                // documents against a null window, and a drag that began outside
                // the application does not always carry a target. Route those to
                // the main window instead of discarding the drag.
                25..=29 => {
                    if !self.windows.contains_key(&id) {
                        match self.main_window_id() {
                            Some(main) => id = main,
                            None => continue,
                        }
                    }
                }
                _ => {
                    if !self.windows.contains_key(&id) {
                        continue;
                    }
                }
            }
            if let Some(input) = decode_event(&event) {
                collected.push(WindowInputEvent {
                    window_id: id,
                    event: input,
                });
            }
        }
        for window in self.windows.values() {
            window.refresh();
        }
        collected
    }
    fn take_deferred_callbacks(&self) -> Vec<Box<dyn FnOnce()>> {
        let mut work = self.complete_dialogs();
        work.extend(self.sampler.take_deferred_callbacks());
        work
    }
    fn wait_for_events(&self, timeout: Duration) -> bool {
        if self.session.is_none() {
            return false;
        } // SAFETY: SDL owns the queue; wait preserves the event for the next poll.
        unsafe { native::cove_shell_wait(timeout.as_nanos().min(u64::MAX as u128) as u64) }
    }
    fn wake_event_loop(&self) {
        Self::wake();
    }
    fn set_text_input_area(&self, id: WindowId, area: Option<TextInputArea>) {
        if let Some(w) = self.windows.get(&id) {
            if w.last_area.get() == area {
                return;
            }
            let values = area.map(|a| [a.x, a.y, a.width, a.height, a.cursor_x]); // SAFETY: five-element area lives through the synchronous call; null disables input.
            unsafe {
                native::cove_shell_text_area(
                    w.pointer.get(),
                    values.as_ref().map_or(std::ptr::null(), |a| a.as_ptr()),
                )
            };
            w.last_area.set(area);
        }
    }
    fn set_cursor(&self, id: WindowId, cursor: SystemCursor) {
        if let Some(w) = self.windows.get(&id) {
            // SAFETY: window pointer is owned and the bridge validates the cursor ID.
            unsafe { native::cove_shell_cursor(w.pointer.get(), crate::cursor::native_id(cursor)) };
        }
    }
    fn global_pointer_position(&self) -> Option<(f32, f32)> {
        let (mut x, mut y) = (0.0, 0.0); // SAFETY: outputs are exclusively writable stack values.
        unsafe { native::cove_shell_global_pointer(&mut x, &mut y) }.then_some((x, y))
    }
    fn window_position(&self, id: WindowId) -> Option<(f32, f32)> {
        self.windows.get(&id).map(|w| {
            let s = w.state();
            (s.x as f32, s.y as f32)
        })
    }
    fn set_window_position(&self, id: WindowId, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.command(id, 1, x.round() as i32, y.round() as i32);
        }
    }
    fn set_window_size(&self, id: WindowId, width: f32, height: f32) {
        if width.is_finite() && height.is_finite() {
            self.command(
                id,
                2,
                width.round().max(1.0) as i32,
                height.round().max(1.0) as i32,
            );
        }
    }
    fn raise_window(&self, id: WindowId) {
        self.command(id, 3, 0, 0);
    }
    fn minimize_window(&self, id: WindowId) {
        self.command(id, 4, 0, 0);
    }
    fn maximize_window(&self, id: WindowId) {
        self.command(id, 5, 0, 0);
    }
    fn restore_window(&self, id: WindowId) {
        self.command(id, 6, 0, 0);
    }
    fn hide_window(&self, id: WindowId) {
        self.command(id, 7, 0, 0);
    }
    fn show_window(&self, id: WindowId) {
        self.command(id, 8, 0, 0);
    }
    fn is_window_maximized(&self, id: WindowId) -> bool {
        self.windows.get(&id).is_some_and(|w| w.state().maximized)
    }
    fn show_window_system_menu(&self, id: WindowId, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.command(id, 9, x.round() as i32, y.round() as i32);
        }
    }
    fn set_window_appearance(&self, id: WindowId, appearance: WindowAppearance) {
        if let Some(w) = self.windows.get(&id) {
            // SAFETY: the window pointer is live while the shell owns it.
            unsafe {
                native::cove_shell_appearance(w.pointer.get(), appearance == WindowAppearance::Dark)
            };
        }
    }
    fn set_window_chrome_hit_test(&self, id: WindowId, hit: Option<&WindowChromeHitTest>) {
        if let Some(w) = self.windows.get(&id) {
            let chrome = hit.map(|h| native::Chrome {
                title_height: h.title_bar_height.max(0.0),
                leading: h.draggable_leading_inset.max(0.0),
                trailing: h.draggable_trailing_inset.max(0.0),
                border: h.resize_border_width.max(0.0),
                explicit_drag: h.uses_explicit_drag_rects,
                drag: h.draggable_rects.as_ptr(),
                non_drag: h.non_draggable_rects.as_ptr(),
                drag_count: h.draggable_rects.len(),
                non_drag_count: h.non_draggable_rects.len(),
            }); // SAFETY: all rectangle slices remain valid while the bridge makes owned copies.
            unsafe {
                native::cove_shell_chrome(
                    w.pointer.get(),
                    chrome.as_ref().map_or(std::ptr::null(), |c| c as *const _),
                )
            };
        }
    }
    fn show_open_folder_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        accept: Box<dyn FnOnce(Option<String>)>,
    ) {
        self.dialog(
            id,
            true,
            path,
            &[],
            false,
            Box::new(move |files| accept(files.into_iter().next())),
        );
    }
    fn show_open_file_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        filters: &[FileDialogFilter],
        multiple: bool,
        accept: DialogCompletion,
    ) {
        self.dialog(id, false, path, filters, multiple, accept);
    }
    fn sample_screen_color(&self, done: ScreenColorCompletion) {
        // The per-window-system backend starts the picker and returns at once.
        // Its result is queued and delivered by `take_deferred_callbacks` on
        // this platform thread, never from inside a blocking native call.
        self.sampler.start(done);
    }
    fn display_refresh_rate(&self, id: Option<WindowId>) -> Option<f64> {
        id.or_else(|| self.main_window_id())
            .and_then(|id| self.windows.get(&id))
            .map(|w| w.state().refresh_rate)
            .filter(|hz| hz.is_finite() && (30.0..=500.0).contains(hz))
    }
    fn set_display_refresh_enabled(&self, id: WindowId, enabled: bool) -> bool {
        self.windows.get(&id).is_some_and(|w| {
            // SAFETY: live thread-confined window handle; native driver owns its callback lifetime.
            unsafe { native::cove_shell_refresh(w.pointer.get(), enabled) }
        })
    }
    fn take_display_refresh(&self, id: WindowId) -> Option<DisplayRefreshTick> {
        let w = self.windows.get(&id)?;
        let (mut timestamp, mut target) = (0.0, 0.0); // SAFETY: output doubles are exclusively writable; native inbox is synchronized.
        unsafe { native::cove_shell_refresh_take(w.pointer.get(), &mut timestamp, &mut target) }
            .then_some(DisplayRefreshTick {
                timestamp,
                target_timestamp: target,
            })
    }
    fn prepare_event_loop(&mut self) -> Result<(), ShellError> {
        if self.session.is_none() {
            self.session = Some(Session::acquire()?);
        }
        Ok(())
    }
    fn shutdown(&mut self) {
        let ids = self.order.clone();
        for id in ids {
            self.destroy_window(id);
        }
        self.dialogs.borrow_mut().callbacks.clear();
        self.sampler.cancel_all();
        self.session = None;
        self.quitting = true;
    }
}
impl Drop for Sdl3Shell {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Decode the fields AppKit's scroll monitor captures into a wheel event.
///
/// Extracted verbatim from the `kind == 19` branch of [`decode_event`] so the
/// unit/device/phase contract can be asserted without a live window: the marker
/// is only ever produced by `apple.m`, which needs a window and event monitor.
/// `flags & 4` is `hasPreciseScrollingDeltas`; `phase`/`momentum` are the values
/// `apple.m::phase_value` stored.
#[allow(
    clippy::too_many_arguments,
    reason = "decodes ten fields of one AppKit scroll event, matching apple.m's payload"
)]
pub fn wheel_event(
    dx: f32,
    dy: f32,
    x: f32,
    y: f32,
    timestamp: f64,
    flags: u16,
    phase: i32,
    momentum: i32,
) -> MouseWheelEvent {
    MouseWheelEvent {
        x: dx,
        y: dy,
        mouse_x: Some(x),
        mouse_y: Some(y),
        input: ScrollInput {
            unit: if flags & 4 != 0 {
                ScrollUnit::Points
            } else {
                ScrollUnit::Lines
            },
            device: if flags & 4 != 0 {
                ScrollDevice::Continuous
            } else {
                ScrollDevice::Wheel
            },
            phase: crate::scroll::phase(phase),
            momentum_phase: crate::scroll::phase(momentum),
            timestamp,
        },
    }
}
/// Decode one platform drop step into [`ExternalDropEvent`].
///
/// Extracted from the `25..=29` branch of [`decode_event`] so the phase and
/// payload contract can be asserted without a live window: a real drop needs a
/// drag from another application, which a headless test cannot produce. The
/// bridge already copied `data` into `text` and `source` into `aux`; this maps
/// them onto the phase that owns them.
pub fn external_drop(
    kind: u32,
    x: f32,
    y: f32,
    data: String,
    source: Option<String>,
) -> ExternalDropEvent {
    let phase = match kind {
        25 => DropPhase::Begin,
        26 => DropPhase::File,
        27 => DropPhase::Text,
        28 => DropPhase::Position,
        _ => DropPhase::Complete,
    };
    // A file drop carries one path per event and a text drop one string; the
    // other payload stays empty so a host never mixes the two.
    let (paths, text) = match phase {
        DropPhase::File => (vec![data], None),
        DropPhase::Text => (Vec::new(), Some(data)),
        _ => (Vec::new(), None),
    };
    ExternalDropEvent {
        phase,
        paths,
        text,
        x,
        y,
        source,
    }
}

fn decode_event(e: &native::Event) -> Option<InputEvent> {
    let key = || KeyEvent {
        scancode: e.a as u32,
        keycode: e.b as u32,
        modifiers: KeyModifiers(e.modifiers),
        is_repeat: e.flags & 1 != 0,
    };
    let kind = if e.flags & 2 != 0 {
        PointerKind::Touch
    } else {
        PointerKind::Mouse
    };
    Some(match e.kind {
        4 => InputEvent::WindowResized {
            width: e.a,
            height: e.b,
        },
        5 => InputEvent::WindowPixelSizeChanged {
            width: e.a,
            height: e.b,
        },
        6 => InputEvent::WindowFocusGained,
        7 => InputEvent::WindowFocusLost,
        8 => InputEvent::WindowMinimized,
        9 => InputEvent::WindowRestored,
        10 => InputEvent::WindowOccluded,
        11 => InputEvent::WindowExposed,
        12 => InputEvent::KeyDown(key()),
        13 => InputEvent::KeyUp(key()),
        14 => InputEvent::TextInput(e.string()),
        15 => InputEvent::TextEditing(if cfg!(target_os = "windows") {
            TextEditingEvent::from_utf16(e.string(), e.a, e.b)
        } else {
            TextEditingEvent {
                text: e.string(),
                start: e.a,
                length: e.b,
            }
        }),
        16 => InputEvent::MouseMotion(MouseMotionEvent {
            x: e.x,
            y: e.y,
            delta_x: e.dx,
            delta_y: e.dy,
            kind,
        }),
        17 | 18 => {
            let button = match e.a {
                1 => MouseButton::Left,
                2 => MouseButton::Middle,
                3 => MouseButton::Right,
                4 => MouseButton::X1,
                5 => MouseButton::X2,
                _ => return None,
            };
            let event = MouseButtonEvent {
                button,
                x: e.x,
                y: e.y,
                clicks: e.b.clamp(0, 255) as u8,
                modifiers: KeyModifiers(e.modifiers),
                kind,
            };
            if e.kind == 17 {
                InputEvent::MouseButtonDown(event)
            } else {
                InputEvent::MouseButtonUp(event)
            }
        }
        19 => InputEvent::MouseWheel(wheel_event(
            e.dx,
            e.dy,
            e.x,
            e.y,
            e.timestamp,
            e.flags,
            e.a,
            e.b,
        )),
        20 | 21 => {
            let buttons = [
                GamepadButton::South,
                GamepadButton::East,
                GamepadButton::West,
                GamepadButton::North,
                GamepadButton::Back,
                GamepadButton::Guide,
                GamepadButton::Start,
                GamepadButton::LeftStick,
                GamepadButton::RightStick,
                GamepadButton::LeftShoulder,
                GamepadButton::RightShoulder,
                GamepadButton::DpadUp,
                GamepadButton::DpadDown,
                GamepadButton::DpadLeft,
                GamepadButton::DpadRight,
            ];
            let event = GamepadButtonEvent {
                gamepad_id: e.identity as u32,
                button: *buttons.get(e.a as usize)?,
            };
            if e.kind == 20 {
                InputEvent::GamepadButtonDown(event)
            } else {
                InputEvent::GamepadButtonUp(event)
            }
        }
        22 => {
            let axes = [
                GamepadAxis::LeftX,
                GamepadAxis::LeftY,
                GamepadAxis::RightX,
                GamepadAxis::RightY,
                GamepadAxis::LeftTrigger,
                GamepadAxis::RightTrigger,
            ];
            InputEvent::GamepadAxisMotion(GamepadAxisEvent {
                gamepad_id: e.identity as u32,
                axis: *axes.get(e.a as usize)?,
                value: e.x,
            })
        }
        23 => InputEvent::GamepadAdded(e.identity as u32),
        24 => InputEvent::GamepadRemoved(e.identity as u32),
        25..=29 => {
            InputEvent::ExternalDrop(external_drop(e.kind, e.x, e.y, e.string(), e.aux_string()))
        }
        _ => return None,
    })
}
