//! Translation of CoveUICore/StateReadTracking.swift. Rust TLS owns and destroys its stack on every platform.
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReadScopeId(pub u64);
type Invalidator = Arc<dyn Fn(Option<&str>) + Send + Sync>;
#[derive(Clone)]
pub struct ReadScope {
    pub id: ReadScopeId,
    pub(crate) invalidate: Invalidator,
}
impl ReadScope {
    pub(crate) fn with_id(
        id: ReadScopeId,
        invalidate: impl Fn(Option<&str>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            id,
            invalidate: Arc::new(invalidate),
        }
    }
    pub fn new(invalidate: impl Fn(Option<&str>) + Send + Sync + 'static) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            id: ReadScopeId(NEXT.fetch_add(1, Ordering::Relaxed)),
            invalidate: Arc::new(invalidate),
        }
    }
}
#[derive(Default)]
struct Stack {
    scopes: Vec<ReadScope>,
    deferred_depth: usize,
}
thread_local! { static STACK: RefCell<Stack> = RefCell::new(Stack::default()); }
type Cleaner = Box<dyn Fn(ReadScopeId) + Send + Sync>;
#[derive(Default)]
struct Registry {
    installed: HashSet<ReadScopeId>,
    cleaners: HashMap<ReadScopeId, HashMap<u64, Cleaner>>,
}
fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(Mutex::default)
}
fn clear(id: ReadScopeId) {
    let cleaners = registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .cleaners
        .remove(&id)
        .unwrap_or_default();
    for clean in cleaners.into_values() {
        clean(id);
    }
}
pub fn remove_read_scope(id: ReadScopeId) {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .installed
        .remove(&id);
    clear(id);
}
pub fn with_read_scope<R>(scope: &ReadScope, body: impl FnOnce() -> R) -> R {
    clear(scope.id);
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .installed
        .insert(scope.id);
    STACK.with(|stack| stack.borrow_mut().scopes.push(scope.clone()));
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            STACK.with(|stack| {
                stack.borrow_mut().scopes.pop();
            });
        }
    }
    let _pop = Pop;
    body()
}
pub fn with_deferred_evaluation<R>(body: impl FnOnce() -> R) -> R {
    STACK.with(|stack| stack.borrow_mut().deferred_depth += 1);
    struct End;
    impl Drop for End {
        fn drop(&mut self) {
            STACK.with(|stack| stack.borrow_mut().deferred_depth -= 1);
        }
    }
    let _end = End;
    body()
}
pub(crate) fn current_read_scope(owner: Option<&ReadScope>) -> Option<ReadScope> {
    STACK.with(|stack| {
        let stack = stack.borrow();
        stack.scopes.last().cloned().or_else(|| {
            if stack.deferred_depth > 0 {
                owner.cloned()
            } else {
                None
            }
        })
    })
}
pub(crate) fn is_read_scope_installed(id: ReadScopeId) -> bool {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .installed
        .contains(&id)
}
pub(crate) fn register_read_cleaner(
    owner: u64,
    scope: ReadScopeId,
    cleaner: impl Fn(ReadScopeId) + Send + Sync + 'static,
) {
    registry()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .cleaners
        .entry(scope)
        .or_default()
        .insert(owner, Box::new(cleaner));
}

/// A clone retains the observed value rather than an alias to mutable reference storage.
/// Custom value types opt in explicitly; opaque/reference fields use `read_opaque` instead.
pub trait SnapshotValue: Clone + PartialEq + Send + Sync + 'static {}
macro_rules! values { ($($t:ty),*) => { $(impl SnapshotValue for $t {})* }; }
values!(
    bool,
    char,
    String,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    f32,
    f64,
    std::time::Duration,
    std::time::SystemTime,
    crate::Color,
    crate::Point,
    crate::Size,
    crate::Rect,
    crate::Font,
    crate::FontWeight,
    crate::FontDesign,
    crate::TextAlignment,
    crate::InputCapabilities,
    crate::Platform,
    crate::DesignLanguage,
    crate::DeviceClass,
    crate::Posture,
    crate::WidthClass,
    crate::HeightClass,
    crate::DisplayFeature
);
impl<T: SnapshotValue> SnapshotValue for Option<T> {}
impl<T: SnapshotValue> SnapshotValue for Vec<T> {}
impl<T: SnapshotValue, const N: usize> SnapshotValue for [T; N] {}
impl<K: SnapshotValue + Ord, V: SnapshotValue> SnapshotValue for std::collections::BTreeMap<K, V> {}
impl<K: SnapshotValue + Eq + std::hash::Hash, V: SnapshotValue> SnapshotValue
    for std::collections::HashMap<K, V>
{
}
impl<T: SnapshotValue + Ord> SnapshotValue for std::collections::BTreeSet<T> {}
impl<T: SnapshotValue + Eq + std::hash::Hash> SnapshotValue for std::collections::HashSet<T> {}

pub(crate) fn allocate_tracking_owner() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
