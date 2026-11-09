use crate::{NativeWindowReference, ShellError};
use cove_platform::NativeMenuSnapshot;
pub struct NativeMenuPopover;
impl NativeMenuPopover {
    pub fn is_supported() -> bool {
        cfg!(target_os = "macos")
    }
    pub fn pop_up(
        snapshot: &NativeMenuSnapshot,
        x: f32,
        y: f32,
        window: Option<NativeWindowReference>,
    ) -> Result<Option<i64>, ShellError> {
        #[cfg(target_os = "macos")]
        {
            use crate::native_menu_wire::{Item, append};
            unsafe extern "C" {
                fn cove_shell_menu_popup(
                    items: *const Item,
                    count: usize,
                    x: f32,
                    y: f32,
                    window: usize,
                    chosen: *mut i64,
                ) -> bool;
            }
            let mut items = Vec::new();
            let mut strings = Vec::new();
            append(snapshot, usize::MAX, &mut items, &mut strings);
            let window = match window {
                Some(NativeWindowReference::AppKit(raw)) => raw,
                _ => 0,
            };
            let mut chosen = 0;
            // SAFETY: native popup runs synchronously; all C strings, icons and window references remain valid throughout.
            Ok(unsafe {
                cove_shell_menu_popup(items.as_ptr(), items.len(), x, y, window, &mut chosen)
            }
            .then_some(chosen))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (snapshot, x, y, window);
            Err(ShellError::UnsupportedPlatform("native context-menu snapshots require AppKit; use the Compose menu presenter on this platform".into()))
        }
    }
}