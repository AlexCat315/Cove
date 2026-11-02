//! Window facts mirrored for the UI thread. The platform thread writes once per
//! loop iteration; the UI thread only reads.
use cove_platform::WindowId;
use cove_shell::{DisplayRefreshTick, WindowSnapshot};
use std::{collections::HashMap, sync::RwLock};

#[derive(Clone, Copy, Default)]
struct PublishedWindow {
    snapshot: WindowSnapshot,
    /// The platform thread's latest answer to an enable request; `None` before
    /// its first.
    refresh_active: Option<bool>,
    tick: Option<DisplayRefreshTick>,
}

#[derive(Default)]
pub struct PlatformState {
    windows: RwLock<HashMap<WindowId, PublishedWindow>>,
    pointer: RwLock<Option<(f32, f32)>>,
}

impl PlatformState {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, HashMap<WindowId, PublishedWindow>> {
        self.windows.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, HashMap<WindowId, PublishedWindow>> {
        self.windows.write().unwrap_or_else(|e| e.into_inner())
    }

    pub fn window(&self, id: WindowId) -> Option<WindowSnapshot> {
        self.read().get(&id).map(|window| window.snapshot)
    }

    /// Returns whether readers would see a change.
    pub fn publish(&self, id: WindowId, snapshot: WindowSnapshot) -> bool {
        let mut windows = self.write();
        let window = windows.entry(id).or_default();
        let changed = window.snapshot != snapshot;
        window.snapshot = snapshot;
        changed
    }

    pub fn remove(&self, id: WindowId) {
        self.write().remove(&id);
    }

    pub fn global_pointer(&self) -> Option<(f32, f32)> {
        *self.pointer.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn publish_global_pointer(&self, position: Option<(f32, f32)>) {
        *self.pointer.write().unwrap_or_else(|e| e.into_inner()) = position;
    }

    pub fn refresh_active(&self, id: WindowId) -> Option<bool> {
        self.read()
            .get(&id)
            .and_then(|window| window.refresh_active)
    }

    /// Drops a tick from before the request that is now pending.
    pub fn discard_refresh_tick(&self, id: WindowId) {
        if let Some(window) = self.write().get_mut(&id) {
            window.tick = None;
        }
    }

    pub fn publish_refresh(
        &self,
        id: WindowId,
        active: bool,
        tick: Option<DisplayRefreshTick>,
    ) -> bool {
        let mut windows = self.write();
        let Some(window) = windows.get_mut(&id) else {
            return false;
        };
        let changed = window.refresh_active != Some(active) || tick.is_some();
        window.refresh_active = Some(active);
        if tick.is_some() {
            window.tick = tick;
        }
        changed
    }

    pub fn take_refresh_tick(&self, id: WindowId) -> Option<DisplayRefreshTick> {
        self.write()
            .get_mut(&id)
            .and_then(|window| window.tick.take())
    }
}
