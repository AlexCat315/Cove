use crate::{
    PlatformFrameRateMode, PlatformHost,
    frame_drive::{HostFrameClock, HostFrameDrive},
};
use cove_core::scene::*;
use cove_platform::*;
use cove_shell::{
    ClipboardCompletion, EventLoopWaker, FileDialogFilter, MenuCompletion, NativeRenderSurface,
    NativeWindowReference, Shell, ShellError, WindowChromeHitTest, WindowHandle, WindowOptions,
    WindowSafeArea,
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    time::Duration,
};
type EventCallback = Rc<dyn Fn(&InputEvent)>;
type FrameCallback = Rc<dyn Fn(NativeRenderSurface) -> FrameOutcome>;
type InitCallback = Rc<dyn Fn(NativeRenderSurface, u32, u32)>;
type ResizeCallback = Rc<dyn Fn(u32, u32)>;

/// What one `on_frame` call managed to do.
///
/// The host paces retries differently for each case, and conflating them is what
/// let a refused frame park a window forever: "no slot available" is pacing,
/// "rendering failed" is an error, and only the latter may wait on an external
/// event before trying again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameOutcome {
    /// A frame was produced and submitted.
    Rendered,
    /// Nothing was produced, but nothing went wrong: every in-flight slot was
    /// taken, the renderer was not configured yet, or there was no root view.
    /// Retry as soon as capacity allows.
    Skipped,
    /// Rendering reported an error; the run loop has already been failed.
    Failed,
}

impl FrameOutcome {
    /// Merges two handlers' results: any failure wins, then any success, and a
    /// skip only survives when nothing else happened.
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Failed, _) | (_, Self::Failed) => Self::Failed,
            (Self::Rendered, _) | (_, Self::Rendered) => Self::Rendered,
            _ => Self::Skipped,
        }
    }
}

/// When a session may render again.
///
/// This replaced a bare `f64` deadline whose "wait for the next display tick"
/// case was spelled `f64::INFINITY`. `now >= INFINITY` is never true, so once a
/// window entered that state it stopped rendering until something else happened
/// to reset the field — and the event-driven loop is allowed to switch the tick
/// source off, which turned a dropped frame into a permanent freeze.
///
/// `AwaitTick` now states that intent explicitly, and the one place that can
/// switch ticks off must release it; see `release_tick_gates`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RenderGate {
    /// Render on the next loop iteration.
    Due,
    /// Render once the host clock passes this deadline. Always finite: the only
    /// constructor is [`RenderGate::after`], which rejects non-finite intervals.
    NotBefore(f64),
    /// Render only when a display-refresh tick arrives.
    AwaitTick,
}

impl RenderGate {
    fn is_due(self, now: f64) -> bool {
        match self {
            Self::Due => true,
            Self::NotBefore(at) => now >= at,
            Self::AwaitTick => false,
        }
    }

    /// The self-imposed deadline, if any. `AwaitTick` has none: it is woken by a
    /// tick, never by the clock.
    fn deadline(self) -> Option<f64> {
        match self {
            Self::Due => Some(0.0),
            Self::NotBefore(at) => Some(at),
            Self::AwaitTick => None,
        }
    }

    /// Paces a retry one interval out. The interval always comes from a clamped
    /// refresh rate, and the assert keeps it that way: a gate that can never
    /// come due must not be expressible.
    fn after(now: f64, interval: f64) -> Self {
        debug_assert!(
            interval.is_finite() && interval >= 0.0,
            "a render retry interval must be finite and non-negative"
        );
        Self::NotBefore(now + interval.max(0.0))
    }
}

fn callback<T: Clone>(slot: &RefCell<Option<T>>) -> Option<T> {
    slot.borrow().clone()
}

/// Refresh rate used to pace a retry, clamped to something a display can do.
fn display_hz(shell: Rc<RefCell<Box<dyn Shell>>>, id: WindowId) -> f64 {
    shell
        .borrow()
        .display_refresh_rate(Some(id))
        .filter(|hz| hz.is_finite() && *hz >= 30.0)
        .unwrap_or(60.0)
        .clamp(1.0, 240.0)
}
#[derive(Default)]
pub struct SessionCallbacks {
    pub on_frame: RefCell<Option<FrameCallback>>,
    pub on_init: RefCell<Option<InitCallback>>,
    pub on_resize: RefCell<Option<ResizeCallback>>,
    pub on_event: RefCell<Option<EventCallback>>,
    pub input_observer: RefCell<Option<EventCallback>>,
    pub input_interceptor: RefCell<Option<Rc<dyn Fn(&InputEvent) -> bool>>>,
    pub on_presentation_changed: RefCell<Option<Rc<dyn Fn(bool)>>>,
}
pub struct PlatformWindowSession {
    pub id: WindowId,
    pub tree: Rc<RefCell<NodeTree>>,
    pub recomposer: Rc<Recomposer>,
    pub input_context: Rc<PlatformInputContext>,
    pub callbacks: SessionCallbacks,
    dispatcher: EventDispatcher,
    did_call_on_init: Cell<bool>,
    last_text_input_area: Cell<Option<TextInputArea>>,
    local_text_focus: Cell<bool>,
    last_text_cursor_tick: Cell<f64>,
    needs_display: Cell<bool>,
    injected_press_held: Cell<bool>,
    gate: Cell<RenderGate>,
    /// When a display request started waiting for a frame that never came, used
    /// by the stall watchdog. `None` while nothing is pending.
    stall_since: Cell<Option<f64>>,
    presentation_active: Cell<bool>,
    keep_rendering: Cell<bool>,
    drawable_size: Cell<(u32, u32)>,
    logical_size: Cell<(u32, u32)>,
    content_scale: Cell<f32>,
}
impl PlatformWindowSession {
    fn new(
        handle: &dyn WindowHandle,
        tree: Rc<RefCell<NodeTree>>,
        recomposer: Rc<Recomposer>,
        input: Rc<PlatformInputContext>,
        shell: Rc<RefCell<Box<dyn Shell>>>,
    ) -> Self {
        let id = handle.id();
        let mut dispatcher = EventDispatcher::new(
            tree.clone(),
            input.interactions.clone(),
            input.pointer_capture.clone(),
            input.focus_chain.clone(),
            id,
        );
        let weak = Rc::downgrade(&shell);
        dispatcher.cursor_sink = Some(Rc::new(move |cursor| {
            if let Some(shell) = weak.upgrade() {
                shell.borrow().set_cursor(id, cursor);
            }
        }));
        Self {
            id,
            tree,
            recomposer,
            input_context: input,
            callbacks: Default::default(),
            dispatcher,
            did_call_on_init: Cell::new(false),
            last_text_input_area: Cell::new(None),
            local_text_focus: Cell::new(false),
            last_text_cursor_tick: Cell::new(0.0),
            needs_display: Cell::new(true),
            injected_press_held: Cell::new(false),
            gate: Cell::new(RenderGate::Due),
            stall_since: Cell::new(None),
            presentation_active: Cell::new(true),
            keep_rendering: Cell::new(false),
            drawable_size: Cell::new(handle.drawable_size()),
            logical_size: Cell::new(handle.logical_size()),
            content_scale: Cell::new(handle.content_scale_factor()),
        }
    }
    pub fn with_current<R>(&self, body: impl FnOnce() -> R) -> R {
        self.input_context.with_current(body)
    }
    pub fn request_display(&self) {
        self.needs_display.set(true);
    }

