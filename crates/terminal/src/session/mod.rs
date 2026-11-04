//! A terminal emulator bound to one program.
//!
//! Create it once and show it with `TerminalView`, which starts the program when
//! it first appears. The session outlives view updates, so recomposition never
//! restarts the program. Like every CoveUI model it is used from the UI thread;
//! program output is read on a background thread and applied there in one batch
//! per frame.

use crate::engine::{
    SelectionPhase, TerminalEngine, TerminalFrame, TerminalKey, TerminalMouseEvent,
    TerminalPointer, TerminalRGB,
};
use crate::process::{PseudoTerminal, PseudoTerminalSize, TerminalCommand, TerminalError};
use cove_core::scene::LocalInvalidationHandle;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    sync::{Arc, Mutex},
    thread,
};

/// Status of the program the session runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    Idle,
    Running,
    Exited(i32),
    Failed(TerminalError),
}

/// Events the session reports to its host, on the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    TitleChanged(String),
    Bell,
    Exited(i32),
}

pub type EventHandler = Rc<dyn Fn(&SessionEvent)>;

/// Mailbox between the PTY read thread and the UI thread.
///
/// The reader appends and wakes the host; it never touches the engine. `Arc` is
/// what lets the reader thread hold it, and the engine is only reached from
/// `Session::pump` on the UI thread.
#[derive(Default)]
struct OutputInbox {
    pending: Mutex<InboxState>,
}

#[derive(Default)]
struct InboxState {
    bytes: Vec<u8>,
    exit_code: Option<i32>,
}

impl OutputInbox {
    fn append(&self, bytes: &[u8]) {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .bytes
            .extend_from_slice(bytes);
    }

    fn finish(&self, code: i32) {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .exit_code = Some(code);
    }

    fn take(&self) -> (Vec<u8>, Option<i32>) {
        let mut state = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let bytes = std::mem::take(&mut state.bytes);
        (bytes, state.exit_code.take())
    }

    fn len(&self) -> usize {
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .bytes
            .len()
    }
}

/// Shared mutable state of a session.
///
/// `Rc`-owned because a session is single-threaded by contract; the background
/// reader only ever sees the `Arc<OutputInbox>` and the wake handle.
pub struct SessionInner {
    engine: RefCell<TerminalEngine>,
    status: RefCell<SessionStatus>,
    title: RefCell<String>,
    frame: RefCell<Option<TerminalFrame>>,
    cell_size: Cell<(usize, usize)>,
    /// The last byte a display session showed, to translate bare newlines.
    last_shown: Cell<u8>,
    observers: RefCell<BTreeMap<usize, Rc<dyn Fn()>>>,
    next_observer: Cell<usize>,
    inbox: Arc<OutputInbox>,
    /// Cross-thread "there is output to apply" signal. Calling it from the PTY
    /// thread schedules the invalidation on the UI thread.
    wake: RefCell<Option<Arc<dyn Fn() + Send + Sync>>>,
    process: RefCell<Option<Arc<PseudoTerminal>>>,
    command: Option<TerminalCommand>,
    on_event: RefCell<Option<EventHandler>>,
    /// Held so the invalidation bridge lives as long as the session.
    _invalidation: RefCell<Option<LocalInvalidationHandle>>,
}

/// A terminal emulator bound to one program.
#[derive(Clone)]
pub struct TerminalSession {
    inner: Rc<SessionInner>,
}

impl TerminalSession {
    /// A session that runs `command`, defaulting to the user's login shell.
    pub fn new(command: TerminalCommand, scrollback_lines: usize) -> Self {
        Self::build(Some(command), scrollback_lines)
    }

    pub fn login_shell(scrollback_lines: usize) -> Self {
        Self::new(TerminalCommand::login_shell(), scrollback_lines)
    }

    /// A session that shows output it is handed instead of running a program,
    /// such as a build another process runs.
    ///
    /// Text a pipe produced, where a line ends in a bare newline, starts each
    /// line at the left margin as a terminal would.
    pub fn display(scrollback_lines: usize) -> Self {
        Self::build(None, scrollback_lines)
    }

