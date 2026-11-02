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

thread_local! {
    static CURRENT_INTERACTIONS: RefCell<Option<Rc<InteractionRegistry>>> = const { RefCell::new(None) };
    static CLIPBOARD_READ: RefCell<Option<ClipboardReader>> = const { RefCell::new(None) };
    static CLIPBOARD_REQUEST: RefCell<Option<ClipboardRequester>> = const { RefCell::new(None) };
    static CLIPBOARD_WRITE: RefCell<Option<ClipboardWriter>> = const { RefCell::new(None) };
    static NATIVE_MENU_PRESENTER: RefCell<Option<NativeMenuPresenterCallback>> = const { RefCell::new(None) };
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
