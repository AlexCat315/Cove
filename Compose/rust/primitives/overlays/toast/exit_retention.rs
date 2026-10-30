//! Translation of Primitives/Overlays/Toast/ExitRetention.swift.
//!
//! Only the host retains exiting visuals. The controller's queue and expiry
//! remain immediately observable and independent of animation progress.
use std::cell::RefCell;

use super::controller::ToastNotification;

/// One retained toast card, visible or animating out.
#[derive(Clone)]
pub struct ToastPresentationEntry {
    pub notification: ToastNotification,
    pub is_visible: bool,
    pub revision: u64,
}

/// Retains exiting toast cards for their animation while bounding the tree.
#[derive(Default)]
pub struct ToastPresentation {
    entries: RefCell<Vec<ToastPresentationEntry>>,
    revision: RefCell<u64>,
}

impl ToastPresentation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> Vec<ToastPresentationEntry> {
        self.entries.borrow().clone()
    }

    /// Reconcile against the controller's active list. Existing cards keep their
    /// identity; a card that left the list stays for one exit animation.
    pub fn update(&self, notifications: &[ToastNotification], limit: usize) -> Vec<ToastPresentationEntry> {
        let mut entries = self.entries.borrow_mut();
        let previous_ids: Vec<String> =
            entries.iter().map(|entry| entry.notification.id.clone()).collect();
        for entry in entries.iter_mut() {
            if let Some(notification) = notifications
                .iter()
                .find(|candidate| candidate.id == entry.notification.id)
            {
                entry.notification = notification.clone();
            }
            let visible = notifications.iter().any(|candidate| candidate.id == entry.notification.id);
            if visible != entry.is_visible {
                *self.revision.borrow_mut() += 1;
                entry.revision = *self.revision.borrow();
                entry.is_visible = visible;
            }
        }
        for notification in notifications {
            if previous_ids.contains(&notification.id) {
                continue;
            }
            *self.revision.borrow_mut() += 1;
            entries.push(ToastPresentationEntry {
                notification: notification.clone(),
                is_visible: true,
                revision: *self.revision.borrow(),
            });
        }
        // Rapid dismiss/post loops must not accumulate an unbounded exit tree.
        let exiting: Vec<String> = entries
            .iter()
            .filter(|entry| !entry.is_visible)
            .rev()
            .take(limit)
            .map(|entry| entry.notification.id.clone())
            .collect();
        entries.retain(|entry| entry.is_visible || exiting.contains(&entry.notification.id));
        entries.clone()
    }

    /// Drop one exited card, but only for the revision that asked to leave.
    pub fn remove(&self, id: &str, revision: u64) {
        let mut entries = self.entries.borrow_mut();
        let matches = entries
            .iter()
            .any(|entry| entry.notification.id == id && entry.revision == revision && !entry.is_visible);
        if !matches {
            return;
        }
        entries.retain(|entry| !(entry.notification.id == id && entry.revision == revision));
    }
}