    fn build(command: Option<TerminalCommand>, scrollback_lines: usize) -> Self {
        let inner = Rc::new(SessionInner {
            engine: RefCell::new(TerminalEngine::new(80, 24, scrollback_lines)),
            status: RefCell::new(SessionStatus::Idle),
            title: RefCell::new(String::new()),
            frame: RefCell::new(None),
            cell_size: Cell::new((0, 0)),
            last_shown: Cell::new(0),
            observers: RefCell::new(BTreeMap::new()),
            next_observer: Cell::new(0),
            inbox: Arc::new(OutputInbox::default()),
            wake: RefCell::new(None),
            process: RefCell::new(None),
            command,
            on_event: RefCell::new(None),
            _invalidation: RefCell::new(None),
        });
        let session = Self { inner };

        // Output arrives on the PTY thread; `pump` applies it here. The
        // invalidation bridge is what gets the UI thread to run it, reusing the
        // host's existing wake rather than owning a queue.
        let weak = Rc::downgrade(&session.inner);
        let handle = LocalInvalidationHandle::new(move |_| {
            if let Some(inner) = weak.upgrade() {
                Session::pump(&inner);
            }
        });
        *session.inner.wake.borrow_mut() = Some(handle.invalidator());
        *session.inner._invalidation.borrow_mut() = Some(handle);

        // The engine answers queries and reports bells and titles synchronously,
        // inside `feed`, so these callbacks reach back through the same `Rc`.
        let weak = Rc::downgrade(&session.inner);
        session
            .inner
            .engine
            .borrow_mut()
            .set_reply_handler(Rc::new(move |bytes| {
                if let Some(inner) = weak.upgrade() {
                    Session::write_reply(&inner, bytes);
                }
            }));
        let weak = Rc::downgrade(&session.inner);
        session
            .inner
            .engine
            .borrow_mut()
            .set_bell_handler(Rc::new(move || {
                if let Some(inner) = weak.upgrade() {
                    Session::emit(&inner, SessionEvent::Bell);
                }
            }));
        let weak = Rc::downgrade(&session.inner);
        session
            .inner
            .engine
            .borrow_mut()
            .set_title_handler(Rc::new(move |title| {
                let Some(inner) = weak.upgrade() else { return };
                if inner.title.borrow().as_str() == title {
                    return;
                }
                *inner.title.borrow_mut() = title.to_owned();
                Session::emit(&inner, SessionEvent::TitleChanged(title.to_owned()));
            }));
        session
    }

    // MARK: Public surface

    pub fn status(&self) -> SessionStatus {
        self.inner.status.borrow().clone()
    }

    pub fn is_running(&self) -> bool {
        matches!(*self.inner.status.borrow(), SessionStatus::Running)
    }

    /// Window title set by the program (OSC 0/2); empty until one is set.
    pub fn title(&self) -> String {
        self.inner.title.borrow().clone()
    }

    pub fn command(&self) -> Option<&TerminalCommand> {
        self.inner.command.as_ref()
    }

    /// Called on the UI thread.
    pub fn set_on_event(&self, handler: Option<EventHandler>) {
        *self.inner.on_event.borrow_mut() = handler;
    }

    /// Shows program output in a display session.
    pub fn show(&self, bytes: &[u8]) {
        if self.inner.command.is_some() || bytes.is_empty() {
            return;
        }
        // A pipe's bare newline has no carriage return, so the terminal would keep
        // the column. Insert one, remembering the last byte shown.
        let mut translated = Vec::with_capacity(bytes.len() + bytes.len() / 16);
        let mut last = self.inner.last_shown.get();
        for &byte in bytes {
            if byte == 0x0A && last != 0x0D {
                translated.push(0x0D);
            }
            translated.push(byte);
            last = byte;
        }
        self.inner.last_shown.set(last);
        if matches!(*self.inner.status.borrow(), SessionStatus::Idle) {
            *self.inner.status.borrow_mut() = SessionStatus::Running;
        }
        Session::feed(&self.inner, &translated);
    }

    /// Ends a display session with the program's exit status.
    pub fn show_exit(&self, code: i32) {
        if self.inner.command.is_some()
            || *self.inner.status.borrow() == SessionStatus::Exited(code)
        {
            return;
        }
        Session::finish(&self.inner, code);
    }