    fn gate_is_due(&self, now: f64) -> bool {
        self.gate.get().is_due(now)
    }

    fn gate_deadline(&self) -> Option<f64> {
        self.gate.get().deadline()
    }

    fn set_gate(&self, gate: RenderGate) {
        self.gate.set(gate);
    }

    /// Parks the session until the next display-refresh tick.
    ///
    /// Every caller must be paired with a release path: `release_tick_gates`
    /// runs wherever ticks can be switched off, so a parked session can never
    /// outlive its wake source.
    fn park_on_tick(&self) {
        self.gate.set(RenderGate::AwaitTick);
    }

    /// Releases a tick-parked session, reporting whether it was parked at all.
    fn release_tick_gate(&self) -> bool {
        if self.gate.get() == RenderGate::AwaitTick {
            self.gate.set(RenderGate::Due);
            return true;
        }
        false
    }

    fn note_frame_rendered(&self) {
        self.gate.set(RenderGate::Due);
        self.stall_since.set(None);
    }
    pub fn drawable_size(&self) -> (u32, u32) {
        self.drawable_size.get()
    }
    pub fn logical_size(&self) -> (u32, u32) {
        self.logical_size.get()
    }
    pub fn content_scale_factor(&self) -> f32 {
        self.content_scale.get()
    }
    pub fn is_presentation_active(&self) -> bool {
        self.presentation_active.get()
    }
    pub fn interactions(&self) -> Rc<InteractionRegistry> {
        self.input_context.interactions.clone()
    }
    pub fn pointer_capture(&self) -> Rc<PointerCapture> {
        self.input_context.pointer_capture.clone()
    }
    pub fn focus_chain(&self) -> Rc<FocusChain> {
        self.input_context.focus_chain.clone()
    }
    fn accepts_native(&self, event: &InputEvent) -> bool {
        if matches!(
            event,
            InputEvent::MouseButtonDown(_) | InputEvent::KeyDown(_) | InputEvent::WindowFocusGained
        ) {
            self.local_text_focus.set(false);
        }
        match event {
            InputEvent::MouseMotion(_) | InputEvent::MouseWheel(_) => {
                !self.injected_press_held.get()
            }
            InputEvent::MouseButtonDown(_) | InputEvent::MouseButtonUp(_) => {
                self.injected_press_held.set(false);
                true
            }
            _ => true,
        }
    }
    fn dispatch(&self, event: &InputEvent) -> bool {
        if let Some(observer) = callback(&self.callbacks.input_observer) {
            observer(event);
        }
        if callback(&self.callbacks.input_interceptor).is_some_and(|f| f(event)) {
            return false;
        }
        self.with_current(|| self.dispatcher.dispatch(event))
    }
    pub fn inject_event(&self, event: InputEvent) {
        match event {
            InputEvent::MouseButtonDown(_) => self.injected_press_held.set(true),
            InputEvent::MouseButtonUp(_) => self.injected_press_held.set(false),
            _ => (),
        };
        if self.dispatch(&event)
            && let Some(handler) = callback(&self.callbacks.on_event)
        {
            self.with_current(|| handler(&event));
        }
        self.request_display();
    }
    /// Dispatches inside this session without touching the platform event queue.
    pub fn dispatch_local(&self, event: InputEvent) {
        let previous_focus = self.focus_chain().focused().map(|node| node.id());
        if let Some(observer) = callback(&self.callbacks.input_observer) {
            observer(&event);
        }
        if callback(&self.callbacks.input_interceptor).is_some_and(|f| f(&event)) {
            return;
        }
        if self.with_current(|| self.dispatcher.dispatch_local(&event))
            && let Some(handler) = callback(&self.callbacks.on_event)
        {
            self.with_current(|| handler(&event));
        }
        if previous_focus != self.focus_chain().focused().map(|node| node.id()) {
            self.local_text_focus.set(true);
        }
        self.request_display();
    }

