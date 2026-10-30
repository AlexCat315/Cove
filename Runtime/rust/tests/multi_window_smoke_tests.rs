//! Translation of `Tests/CoveUIRuntimeTests/MultiWindowSmokeTests.swift`.
//!
//! The Swift suite drives `SDL3PlatformHost` through a `MockShell` double. The
//! Rust host takes the same shape: a `Shell` implementation owned by the host,
//! one `PlatformWindowSession` per open window, and a run loop that polls the
//! shell, commits recompositions, ticks the task-local `AnimatorScheduler` and
//! renders through per-window `on_frame` callbacks.
//!
//! Behaviour checklist:
//! - committing an unchanged composition does not submit another frame
//! - an entirely hidden fixed-rate host waits for events instead of spinning
//! - hidden windows suspend synchronously and pending frames cannot reactivate
//! - each routed event reaches only its own window session, with its holders
//!   installed for the callback
//! - cursor requests are routed to the matching native window
//! - render-dirty nodes trigger a frame without global animation forcing
//! - window input events request a redraw after the initial frame
//! - external display requests redraw every open window
//! - event-driven hosts wait for events when no frame is pending
//! - failed frame callbacks keep the window pending for retry
//! - unavailable surfaces preserve dirty work and throttle repeated attempts
//! - an OS close request releases window resources before native destroy
//! - an OS close veto keeps the native window alive
//! - the focused text input area syncs to the shell from the live node

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use cove_core::scene::{
    EventResult, FocusChainHolder, InputHandlerRoute, InteractionRegistry,
    InteractionRegistryHolder, Node, NodeTree, PlatformInputContext, PointerCaptureHolder,
    PointerPhase, Recomposer, TextInputAttachmentKey,
};
use cove_core::Rect;
use cove_platform::{
    InputEvent, MouseButton, MouseButtonEvent, MouseMotionEvent, SystemCursor, TextInputArea,
    WindowId, WindowInputEvent,
};
use cove_runtime::{PlatformFrameRateMode, PlatformWindowSession, Sdl3PlatformHost};
use cove_shell::{
    DialogCompletion, DisplayRefreshTick, FileDialogFilter, NativeRenderSurface,
    NativeWindowReference, Shell, ShellError, WindowChromeHitTest, WindowHandle, WindowOptions,
    WindowSafeArea,
};

/// Process-wide serialization: the host installs the scene invalidation wake
/// hook and per-window input holders, both process-global.
struct GlobalTestLock;

impl GlobalTestLock {
    fn mutex() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn locked<T>(body: impl FnOnce() -> T) -> T {
        let _guard: MutexGuard<'_, ()> = Self::mutex()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        body()
    }
}

const MOCK_SURFACE: NativeRenderSurface = NativeRenderSurface::MetalLayer(1);
const IDLE_WAIT: Duration = Duration::from_millis(250);

struct MockWindowHandle {
    id: WindowId,
    render_surface: Cell<Option<NativeRenderSurface>>,
    size: (u32, u32),
    is_visible: Cell<bool>,
}

impl MockWindowHandle {
    fn new(id: WindowId, size: (u32, u32)) -> Self {
        Self { id, render_surface: Cell::new(None), size, is_visible: Cell::new(true) }
    }
}

