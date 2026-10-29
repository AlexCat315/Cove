use std::sync::{Mutex, Arc};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayRefreshTick { pub timestamp: f64, pub target_timestamp: f64 }
struct Inbox { enabled: bool, latest: Option<DisplayRefreshTick> }
pub struct DisplayRefreshInbox { inner: Mutex<Inbox>, wake: Arc<dyn Fn() + Send + Sync> }
impl DisplayRefreshInbox {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self { Self { inner: Mutex::new(Inbox { enabled: false, latest: None }), wake: Arc::new(wake) } }
    pub fn set_enabled(&self, enabled: bool) { let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner()); inner.enabled = enabled; if !enabled { inner.latest = None; } }
    pub fn publish(&self, tick: DisplayRefreshTick) { let wake = { let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner()); if !inner.enabled { return; } let empty = inner.latest.is_none(); inner.latest = Some(tick); empty }; if wake { (self.wake)(); } }
    pub fn take(&self) -> Option<DisplayRefreshTick> { self.inner.lock().unwrap_or_else(|e| e.into_inner()).latest.take() }
}
