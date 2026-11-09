//! Translation of CoveUIScene/Input/PlatformInputContext.swift.
use crate::Point;
use crate::scene::{
    FocusChain, FocusChainHolder, InteractionRegistry, InvalidationLog, InvalidationLogHolder,
    PointerCapture, PointerCaptureHolder,
};
use cove_platform::NativeMenuSnapshot;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

// MARK: - Per-window ambients
//
// These hold the *current window's* registry, made current only for the
// duration of `PlatformInputContext::with_current` and restored on exit. Each
// window owns its own registries; the holder is the ambient pointer the call
// sites read.

/// Host clipboard reader installed by the platform shell.
pub type ClipboardReader = Rc<dyn Fn() -> Option<String>>;
/// Receives the clipboard's text once the platform has read it.
pub type ClipboardCompletion = Box<dyn FnOnce(Option<String>)>;
/// Host clipboard read that answers later, for a platform on another thread.
pub type ClipboardRequester = Rc<dyn Fn(ClipboardCompletion)>;
/// Host clipboard writer installed by the platform shell.
pub type ClipboardWriter = Rc<dyn Fn(&str)>;
/// Receives the chosen item once the native menu closed.
pub type NativeMenuCompletion = Box<dyn FnOnce(Option<i64>)>;
/// Host native-menu presenter installed by the platform shell.
pub type NativeMenuPresenterCallback = Rc<dyn Fn(&NativeMenuSnapshot, Point, NativeMenuCompletion)>;
/// Receives the sampled screen colour once the platform picked it.
pub type ColorSampleCompletion = Box<dyn FnOnce(Option<crate::Color>)>;
/// Host screen-colour sampler installed by the platform shell.
pub type ColorSamplerCallback = Rc<dyn Fn(ColorSampleCompletion)>;

/// A file-type filter a native open panel understands.
///
/// Core cannot name `cove-shell::FileDialogFilter` (no dependency), so the
/// contract is restated here and the host adapts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePickerFilter {
    pub name: String,
    /// Extensions without the leading dot; empty matches every file.
    pub extensions: Vec<String>,
}

impl FilePickerFilter {
    pub fn new(name: impl Into<String>, extensions: impl IntoIterator<Item = String>) -> Self {
        Self {
            name: name.into(),
            extensions: extensions.into_iter().collect(),
        }
    }
}

/// Receives the chosen paths once the native open panel closed. Empty when the
/// user cancelled.
pub type FilePickCompletion = Box<dyn FnOnce(Vec<String>)>;
/// Host open-panel presenter installed by the platform shell.
pub type FilePickerCallback = Rc<dyn Fn(&[FilePickerFilter], bool, FilePickCompletion)>;

/// A document the application wants written through the native save panel.
///
/// The bytes travel with the request because iOS and Android export the
/// contents themselves rather than handing back a path to write to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSaveRequest {
    /// Name the panel pre-fills; the user may change it.
    pub suggested_name: String,
    pub filters: Vec<FilePickerFilter>,
    pub contents: Vec<u8>,
}
/// The result of a native save panel, after the host has written the bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileSaveOutcome {
    /// `destination` is where the write landed, `None` where the OS does not
    /// disclose one (the iOS export picker).
    Saved {
        destination: Option<String>,
    },
    Cancelled,
    /// No native save panel on this platform; the caller keeps its in-app path.
    Unsupported,
    Failed(String),
}
/// Receives the save result once the native panel closed and the bytes landed.
pub type FileSaveCompletion = Box<dyn FnOnce(FileSaveOutcome)>;
/// Host save-panel presenter installed by the platform shell.
pub type FileSaverCallback = Rc<dyn Fn(FileSaveRequest, FileSaveCompletion)>;

/// Receives documents and URLs the OS opened with this application: Finder
/// "Open With", a double-clicked file association, or a registered URL scheme.
/// This is distinct from a drag onto a window ([`crate::scene::ExternalDropEvent`]).
pub type OpenedDocumentsCallback = Rc<dyn Fn(Vec<String>)>;