impl WindowHandle for MockWindowHandle {
    fn id(&self) -> WindowId {
        self.id
    }
    fn native_window_reference(&self) -> Option<NativeWindowReference> {
        None
    }
    fn render_surface(&self) -> Option<NativeRenderSurface> {
        self.render_surface.get()
    }
    fn safe_area(&self) -> WindowSafeArea {
        WindowSafeArea::default()
    }
    fn drawable_size(&self) -> (u32, u32) {
        self.size
    }
    fn logical_size(&self) -> (u32, u32) {
        self.size
    }
    fn content_scale_factor(&self) -> f32 {
        1.0
    }
    fn is_focused(&self) -> bool {
        true
    }
    fn is_visible(&self) -> bool {
        self.is_visible.get()
    }
    fn is_minimized(&self) -> bool {
        false
    }
    fn is_occluded(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct NativeRefresh {
    supported: bool,
    pending: HashMap<WindowId, DisplayRefreshTick>,
    requests: HashMap<WindowId, Vec<bool>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct CursorRequest {
    window_id: WindowId,
    cursor: SystemCursor,
}

type PollHook = Box<dyn FnMut()>;
type WaitHook = Box<dyn FnMut(Duration) -> bool>;
type CloseHook = Box<dyn FnMut(Option<WindowId>) -> bool>;

struct ShellState {
    event_batches: Vec<Vec<WindowInputEvent>>,
    refresh_rate: Option<f64>,
    on_poll: Option<PollHook>,
    on_wait: Option<WaitHook>,
    wait_timeouts: Vec<Duration>,
    cursor_requests: Vec<CursorRequest>,
    text_input_areas: HashMap<WindowId, Option<TextInputArea>>,
    native_refresh: NativeRefresh,
    handles: HashMap<WindowId, Rc<MockWindowHandle>>,
    order: Vec<WindowId>,
    next_window_id: u32,
    poll_index: usize,
    loop_running: bool,
    close_interceptor: Option<CloseHook>,
    /// Work the shell runs once the host releases its exclusive borrow.
    deferred: Vec<Box<dyn FnOnce()>>,
}

/// The shell double. The host owns a `Box<dyn Shell>`, so the observable state
/// lives behind a shared handle the test also holds.
#[derive(Clone)]
struct MockShell(Rc<RefCell<ShellState>>);

impl MockShell {
    fn new(event_batches: Vec<Vec<WindowInputEvent>>) -> Self {
        Self(Rc::new(RefCell::new(ShellState {
            event_batches,
            refresh_rate: None,
            on_poll: None,
            on_wait: None,
            wait_timeouts: Vec::new(),
            cursor_requests: Vec::new(),
            text_input_areas: HashMap::new(),
            native_refresh: NativeRefresh::default(),
            handles: HashMap::new(),
            order: Vec::new(),
            next_window_id: 1,
            poll_index: 0,
            loop_running: true,
            close_interceptor: None,
            deferred: Vec::new(),
        })))
    }

    fn window(&self, id: WindowId) -> Option<Rc<MockWindowHandle>> {
        self.0.borrow().handles.get(&id).cloned()
    }

    fn set_surface(&self, id: WindowId, surface: NativeRenderSurface) {
        if let Some(handle) = self.window(id) {
            handle.render_surface.set(Some(surface));
        }
    }

    fn wait_timeouts(&self) -> Vec<Duration> {
        self.0.borrow().wait_timeouts.clone()
    }

    fn cursor_requests(&self) -> Vec<CursorRequest> {
        self.0.borrow().cursor_requests.clone()
    }

    fn set_poll(&self, hook: PollHook) {
        self.0.borrow_mut().on_poll = Some(hook);
    }

    fn set_wait(&self, hook: WaitHook) {
        self.0.borrow_mut().on_wait = Some(hook);
    }

    /// Queues `work` for the host's post-poll deferred drain. The host holds an
    /// exclusive borrow across `poll_window_events`, so hooks that need the host
    /// must not call it inline.
    fn defer(&self, work: impl FnOnce() + 'static) {
        self.0.borrow_mut().deferred.push(Box::new(work));
    }

    fn request_close(&self, id: WindowId) -> bool {
        let mut state = self.0.borrow_mut();
        match state.close_interceptor.as_mut() {
            Some(interceptor) => interceptor(Some(id)),
            None => true,
        }
    }

    fn set_batches(&self, batches: Vec<Vec<WindowInputEvent>>) {
        let mut state = self.0.borrow_mut();
        state.event_batches = batches;
        state.poll_index = 0;
        state.loop_running = true;
    }
}

impl Shell for MockShell {
    fn main_window_id(&self) -> Option<WindowId> {
        self.0.borrow().order.first().copied()
    }
    fn window_ids(&self) -> Vec<WindowId> {
        let state = self.0.borrow();
        state.order.iter().copied().filter(|id| state.handles.contains_key(id)).collect()
    }
    fn is_running(&self) -> bool {
        let state = self.0.borrow();
        state.loop_running && !state.handles.is_empty()
    }
    fn set_close_interceptor(&mut self, interceptor: Option<CloseHook>) {
        self.0.borrow_mut().close_interceptor = interceptor;
    }
    fn create_window(
        &mut self,
        _title: &str,
        options: WindowOptions,
    ) -> Result<Rc<dyn WindowHandle>, ShellError> {
        let mut state = self.0.borrow_mut();
        let id = WindowId(state.next_window_id);
        state.next_window_id += 1;
        let size = (options.width.max(1) as u32, options.height.max(1) as u32);
        let handle = Rc::new(MockWindowHandle::new(id, size));
        state.handles.insert(id, handle.clone());
        state.order.push(id);
        Ok(handle)
    }
    fn window(&self, id: WindowId) -> Option<Rc<dyn WindowHandle>> {
        let handle = self.0.borrow().handles.get(&id).cloned()?;
        Some(handle)
    }
    fn destroy_window(&mut self, id: WindowId) {
        let mut state = self.0.borrow_mut();
        state.handles.remove(&id);
        state.order.retain(|candidate| *candidate != id);
        if state.handles.is_empty() {
            state.loop_running = false;
        }
    }
    fn poll_window_events(&mut self) -> Vec<WindowInputEvent> {
        let hook = self.0.borrow_mut().on_poll.take();
        if let Some(mut hook) = hook {
            hook();
            self.0.borrow_mut().on_poll = Some(hook);
        }
        let mut state = self.0.borrow_mut();
        let index = state.poll_index;
        state.poll_index += 1;
        if state.poll_index >= state.event_batches.len() {
            state.loop_running = false;
        }
        state.event_batches.get(index).cloned().unwrap_or_default()
    }
    fn wait_for_events(&self, timeout: Duration) -> bool {
        let hook = self.0.borrow_mut().on_wait.take();
        self.0.borrow_mut().wait_timeouts.push(timeout);
        let signalled = match hook {
            Some(mut hook) => {
                let signalled = hook(timeout);
                self.0.borrow_mut().on_wait = Some(hook);
                signalled
            }
            None => false,
        };
        signalled
    }
    fn take_deferred_callbacks(&self) -> Vec<Box<dyn FnOnce()>> {
        std::mem::take(&mut self.0.borrow_mut().deferred)
    }
    fn wake_event_loop(&self) {}
    fn set_text_input_area(&self, id: WindowId, area: Option<TextInputArea>) {
        self.0.borrow_mut().text_input_areas.insert(id, area);
    }
    fn set_cursor(&self, id: WindowId, cursor: SystemCursor) {
        self.0.borrow_mut().cursor_requests.push(CursorRequest { window_id: id, cursor });
    }
    fn global_pointer_position(&self) -> Option<(f32, f32)> {
        None
    }
    fn window_position(&self, _id: WindowId) -> Option<(f32, f32)> {
        None
    }
    fn set_window_position(&self, _id: WindowId, _x: f32, _y: f32) {}
    fn set_window_size(&self, _id: WindowId, _width: f32, _height: f32) {}
    fn raise_window(&self, _id: WindowId) {}
    fn minimize_window(&self, _id: WindowId) {}
    fn maximize_window(&self, _id: WindowId) {}
    fn restore_window(&self, _id: WindowId) {}
    fn hide_window(&self, id: WindowId) {
        if let Some(handle) = self.window(id) {
            handle.is_visible.set(false);
        }
    }
    fn show_window(&self, id: WindowId) {
        if let Some(handle) = self.window(id) {
            handle.is_visible.set(true);
        }
    }
    fn is_window_maximized(&self, _id: WindowId) -> bool {
        false
    }
    fn show_window_system_menu(&self, _id: WindowId, _x: f32, _y: f32) {}
    fn set_window_chrome_hit_test(&self, _id: WindowId, _hit: Option<&WindowChromeHitTest>) {}
    fn show_open_folder_dialog(
        &self,
        _id: Option<WindowId>,
        _path: Option<&str>,
        accept: Box<dyn FnOnce(Option<String>)>,
    ) {
        accept(None);
    }
    fn show_open_file_dialog(
        &self,
        _id: Option<WindowId>,
        _path: Option<&str>,
        _filters: &[FileDialogFilter],
        _multiple: bool,
        accept: DialogCompletion,
    ) {
        accept(Vec::new());
    }
    fn display_refresh_rate(&self, _id: Option<WindowId>) -> Option<f64> {
        self.0.borrow().refresh_rate
    }
    fn set_display_refresh_enabled(&self, id: WindowId, enabled: bool) -> bool {
        let mut state = self.0.borrow_mut();
        let refresh = &mut state.native_refresh;
        refresh.requests.entry(id).or_default().push(enabled);
        if !enabled {
            refresh.pending.remove(&id);
        }
        refresh.supported
    }
    fn take_display_refresh(&self, id: WindowId) -> Option<DisplayRefreshTick> {
        self.0.borrow_mut().native_refresh.pending.remove(&id)
    }
    fn shutdown(&mut self) {
        let mut state = self.0.borrow_mut();
        state.handles.clear();
        state.order.clear();
        state.loop_running = false;
    }
}

/// The host plus the shell double it was built from.
struct Harness {
    host: Sdl3PlatformHost,
    shell: MockShell,
}

fn harness(batch_count: usize) -> Harness {
    let shell = MockShell::new(vec![Vec::new(); batch_count]);
    let factory_shell = shell.clone();
    let host = Sdl3PlatformHost::with_services(
        "CoveUI",
        WindowOptions::default(),
        Rc::new(Recomposer::new()),
        Rc::new(PlatformInputContext::new()),
        move || Ok(Box::new(factory_shell.clone()) as Box<dyn Shell>),
    );
    Harness { host, shell }
}

impl Harness {
    fn open(&self, title: &str) -> Rc<PlatformWindowSession> {
        self.host
            .open_window(
                title,
                Rc::new(RefCell::new(NodeTree::new())),
                Rc::new(Recomposer::new()),
                Rc::new(PlatformInputContext::new()),
                WindowOptions::default(),
            )
            .expect("the mock shell opens a window")
    }

    fn open_tree(&self, title: &str, tree: &Rc<RefCell<NodeTree>>) -> Rc<PlatformWindowSession> {
        self.host
            .open_window(
                title,
                tree.clone(),
                Rc::new(Recomposer::new()),
                Rc::new(PlatformInputContext::new()),
                WindowOptions::default(),
            )
            .expect("the mock shell opens a window")
    }

    fn run(&self) {
        self.host.run_open_windows().expect("the mock shell keeps the loop runnable");
    }
}

fn left_down(x: f32, y: f32) -> MouseButtonEvent {
    MouseButtonEvent {
        button: MouseButton::Left,
        x,
        y,
        clicks: 1,
        modifiers: Default::default(),
        kind: Default::default(),
    }
}

fn motion(x: f32, y: f32, delta_x: f32, delta_y: f32) -> InputEvent {
    InputEvent::MouseMotion(MouseMotionEvent {
        x,
        y,
        delta_x,
        delta_y,
        kind: Default::default(),
    })
}

fn count_frames(session: &Rc<PlatformWindowSession>) -> Rc<Cell<usize>> {
    let frames = Rc::new(Cell::new(0));
    let counter = frames.clone();
    *session.callbacks.on_frame.borrow_mut() = Some(Rc::new(move |_: NativeRenderSurface| {
        counter.set(counter.get() + 1);
        true
    }));
    frames
}

fn hit_tree(interactions: &InteractionRegistry, on_pointer_down: impl Fn() + 'static) -> Node {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    let leaf = Node::new();
    leaf.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    leaf.set_is_hit_testable(true);
    interactions.set_pointer(
        &leaf,
        InputHandlerRoute::control(),
        Rc::new(move |_: &MouseButtonEvent, phase: PointerPhase, _| {
            if phase == PointerPhase::Down {
                on_pointer_down();
            }
            EventResult::Handled
        }),
    );
    root.add_child(&leaf);
    root
}

fn cursor_tree(cursor: SystemCursor) -> Node {
    let root = Node::new();
    root.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    let leaf = Node::new();
    leaf.set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    leaf.set_is_hit_testable(true);
    leaf.set_cursor(Some(cursor));
    root.add_child(&leaf);
    root
}

#[test]
fn unchanged_commit_does_not_request_frame() {
    GlobalTestLock::locked(|| {
        let harness = harness(4);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let root = Node::new();
        tree.borrow_mut().set_root(Some(root.clone()));
        let session = harness.open_tree("A", &tree);
        harness.shell.set_surface(session.id, MOCK_SURFACE);
        let frames = count_frames(&session);

        let iteration = Cell::new(0);
        let poll_session = session.clone();
        let scope = root.id().0 as usize;
        harness.shell.set_poll(Box::new(move || {
            iteration.set(iteration.get() + 1);
            if iteration.get() == 2 {
                poll_session.recomposer.invalidate(scope, None, None, || {});
            }
            if iteration.get() == 3 {
                let target = root.clone();
                poll_session.recomposer.invalidate(scope, None, None, move || {
                    target.set_opacity(0.5);
                });
            }
        }));

        harness.run();
        assert_eq!(frames.get(), 2, "an unchanged commit submits no extra frame");
    });
}

#[test]
fn hidden_fixed_rate_host_sleeps() {
    GlobalTestLock::locked(|| {
        let harness = harness(4);
        let session = harness.open("A");
        harness.host.set_frame_rate_mode(PlatformFrameRateMode::Fixed(60.0));

        let iteration = Cell::new(0);
        let host = harness.host.clone();
        let shell = harness.shell.clone();
        let id = session.id;
        harness.shell.set_poll(Box::new(move || {
            iteration.set(iteration.get() + 1);
            if iteration.get() == 2 {
                let hide_host = host.clone();
                shell.defer(move || hide_host.hide_window(id));
            }
        }));

        harness.run();
        assert_eq!(
            harness.shell.wait_timeouts().last().copied(),
            Some(IDLE_WAIT),
            "the hidden fixed-rate host waits for events",
        );
    });
}

#[test]
fn presentation_lifecycle_is_per_window() {
    GlobalTestLock::locked(|| {
        let harness = harness(4);
        let first = harness.open("A");
        let second = harness.open("B");
        harness.shell.set_surface(first.id, MOCK_SURFACE);
        harness.shell.set_surface(second.id, MOCK_SURFACE);

        let transitions: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));
        let log = transitions.clone();
        *first.callbacks.on_presentation_changed.borrow_mut() =
            Some(Rc::new(move |active: bool| log.borrow_mut().push(active)));
        let frames = count_frames(&first);
        let sibling_frames = count_frames(&second);

        let suspended_at_hide = Rc::new(Cell::new(None));
        let suspended_hook = suspended_at_hide.clone();
        let iteration = Rc::new(Cell::new(0));
        let poll_iteration = iteration.clone();
        let host = harness.host.clone();
        let first_id = first.id;
        let first_session = first.clone();
        let second_session = second.clone();
        let shell = harness.shell.clone();
        harness.shell.set_poll(Box::new(move || {
            poll_iteration.set(poll_iteration.get() + 1);
            if poll_iteration.get() == 2 {
                let hide_host = host.clone();
                let probe = first_session.clone();
                let probe_log = suspended_hook.clone();
                shell.defer(move || {
                    hide_host.hide_window(first_id);
                    probe_log.set(Some(!probe.is_presentation_active()));
                });
            }
            if poll_iteration.get() == 4 {
                let show_host = host.clone();
                shell.defer(move || show_host.show_window(first_id));
            }
            first_session.request_display();
            second_session.request_display();
        }));

        harness.run();
        assert_eq!(*transitions.borrow(), [false, true], "presentation suspends then restores");
        assert_eq!(
            suspended_at_hide.get(),
            Some(true),
            "hiding suspends presentation synchronously",
        );
        assert_eq!(frames.get(), 2, "the hidden window skips frames while suspended");
        assert_eq!(sibling_frames.get(), 4, "the sibling window keeps rendering");
    });
}

#[test]
fn routes_events_per_window() {
    GlobalTestLock::locked(|| {
        let harness = harness(1);
        let tree_a = Rc::new(RefCell::new(NodeTree::new()));
        let tree_b = Rc::new(RefCell::new(NodeTree::new()));
        let hits_a = Rc::new(Cell::new(0));
        let hits_b = Rc::new(Cell::new(0));

        let session_a = harness.open_tree("A", &tree_a);
        let session_b = harness.open_tree("B", &tree_b);

        let a_counter = hits_a.clone();
        tree_a.borrow_mut().set_root(Some(hit_tree(&session_a.interactions(), move || {
            a_counter.set(a_counter.get() + 1)
        })));
        let b_counter = hits_b.clone();
        tree_b.borrow_mut().set_root(Some(hit_tree(&session_b.interactions(), move || {
            b_counter.set(b_counter.get() + 1)
        })));

        let context_a = Rc::new(Cell::new(false));
        let expected = (
            session_a.interactions(),
            session_a.focus_chain(),
            session_a.pointer_capture(),
        );
        let flag = context_a.clone();
        *session_a.callbacks.on_event.borrow_mut() = Some(Rc::new(move |_: &InputEvent| {
            let matches = InteractionRegistryHolder::current()
                .is_some_and(|current| Rc::ptr_eq(&current, &expected.0))
                && FocusChainHolder::current().is_some_and(|current| Rc::ptr_eq(&current, &expected.1))
                && PointerCaptureHolder::current()
                    .is_some_and(|current| Rc::ptr_eq(&current, &expected.2));
            flag.set(matches);
        }));

        let context_b = Rc::new(Cell::new(false));
        let expected = (
            session_b.interactions(),
            session_b.focus_chain(),
            session_b.pointer_capture(),
        );
        let flag = context_b.clone();
        *session_b.callbacks.on_event.borrow_mut() = Some(Rc::new(move |_: &InputEvent| {
            let matches = InteractionRegistryHolder::current()
                .is_some_and(|current| Rc::ptr_eq(&current, &expected.0))
                && FocusChainHolder::current().is_some_and(|current| Rc::ptr_eq(&current, &expected.1))
                && PointerCaptureHolder::current()
                    .is_some_and(|current| Rc::ptr_eq(&current, &expected.2));
            flag.set(matches);
        }));

        harness.shell.set_batches(vec![vec![
            WindowInputEvent {
                window_id: session_a.id,
                event: InputEvent::MouseButtonDown(left_down(12.0, 12.0)),
            },
            WindowInputEvent {
                window_id: session_b.id,
                event: InputEvent::MouseButtonDown(left_down(18.0, 18.0)),
            },
        ]]);

        harness.run();

        assert_eq!(hits_a.get(), 1, "window A receives its own routed event");
        assert_eq!(hits_b.get(), 1, "window B receives its own routed event");
        assert!(context_a.get(), "window A's holders are current for its callback");
        assert!(context_b.get(), "window B's holders are current for its callback");
    });
}

#[test]
fn routes_cursor_per_window() {
    GlobalTestLock::locked(|| {
        let harness = harness(0);
        let tree_a = Rc::new(RefCell::new(NodeTree::new()));
        let tree_b = Rc::new(RefCell::new(NodeTree::new()));
        let session_a = harness.open_tree("A", &tree_a);
        let session_b = harness.open_tree("B", &tree_b);

        tree_a.borrow_mut().set_root(Some(cursor_tree(SystemCursor::Pointer)));
        tree_b.borrow_mut().set_root(Some(cursor_tree(SystemCursor::Move)));

        harness.shell.set_batches(vec![vec![
            WindowInputEvent { window_id: session_a.id, event: motion(16.0, 16.0, 1.0, 1.0) },
            WindowInputEvent { window_id: session_b.id, event: motion(16.0, 16.0, 1.0, 1.0) },
        ]]);

        harness.run();

        let requests = harness.shell.cursor_requests();
        assert!(
            requests
                .iter()
                .any(|request| request.window_id == session_a.id && request.cursor == SystemCursor::Pointer),
            "window A resolves the pointer cursor",
        );
        assert!(
            requests
                .iter()
                .any(|request| request.window_id == session_b.id && request.cursor == SystemCursor::Move),
            "window B resolves the move cursor",
        );
    });
}

#[test]
fn render_dirty_triggers_frame() {
    GlobalTestLock::locked(|| {
        let harness = harness(3);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let session = harness.open_tree("A", &tree);
        let root = Node::new();
        let leaf = Node::new();
        root.add_child(&leaf);
        tree.borrow_mut().set_root(Some(root));
        harness.shell.set_surface(session.id, MOCK_SURFACE);

        let frames = Rc::new(Cell::new(0));
        let counter = frames.clone();
        let frame_leaf = leaf.clone();
        let frame_session = session.clone();
        *session.callbacks.on_frame.borrow_mut() = Some(Rc::new(move |_: NativeRenderSurface| {
            counter.set(counter.get() + 1);
            if counter.get() == 1 {
                frame_leaf.set_opacity(0.5);
                frame_session.request_display();
            }
            true
        }));

        harness.run();
        assert_eq!(frames.get(), 2, "a render-dirty node requests the next frame");
    });
}

#[test]
fn input_events_request_redraw() {
    GlobalTestLock::locked(|| {
        let harness = harness(3);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let session = harness.open_tree("A", &tree);
        tree.borrow_mut().set_root(Some(hit_tree(&session.interactions(), || {})));
        harness.shell.set_surface(session.id, MOCK_SURFACE);

        harness.shell.set_batches(vec![
            Vec::new(),
            vec![WindowInputEvent {
                window_id: session.id,
                event: InputEvent::MouseButtonDown(left_down(12.0, 12.0)),
            }],
            Vec::new(),
        ]);
        let frames = count_frames(&session);

        harness.run();
        assert_eq!(frames.get(), 2, "window input requests a redraw after the initial frame");
    });
}

#[test]
fn external_display_requests_redraw_every_window() {
    GlobalTestLock::locked(|| {
        let harness = harness(3);
        let tree_a = Rc::new(RefCell::new(NodeTree::new()));
        let tree_b = Rc::new(RefCell::new(NodeTree::new()));
        let session_a = harness.open_tree("A", &tree_a);
        let session_b = harness.open_tree("B", &tree_b);
        tree_a.borrow_mut().set_root(Some(Node::new()));
        tree_b.borrow_mut().set_root(Some(Node::new()));
        harness.shell.set_surface(session_a.id, MOCK_SURFACE);
        harness.shell.set_surface(session_b.id, MOCK_SURFACE);

        let drain = Rc::new(Cell::new(0));
        let drain_counter = drain.clone();
        *harness.host.callbacks().external_display_request_drain.borrow_mut() =
            Some(Rc::new(move || {
                drain_counter.set(drain_counter.get() + 1);
                drain_counter.get() == 2
            }));

        let frames_a = count_frames(&session_a);
        let frames_b = count_frames(&session_b);

        harness.run();
        assert_eq!(frames_a.get(), 2, "the external request redraws window A");
        assert_eq!(frames_b.get(), 2, "the external request redraws window B");
    });
}

#[test]
fn event_driven_host_waits_when_idle() {
    GlobalTestLock::locked(|| {
        let harness = harness(3);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let session = harness.open_tree("A", &tree);
        tree.borrow_mut().set_root(Some(Node::new()));
        harness.shell.set_surface(session.id, MOCK_SURFACE);
        count_frames(&session);
        harness.host.set_frame_rate_mode(PlatformFrameRateMode::EventDriven);

        harness.run();
        let timeouts = harness.shell.wait_timeouts();
        assert!(!timeouts.is_empty(), "an idle event-driven host waits");
        assert!(timeouts.iter().all(|timeout| !timeout.is_zero()), "idle waits are non-zero");
    });
}

#[test]
fn failed_frame_callbacks_retry() {
    GlobalTestLock::locked(|| {
        let harness = harness(4);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let session = harness.open_tree("A", &tree);
        tree.borrow_mut().set_root(Some(Node::new()));
        harness.shell.set_surface(session.id, MOCK_SURFACE);

        let attempts = Rc::new(Cell::new(0));
        let counter = attempts.clone();
        *session.callbacks.on_frame.borrow_mut() = Some(Rc::new(move |_: NativeRenderSurface| {
            counter.set(counter.get() + 1);
            counter.get() >= 2
        }));

        let time = Rc::new(Cell::new(0.0_f64));
        let clock = time.clone();
        harness.host.set_frame_clock(move || clock.get());
        let tick = time.clone();
        let poll_shell = harness.shell.clone();
        harness.shell.set_poll(Box::new(move || tick.set(tick.get() + 0.01)));

        let delayed_waits = Rc::new(Cell::new(0));
        let recorder = delayed_waits.clone();
        let wait_time = time.clone();
        let wait_shell = poll_shell.clone();
        wait_shell.set_wait(Box::new(move |timeout| {
            if timeout > Duration::from_millis(5) {
                recorder.set(recorder.get() + 1);
            }
            wait_time.set(wait_time.get() + timeout.as_secs_f64());
            false
        }));

        harness.run();
        assert_eq!(attempts.get(), 2, "the failed frame is retried exactly once");
        assert!(delayed_waits.get() > 0, "the retry is throttled by a wait deadline");
    });
}

#[test]
fn unavailable_surface_preserves_dirty_work() {
    GlobalTestLock::locked(|| {
        let harness = harness(4);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let session = harness.open_tree("Occluded", &tree);
        let root = Node::new();
        tree.borrow_mut().set_root(Some(root.clone()));
        root.mark_render_dirty(cove_core::scene::InvalidationSource::StyleSet { field: "test".into() });
        harness.shell.set_surface(session.id, MOCK_SURFACE);

        let time = Rc::new(Cell::new(0.0_f64));
        let clock = time.clone();
        harness.host.set_frame_clock(move || clock.get());
        let tick = time.clone();
        harness.shell.set_poll(Box::new(move || tick.set(tick.get() + 0.002)));

        let attempts = Rc::new(Cell::new(0));
        let counter = attempts.clone();
        *session.callbacks.on_frame.borrow_mut() = Some(Rc::new(move |_: NativeRenderSurface| {
            counter.set(counter.get() + 1);
            false
        }));

        harness.run();
        assert_eq!(attempts.get(), 1, "a failing frame is not retried within the throttle window");
        assert!(tree.borrow().has_render_updates(), "dirty work survives the failed attempt");
        assert!(
            harness
                .shell
                .wait_timeouts()
                .iter()
                .all(|timeout| *timeout >= Duration::from_millis(10)),
            "the retry deadline is at least one refresh period",
        );
    });
}

/// Records whether the native window survives each event-loop iteration. The
/// loop stops once the last window is destroyed, so a released close yields a
/// single sample while a vetoed close keeps sampling to the end of the batch.
fn sample_window_liveness(harness: &Harness, batches: usize, id: WindowId) -> Rc<RefCell<Vec<bool>>> {
    harness.shell.set_batches(vec![Vec::new(); batches]);
    let samples = Rc::new(RefCell::new(Vec::new()));
    let recorder = samples.clone();
    let shell = harness.shell.clone();
    harness.shell.set_poll(Box::new(move || {
        recorder.borrow_mut().push(shell.window(id).is_some());
    }));
    samples
}

#[test]
fn os_close_request_releases_window_resources() {
    GlobalTestLock::locked(|| {
        let harness = harness(4);
        let session = harness.open("A");
        let torn_down = Rc::new(RefCell::new(Vec::new()));
        let log = torn_down.clone();
        *harness.host.callbacks().on_window_teardown.borrow_mut() =
            Some(Rc::new(move |id: WindowId| log.borrow_mut().push(id)));

        assert!(
            !harness.shell.request_close(session.id),
            "the host owns teardown, so the shell must not destroy",
        );
        let samples = sample_window_liveness(&harness, 4, session.id);
        harness.run();

        assert_eq!(*torn_down.borrow(), [session.id], "the host tore the window down");
        assert_eq!(
            samples.borrow().as_slice(),
            [true],
            "the window is gone after the first frame, so the loop cannot continue",
        );
    });
}

#[test]
fn os_close_veto_keeps_native_window_alive() {
    GlobalTestLock::locked(|| {
        let harness = harness(4);
        let session = harness.open("A");
        let torn_down = Rc::new(RefCell::new(Vec::new()));
        let log = torn_down.clone();
        *harness.host.callbacks().on_window_teardown.borrow_mut() =
            Some(Rc::new(move |id: WindowId| log.borrow_mut().push(id)));
        *harness.host.callbacks().window_close_interceptor.borrow_mut() =
            Some(Rc::new(move |_: Option<WindowId>| false));

        assert!(
            !harness.shell.request_close(session.id),
            "a vetoed close never asks the shell to destroy",
        );
        let samples = sample_window_liveness(&harness, 4, session.id);
        harness.run();

        assert!(torn_down.borrow().is_empty(), "a vetoed close tears nothing down");
        assert_eq!(
            samples.borrow().as_slice(),
            [true, true, true, true],
            "the vetoed window survives every loop iteration",
        );
    });
}

#[test]
fn focused_text_input_area_syncs_to_shell() {
    GlobalTestLock::locked(|| {
        let harness = harness(2);
        let tree = Rc::new(RefCell::new(NodeTree::new()));
        let session = harness.open_tree("A", &tree);
        harness.shell.set_surface(session.id, MOCK_SURFACE);

        let root = Node::new();
        let field = Node::new();
        field.set_is_focusable(true);
        let area = TextInputArea { x: 18.0, y: 24.0, width: 12.0, height: 20.0, cursor_x: 0.0 };
        field.set_attachment(TextInputAttachmentKey::AREA, area.clone());
        root.add_child(&field);
        tree.borrow_mut().set_root(Some(root));

        session.focus_chain().focus(Some(&field), false);
        count_frames(&session);

        harness.run();
        assert_eq!(
            harness.shell.0.borrow().text_input_areas.get(&session.id).cloned(),
            Some(Some(area)),
            "the host reads the focused node's text-input area live",
        );
    });
}
