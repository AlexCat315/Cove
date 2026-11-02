//! The accessibility tree as owned data, and its main-thread presenter.
//!
//! The UI thread flattens its tree into an [`AccessibilitySnapshot`]; the platform
//! thread commits it to AppKit or UIKit. Assistive technology reads values from
//! the last snapshot and queues actions for the UI thread, so a native accessibility
//! callback never touches the scene.
use cove_platform::WindowId;
use std::ffi::CString;

#[derive(Clone, Debug)]
pub struct AccessibilityItem {
    pub id: u64,
    pub parent: u64,
    pub role: u32,
    pub flags: u32,
    pub actions: u32,
    pub frame: [f32; 4],
    pub label: CString,
    pub value: CString,
    /// What assistive technology reads when it differs from `value`.
    pub live_value: Option<CString>,
    pub help: CString,
    pub identifier: CString,
    pub revision: u64,
}

#[derive(Clone, Debug)]
pub struct AccessibilitySnapshot {
    /// Routes queued actions back to the bridge that built this snapshot.
    pub bridge: u64,
    pub items: Vec<AccessibilityItem>,
}

/// An action assistive technology requested, waiting for the UI thread.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) struct AccessibilityAction {
    pub bridge: u64,
    pub element: u64,
    pub kind: u32,
    pub value: Option<String>,
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) fn take_actions() -> Vec<AccessibilityAction> {
    std::mem::take(&mut *pending_actions().lock().unwrap_or_else(|e| e.into_inner()))
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn pending_actions() -> &'static std::sync::Mutex<Vec<AccessibilityAction>> {
    static PENDING: std::sync::Mutex<Vec<AccessibilityAction>> = std::sync::Mutex::new(Vec::new());
    &PENDING
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) use apple::Presenter;

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod apple {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        collections::HashMap,
        ffi::{CStr, c_char, c_void},
    };
    #[repr(C)]
    struct Item {
        id: u64,
        parent: u64,
        role: u32,
        flags: u32,
        actions: u32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        label: *const c_char,
        value: *const c_char,
        help: *const c_char,
        identifier: *const c_char,
        revision: u64,
    }
    unsafe extern "C" {
        fn cove_runtime_ax_create(
            window: usize,
            context: *mut c_void,
            action: unsafe extern "C" fn(*mut c_void, u64, u32, *const c_char) -> bool,
            value: unsafe extern "C" fn(*mut c_void, u64) -> *mut c_char,
            free_value: unsafe extern "C" fn(*mut c_char),
        ) -> *mut c_void;
        fn cove_runtime_ax_destroy(bridge: *mut c_void);
        fn cove_runtime_ax_update(bridge: *mut c_void, items: *const Item, count: usize) -> usize;
    }
    struct Context {
        bridge: Cell<u64>,
        values: RefCell<HashMap<u64, CString>>,
    }
    /// One window's native accessibility bridge. Confined to the main thread.
    pub(crate) struct Presenter {
        pointer: *mut c_void,
        context: *mut Context,
    }
    impl Presenter {
        pub fn new(window: usize, bridge: u64) -> Self {
            unsafe extern "C" fn action(
                context: *mut c_void,
                id: u64,
                kind: u32,
                value: *const c_char,
            ) -> bool {
                // SAFETY: context outlives the native bridge, which invalidates this callback on destroy.
                let context = unsafe { &*context.cast::<Context>() };
                let value = (!value.is_null()).then(|| {
                    // SAFETY: native passes a NUL terminated UTF-8 value for this synchronous callback.
                    unsafe { CStr::from_ptr(value) }
                        .to_string_lossy()
                        .into_owned()
                });
                pending_actions()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(AccessibilityAction {
                        bridge: context.bridge.get(),
                        element: id,
                        kind,
                        value,
                    });
                cove_core::scene::wake_scene_thread();
                true
            }
            unsafe extern "C" fn value(context: *mut c_void, id: u64) -> *mut c_char {
                // SAFETY: context outlives the native bridge, which invalidates this callback on destroy.
                let context = unsafe { &*context.cast::<Context>() };
                context
                    .values
                    .borrow()
                    .get(&id)
                    .cloned()
                    .map_or(std::ptr::null_mut(), CString::into_raw)
            }
            unsafe extern "C" fn free_value(value: *mut c_char) {
                if !value.is_null() {
                    // SAFETY: pointer came from CString::into_raw in value(), and native frees it once.
                    unsafe { drop(CString::from_raw(value)) };
                }
            }
            let context = Box::into_raw(Box::new(Context {
                bridge: Cell::new(bridge),
                values: RefCell::default(),
            }));
            // SAFETY: the boxed context and callbacks outlive the native bridge; the window is owned by the shell.
            let pointer = unsafe {
                cove_runtime_ax_create(window, context.cast(), action, value, free_value)
            };
            Self { pointer, context }
        }
        pub fn bridge(&self) -> u64 {
            // SAFETY: context lives until Drop; the main thread is the only user.
            unsafe { &*self.context }.bridge.get()
        }
        pub fn apply(&self, snapshot: &AccessibilitySnapshot) -> usize {
            if self.pointer.is_null() {
                return 0;
            }
            // SAFETY: context lives until Drop; the main thread is the only user.
            let context = unsafe { &*self.context };
            // A rebuilt root brings a new bridge to the same window.
            context.bridge.set(snapshot.bridge);
            let mut values = context.values.borrow_mut();
            values.clear();
            let items = snapshot
                .items
                .iter()
                .map(|item| {
                    values.insert(
                        item.id,
                        item.live_value.as_ref().unwrap_or(&item.value).clone(),
                    );
                    Item {
                        id: item.id,
                        parent: item.parent,
                        role: item.role,
                        flags: item.flags,
                        actions: item.actions,
                        x: item.frame[0],
                        y: item.frame[1],
                        width: item.frame[2],
                        height: item.frame[3],
                        label: item.label.as_ptr(),
                        value: item.value.as_ptr(),
                        help: item.help.as_ptr(),
                        identifier: item.identifier.as_ptr(),
                        revision: item.revision,
                    }
                })
                .collect::<Vec<_>>();
            drop(values);
            // SAFETY: native copies every string and frame before returning; snapshot outlives the call.
            unsafe { cove_runtime_ax_update(self.pointer, items.as_ptr(), items.len()) }
        }
    }
    impl Drop for Presenter {
        fn drop(&mut self) {
            if !self.pointer.is_null() {
                // SAFETY: native destroy invalidates all callbacks before the context is freed.
                unsafe { cove_runtime_ax_destroy(self.pointer) };
            }
            // SAFETY: context came from Box::into_raw and is released once, after native invalidation.
            unsafe { drop(Box::from_raw(self.context)) };
        }
    }
}

