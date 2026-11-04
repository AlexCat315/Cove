//! Mailbox from the UI thread to the platform thread.
//!
//! Commands fall in two classes. Idempotent ones land in a per-window slot where
//! a later write replaces an earlier one, so a burst costs one platform call.
//! Ordered ones keep strict FIFO. An ordered command first flushes the slots
//! written before it, so coalescing never reorders what a caller observes.
use super::AccessibilitySnapshot;
use cove_platform::{NativeMenuSnapshot, SystemCursor, TextInputArea, WindowId};
use cove_shell::{EventLoopWaker, FileDialogFilter, WindowChromeHitTest, WindowOptions};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
};

#[derive(Clone, Debug, PartialEq)]
pub struct DialogRequest {
    pub request: u64,
    pub window: Option<WindowId>,
    pub folder: bool,
    pub path: Option<String>,
    pub filters: Vec<FileDialogFilter>,
    pub multiple: bool,
}

pub enum PlatformCommand {
    SetCursor(WindowId, SystemCursor),
    SetTextInputArea(WindowId, Option<TextInputArea>),
    SetChromeHitTest(WindowId, Option<WindowChromeHitTest>),
    SetPosition(WindowId, f32, f32),
    SetSize(WindowId, f32, f32),
    SetDisplayRefresh(WindowId, bool),
    UpdateAccessibility(WindowId, AccessibilitySnapshot),

    Raise(WindowId),
    Minimize(WindowId),
    Maximize(WindowId),
    Restore(WindowId),
    Hide(WindowId),
    Show(WindowId),
    ShowSystemMenu(WindowId, f32, f32),
    WriteClipboard(String),
    ReadClipboard(u64),
    SampleColor(u64),
    OpenDialog(DialogRequest),
    ShowMenu {
        request: u64,
        window: Option<WindowId>,
        snapshot: NativeMenuSnapshot,
        x: f32,
        y: f32,
    },
    CreateWindow {
        window: WindowId,
        title: String,
        options: WindowOptions,
    },
    /// Only closes the presenter while it still serves this bridge.
    CloseAccessibility(WindowId, u64),
    CloseWindow(WindowId),
    /// Arbitrary work bound to the platform thread, in order with the rest.
    Run(Box<dyn FnOnce() + Send>),
    Shutdown,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    Cursor,
    TextInputArea,
    ChromeHitTest,
    Position,
    Size,
    DisplayRefresh,
    Accessibility,
}

impl PlatformCommand {
    fn slot(&self) -> Option<(u32, Slot)> {
        Some(match self {
            Self::SetCursor(id, _) => (id.0, Slot::Cursor),
            Self::SetTextInputArea(id, _) => (id.0, Slot::TextInputArea),
            Self::SetChromeHitTest(id, _) => (id.0, Slot::ChromeHitTest),
            Self::SetPosition(id, ..) => (id.0, Slot::Position),
            Self::SetSize(id, ..) => (id.0, Slot::Size),
            Self::SetDisplayRefresh(id, _) => (id.0, Slot::DisplayRefresh),
            Self::UpdateAccessibility(id, _) => (id.0, Slot::Accessibility),
            _ => return None,
        })
    }
}

#[derive(Default)]
struct Mailbox {
    queue: VecDeque<PlatformCommand>,
    slots: BTreeMap<(u32, Slot), PlatformCommand>,
}

impl Mailbox {
    fn flush_slots(&mut self) {
        let slots = std::mem::take(&mut self.slots);
        self.queue.extend(slots.into_values());
    }
}

pub struct MainThreadBridge {
    mailbox: Mutex<Mailbox>,
    wake: EventLoopWaker,
}

impl MainThreadBridge {
    pub fn new(wake: EventLoopWaker) -> Self {
        Self {
            mailbox: Mutex::default(),
            wake,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Mailbox> {
        self.mailbox.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queues `command` without waiting for the platform thread. Only the post that
    /// finds the mailbox empty wakes it: an earlier post's wake is still owed a
    /// [`Self::take`], which picks this command up too.
    pub fn post(&self, command: PlatformCommand) {
        let was_empty = {
            let mut mailbox = self.lock();
            let was_empty = mailbox.queue.is_empty() && mailbox.slots.is_empty();
            match command.slot() {
                Some(slot) => {
                    mailbox.slots.insert(slot, command);
                }
                None => {
                    mailbox.flush_slots();
                    mailbox.queue.push_back(command);
                }
            }
            was_empty
        };
        if was_empty {
            (self.wake)();
        }
    }

    /// Every pending command in the order it must apply.
    pub fn take(&self) -> VecDeque<PlatformCommand> {
        let mut mailbox = self.lock();
        mailbox.flush_slots();
        std::mem::take(&mut mailbox.queue)
    }

    pub fn has_pending(&self) -> bool {
        let mailbox = self.lock();
        !mailbox.queue.is_empty() || !mailbox.slots.is_empty()
    }
}