    /// Launches the program. `TerminalView` calls this when it first shows an idle
    /// session. A session runs one program once; create a new session to run
    /// again. Failures are also shown in the terminal itself.
    pub fn start(&self) -> Result<(), TerminalError> {
        if !matches!(*self.inner.status.borrow(), SessionStatus::Idle) {
            return Err(TerminalError::AlreadyStarted);
        }
        let Some(command) = self.inner.command.clone() else {
            *self.inner.status.borrow_mut() = SessionStatus::Running;
            return Ok(());
        };
        let (columns, rows) = {
            let engine = self.inner.engine.borrow();
            (engine.columns(), engine.rows())
        };
        let (cell_width, cell_height) = self.inner.cell_size.get();
        let size = PseudoTerminalSize::new(columns, rows, cell_width, cell_height);

        let output_inbox = self.inner.inbox.clone();
        let output_wake = self.inner.wake.borrow().clone();
        let exit_inbox = self.inner.inbox.clone();
        let exit_wake = self.inner.wake.borrow().clone();
        let spawned = PseudoTerminal::spawn(
            &command,
            size,
            move |bytes| {
                output_inbox.append(bytes);
                if let Some(wake) = output_wake.as_ref() {
                    wake();
                }
            },
            move |code| {
                exit_inbox.finish(code);
                if let Some(wake) = exit_wake.as_ref() {
                    wake();
                }
            },
        );
        match spawned {
            Ok(process) => {
                *self.inner.process.borrow_mut() = Some(Arc::new(process));
                *self.inner.status.borrow_mut() = SessionStatus::Running;
                Ok(())
            }
            Err(error) => {
                *self.inner.status.borrow_mut() = SessionStatus::Failed(error.clone());
                Session::feed(&self.inner, format!("[{error}]\r\n").as_bytes());
                Err(error)
            }
        }
    }

    /// Sends text as if typed, e.g. `send("ls\r")`.
    pub fn send(&self, text: &str) {
        self.write(text.as_bytes());
    }

    /// Pastes text using the program's bracketed-paste mode when enabled.
    pub fn paste(&self, text: &str) {
        let bytes = self.inner.engine.borrow_mut().encode_paste(text);
        self.write(&bytes);
    }

    /// Hangs up the program (SIGHUP). `status` becomes `Exited` once it quits.
    pub fn terminate(&self) {
        let process = self.inner.process.borrow().clone();
        if let Some(process) = process {
            process.terminate();
        }
    }

    // MARK: View-facing surface

    /// The current frame, captured on demand and cached until output arrives.
    pub fn frame(&self) -> TerminalFrame {
        if let Some(frame) = self.inner.frame.borrow().as_ref() {
            return frame.clone();
        }
        let frame = self.inner.engine.borrow_mut().snapshot();
        *self.inner.frame.borrow_mut() = Some(frame.clone());
        frame
    }

    /// Applies whatever the reader thread has produced. Called on the UI thread
    /// when the session reports pending output; also public so a host with its
    /// own queue can drive it.
    pub fn pump(&self) {
        Session::pump(&self.inner);
    }

    pub fn has_pending_output(&self) -> bool {
        self.inner.inbox.len() > 0
    }

    pub fn feed_bytes(&self, bytes: &[u8]) {
        Session::feed(&self.inner, bytes);
    }

    /// Writes user input. Like typing in any terminal, input returns the viewport
    /// to the live screen and drops the selection.
    pub fn write(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let scrolled = !self.frame().scroll.is_at_bottom();
        if scrolled {
            self.inner.engine.borrow_mut().scroll_to_bottom();
        }
        let cleared = self.inner.engine.borrow_mut().clear_selection();
        if cleared || scrolled {
            Session::invalidate(&self.inner);
        }
        let process = self.inner.process.borrow().clone();
        if let Some(process) = process {
            let owned = bytes.to_vec();
            // Writing may block until the program reads; keep it off the caller's
            // thread so a busy program cannot stall the UI, as the Swift session
            // does with its write queue.
            let _ = thread::Builder::new()
                .name("cove.terminal.pty.write".into())
                .spawn(move || {
                    process.write(&owned);
                });
        }
    }

    /// Bytes the program asked for (mouse reports) that are not user typing.
    pub fn report(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Some(process) = self.inner.process.borrow().clone() {
            process.write(bytes);
        }
    }

    /// Encodes and writes a key press; returns whether it produced bytes.
    pub fn press(&self, key: &TerminalKey) -> bool {
        let bytes = self.inner.engine.borrow_mut().encode_key(key);
        self.write(&bytes);
        !bytes.is_empty()
    }

    pub fn scroll(&self, rows: i32) {
        if rows == 0 {
            return;
        }
        self.inner.engine.borrow_mut().scroll(rows);
        Session::invalidate(&self.inner);
    }

    pub fn is_alternate_screen(&self) -> bool {
        self.inner.engine.borrow().is_alternate_screen()
    }

    pub fn fit(&self, columns: usize, rows: usize, cell_width: usize, cell_height: usize) {
        let unchanged = {
            let engine = self.inner.engine.borrow();
            columns == engine.columns()
                && rows == engine.rows()
                && (cell_width, cell_height) == self.inner.cell_size.get()
        };
        if unchanged {
            return;
        }
        self.inner.cell_size.set((cell_width, cell_height));
        self.inner
            .engine
            .borrow_mut()
            .resize(columns, rows, cell_width, cell_height);
        let (columns, rows) = {
            let engine = self.inner.engine.borrow();
            (engine.columns(), engine.rows())
        };
        if let Some(process) = self.inner.process.borrow().as_ref() {
            process.resize(PseudoTerminalSize::new(
                columns,
                rows,
                cell_width,
                cell_height,
            ));
        }
        *self.inner.frame.borrow_mut() = None;
    }