/// The platform thread's presenters, one per window.
#[derive(Default)]
pub(crate) struct AccessibilityPresenters {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    presenters: std::collections::HashMap<WindowId, Presenter>,
}

impl AccessibilityPresenters {
    pub fn update(
        &mut self,
        window: WindowId,
        native: cove_shell::NativeWindowReference,
        snapshot: AccessibilitySnapshot,
    ) {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            let raw = match native {
                cove_shell::NativeWindowReference::AppKit(raw)
                | cove_shell::NativeWindowReference::UIKit(raw) => raw,
                cove_shell::NativeWindowReference::Win32(_) => return,
            };
            self.presenters
                .entry(window)
                .or_insert_with(|| Presenter::new(raw, snapshot.bridge))
                .apply(&snapshot);
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        {
            let _ = (window, native, snapshot);
        }
    }

    /// Closes the window's presenter unless a newer bridge took it over.
    pub fn close_bridge(&mut self, window: WindowId, bridge: u64) {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if self
            .presenters
            .get(&window)
            .is_some_and(|presenter| presenter.bridge() == bridge)
        {
            self.presenters.remove(&window);
        }
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let _ = (window, bridge);
    }

    pub fn remove(&mut self, window: WindowId) {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        self.presenters.remove(&window);
        #[cfg(not(any(target_os = "macos", target_os = "ios")))]
        let _ = window;
    }

    pub fn clear(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        self.presenters.clear();
    }
}
