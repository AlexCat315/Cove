//! Translation of CoveUIScene/AmbientSlot.swift.
use std::sync::Mutex;
pub struct AmbientSlot<T> { value: Mutex<T> }
impl<T> AmbientSlot<T> {
    pub const fn new(value: T) -> Self { Self { value: Mutex::new(value) } }
    pub fn current(&self) -> T where T: Clone { self.value.lock().unwrap_or_else(|e| e.into_inner()).clone() }
    pub fn set(&self, value: T) { *self.value.lock().unwrap_or_else(|e| e.into_inner()) = value; }
}