    pub fn set_default_colors(&self, foreground: TerminalRGB, background: TerminalRGB) {
        self.inner
            .engine
            .borrow_mut()
            .set_default_colors(foreground, background);
        *self.inner.frame.borrow_mut() = None;
    }

    /// Registers an observer and returns the token that removes it again.
    ///
    /// The callback must not capture the session strongly; the surface binds a
    /// weak link so a session that outlives its view does not keep it alive.
    pub fn observe(&self, on_change: impl Fn() + 'static) -> usize {
        let token = self.inner.next_observer.get() + 1;
        self.inner.next_observer.set(token);
        self.inner
            .observers
            .borrow_mut()
            .insert(token, Rc::new(on_change));
        token
    }

    pub fn remove_observer(&self, token: usize) {
        self.inner.observers.borrow_mut().remove(&token);
    }

    pub fn selected_text(&self) -> Option<String> {
        self.inner.engine.borrow().selected_text()
    }

    pub fn hyperlink(&self, column: usize, row: usize) -> Option<String> {
        self.inner.engine.borrow().hyperlink(column, row)
    }

    pub fn is_mouse_tracking(&self) -> bool {
        self.inner.engine.borrow().is_mouse_tracking()
    }

    pub fn encode_mouse(&self, event: &TerminalMouseEvent) -> Vec<u8> {
        self.inner.engine.borrow().encode_mouse(event)
    }

    /// Cell size last reported to the engine, in the units it measures pixels in.
    pub fn engine_cell_size(&self) -> (usize, usize) {
        self.inner.engine.borrow().cell_size()
    }

    pub fn has_selection(&self) -> bool {
        self.inner.engine.borrow().has_selection()
    }

    pub fn select(
        &self,
        phase: SelectionPhase,
        pointer: &TerminalPointer,
        rectangle: bool,
    ) -> bool {
        let changed = self
            .inner
            .engine
            .borrow_mut()
            .select(phase, pointer, rectangle);
        if changed {
            Session::invalidate(&self.inner);
        }
        changed
    }

    pub fn clear_selection(&self) -> bool {
        let changed = self.inner.engine.borrow_mut().clear_selection();
        if changed {
            Session::invalidate(&self.inner);
        }
        changed
    }

    pub fn image_pixels(&self, image_id: u32) -> Option<cove_imaging::DecodedImage> {
        self.inner.engine.borrow().image_pixels(image_id)
    }
}

/// Session operations that work on the shared inner state.
struct Session;

impl Session {
    fn pump(inner: &Rc<SessionInner>) {
        let (bytes, code) = inner.inbox.take();
        if !bytes.is_empty() {
            Self::feed(inner, &bytes);
        }
        if let Some(code) = code {
            Self::finish(inner, code);
        }
    }

    fn feed(inner: &Rc<SessionInner>, bytes: &[u8]) {
        inner.engine.borrow_mut().feed(bytes);
        Self::invalidate(inner);
    }

    fn finish(inner: &Rc<SessionInner>, code: i32) {
        *inner.process.borrow_mut() = None;
        *inner.status.borrow_mut() = SessionStatus::Exited(code);
        Self::feed(
            inner,
            format!("\r\n[Process exited with code {code}]\r\n").as_bytes(),
        );
        Self::emit(inner, SessionEvent::Exited(code));
    }

    /// The program's own answer to a query; sent synchronously from inside `feed`.
    fn write_reply(inner: &Rc<SessionInner>, bytes: &[u8]) {
        let process = inner.process.borrow().clone();
        if let Some(process) = process {
            process.write(bytes);
        }
    }

    fn emit(inner: &Rc<SessionInner>, event: SessionEvent) {
        let handler = inner.on_event.borrow().clone();
        if let Some(handler) = handler {
            handler(&event);
        }
    }

    fn invalidate(inner: &Rc<SessionInner>) {
        *inner.frame.borrow_mut() = None;
        let observers: Vec<Rc<dyn Fn()>> = inner.observers.borrow().values().cloned().collect();
        for observer in observers {
            observer();
        }
    }
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        if let Some(process) = self.process.borrow().as_ref() {
            process.terminate();
        }
    }
}

/// The mailbox crosses to the reader threads, which only append bytes.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Arc<OutputInbox>>();
};
