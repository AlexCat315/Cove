//! Translation of `Browser/Sources/CoveUIBrowserPrototype/main.swift`
//! (`ExportBuffer`).
//!
//! JavaScript reads the buffers through raw pointers, so the allocation must
//! outlive each export call and stay stable until the next replacement. A zero
//! byte allocation is never produced: the pointer is always non-null and
//! `count` describes the meaningful prefix.

use std::ffi::c_void;

/// Owns one CPU allocation handed to JavaScript by pointer.
pub struct ExportBuffer {
    pointer: *mut u8,
    capacity: usize,
    count: usize,
}

impl ExportBuffer {
    pub fn new() -> Self {
        Self {
            pointer: std::ptr::NonNull::<u8>::dangling().as_ptr(),
            capacity: 0,
            count: 0,
        }
    }

    /// Copies `bytes` into a fresh allocation, then frees the previous one.
    pub fn replace(&mut self, bytes: &[u8]) {
        let capacity = bytes.len().max(1);
        let mut next = Vec::<u8>::with_capacity(capacity);
        next.extend_from_slice(bytes);
        let pointer = next.as_mut_ptr();
        std::mem::forget(next);
        self.release();
        self.pointer = pointer;
        self.capacity = capacity;
        self.count = bytes.len();
    }

    /// Serialises `value` as JSON and publishes the encoded bytes.
    pub fn encode<T: serde::Serialize>(&mut self, value: &T) {
        match serde_json::to_vec(value) {
            Ok(bytes) => self.replace(&bytes),
            // The prototype only encodes finite, serialisable values; a failure
            // still has to leave a readable, empty buffer rather than a stale one.
            Err(_) => self.replace(&[]),
        }
    }

    pub fn pointer(&self) -> *mut u8 {
        self.pointer
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn as_void(&self) -> *mut c_void {
        self.pointer.cast()
    }

    fn release(&mut self) {
        if self.capacity > 0 {
            // SAFETY: the allocation was produced by `Vec<u8>` with this exact
            // capacity in `replace` and has not been freed since.
            unsafe { drop(Vec::from_raw_parts(self.pointer, 0, self.capacity)) };
        }
        self.pointer = std::ptr::NonNull::<u8>::dangling().as_ptr();
        self.capacity = 0;
        self.count = 0;
    }
}

impl Default for ExportBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ExportBuffer {
    fn drop(&mut self) {
        self.release();
    }
}

/// Allocates `count` bytes for a host to fill in place.
///
/// # Safety
/// The returned pointer must be released with [`free_export_allocation`] and
/// the caller must not write past `count` bytes.
pub unsafe fn allocate_export_allocation(count: usize) -> *mut u8 {
    if count == 0 || count > 32 * 1024 * 1024 {
        return std::ptr::null_mut();
    }
    let mut bytes = Vec::<u8>::with_capacity(count);
    let pointer = bytes.as_mut_ptr();
    std::mem::forget(bytes);
    pointer
}

/// Frees memory from [`allocate_export_allocation`].
///
/// # Safety
/// `pointer` and `count` must come from one matching call to
/// [`allocate_export_allocation`] and must not be used afterwards.
pub unsafe fn free_export_allocation(pointer: *mut u8, count: usize) {
    if pointer.is_null() || count == 0 {
        return;
    }
    // SAFETY: the caller guarantees the pointer came from a `Vec<u8>` with this
    // capacity and has not been freed.
    unsafe { drop(Vec::from_raw_parts(pointer, 0, count)) };
}
