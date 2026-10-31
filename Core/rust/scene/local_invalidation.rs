//! Transfers concurrent state invalidations to the scene thread without sending Rc views.
use super::RecompositionReason;
use crate::{ReadScope, ReadScopeId};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread::ThreadId,
};
type Callback = Rc<dyn Fn(RecompositionReason)>;
#[derive(Default)]
struct Inbox {
    pending: Mutex<Vec<(ReadScopeId, RecompositionReason)>>,
}
thread_local! {
    static INBOX:Arc<Inbox>=Arc::new(Inbox::default());
    static HANDLERS:RefCell<HashMap<ReadScopeId,Callback>>=RefCell::new(HashMap::new());
}
fn wake_slot() -> &'static Mutex<Option<Arc<dyn Fn() + Send + Sync>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>> = OnceLock::new();
    SLOT.get_or_init(Mutex::default)
}
pub fn set_scene_invalidation_wake(wake: Option<Arc<dyn Fn() + Send + Sync>>) {
    *wake_slot().lock().unwrap_or_else(|e| e.into_inner()) = wake;
}
fn deliver(id: ReadScopeId, reason: RecompositionReason) {
    let callback = HANDLERS.with(|handlers| handlers.borrow().get(&id).cloned());
    if let Some(callback) = callback {
        callback(reason);
    }
}
pub fn drain_scene_invalidations() {
    let pending = INBOX.with(|inbox| {
        std::mem::take(&mut *inbox.pending.lock().unwrap_or_else(|e| e.into_inner()))
    });
    for (id, reason) in pending {
        deliver(id, reason);
    }
}
pub struct LocalInvalidationHandle {
    scope: ReadScope,
    tracked: Arc<dyn Fn(RecompositionReason) + Send + Sync>,
    _thread: ThreadId,
    _not_send: std::marker::PhantomData<Rc<()>>,
}
impl LocalInvalidationHandle {
    pub fn new(invalidate: impl Fn(RecompositionReason) + 'static) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self::with_id(
            ReadScopeId(NEXT.fetch_add(1, Ordering::Relaxed)),
            invalidate,
        )
    }
    /// Create a local invalidation bridge using a caller-owned stable scope ID.
    /// The ID must be unique among concurrently installed scene read scopes.
    pub fn with_id(id: ReadScopeId, invalidate: impl Fn(RecompositionReason) + 'static) -> Self {
        let thread = std::thread::current().id();
        let inbox = INBOX.with(Clone::clone);
        let tracked: Arc<dyn Fn(RecompositionReason) + Send + Sync> = Arc::new(move |reason| {
            if std::thread::current().id() == thread {
                deliver(id, reason);
            } else {
                inbox
                    .pending
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push((id, reason));
                let wake = wake_slot()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                if let Some(wake) = wake {
                    wake();
                }
            }
        });
        let callback = tracked.clone();
        let scope = ReadScope::with_id(id, move |field| {
            callback(RecompositionReason::new("state").with_detail(field.unwrap_or_default()))
        });
        HANDLERS.with(|handlers| handlers.borrow_mut().insert(scope.id, Rc::new(invalidate)));
        Self {
            scope,
            tracked,
            _thread: thread,
            _not_send: std::marker::PhantomData,
        }
    }
    pub fn read_scope(&self) -> ReadScope {
        self.scope.clone()
    }
    pub fn tracked(&self) -> Arc<dyn Fn(RecompositionReason) + Send + Sync> {
        self.tracked.clone()
    }
    pub fn invalidator(&self) -> Arc<dyn Fn() + Send + Sync> {
        let tracked = self.tracked.clone();
        Arc::new(move || tracked(RecompositionReason::new("observable")))
    }
}
impl Drop for LocalInvalidationHandle {
    fn drop(&mut self) {
        HANDLERS.with(|handlers| handlers.borrow_mut().remove(&self.scope.id));
        super::ObservableStateTracking::remove_scope(self.scope.id);
    }
}
