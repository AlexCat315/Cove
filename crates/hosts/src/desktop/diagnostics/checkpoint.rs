//! Translation of Desktop/Sources/CoveUI/Diagnostics/Checkpoint.swift.

use std::collections::BTreeMap;
use std::rc::Rc;

/// Optional DevTools checkpoint hooks for application-owned state.
///
/// State capture does not include transient input, composition or GPU resources.
#[derive(Clone)]
pub struct ApplicationCheckpoint {
    capture: Rc<dyn Fn() -> BTreeMap<String, String>>,
    restore: Rc<dyn Fn(&BTreeMap<String, String>) -> bool>,
}

impl ApplicationCheckpoint {
    pub fn new(
        capture: impl Fn() -> BTreeMap<String, String> + 'static,
        restore: impl Fn(&BTreeMap<String, String>) -> bool + 'static,
    ) -> Self {
        Self {
            capture: Rc::new(capture),
            restore: Rc::new(restore),
        }
    }

    pub fn capture(&self) -> BTreeMap<String, String> {
        (self.capture)()
    }

    pub fn restore(&self, state: &BTreeMap<String, String>) -> bool {
        (self.restore)(state)
    }
}