thread_local! {
    static CURRENT_INTERACTIONS: RefCell<Option<Rc<InteractionRegistry>>> = const { RefCell::new(None) };
    static CLIPBOARD_READ: RefCell<Option<ClipboardReader>> = const { RefCell::new(None) };
    static CLIPBOARD_REQUEST: RefCell<Option<ClipboardRequester>> = const { RefCell::new(None) };
    static CLIPBOARD_WRITE: RefCell<Option<ClipboardWriter>> = const { RefCell::new(None) };
    static NATIVE_MENU_PRESENTER: RefCell<Option<NativeMenuPresenterCallback>> = const { RefCell::new(None) };
    static COLOR_SAMPLER: RefCell<Option<ColorSamplerCallback>> = const { RefCell::new(None) };
    static FILE_PICKER: RefCell<Option<FilePickerCallback>> = const { RefCell::new(None) };
    static FILE_SAVER: RefCell<Option<FileSaverCallback>> = const { RefCell::new(None) };
    static OPENED_DOCUMENTS: RefCell<Option<OpenedDocumentsCallback>> = const { RefCell::new(None) };
    static PENDING_DOCUMENTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Holds the active window's interaction registry.
pub struct InteractionRegistryHolder;
impl InteractionRegistryHolder {
    pub fn current() -> Option<Rc<InteractionRegistry>> {
        CURRENT_INTERACTIONS.with(|slot| slot.borrow().clone())
    }
    pub fn set_current(registry: Option<Rc<InteractionRegistry>>) {
        CURRENT_INTERACTIONS.with(|slot| *slot.borrow_mut() = registry);
    }
}

/// Process-wide clipboard bridge (genuinely global: one system pasteboard).
pub struct ClipboardHolder;
impl ClipboardHolder {
    pub fn read() -> Option<String> {
        CLIPBOARD_READ.with(|slot| slot.borrow().as_ref().and_then(|read| read()))
    }
    pub fn set_read(read: Option<ClipboardReader>) {
        CLIPBOARD_READ.with(|slot| *slot.borrow_mut() = read);
    }
    /// Delivers the clipboard's text to `done`, later when the platform lives on
    /// another thread. Prefer this over [`Self::read`], which only answers when
    /// the host installed a synchronous reader.
    pub fn request_read(done: ClipboardCompletion) {
        match CLIPBOARD_REQUEST.with(|slot| slot.borrow().clone()) {
            Some(request) => request(done),
            None => done(Self::read()),
        }
    }
    pub fn set_read_requester(request: Option<ClipboardRequester>) {
        CLIPBOARD_REQUEST.with(|slot| *slot.borrow_mut() = request);
    }
    pub fn write(text: &str) {
        CLIPBOARD_WRITE.with(|slot| {
            if let Some(write) = slot.borrow().as_ref() {
                write(text);
            }
        });
    }
    pub fn set_write(write: Option<ClipboardWriter>) {
        CLIPBOARD_WRITE.with(|slot| *slot.borrow_mut() = write);
    }
}

/// Holds the native menu presenter, injected by the platform shell.
pub struct NativeMenuPresenter;
impl NativeMenuPresenter {
    /// Presents the snapshot at a point. `done` receives the chosen item's id once
    /// the menu closed; the caller keeps running meanwhile.
    pub fn present(snapshot: &NativeMenuSnapshot, at: Point, done: NativeMenuCompletion) {
        match NATIVE_MENU_PRESENTER.with(|slot| slot.borrow().clone()) {
            Some(present) => present(snapshot, at, done),
            None => done(None),
        }
    }
    /// Whether a platform shell installed a presenter on this thread.
    pub fn is_installed() -> bool {
        NATIVE_MENU_PRESENTER.with(|slot| slot.borrow().is_some())
    }
    pub fn set_present(present: Option<NativeMenuPresenterCallback>) {
        NATIVE_MENU_PRESENTER.with(|slot| *slot.borrow_mut() = present);
    }
}

/// Holds the native screen-colour sampler, injected by the platform shell.
///
/// Sampling may complete later and off the UI thread's call site; the host
/// answers through `done`. When no sampler is installed the request reports no
/// colour, so callers can hide the affordance.
pub struct ColorSampler;
impl ColorSampler {
    /// Asks the platform for a colour under the user's pick.
    pub fn request(done: ColorSampleCompletion) {
        match COLOR_SAMPLER.with(|slot| slot.borrow().clone()) {
            Some(request) => request(done),
            None => done(None),
        }
    }
    /// Whether a platform shell installed a sampler on this thread.
    pub fn is_installed() -> bool {
        COLOR_SAMPLER.with(|slot| slot.borrow().is_some())
    }
    pub fn set_requester(request: Option<ColorSamplerCallback>) {
        COLOR_SAMPLER.with(|slot| *slot.borrow_mut() = request);
    }
}

/// Holds the native open panel, injected by the platform shell.
///
/// The panel may close later; the host answers through `done` on the UI thread.
/// Without an installed picker every request completes with no paths, so a
/// caller can disable its browse affordance instead of pretending to open one.
pub struct FilePicker;
impl FilePicker {
    /// Shows an open panel filtered to `filters`; `multiple` allows a multi-file
    /// choice. `done` receives the chosen paths, empty when the user cancelled.
    pub fn request(filters: &[FilePickerFilter], multiple: bool, done: FilePickCompletion) {
        match FILE_PICKER.with(|slot| slot.borrow().clone()) {
            Some(present) => present(filters, multiple, done),
            None => done(Vec::new()),
        }
    }
    /// Whether a platform shell installed a picker on this thread.
    pub fn is_installed() -> bool {
        FILE_PICKER.with(|slot| slot.borrow().is_some())
    }
    pub fn set_presenter(present: Option<FilePickerCallback>) {
        FILE_PICKER.with(|slot| *slot.borrow_mut() = present);
    }
}

/// Holds the native save panel, injected by the platform shell.
///
/// The panel may close later; the host writes the bytes and answers through
/// `done` on the UI thread. Without an installed saver every request reports
/// [`FileSaveOutcome::Unsupported`], so a caller can fall back to its in-app
/// save path instead of pretending a panel opened.
pub struct FileSaver;
impl FileSaver {
    /// Shows a save panel pre-filled with `request.suggested_name` and writes
    /// `request.contents` where the user chooses.
    pub fn request(request: FileSaveRequest, done: FileSaveCompletion) {
        match FILE_SAVER.with(|slot| slot.borrow().clone()) {
            Some(present) => present(request, done),
            None => done(FileSaveOutcome::Unsupported),
        }
    }
    /// Whether a platform shell installed a saver on this thread.
    pub fn is_installed() -> bool {
        FILE_SAVER.with(|slot| slot.borrow().is_some())
    }
    pub fn set_presenter(present: Option<FileSaverCallback>) {
        FILE_SAVER.with(|slot| *slot.borrow_mut() = present);
    }
}

/// Holds the application's handler for documents the OS opened with it.
///
/// The OS may deliver a document before the application installs its handler
/// (a cold launch via "Open With"), so [`Self::deliver`] queues documents until
/// a handler is set, then [`Self::set_handler`] replays them in arrival order.
/// Both calls run on the UI thread; the host marshals native open events there.
pub struct OpenedDocuments;
impl OpenedDocuments {
    /// Installs the application's handler and replays any documents that
    /// arrived before it. Passing `None` removes the handler; later documents
    /// queue again.
    pub fn set_handler(handler: Option<OpenedDocumentsCallback>) {
        OPENED_DOCUMENTS.with(|slot| *slot.borrow_mut() = handler.clone());
        if let Some(handler) = handler {
            let pending = PENDING_DOCUMENTS.with(|slot| std::mem::take(&mut *slot.borrow_mut()));
            if !pending.is_empty() {
                handler(pending);
            }
        }
    }
    /// Whether an application handler is installed on this thread.
    pub fn is_installed() -> bool {
        OPENED_DOCUMENTS.with(|slot| slot.borrow().is_some())
    }
    /// Delivers documents the OS opened. With no handler installed they queue
    /// for the next [`Self::set_handler`]; an empty list is ignored.
    pub fn deliver(documents: Vec<String>) {
        if documents.is_empty() {
            return;
        }
        let handler = OPENED_DOCUMENTS.with(|slot| slot.borrow().clone());
        match handler {
            Some(handler) => handler(documents),
            None => PENDING_DOCUMENTS.with(|slot| slot.borrow_mut().extend(documents)),
        }
    }
}

/// Work for the thread that owns the platform, posted by a UI thread.
pub type PlatformWork = Box<dyn FnOnce() + Send>;
/// Host hook that delivers [`PlatformWork`] to the platform thread.
pub type PlatformThreadExecutor = Rc<dyn Fn(PlatformWork)>;

thread_local! {
    static PLATFORM_THREAD_EXECUTOR: RefCell<Option<PlatformThreadExecutor>> = const { RefCell::new(None) };
}

/// Reaches native APIs bound to the platform thread, such as an embedded web view.
pub struct PlatformThread;
impl PlatformThread {
    /// Runs `work` on the platform thread, in order with earlier posts and without
    /// waiting for it. Runs it at once when this thread owns the platform.
    pub fn post(work: PlatformWork) {
        match PLATFORM_THREAD_EXECUTOR.with(|slot| slot.borrow().clone()) {
            Some(execute) => execute(work),
            None => work(),
        }
    }
    pub fn set_executor(executor: Option<PlatformThreadExecutor>) {
        PLATFORM_THREAD_EXECUTOR.with(|slot| *slot.borrow_mut() = executor);
    }
}

/// A per-window ambient owned by a higher layer that must be made "current"
/// exactly while this window's context is current.
pub trait ScopedAmbient {
    /// Make this ambient current; return a closure that restores the previous.
    fn activate(&self) -> Box<dyn FnOnce()>;
}

/// Runtime services bound to one platform window.
///
/// Every registry the UI consults — interactions, focus, pointer capture, the
/// invalidation log — is owned here and made current only for the duration of
/// `with_current`. One window's state can never pollute another's.
pub struct PlatformInputContext {
    pub interactions: Rc<InteractionRegistry>,
    pub focus_chain: Rc<FocusChain>,
    pub pointer_capture: Rc<PointerCapture>,
    pub invalidation_log: Arc<InvalidationLog>,
    scoped_ambients: RefCell<Vec<Rc<dyn ScopedAmbient>>>,
}
impl PlatformInputContext {
    pub fn new() -> Self {
        Self {
            interactions: Rc::new(InteractionRegistry::new()),
            focus_chain: Rc::new(FocusChain::new()),
            pointer_capture: Rc::new(PointerCapture::new()),
            invalidation_log: Arc::new(InvalidationLog::default()),
            scoped_ambients: RefCell::new(Vec::new()),
        }
    }
    /// Register a higher-layer ambient to be scoped with this context.
    pub fn add_scoped_ambient(&self, ambient: Rc<dyn ScopedAmbient>) {
        let mut ambients = self.scoped_ambients.borrow_mut();
        if !ambients
            .iter()
            .any(|existing| Rc::ptr_eq(existing, &ambient))
        {
            ambients.push(ambient);
        }
    }
    pub fn with_current<R>(&self, body: impl FnOnce() -> R) -> R {
        let previous_interactions = InteractionRegistryHolder::current();
        let previous_focus = FocusChainHolder::current();
        let previous_capture = PointerCaptureHolder::current();
        let previous_log = InvalidationLogHolder::current();

        InteractionRegistryHolder::set_current(Some(self.interactions.clone()));
        FocusChainHolder::set_current(Some(self.focus_chain.clone()));
        PointerCaptureHolder::set_current(Some(self.pointer_capture.clone()));
        InvalidationLogHolder::set_current(Some(self.invalidation_log.clone()));

        let restores: Vec<Box<dyn FnOnce()>> = self
            .scoped_ambients
            .borrow()
            .iter()
            .map(|ambient| ambient.activate())
            .collect();
        struct Restore {
            restores: Vec<Box<dyn FnOnce()>>,
            interactions: Option<Rc<InteractionRegistry>>,
            focus: Option<Rc<FocusChain>>,
            capture: Option<Rc<PointerCapture>>,
            log: Option<Arc<InvalidationLog>>,
        }
        impl Drop for Restore {
            fn drop(&mut self) {
                for restore in self.restores.drain(..).rev() {
                    restore();
                }
                InteractionRegistryHolder::set_current(self.interactions.take());
                FocusChainHolder::set_current(self.focus.take());
                PointerCaptureHolder::set_current(self.capture.take());
                InvalidationLogHolder::set_current(self.log.take());
            }
        }
        let _restore = Restore {
            restores,
            interactions: previous_interactions,
            focus: previous_focus,
            capture: previous_capture,
            log: previous_log,
        };
        body()
    }
}
impl Default for PlatformInputContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_installs_and_restores_ambients() {
        let context = PlatformInputContext::new();
        let pointer = context.pointer_capture.clone();
        let observed = context.with_current(|| {
            PointerCaptureHolder::current().map(|capture| Rc::ptr_eq(&capture, &pointer))
        });
        assert_eq!(observed, Some(true));
        assert!(PointerCaptureHolder::current().is_none());
        assert!(FocusChainHolder::current().is_none());
    }
}
