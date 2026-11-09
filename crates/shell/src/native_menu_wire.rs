//! Shared wire format for handing a menu snapshot to AppKit.
//!
//! Both the context popup and the application menu bar flatten a snapshot into
//! the same parent-indexed array, so the native side builds one tree shape.

use cove_platform::{NativeMenuKind, NativeMenuSnapshot};
use std::ffi::CString;

/// Mirrors `CoveShellMenuItem` in `native/native.h`. `parent` is an index into
/// this same array, or `usize::MAX` for a root.
#[repr(C)]
pub(crate) struct Item {
    pub parent: usize,
    pub id: i64,
    pub kind: u32,
    pub flags: u32,
    pub title: *const std::ffi::c_char,
    pub key: *const std::ffi::c_char,
    pub png: *const u8,
    pub png_size: usize,
}

pub(crate) const ENABLED: u32 = 1;
pub(crate) const CHECKED: u32 = 2;
pub(crate) const COMMAND: u32 = 4;
pub(crate) const CONTROL: u32 = 8;
pub(crate) const OPTION: u32 = 16;
pub(crate) const SHIFT: u32 = 32;
pub(crate) const TEMPLATE_ICON: u32 = 64;

pub(crate) const REGULAR: u32 = 0;
pub(crate) const SEPARATOR: u32 = 1;
pub(crate) const SUBMENU: u32 = 2;

/// Appends `snapshot` and its descendants. `strings` owns every C string the
/// items point at and must outlive the native call.
pub(crate) fn append(
    snapshot: &NativeMenuSnapshot,
    parent: usize,
    items: &mut Vec<Item>,
    strings: &mut Vec<CString>,
) {
    for item in &snapshot.items {
        strings.push(CString::new(item.title.replace('\0', "")).unwrap());
        let title = strings.last().unwrap().as_ptr();
        strings.push(
            CString::new(
                item.shortcut
                    .as_ref()
                    .map_or("", |shortcut| shortcut.key.as_str())
                    .replace('\0', ""),
            )
            .unwrap(),
        );
        let key = strings.last().unwrap().as_ptr();
        let mut flags = if item.enabled { ENABLED } else { 0 }
            | if item.checked { CHECKED } else { 0 };
        if let Some(shortcut) = &item.shortcut {
            if shortcut.meta {
                flags |= COMMAND;
            }
            if shortcut.ctrl {
                flags |= CONTROL;
            }
            if shortcut.alt {
                flags |= OPTION;
            }
            if shortcut.shift {
                flags |= SHIFT;
            }
        }
        if item.icon.as_ref().is_some_and(|icon| icon.is_template) {
            flags |= TEMPLATE_ICON;
        }
        let index = items.len();
        items.push(Item {
            parent,
            id: item.id,
            kind: match &item.kind {
                NativeMenuKind::Regular => REGULAR,
                NativeMenuKind::Separator => SEPARATOR,
                NativeMenuKind::Submenu(_) => SUBMENU,
            },
            flags,
            title,
            key,
            png: item
                .icon
                .as_ref()
                .map_or(std::ptr::null(), |icon| icon.png.as_ptr()),
            png_size: item.icon.as_ref().map_or(0, |icon| icon.png.len()),
        });
        if let NativeMenuKind::Submenu(branch) = &item.kind {
            append(branch, index, items, strings);
        }
    }
}