    fn update_presentation(&self, active: bool) -> bool {
        if self.presentation_active.replace(active) == active {
            return false;
        }
        self.set_gate(RenderGate::Due);
        if active {
            self.request_display();
        }
        if let Some(handler) = callback(&self.callbacks.on_presentation_changed) {
            handler(active);
        }
        true
    }
    fn update_metrics(&self, handle: &dyn WindowHandle) -> bool {
        let (d, l, s) = (
            handle.drawable_size(),
            handle.logical_size(),
            handle.content_scale_factor(),
        );
        if (d, l, s)
            == (
                self.drawable_size.get(),
                self.logical_size.get(),
                self.content_scale.get(),
            )
        {
            return false;
        }
        self.drawable_size.set(d);
        self.logical_size.set(l);
        self.content_scale.set(s);
        true
    }
    fn focused_text_area(&self) -> Option<TextInputArea> {
        self.input_context.focus_chain.focused().and_then(|node| {
            let area = match node.attachment::<cove_core::scene::TextInputAreaResolver>(
                TextInputAttachmentKey::AREA_RESOLVER,
            ) {
                Some(resolve) => resolve(&node, node.absolute_origin()),
                None => node.attachment(TextInputAttachmentKey::AREA),
            };
            if let Some(area) = area {
                node.set_attachment(TextInputAttachmentKey::AREA, area);
            } else {
                node.remove_attachment(TextInputAttachmentKey::AREA);
            }
            area
        })
    }
}
#[derive(Default)]
pub struct HostCallbacks {
    pub on_frame: RefCell<Option<FrameCallback>>,
    pub on_init: RefCell<Option<InitCallback>>,
    pub on_resize: RefCell<Option<ResizeCallback>>,
    pub on_event: RefCell<Option<EventCallback>>,
    /// Runs once per loop iteration whether or not a frame is produced. The
    /// diagnostic backend uses it to drain its transport while the app is idle.
    pub on_tick: RefCell<Option<Rc<dyn Fn()>>>,
    pub on_before_commit: RefCell<Option<Rc<dyn Fn(f64)>>>,
    pub on_window_teardown: RefCell<Option<Rc<dyn Fn(WindowId)>>>,
    pub on_window_presentation_changed: RefCell<Option<Rc<dyn Fn(WindowId, bool)>>>,
    pub on_teardown: RefCell<Option<Rc<dyn Fn()>>>,
    pub external_display_request_drain: RefCell<Option<Rc<dyn Fn() -> bool>>>,
    pub window_close_interceptor: RefCell<Option<Rc<dyn Fn(Option<WindowId>) -> bool>>>,
}
#[derive(Clone)]
pub struct HostWorkSender {
    sender: Sender<Box<dyn FnOnce() + Send>>,
    wake: Arc<Mutex<EventLoopWaker>>,
}
impl HostWorkSender {
    pub fn enqueue(&self, work: impl FnOnce() + Send + 'static) -> bool {
        let sent = self.sender.send(Box::new(work)).is_ok();
        if sent {
            self.wake_event_loop();
        }
        sent
    }
    pub fn wake_event_loop(&self) {
        let wake = self.waker();
        wake();
    }
    fn waker(&self) -> EventLoopWaker {
        self.wake.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn set_waker(&self, wake: EventLoopWaker) {
        *self.wake.lock().unwrap_or_else(|e| e.into_inner()) = wake;
    }
}
struct HostData {
    title: String,
    options: WindowOptions,
    shell_factory: Box<dyn Fn() -> Result<Box<dyn Shell>, ShellError>>,
    shell: RefCell<Option<Rc<RefCell<Box<dyn Shell>>>>>,
    pending_menu_bar: RefCell<Option<(NativeMenuBarSnapshot, cove_shell::MenuBarCompletion)>>,
    sessions: RefCell<HashMap<WindowId, Rc<PlatformWindowSession>>>,
    order: RefCell<Vec<WindowId>>,
    main: Cell<Option<WindowId>>,
    running: Cell<bool>,
    finishing: Cell<bool>,
    closing: RefCell<HashSet<WindowId>>,
    close_requests: Rc<RefCell<Vec<Option<WindowId>>>>,
    recomposer: Rc<Recomposer>,
    input: Rc<PlatformInputContext>,
    drive: HostFrameDrive,
    callbacks: HostCallbacks,
    work_sender: HostWorkSender,
    work_receiver: RefCell<Receiver<Box<dyn FnOnce() + Send>>>,
    local_work: RefCell<Vec<Box<dyn FnOnce()>>>,
    clock: RefCell<Rc<dyn Fn() -> f64>>,
    startup_error: RefCell<Option<ShellError>>,
    #[cfg(target_os = "ios")]
    driver: RefCell<Option<crate::ios::IOSFrameDriver>>,
}
#[derive(Clone)]
pub struct Sdl3PlatformHost(Rc<HostData>);
impl Default for Sdl3PlatformHost {
    fn default() -> Self {
        Self::new("CoveUI", WindowOptions::default())
    }
}
impl Sdl3PlatformHost {
    pub fn new(title: impl Into<String>, options: WindowOptions) -> Self {
        Self::with_services(
            title,
            options,
            Rc::new(Recomposer::new()),
            Rc::new(PlatformInputContext::new()),
            || match crate::platform::PlatformLink::current() {
                Some(link) => {
                    Ok(Box::new(crate::platform::ProxyShell::new(link)) as Box<dyn Shell>)
                }
                None => cove_shell::make_default_shell().map(|s| Box::new(s) as Box<dyn Shell>),
            },
        )
    }
    pub fn with_services(
        title: impl Into<String>,
        options: WindowOptions,
        recomposer: Rc<Recomposer>,
        input: Rc<PlatformInputContext>,
        factory: impl Fn() -> Result<Box<dyn Shell>, ShellError> + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self(Rc::new(HostData {
            title: title.into(),
            options,
            shell_factory: Box::new(factory),
            shell: RefCell::new(None),
            pending_menu_bar: RefCell::new(None),
            sessions: RefCell::new(HashMap::new()),
            order: RefCell::new(Vec::new()),
            main: Cell::new(None),
            running: Cell::new(false),
            finishing: Cell::new(false),
            closing: RefCell::new(HashSet::new()),
            close_requests: Rc::new(RefCell::new(Vec::new())),
            recomposer,
            input,
            drive: Default::default(),
            callbacks: Default::default(),
            work_sender: HostWorkSender {
                sender,
                wake: Arc::new(Mutex::new(Arc::new(cove_shell::Sdl3Shell::wake))),
            },
            work_receiver: RefCell::new(receiver),
            local_work: RefCell::new(Vec::new()),
            clock: RefCell::new(Rc::new(cove_shell::monotonic_seconds)),
            startup_error: RefCell::new(None),
            #[cfg(target_os = "ios")]
            driver: RefCell::new(None),
        }))
    }
    pub fn callbacks(&self) -> &HostCallbacks {
        &self.0.callbacks
    }
    pub fn recomposer(&self) -> Rc<Recomposer> {
        self.0.recomposer.clone()
    }
    pub fn input_context(&self) -> Rc<PlatformInputContext> {
        self.0.input.clone()
    }
    pub fn set_frame_clock(&self, clock: impl Fn() -> f64 + 'static) {
        *self.0.clock.borrow_mut() = Rc::new(clock);
    }
    fn now(&self) -> f64 {
        let clock = self.0.clock.borrow().clone();
        clock()
    }
    fn shell(&self) -> Option<Rc<RefCell<Box<dyn Shell>>>> {
        self.0.shell.borrow().clone()
    }
    fn resolve_shell(&self) -> Result<Rc<RefCell<Box<dyn Shell>>>, ShellError> {
        if let Some(shell) = self.shell() {
            return Ok(shell);
        }
        let mut shell = (self.0.shell_factory)()?;
        self.0.work_sender.set_waker(shell.waker());
        let requests = self.0.close_requests.clone();
        shell.set_close_interceptor(Some(Box::new(move |id| {
            requests.borrow_mut().push(id);
            false
        })));
        let shell = Rc::new(RefCell::new(shell));
        *self.0.shell.borrow_mut() = Some(shell.clone());
        Ok(shell)
    }
    pub fn session(&self, id: WindowId) -> Option<Rc<PlatformWindowSession>> {
        self.0.sessions.borrow().get(&id).cloned()
    }
    pub fn main_session(&self) -> Option<Rc<PlatformWindowSession>> {
        self.0.main.get().and_then(|id| self.session(id))
    }
    pub fn main_window(&self) -> Option<WindowId> {
        self.0.main.get()
    }
    pub fn window_ids(&self) -> Vec<WindowId> {
        self.0.order.borrow().clone()
    }
    fn sessions(&self) -> Vec<Rc<PlatformWindowSession>> {
        self.window_ids()
            .into_iter()
            .filter_map(|id| self.session(id))
            .collect()
    }
    fn make_session(
        &self,
        handle: Rc<dyn WindowHandle>,
        tree: Rc<RefCell<NodeTree>>,
        recomposer: Rc<Recomposer>,
        input: Rc<PlatformInputContext>,
    ) -> Rc<PlatformWindowSession> {
        let session = Rc::new(PlatformWindowSession::new(
            &*handle,
            tree,
            recomposer,
            input,
            self.shell().unwrap(),
        ));
        self.0
            .sessions
            .borrow_mut()
            .insert(handle.id(), session.clone());
        self.0.order.borrow_mut().push(handle.id());
        if self.0.main.get().is_none() {
            self.0.main.set(Some(handle.id()));
        }
        session
    }
    pub fn open_window(
        &self,
        title: &str,
        tree: Rc<RefCell<NodeTree>>,
        recomposer: Rc<Recomposer>,
        input: Rc<PlatformInputContext>,
        options: WindowOptions,
    ) -> Result<Rc<PlatformWindowSession>, ShellError> {
        let shell = self.resolve_shell()?;
        let handle = shell.borrow_mut().create_window(title, options)?;
        Ok(self.make_session(handle, tree, recomposer, input))
    }
    fn ensure_main(&self, tree: Rc<RefCell<NodeTree>>) -> Result<(), ShellError> {
        if self.main_session().is_some() {
            return Ok(());
        }
        let shell = self.resolve_shell()?;
        if shell.borrow().main_window_id().is_none() {
            shell
                .borrow_mut()
                .create_window(&self.0.title, self.0.options.clone())?;
        }
        let id = shell.borrow().main_window_id().ok_or_else(|| {
            ShellError::InitializationFailed("main window was not registered".into())
        })?;
        if let Some((bar, on_choice)) = self.0.pending_menu_bar.borrow_mut().take() {
            shell.borrow().install_menu_bar(&bar, on_choice);
        }
        let handle = shell
            .borrow()
            .window(id)
            .ok_or_else(|| ShellError::InitializationFailed("missing main window handle".into()))?;
        self.make_session(
            handle,
            tree,
            self.0.recomposer.clone(),
            self.0.input.clone(),
        );
        Ok(())
    }
    pub fn drawable_size(&self) -> (u32, u32) {
        self.main_session().map_or((1, 1), |s| s.drawable_size())
    }
    pub fn logical_size(&self) -> (u32, u32) {
        self.main_session().map_or((1, 1), |s| s.logical_size())
    }
    pub fn content_scale_factor(&self) -> f32 {
        self.main_session()
            .map_or(1.0, |s| s.content_scale_factor())
    }
    pub fn native_window_reference(&self, id: WindowId) -> Option<NativeWindowReference> {
        self.shell()?.borrow().window(id)?.native_window_reference()
    }
    /// The backdrop the main window's platform applied.
    pub fn backdrop(&self) -> cove_shell::WindowBackdrop {
        self.shell()
            .and_then(|s| self.main_window().and_then(|id| s.borrow().window(id)))
            .map_or(cove_shell::WindowBackdrop::Opaque, |w| w.backdrop())
    }
    /// The system light/dark appearance reported for the main window.
    pub fn system_appearance(&self) -> cove_shell::SystemAppearance {
        self.shell()
            .and_then(|s| self.main_window().and_then(|id| s.borrow().window(id)))
            .map_or(cove_shell::SystemAppearance::Unknown, |w| {
                w.system_appearance()
            })
    }
    pub fn safe_area(&self) -> WindowSafeArea {
        self.shell()
            .and_then(|s| self.main_window().and_then(|id| s.borrow().window(id)))
            .map_or(WindowSafeArea::default(), |w| w.safe_area())
    }
    /// On-screen keyboard inset in logical points, refreshed every frame.
    pub fn keyboard_height(&self) -> f32 {
        self.shell()
            .and_then(|s| self.main_window().map(|id| s.borrow().keyboard_height(id)))
            .unwrap_or(0.0)
    }
    pub fn device_form(&self) -> cove_shell::DeviceForm {
        self.shell().map_or(cove_shell::DeviceForm::Computer, |s| {
            s.borrow().device_form()
        })
    }
    pub fn window_position(&self, id: WindowId) -> Option<(f32, f32)> {
        self.shell()?.borrow().window_position(id)
    }
    pub fn set_window_position(&self, id: WindowId, x: f32, y: f32) {
        if let Some(s) = self.shell() {
            s.borrow().set_window_position(id, x, y);
        }
    }
    pub fn set_window_size(&self, id: WindowId, width: f32, height: f32) {
        if let Some(s) = self.shell() {
            s.borrow().set_window_size(id, width, height);
        }
    }
    pub fn minimize_window(&self, id: WindowId) {
        if let Some(s) = self.shell() {
            s.borrow().minimize_window(id);
        }
        self.synchronize_presentation(id);
    }
    pub fn maximize_window(&self, id: WindowId) {
        if let Some(s) = self.shell() {
            s.borrow().maximize_window(id);
        }
    }
    pub fn restore_window(&self, id: WindowId) {
        if let Some(s) = self.shell() {
            s.borrow().restore_window(id);
        }
        self.synchronize_presentation(id);
    }
    pub fn hide_window(&self, id: WindowId) {
        if let Some(s) = self.shell() {
            s.borrow().hide_window(id);
        }
        self.synchronize_presentation(id);
    }
    pub fn show_window(&self, id: WindowId) {
        if let Some(s) = self.shell() {
            s.borrow().show_window(id);
        }
        self.synchronize_presentation(id);
    }
    pub fn set_window_fullscreen(&self, id: WindowId, fullscreen: bool) {
        if let Some(s) = self.shell() {
            s.borrow().set_window_fullscreen(id, fullscreen);
        }
        self.synchronize_presentation(id);
    }
    pub fn is_window_fullscreen(&self, id: WindowId) -> bool {
        self.shell()
            .is_some_and(|s| s.borrow().is_window_fullscreen(id))
    }
    pub fn set_window_always_on_top(&self, id: WindowId, on_top: bool) {
        if let Some(s) = self.shell() {
            s.borrow().set_window_always_on_top(id, on_top);
        }
    }
    pub fn is_window_maximized(&self, id: WindowId) -> bool {
        self.shell()
            .is_some_and(|s| s.borrow().is_window_maximized(id))
    }
    pub fn show_window_system_menu(&self, id: WindowId, x: f32, y: f32) {
        if let Some(s) = self.shell() {
            s.borrow().show_window_system_menu(id, x, y);
        }
    }
    pub fn set_window_appearance(&self, id: WindowId, appearance: cove_shell::WindowAppearance) {
        if let Some(s) = self.shell() {
            s.borrow().set_window_appearance(id, appearance);
        }
    }
    pub fn set_window_chrome_hit_test(&self, id: WindowId, hit: Option<&WindowChromeHitTest>) {
        if let Some(s) = self.shell() {
            s.borrow().set_window_chrome_hit_test(id, hit);
        }
    }
    pub fn show_open_folder_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        accept: Box<dyn FnOnce(Option<String>)>,
    ) {
        if let Some(s) = self.shell() {
            s.borrow().show_open_folder_dialog(id, path, accept);
        } else {
            accept(None);
        }
    }
    pub fn show_open_file_dialog(
        &self,
        id: Option<WindowId>,
        path: Option<&str>,
        filters: &[FileDialogFilter],
        multiple: bool,
        accept: Box<dyn FnOnce(Vec<String>)>,
    ) {
        if let Some(s) = self.shell() {
            s.borrow()
                .show_open_file_dialog(id, path, filters, multiple, accept);
        } else {
            accept(Vec::new());
        }
    }
    pub fn close_window(&self, id: WindowId) {
        if !self.0.closing.borrow_mut().insert(id) {
            return;
        }
        if let Some(handler) = callback(&self.0.callbacks.on_window_teardown) {
            handler(id);
        }
        if let Some(s) = self.shell() {
            s.borrow_mut().destroy_window(id);
        }
        self.0.closing.borrow_mut().remove(&id);
        self.prune_closed_sessions();
    }
    pub fn write_clipboard(&self, text: &str) {
        if let Ok(shell) = self.resolve_shell() {
            shell.borrow().write_clipboard(text);
        }
    }
    pub fn read_clipboard(&self, done: ClipboardCompletion) {
        match self.resolve_shell() {
            Ok(shell) => shell.borrow().read_clipboard(done),
            Err(_) => done(None),
        }
    }
    pub fn sample_screen_color(&self, done: ScreenColorCompletion) {
        match self.resolve_shell() {
            Ok(shell) => shell.borrow().sample_screen_color(done),
            Err(_) => done(ScreenColorSample::Unsupported(
                "the platform shell is not available".into(),
            )),
        }
    }
    /// The native sampling backend, used by hosts to decide whether the
    /// eyedropper affordance can do anything on this platform/session.
    pub fn screen_color_backend(&self) -> ScreenColorBackend {
        cove_shell::screen_color_backend()
    }
    pub fn present_menu(
        &self,
        id: Option<WindowId>,
        snapshot: &NativeMenuSnapshot,
        x: f32,
        y: f32,
        done: MenuCompletion,
    ) {
        match self.shell() {
            Some(shell) => shell.borrow().present_menu(id, snapshot, x, y, done),
            None => done(None),
        }
    }
    /// Installs the application menu bar. `on_choice` runs on the UI thread with
    /// the chosen authored item id; a shell without a native bar ignores it.
    pub fn install_menu_bar(
        &self,
        bar: &cove_platform::NativeMenuBarSnapshot,
        on_choice: cove_shell::MenuBarCompletion,
    ) {
        if let Some(shell) = self.shell().filter(|_| self.main_window().is_some()) {
            shell.borrow().install_menu_bar(bar, on_choice);
        } else {
            *self.0.pending_menu_bar.borrow_mut() = Some((bar.clone(), on_choice));
        }
    }
    /// Drains documents the OS opened with the application. Call on the UI
    /// thread; both the direct shell and the proxy answer through this.
    pub fn take_open_documents(&self) -> Vec<String> {
        self.shell()
            .map(|shell| shell.borrow().take_open_documents())
            .unwrap_or_default()
    }
    /// Shows the platform save panel and writes `request.contents` where the
    /// user chooses. The write runs in the completion on this (UI) thread, not
    /// the platform thread the panel returns on. Without a shell the request
    /// reports no panel.
    pub fn show_save_file_dialog(
        &self,
        id: Option<WindowId>,
        request: cove_shell::SaveFileRequest,
        accept: cove_shell::SaveFileCompletion,
    ) {
        match self.shell() {
            Some(shell) => {
                let contents = request.contents.clone();
                shell.borrow().show_save_file_dialog(
                    id,
                    request,
                    Box::new(move |outcome| accept(outcome.finish(&contents))),
                );
            }
            None => accept(cove_shell::SaveFileOutcome::Unsupported(
                "the platform shell is not available".into(),
            )),
        }
    }
    /// A window that could not present stays dirty and tries again one display
    /// interval from now, so a failing present cannot spin the loop.
    pub fn retry_display(&self, id: Option<WindowId>) {
        let Some(session) = id
            .or_else(|| self.main_window())
            .and_then(|id| self.session(id))
        else {
            return;
        };
        let hz = self
            .current_display_refresh_rate(Some(session.id))
            .filter(|hz| *hz >= 30.0)
            .unwrap_or(60.0);
        session.request_display();
        session.set_gate(RenderGate::after(self.now(), 1.0 / hz));
        self.wake_event_loop();
    }
    pub fn request_display(&self, id: Option<WindowId>) {
        if let Some(session) = id
            .or_else(|| self.main_window())
            .and_then(|id| self.session(id))
        {
            session.request_display();
        }
        self.wake_event_loop();
    }
    pub fn set_window_keep_rendering(&self, id: WindowId, keep: bool) {
        if let Some(s) = self.session(id) {
            s.keep_rendering.set(keep);
            s.request_display();
        }
    }
    pub fn work_sender(&self) -> HostWorkSender {
        self.0.work_sender.clone()
    }
    pub fn enqueue_main_thread_work(&self, work: impl FnOnce() + 'static) {
        self.0.local_work.borrow_mut().push(Box::new(work));
        self.wake_event_loop();
    }
    pub fn wake_event_loop(&self) {
        self.0.work_sender.wake_event_loop();
    }
    pub fn set_target_frame_rate(&self, fps: Option<f64>) {
        self.set_frame_rate_mode(fps.filter(|f| f.is_finite() && *f > 0.0).map_or(
            PlatformFrameRateMode::EventDriven,
            PlatformFrameRateMode::Fixed,
        ));
    }
    pub fn set_frame_rate_mode(&self, mode: PlatformFrameRateMode) {
        self.0.drive.mode.set(match mode {
            PlatformFrameRateMode::Fixed(fps) => PlatformFrameRateMode::Fixed(if fps.is_finite() {
                fps.clamp(1.0, 240.0)
            } else {
                60.0
            }),
            mode => mode,
        });
        self.wake_event_loop();
    }
    pub fn current_display_refresh_rate(&self, id: Option<WindowId>) -> Option<f64> {
        self.shell()
            .and_then(|s| {
                s.borrow()
                    .display_refresh_rate(id.or_else(|| self.main_window()))
            })
            .filter(|f| f.is_finite() && *f > 0.0)
            .map(|f| f.clamp(1.0, 240.0))
    }
    fn synchronize_presentation(&self, id: WindowId) {
        let Some(session) = self.session(id) else {
            return;
        };
        let Some(handle) = self.shell().and_then(|s| s.borrow().window(id)) else {
            return;
        };
        let size = handle.drawable_size();
        let active = handle.is_visible()
            && !handle.is_minimized()
            && !handle.is_occluded()
            && size.0 > 0
            && size.1 > 0;
        if session.update_presentation(active)
            && let Some(handler) = callback(&self.0.callbacks.on_window_presentation_changed)
        {
            handler(id, active);
        }
    }
    fn prune_closed_sessions(&self) {
        let live = self
            .shell()
            .map(|s| s.borrow().window_ids().into_iter().collect::<HashSet<_>>())
            .unwrap_or_default();
        self.0
            .sessions
            .borrow_mut()
            .retain(|id, _| live.contains(id));
        self.0.order.borrow_mut().retain(|id| live.contains(id));
        if self.0.main.get().is_some_and(|id| !live.contains(&id)) {
            self.0.main.set(self.0.order.borrow().first().copied());
        }
    }
    pub fn run_open_windows(&self) -> Result<(), ShellError> {
        if self.0.sessions.borrow().is_empty() {
            return Err(ShellError::InitializationFailed(
                "run called without registered windows".into(),
            ));
        }
        self.run_loop();
        match self.0.startup_error.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    fn run_loop(&self) {
        if self.0.running.replace(true) {
            return;
        }
        set_scene_invalidation_wake(Some(self.0.work_sender.waker()));
        self.0.finishing.set(false);
        *self.0.drive.clock.borrow_mut() = Rc::new(HostFrameClock::new(self.now()));
        #[cfg(target_os = "ios")]
        {
            let host = self.clone();
            let driver = crate::ios::IOSFrameDriver::new(move || {
                if !host.should_continue() {
                    host.finish_run_loop();
                    return false;
                }
                host.run_frame(false);
                true
            });
            *self.0.driver.borrow_mut() = Some(driver);
        }
        #[cfg(not(target_os = "ios"))]
        {
            let _timer = crate::lifecycle::TimerResolution::new();
            while self.should_continue() {
                crate::lifecycle::with_autorelease_pool(|| self.run_frame(true));
            }
            self.finish_run_loop();
        }
    }
    fn should_continue(&self) -> bool {
        self.0.running.get()
            && self.shell().is_some_and(|s| s.borrow().is_running())
            && !self.0.sessions.borrow().is_empty()
    }
    /// One loop iteration; callbacks may request display, close or create windows.
    pub fn run_frame(&self, wait: bool) {
        let Some(shell) = self.shell() else {
            return;
        };
        let state = self.0.drive.clock.borrow().clone();
        let start = self.now();
        let loop_delta = start - state.last_loop_time.replace(start);
        if let Some(tick) = callback(&self.0.callbacks.on_tick) {
            tick();
        }
        let events = shell.borrow_mut().poll_window_events();
        let deferred = shell.borrow().take_deferred_callbacks();
        for work in deferred {
            work();
        }
        // After the poll: the poll consumes a coalesced wake, so work posted
        // before this drain is seen and work posted after it wakes the next wait.
        drain_scene_invalidations();
        crate::NativeAccessibility::perform_pending();
        let close_requests = std::mem::take(&mut *self.0.close_requests.borrow_mut());
        for id in close_requests {
            if callback(&self.0.callbacks.window_close_interceptor).is_none_or(|f| f(id)) {
                if let Some(id) = id {
                    self.close_window(id);
                } else {
                    self.stop();
                }
            }
        }
        for routed in events {
            let Some(session) = self.session(routed.window_id) else {
                continue;
            };
            if !session.accepts_native(&routed.event) {
                continue;
            }
            if session.dispatch(&routed.event) {
                if let Some(handler) = callback(&session.callbacks.on_event) {
                    session.with_current(|| handler(&routed.event));
                }
                if Some(routed.window_id) == self.main_window()
                    && let Some(handler) = callback(&self.0.callbacks.on_event)
                {
                    session.with_current(|| handler(&routed.event));
                }
            }
            session.request_display();
        }
        if callback(&self.0.callbacks.external_display_request_drain).is_some_and(|f| f()) {
            for session in self.sessions() {
                session.request_display();
            }
        }
        for id in self.window_ids() {
            self.synchronize_presentation(id);
        }
        let mut work = std::mem::take(&mut *self.0.local_work.borrow_mut());
        while let Ok(item) = self.0.work_receiver.borrow_mut().try_recv() {
            work.push(item);
        }
        if !work.is_empty() {
            for item in work {
                item();
            }
            for session in self.sessions() {
                session.request_display();
            }
        }
        let sessions = self.sessions();
        let has_work = sessions.iter().any(|s| {
            s.needs_display.get()
                || s.tree.borrow().has_render_updates()
                || s.recomposer.has_pending()
        });
        let scheduler = AnimatorScheduler::current();
        let animations_were_active = scheduler.has_active_animations();
        if !state.animations_previously_active.get() || !animations_were_active {
            state.last_animation_tick_time.set(start);
        }
        let mode = self.0.drive.mode.get();
        let wants_native = mode == PlatformFrameRateMode::DisplayRefresh
            || (mode == PlatformFrameRateMode::EventDriven && animations_were_active);
        let keep_occluded = sessions
            .iter()
            .any(|s| s.keep_rendering.get() && !s.presentation_active.get());
        let mut native_windows = HashSet::new();
        let mut ticks = HashMap::new();
        for session in &sessions {
            let enabled =
                wants_native && (session.presentation_active.get() || session.keep_rendering.get());
            if shell
                .borrow()
                .set_display_refresh_enabled(session.id, enabled)
                && enabled
            {
                native_windows.insert(session.id);
                if let Some(tick) = shell.borrow().take_display_refresh(session.id) {
                    ticks.insert(session.id, tick);
                }
            }
        }
        let active = sessions
            .iter()
            .filter(|s| s.presentation_active.get())
            .count();
        let needs_fallback = wants_native && native_windows.len() < active;
        if needs_fallback && !state.reported_fallback.replace(true) {
            crate::log::log(
                cove_logging::Level::Warning,
                "native display refresh unavailable; using fixed-rate deadline fallback",
            );
        }
        let fallback_interval = match mode {
            PlatformFrameRateMode::Fixed(fps) if active > 0 => Some(1.0 / fps),
            _ if keep_occluded || needs_fallback => Some(1.0 / self.fallback_refresh_rate()),
            _ => None,
        };
        let fallback_due = {
            let mut deadline = state.fallback_deadline.borrow_mut();
            deadline.update(fallback_interval, start);
            deadline.consume_if_due(start)
        };
        let animation_time = ticks
            .values()
            .map(|t| t.target_timestamp)
            .filter(|t| t.is_finite())
            .fold(None, |value: Option<f64>, next| {
                Some(value.map_or(next, |v| v.max(next)))
            })
            .unwrap_or(start);
        let cadence = mode != PlatformFrameRateMode::EventDriven;
        let frame_due = !ticks.is_empty() || fallback_due;
        let prepare = frame_due || (!cadence && has_work);
        if cadence {
            for session in &sessions {
                if ticks.contains_key(&session.id)
                    || (!native_windows.contains(&session.id) && fallback_due)
                {
                    session.request_display();
                }
            }
        }
        if prepare {
            let preparation_time = if cadence { animation_time } else { start };
            let delta = if !cadence {
                loop_delta
            } else if let Some(previous) = state.last_preparation_time.get() {
                (preparation_time - previous).max(0.0)
            } else {
                fallback_interval
                    .or_else(|| {
                        ticks
                            .values()
                            .next()
                            .map(|t| (t.target_timestamp - t.timestamp).max(0.0))
                    })
                    .unwrap_or(0.0)
            };
            state.last_preparation_time.set(Some(
                state
                    .last_preparation_time
                    .get()
                    .unwrap_or(preparation_time)
                    .max(preparation_time),
            ));
            if let Some(handler) = callback(&self.0.callbacks.on_before_commit) {
                handler(delta);
            }
        }
        if frame_due || (prepare && !animations_were_active) {
            scheduler.tick(scale_animation_delta(
                (animation_time - state.last_animation_tick_time.get()).max(0.0),
            ));
            state
                .last_animation_tick_time
                .set(state.last_animation_tick_time.get().max(animation_time));
        }
        if prepare {
            for session in &sessions {
                let committed = session.with_current(|| session.recomposer.commit_all());
                WindowRevealContext::clear_activation(session.tree.borrow().root().as_ref());
                if committed && session.tree.borrow().has_render_updates() {
                    session.request_display();
                }
            }
        }
        let animations_active = scheduler.has_active_animations();
        state.animations_previously_active.set(animations_active);
        for session in self.sessions() {
            let Some(handle) = shell.borrow().window(session.id) else {
                continue;
            };
            let is_main = Some(session.id) == self.main_window();
            if session.update_metrics(&*handle) {
                session.request_display();
                session.set_gate(RenderGate::Due);
                let (w, h) = session.drawable_size();
                if let Some(handler) = callback(&session.callbacks.on_resize) {
                    handler(w, h);
                }
                if is_main && let Some(handler) = callback(&self.0.callbacks.on_resize) {
                    handler(w, h);
                }
            }
            if !session.did_call_on_init.get()
                && session.presentation_active.get()
                && let Some(surface) = handle.render_surface()
            {
                session.did_call_on_init.set(true);
                session.request_display();
                let (w, h) = session.drawable_size();
                if let Some(handler) = callback(&session.callbacks.on_init) {
                    handler(surface, w, h);
                }
                if is_main && let Some(handler) = callback(&self.0.callbacks.on_init) {
                    handler(surface, w, h);
                }
            }
            let dirty = session.tree.borrow().has_render_updates();
            let native = native_windows.contains(&session.id);
            let tick = ticks.contains_key(&session.id);
            if native && tick {
                session.set_gate(RenderGate::Due);
            }
            let should_render = (session.presentation_active.get() || session.keep_rendering.get())
                && (session.needs_display.get() || dirty)
                && session.gate_is_due(start)
                && (!cadence || tick || (!native && fallback_due))
                && (cadence || !native || tick || session.needs_display.get());
            if should_render && let Some(surface) = handle.render_surface() {
                session.needs_display.set(false);
                let local = callback(&session.callbacks.on_frame);
                let main = if is_main {
                    callback(&self.0.callbacks.on_frame)
                } else {
                    None
                };
                let mut outcome = if local.is_none() && main.is_none() {
                    FrameOutcome::Rendered
                } else {
                    FrameOutcome::Skipped
                };
                if let Some(handler) = local {
                    outcome = outcome.merge(handler(surface));
                }
                if let Some(handler) = main {
                    outcome = outcome.merge(handler(surface));
                }
                match outcome {
                    FrameOutcome::Rendered => {
                        session.note_frame_rendered();
                        session.with_current(|| session.tree.borrow_mut().flush());
                    }
                    FrameOutcome::Skipped => {
                        // Pacing, not failure: every slot was in flight or the
                        // renderer was not ready. Retry one interval out, and
                        // the renderer's own backpressure wake makes that
                        // interval the upper bound rather than the only chance.
                        session.request_display();
                        let hz = display_hz(shell.clone(), session.id);
                        session.set_gate(RenderGate::after(self.now(), 1.0 / hz));
                    }
                    FrameOutcome::Failed => {
                        session.request_display();
                        let hz = display_hz(shell.clone(), session.id);
                        if native {
                            // The tick is the retry source. `release_tick_gates`
                            // guarantees one still exists whenever ticks stop.
                            session.park_on_tick();
                        } else {
                            session.set_gate(RenderGate::after(self.now(), 1.0 / hz));
                        }
                    }
                }
            }
            let area = session.focused_text_area();
            if !session.local_text_focus.get() && area != session.last_text_input_area.get() {
                shell.borrow().set_text_input_area(session.id, area);
                session.last_text_input_area.set(area);
            }
            self.schedule_focused_text_refresh(&session);
        }
        self.prune_closed_sessions();
        if wait && self.0.running.get() {
            if mode == PlatformFrameRateMode::EventDriven && !animations_active {
                state.fallback_deadline.borrow_mut().update(None, start);
                for id in &native_windows {
                    shell.borrow().set_display_refresh_enabled(*id, false);
                }
                // Ticks just stopped, so anything parked on one has lost its
                // wake source. Release it here, where that fact is known.
                for session in self.sessions() {
                    if session.release_tick_gate() {
                        crate::log::log(
                            cove_logging::Level::Warning,
                            "released a render gate parked on a display tick that was switched off",
                        );
                    }
                }
            }
            let now = self.now();
            // Stall watchdog. A display request that produces no frame for this
            // long is parked on a wake that never arrived. Nothing should reach
            // that state; if something does, a forced redraw beats a frozen
            // window, and the warning names the window so the cause is findable.
            const STALL_WATCHDOG_SECS: f64 = 0.75;
            for session in self.sessions() {
                if !(session.presentation_active.get() || session.keep_rendering.get()) {
                    continue;
                }
                if !session.needs_display.get() {
                    session.stall_since.set(None);
                    continue;
                }
                let since = session.stall_since.get().unwrap_or(now);
                session.stall_since.set(Some(since));
                if now - since < STALL_WATCHDOG_SECS {
                    continue;
                }
                let was_parked = session.release_tick_gate();
                session.set_gate(RenderGate::Due);
                session.stall_since.set(Some(now));
                crate::log::log(
                    cove_logging::Level::Warning,
                    &format!(
                        "render stall watchdog: window {:?} asked for a frame {:.0}ms ago and produced none{}; forcing a redraw",
                        session.id,
                        (now - since) * 1000.0,
                        if was_parked {
                            " (it was parked on a display tick)"
                        } else {
                            ""
                        }
                    ),
                );
            }
            let starts_animation = mode == PlatformFrameRateMode::EventDriven
                && !animations_were_active
                && scheduler.has_active_animations();
            let immediate = starts_animation
                || self.sessions().iter().any(|s| {
                    !cadence
                        && (s.recomposer.has_pending()
                            || (s.presentation_active.get()
                                && (s.needs_display.get()
                                    || (s.tree.borrow().has_render_updates()
                                        && !native_windows.contains(&s.id)))
                                && s.gate_is_due(now)))
                });
            let mut timeout = if immediate {
                0.0
            } else {
                self.event_wait_timeout(now)
            };
            if let Some(remaining) = state.fallback_deadline.borrow().remaining(now) {
                timeout = timeout.min(remaining);
            }
            for session in self.sessions() {
                if session.presentation_active.get()
                    && !cadence
                    && !native_windows.contains(&session.id)
                    && (session.needs_display.get() || session.tree.borrow().has_render_updates())
                    && let Some(deadline) = session.gate_deadline()
                {
                    timeout = timeout.min((deadline - now).max(0.001));
                }
            }
            if timeout > 0.0 && timeout.is_finite() {
                shell
                    .borrow()
                    .wait_for_events(Duration::from_secs_f64(timeout.min(0.25)));
            }
        }
    }
    fn fallback_refresh_rate(&self) -> f64 {
        self.current_display_refresh_rate(self.main_window())
            .filter(|hz| *hz >= 30.0)
            .unwrap_or(60.0)
    }
    fn event_wait_timeout(&self, now: f64) -> f64 {
        let mut timeout: f64 = 0.25;
        for session in self.sessions() {
            if session.focused_text_area().is_some() {
                let last = session.last_text_cursor_tick.get();
                timeout = timeout.min(if last > 0.0 {
                    (0.25 - (now - last)).max(0.001)
                } else {
                    0.25
                });
            }
        }
        timeout
    }
    fn schedule_focused_text_refresh(&self, session: &PlatformWindowSession) {
        if session.focused_text_area().is_none() {
            session.last_text_cursor_tick.set(0.0);
            return;
        }
        let now = self.now();
        let previous = session.last_text_cursor_tick.get();
        if previous == 0.0 {
            session.last_text_cursor_tick.set(now);
            return;
        }
        if now - previous >= 0.25 {
            session.last_text_cursor_tick.set(now);
            session.request_display();
            if let Some(node) = session.input_context.focus_chain.focused() {
                node.mark_render_dirty(InvalidationSource::StyleSet {
                    field: "textFieldCaretBlink".into(),
                });
            }
        }
    }
    pub(super) fn finish_run_loop(&self) {
        if self.0.finishing.replace(true) {
            return;
        }
        self.0.running.set(false);
        #[cfg(target_os = "ios")]
        {
            let driver = self.0.driver.borrow_mut().take();
            drop(driver);
        }
        if let Some(handler) = callback(&self.0.callbacks.on_teardown) {
            handler();
        }
        let shell = self.0.shell.borrow_mut().take();
        if let Some(shell) = shell {
            *self.0.startup_error.borrow_mut() = shell.borrow_mut().take_startup_error();
            shell.borrow_mut().shutdown();
        }
        self.0.sessions.borrow_mut().clear();
        self.0.order.borrow_mut().clear();
        self.0.main.set(None);
        crate::log::log(cove_logging::Level::Info, "stopped");
    }
}
impl PlatformHost for Sdl3PlatformHost {
    fn run(&self, tree: Rc<RefCell<NodeTree>>) -> Result<(), ShellError> {
        self.ensure_main(tree)?;
        self.run_loop();
        match self.0.startup_error.borrow_mut().take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    fn stop(&self) {
        self.0.running.set(false);
        self.wake_event_loop();
    }
    fn is_running(&self) -> bool {
        self.0.running.get()
    }
}
