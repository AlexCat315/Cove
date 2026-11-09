//! The macOS application menu bar (`NSApplication.mainMenu`).
//!
//! Context popups are a different entry point: see `native_menu_popover`.

use crate::ShellError;
use cove_platform::{NativeMenuBranch, NativeMenuBarSnapshot, NativeMenuKind, NativeMenuItem};

pub struct NativeMenuBarInstaller;

impl NativeMenuBarInstaller {
    pub fn is_supported() -> bool {
        cfg!(target_os = "macos")
    }

    /// Replaces the application menu bar. Calling it again swaps the tree, which
    /// is how an application refreshes item titles and enabled state.
    pub fn install(bar: &NativeMenuBarSnapshot) -> Result<(), ShellError> {
        #[cfg(target_os = "macos")]
        {
            use crate::native_menu_wire::{Item, append};
            use std::ffi::{CString, c_char};

            #[repr(C)]
            struct Bar {
                app_name: *const c_char,
                services: *const c_char,
                hide: *const c_char,
                hide_others: *const c_char,
                show_all: *const c_char,
                quit: *const c_char,
                edit: *const c_char,
                undo: *const c_char,
                redo: *const c_char,
                cut: *const c_char,
                copy: *const c_char,
                paste: *const c_char,
                delete_item: *const c_char,
                select_all: *const c_char,
                window: *const c_char,
                minimize: *const c_char,
                zoom: *const c_char,
                bring_all_to_front: *const c_char,
                application_items: *const Item,
                application_count: usize,
                menus: *const Item,
                menu_count: usize,
            }
            unsafe extern "C" {
                fn cove_shell_menu_bar_install(bar: *const Bar) -> bool;
            }

            // Top-level menus are submenu items whose parent is the root, the
            // same shape the popup wire format already understands.
            let branches = cove_platform::NativeMenuSnapshot {
                items: bar
                    .menus
                    .iter()
                    .map(|branch: &NativeMenuBranch| NativeMenuItem {
                        id: 0,
                        kind: NativeMenuKind::Submenu(branch.items.clone()),
                        title: branch.title.clone(),
                        enabled: true,
                        checked: false,
                        shortcut: None,
                        icon: None,
                    })
                    .collect(),
            };
            let mut application_items = Vec::new();
            let mut menus = Vec::new();
            let mut strings = Vec::new();
            append(
                &bar.application_items,
                usize::MAX,
                &mut application_items,
                &mut strings,
            );
            append(&branches, usize::MAX, &mut menus, &mut strings);

            let titles = &bar.application_titles;
            let standard = &bar.standard_titles;
            let mut label = |text: &str| {
                strings.push(CString::new(text.replace('\0', "")).unwrap());
                strings.last().unwrap().as_ptr()
            };
            let app_name = label(&bar.app_name);
            let services = label(&titles.services);
            let hide = label(&titles.hide);
            let hide_others = label(&titles.hide_others);
            let show_all = label(&titles.show_all);
            let quit = label(&titles.quit);
            let edit = label(&standard.edit);
            let undo = label(&standard.undo);
            let redo = label(&standard.redo);
            let cut = label(&standard.cut);
            let copy = label(&standard.copy);
            let paste = label(&standard.paste);
            let delete_item = label(&standard.delete);
            let select_all = label(&standard.select_all);
            let window = label(&standard.window);
            let minimize = label(&standard.minimize);
            let zoom = label(&standard.zoom);
            let bring_all_to_front = label(&standard.bring_all_to_front);
            let native = Bar {
                app_name,
                services,
                hide,
                hide_others,
                show_all,
                quit,
                edit,
                undo,
                redo,
                cut,
                copy,
                paste,
                delete_item,
                select_all,
                window,
                minimize,
                zoom,
                bring_all_to_front,
                application_items: application_items.as_ptr(),
                application_count: application_items.len(),
                menus: menus.as_ptr(),
                menu_count: menus.len(),
            };
            // SAFETY: the native side copies every string and icon before returning, and `strings`, `application_items` and `menus` outlive the call.
            if unsafe { cove_shell_menu_bar_install(&native) } {
                Ok(())
            } else {
                Err(ShellError::InitializationFailed(
                    "the application menu bar could not be installed".into(),
                ))
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = bar;
            Err(ShellError::UnsupportedPlatform(
                "an application menu bar requires AppKit; this platform presents Compose menus"
                    .into(),
            ))
        }
    }

    /// Chosen authored item ids, oldest first. AppKit enqueues on the main
    /// thread; the shell drains here and runs the closures on the UI thread.
    pub fn take_choices() -> Vec<i64> {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn cove_shell_menu_bar_take_choice(chosen: *mut i64) -> bool;
            }
            let mut choices = Vec::new();
            let mut chosen = 0i64;
            // SAFETY: the native side writes one id per true return and owns the queue lock.
            while unsafe { cove_shell_menu_bar_take_choice(&mut chosen) } {
                choices.push(chosen);
            }
            choices
        }
        #[cfg(not(target_os = "macos"))]
        {
            Vec::new()
        }
    }

    pub fn clear() {
        #[cfg(target_os = "macos")]
        {
            unsafe extern "C" {
                fn cove_shell_menu_bar_clear();
            }
            // SAFETY: removes the bar and empties the queue on the platform thread.
            unsafe { cove_shell_menu_bar_clear() }
        }
    }
}