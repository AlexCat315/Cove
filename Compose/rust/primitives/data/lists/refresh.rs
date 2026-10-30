//! Translation of Primitives/Data/Lists/Refresh.swift.
//!
//! Pull-to-refresh state for a vertical list. The caller owns the work; this
//! value only carries the pull threshold and the in-flight flag.
use std::rc::Rc;

/// Pull-to-refresh configuration for a list.
#[derive(Clone)]
pub struct RefreshOptions {
    /// Whether a pull is currently running.
    pub is_refreshing: bool,
    pub on_refresh: Rc<dyn Fn()>,
    /// Points the pull must travel before it triggers.
    pub threshold: f32,
    /// Label shown beside the indicator, if any.
    pub label: Option<String>,
}

impl RefreshOptions {
    pub fn new(on_refresh: impl Fn() + 'static) -> Self {
        Self { is_refreshing: false, on_refresh: Rc::new(on_refresh), threshold: 64.0, label: None }
    }

    pub fn refreshing(mut self, is_refreshing: bool) -> Self {
        self.is_refreshing = is_refreshing;
        self
    }

    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = if threshold.is_finite() { threshold.max(0.0) } else { 64.0 };
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Whether a pull of `distance` should trigger a refresh.
    pub fn should_trigger(&self, distance: f32) -> bool {
        !self.is_refreshing && distance >= self.threshold
    }

    pub fn trigger(&self) {
        if !self.is_refreshing {
            (self.on_refresh)();
        }
    }
}
