//! Translation of Primitives/Overlays/Toast/Controller.swift.
//!
//! Window/application-owned queue with stable-ID replacement, bounded visibility
//! and deterministic time.
use std::cell::RefCell;
use std::rc::Rc;

use crate::primitives::feedback::StatusTone;
use crate::theme::styles::buttons::ButtonRole;

/// One inline action rendered inside a toast card.
///
/// A toast may offer up to two actions (for example Undo and Retry). The action
/// is a real button, so it keeps the normal keyboard and accessibility contract;
/// invoking it dismisses the notification by default.
#[derive(Clone)]
pub struct ToastAction {
    pub id: String,
    pub title: String,
    pub role: ButtonRole,
    /// Whether invoking the action also dismisses its notification.
    pub dismisses_notification: bool,
    pub action: Rc<dyn Fn()>,
}

impl ToastAction {
    pub fn new(title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self {
            id: next_action_id(),
            title: title.into(),
            role: ButtonRole::Normal,
            dismisses_notification: true,
            action: Rc::new(action),
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn role(mut self, role: ButtonRole) -> Self {
        self.role = role;
        self
    }

    pub fn keeps_notification(mut self) -> Self {
        self.dismisses_notification = false;
        self
    }
}

fn next_action_id() -> String {
    thread_local! {
        static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    }
    NEXT.with(|next| {
        let value = next.get();
        next.set(value.wrapping_add(1));
        format!("toast-action-{value}")
    })
}

/// One queued notification.
#[derive(Clone)]
pub struct ToastNotification {
    pub id: String,
    pub title: String,
    pub message: String,
    pub tone: StatusTone,
    /// `None` keeps a notification visible until explicitly dismissed.
    pub duration: Option<f64>,
    pub is_dismissible: bool,
    /// At most two actions are rendered; extra entries are ignored.
    pub actions: Vec<ToastAction>,
}

impl ToastNotification {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: next_notification_id(),
            title: title.into(),
            message: String::new(),
            tone: StatusTone::Info,
            duration: Some(5.0),
            is_dismissible: true,
            actions: Vec::new(),
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    /// `None` keeps the notification visible until dismissed.
    pub fn duration(mut self, duration: Option<f64>) -> Self {
        self.duration = duration.map(|value| if value.is_finite() { value.max(0.0) } else { 5.0 });
        self
    }

    pub fn dismissible(mut self, is_dismissible: bool) -> Self {
        self.is_dismissible = is_dismissible;
        self
    }

    pub fn action(mut self, action: ToastAction) -> Self {
        self.actions.push(action);
        self
    }
}

fn next_notification_id() -> String {
    thread_local! {
        static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
    }
    NEXT.with(|next| {
        let value = next.get();
        next.set(value.wrapping_add(1));
        format!("toast-{value}")
    })
}

/// One queued notification plus its remaining lifetime.
#[derive(Clone)]
struct ToastSession {
    notification: ToastNotification,
    remaining: Option<f64>,
    is_paused: bool,
}

/// Window/application-owned toast queue.
pub struct ToastController {
    pub max_visible: usize,
    sessions: RefCell<Vec<ToastSession>>,
}

impl ToastController {
    pub fn new(max_visible: usize) -> Rc<Self> {
        Rc::new(Self { max_visible: max_visible.max(1), sessions: RefCell::new(Vec::new()) })
    }

    pub fn notifications(&self) -> Vec<ToastNotification> {
        self.sessions
            .borrow()
            .iter()
            .take(self.max_visible)
            .map(|session| session.notification.clone())
            .collect()
    }

    pub fn queued_count(&self) -> usize {
        self.sessions.borrow().len().saturating_sub(self.max_visible)
    }

    pub fn needs_clock(&self) -> bool {
        self.sessions
            .borrow()
            .iter()
            .take(self.max_visible)
            .any(|session| !session.is_paused && session.remaining.is_some())
    }

    /// Post a notification; an existing ID replaces its session in place so
    /// repeated posts update rather than stack.
    pub fn post(&self, notification: ToastNotification) {
        let session = ToastSession {
            remaining: notification.duration,
            notification,
            is_paused: false,
        };
        let mut sessions = self.sessions.borrow_mut();
        match sessions.iter().position(|existing| existing.notification.id == session.notification.id) {
            Some(index) => sessions[index] = session,
            None => sessions.push(session),
        }
    }

    pub fn dismiss(&self, id: &str) {
        let mut sessions = self.sessions.borrow_mut();
        sessions.retain(|session| session.notification.id != id);
    }

    pub fn set_paused(&self, paused: bool, id: &str) {
        let mut sessions = self.sessions.borrow_mut();
        if let Some(session) = sessions.iter_mut().find(|session| session.notification.id == id)
            && session.is_paused != paused
        {
            session.is_paused = paused;
        }
    }

    /// Advance visible lifetimes. A paused or hidden notification never expires.
    pub fn advance(&self, seconds: f64) {
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        let mut sessions = self.sessions.borrow_mut();
        let visible: Vec<String> = sessions
            .iter()
            .take(self.max_visible)
            .map(|session| session.notification.id.clone())
            .collect();
        for session in sessions.iter_mut() {
            if !visible.contains(&session.notification.id) || session.is_paused {
                continue;
            }
            if let Some(remaining) = session.remaining {
                session.remaining = Some((remaining - seconds).max(0.0));
            }
        }
        sessions.retain(|session| {
            !(session.remaining == Some(0.0)
                && !session.is_paused
                && visible.contains(&session.notification.id))
        });
    }

    pub fn clear(&self) {
        self.sessions.borrow_mut().clear();
    }
}
