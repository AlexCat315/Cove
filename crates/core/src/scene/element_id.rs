//! Translation of CoveUIScene/ElementID.swift.
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementId(pub u64);
impl std::fmt::Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "e{}", self.0)
    }
}
pub struct IdentityAllocator {
    next: AtomicU64,
}
impl Default for IdentityAllocator {
    fn default() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }
}
impl IdentityAllocator {
    pub fn shared() -> &'static Self {
        static SHARED: OnceLock<IdentityAllocator> = OnceLock::new();
        SHARED.get_or_init(Self::default)
    }
    pub fn allocate(&self) -> ElementId {
        ElementId(
            self.next
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                    next.checked_add(1)
                })
                .expect("element identity space exhausted"),
        )
    }
}
impl crate::SnapshotValue for ElementId {}
