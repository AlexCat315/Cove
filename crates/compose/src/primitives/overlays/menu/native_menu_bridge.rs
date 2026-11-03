//! Translation of Primitives/Overlays/Menu/NativeMenuBridge.swift.
//!
//! A menu snapshot plus the actions of its items, so the chosen item's action
//! can be invoked after the native menu returns. The in-app `MenuEntry` model is
//! converted here into the platform-neutral snapshot `PlatformCore` defines and
//! the shell consumes; nothing about the native presentation crosses this
//! boundary.
//!
//! Behaviour checklist:
//! - ids are assigned in traversal order so an action routes back to its closure
//! - a label becomes a disabled regular item, i.e. a non-selectable group header
//! - a submenu recurses and keeps its own enabled flag
//! - a regular item records its action and carries checked/shortcut/icon state
//! - a shortcut display string parses both forms: the macOS glyphs ⌘⌃⌥⇧ and the
//!   word form Ctrl/Meta/Alt/Shift joined by `+`
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::primitives::overlays::menu::{MenuEntry, MenuItemRole};
use cove_platform::{
    NativeMenuIcon, NativeMenuItem, NativeMenuKind, NativeMenuSnapshot, NativeShortcut,
};

/// A menu snapshot plus the actions of its items.
#[derive(Clone, Default)]
pub struct NativeMenuBuild {
    pub snapshot: NativeMenuSnapshot,
    actions: BTreeMap<i64, Rc<dyn Fn()>>,
}

impl NativeMenuBuild {
    pub fn new(snapshot: NativeMenuSnapshot, actions: BTreeMap<i64, Rc<dyn Fn()>>) -> Self {
        Self { snapshot, actions }
    }

    /// Runs the action the native menu reported.
    pub fn run(&self, id: i64) {
        if let Some(action) = self.actions.get(&id) {
            action();
        }
    }

    pub fn action_ids(&self) -> Vec<i64> {
        self.actions.keys().copied().collect()
    }
}

/// Converts the in-app menu model into a platform-neutral snapshot.
pub struct NativeMenuBuilder;

/// The authored glyph size, in points. The template raster is emitted at 2x.
const ICON_POINTS: f32 = 16.0;

impl NativeMenuBuilder {
    pub fn build(entries: &[MenuEntry]) -> NativeMenuBuild {
        let mut next_id: i64 = 0;
        let mut actions: BTreeMap<i64, Rc<dyn Fn()>> = BTreeMap::new();
        let snapshot = Self::snapshot(entries, &mut next_id, &mut actions);
        NativeMenuBuild { snapshot, actions }
    }

    fn snapshot(
        entries: &[MenuEntry],
        next_id: &mut i64,
        actions: &mut BTreeMap<i64, Rc<dyn Fn()>>,
    ) -> NativeMenuSnapshot {
        let mut items = Vec::with_capacity(entries.len());
        for entry in entries {
            let id = *next_id;
            *next_id += 1;
            items.push(match entry {
                MenuEntry::Separator { .. } => NativeMenuItem {
                    id,
                    kind: NativeMenuKind::Separator,
                    title: String::new(),
                    enabled: false,
                    checked: false,
                    shortcut: None,
                    icon: None,
                },
                // A non-selectable group header: a disabled regular item.
                MenuEntry::Label { title, .. } => NativeMenuItem {
                    id,
                    kind: NativeMenuKind::Regular,
                    title: title.clone(),
                    enabled: false,
                    checked: false,
                    shortcut: None,
                    icon: None,
                },
                MenuEntry::Submenu(branch) => NativeMenuItem {
                    id,
                    kind: NativeMenuKind::Submenu(Self::snapshot(
                        &branch.entries,
                        next_id,
                        actions,
                    )),
                    title: branch.title.clone(),
                    enabled: branch.is_enabled,
                    checked: false,
                    shortcut: None,
                    icon: None,
                },
                MenuEntry::Item(item) => {
                    actions.insert(id, item.action.clone());
                    NativeMenuItem {
                        id,
                        kind: NativeMenuKind::Regular,
                        title: item.title.clone(),
                        enabled: item.is_enabled,
                        checked: item.is_selected,
                        shortcut: Self::parse_shortcut(item.shortcut.as_deref()),
                        icon: Self::make_icon(item.content.icon.as_ref()),
                    }
                }
            });
        }
        NativeMenuSnapshot { items }
    }

    /// Parses a platform display string into a neutral shortcut.
    ///
    /// macOS uses the glyphs ⌘⌃⌥⇧; other platforms use the words Ctrl / Meta /
    /// Alt / Shift joined by `+`. Both forms may appear, and the words are
    /// matched before their own `+` separators are stripped.
    pub fn parse_shortcut(display: Option<&str>) -> Option<NativeShortcut> {
        let display = display.filter(|value| !value.is_empty())?;
        let mut ctrl = false;
        let mut alt = false;
        let mut shift = false;
        let mut meta = false;
        let mut rest = display.to_string();

        for (glyph, slot) in [
            ("⌘", &mut meta),
            ("⌃", &mut ctrl),
            ("⌥", &mut alt),
            ("⇧", &mut shift),
        ] {
            if rest.contains(glyph) {
                *slot = true;
                rest = rest.replace(glyph, "");
            }
        }
        for (word, slot) in [
            ("Ctrl", &mut ctrl),
            ("Meta", &mut meta),
            ("Alt", &mut alt),
            ("Shift", &mut shift),
        ] {
            if rest.contains(word) {
                *slot = true;
                rest = rest.replace(word, "");
            }
        }

        let key: String = rest.chars().filter(|character| *character != '+').collect();
        let key = key.trim().to_string();
        if key.is_empty() {
            return None;
        }
        Some(NativeShortcut {
            key,
            ctrl,
            alt,
            shift,
            meta,
        })
    }

    /// The template glyph for an item.
    ///
    /// The bundle glyph is an alpha-mask SVG. It is rasterized into a
    /// black-on-transparent PNG and marked as a template so the native menu can
    /// tint it alongside the item text. An item with no icon, or one whose glyph
    /// cannot be resolved, carries no icon.
    fn make_icon(resource: Option<&cove_imaging::BundleImageResource>) -> Option<NativeMenuIcon> {
        let resource = resource?;
        let path = resource.path()?;
        let data = std::fs::read(path).ok()?;
        let png =
            cove_imaging::ImageDecoder::template_png(&data, &resource.file_extension, ICON_POINTS)
                .ok()?;
        Some(NativeMenuIcon {
            png,
            is_template: true,
        })
    }
}

/// Whether an item role marks a destructive command, which native menus group
/// last and tint.
pub fn is_destructive(role: MenuItemRole) -> bool {
    role == MenuItemRole::Destructive
}
