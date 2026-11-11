//! Translation of Desktop/Sources/CoveUI/Window/NativeMenu.swift.
//!
//! macOS installs an application `NSMenu`. Other desktop platforms draw a
//! single-row menu bar inside the window (see `window::immersive_menu`).

use cove_platform::NativeShortcut;

pub use cove_platform::NativeApplicationMenuTitles;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeMenuKeyModifiers(pub u8);

impl NativeMenuKeyModifiers {
    pub const COMMAND: Self = Self(1 << 0);
    pub const SHIFT: Self = Self(1 << 1);
    pub const OPTION: Self = Self(1 << 2);
    pub const CONTROL: Self = Self(1 << 3);

    /// macOS uses Command; other platforms use Control for primary shortcuts.
    pub fn primary() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::COMMAND
        }
        #[cfg(not(target_os = "macos"))]
        {
            Self::CONTROL
        }
    }

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Shortcut description for the platform snapshot format.
    pub fn shortcut(self, key: &str) -> NativeShortcut {
        NativeShortcut {
            key: key.to_owned(),
            ctrl: self.contains(Self::CONTROL),
            alt: self.contains(Self::OPTION),
            shift: self.contains(Self::SHIFT),
            meta: self.contains(Self::COMMAND),
        }
    }
}

#[derive(Clone)]
pub struct NativeMenuAction {
    pub title: String,
    pub key_equivalent: String,
    pub key_modifiers: NativeMenuKeyModifiers,
    pub is_enabled: bool,
    pub is_selected: bool,
    /// Re-evaluated when the native menu opens or a key equivalent runs.
    pub is_enabled_provider: Option<std::rc::Rc<dyn Fn() -> bool>>,
    pub action: std::rc::Rc<dyn Fn()>,
}

impl NativeMenuAction {
    pub fn new(title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self {
            title: title.into(),
            key_equivalent: String::new(),
            key_modifiers: NativeMenuKeyModifiers::primary(),
            is_enabled: true,
            is_selected: false,
            is_enabled_provider: None,
            action: std::rc::Rc::new(action),
        }
    }

    pub fn with_shortcut(
        mut self,
        key: impl Into<String>,
        modifiers: NativeMenuKeyModifiers,
    ) -> Self {
        self.key_equivalent = key.into();
        self.key_modifiers = modifiers;
        self
    }

    pub fn with_enabled_provider(mut self, provider: impl Fn() -> bool + 'static) -> Self {
        self.is_enabled_provider = Some(std::rc::Rc::new(provider));
        self
    }

    pub fn with_selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    pub fn resolved_is_enabled(&self) -> bool {
        self.is_enabled_provider
            .as_ref()
            .map_or(self.is_enabled, |provider| provider())
    }
}

#[derive(Clone)]
pub enum NativeMenuItem {
    Action(NativeMenuAction),
    Submenu(NativeMenu),
    Separator,
}

#[derive(Clone)]
pub struct NativeMenu {
    pub title: String,
    pub items: Vec<NativeMenuItem>,
}

impl NativeMenu {
    pub fn named(title: impl Into<String>) -> Self {
        Self::new(title, Vec::new())
    }

    pub fn new(title: impl Into<String>, items: Vec<NativeMenuItem>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }

    pub fn action(self, title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        self.item(NativeMenuAction::new(title, action))
    }

    pub fn item(mut self, action: NativeMenuAction) -> Self {
        self.items.push(NativeMenuItem::Action(action));
        self
    }

    pub fn submenu(mut self, menu: NativeMenu) -> Self {
        self.items.push(NativeMenuItem::Submenu(menu));
        self
    }

    pub fn separator(mut self) -> Self {
        self.items.push(NativeMenuItem::Separator);
        self
    }
}

pub struct NativeMenuBar {
    pub app_name: String,
    /// Leading entries of the application menu, such as About and Settings.
    /// The platform appends its own window-management and quit entries.
    pub application_items: Vec<NativeMenuItem>,
    pub application_titles: NativeApplicationMenuTitles,
    /// Labels of the Edit and Window menus the platform appends.
    pub standard_titles: cove_platform::NativeStandardMenuTitles,
    pub menus: Vec<NativeMenu>,
}

impl NativeMenuBar {
    /// Whether the host window system provides a native menu bar.
    pub fn is_supported() -> bool {
        cove_shell::Sdl3Shell::native_menu_bar_supported()
    }

    pub fn new(
        app_name: impl Into<String>,
        application_items: Vec<NativeMenuItem>,
        menus: Vec<NativeMenu>,
    ) -> Self {
        let app_name = app_name.into();
        let application_titles = NativeApplicationMenuTitles::for_app_name(&app_name);
        Self {
            app_name,
            application_items,
            application_titles,
            standard_titles: cove_platform::NativeStandardMenuTitles::default(),
            menus,
        }
    }

    pub fn standard_titles(mut self, titles: cove_platform::NativeStandardMenuTitles) -> Self {
        self.standard_titles = titles;
        self
    }

    /// The `NSMenu`-neutral bar the shell installs, plus every authored action
    /// paired with the id the bar reports when the item is chosen.
    pub fn snapshot(
        &self,
    ) -> (
        cove_platform::NativeMenuBarSnapshot,
        Vec<(i64, NativeMenuAction)>,
    ) {
        fn walk(
            items: &[NativeMenuItem],
            next: &mut i64,
            actions: &mut Vec<(i64, NativeMenuAction)>,
            out: &mut Vec<cove_platform::NativeMenuItem>,
        ) {
            for item in items {
                match item {
                    NativeMenuItem::Separator => out.push(cove_platform::NativeMenuItem {
                        id: 0,
                        kind: cove_platform::NativeMenuKind::Separator,
                        title: String::new(),
                        enabled: false,
                        checked: false,
                        shortcut: None,
                        icon: None,
                    }),
                    NativeMenuItem::Submenu(menu) => {
                        let mut children = Vec::new();
                        walk(&menu.items, next, actions, &mut children);
                        out.push(cove_platform::NativeMenuItem {
                            id: 0,
                            kind: cove_platform::NativeMenuKind::Submenu(
                                cove_platform::NativeMenuSnapshot { items: children },
                            ),
                            title: menu.title.clone(),
                            enabled: true,
                            checked: false,
                            shortcut: None,
                            icon: None,
                        });
                    }
                    NativeMenuItem::Action(action) => {
                        let id = *next;
                        *next += 1;
                        out.push(cove_platform::NativeMenuItem {
                            id,
                            kind: cove_platform::NativeMenuKind::Regular,
                            title: action.title.clone(),
                            enabled: action.resolved_is_enabled(),
                            checked: action.is_selected,
                            shortcut: (!action.key_equivalent.is_empty())
                                .then(|| action.key_modifiers.shortcut(&action.key_equivalent)),
                            icon: None,
                        });
                        actions.push((id, action.clone()));
                    }
                }
            }
        }
        let mut actions = Vec::new();
        let mut next = 1;
        let mut application_items = Vec::new();
        walk(
            &self.application_items,
            &mut next,
            &mut actions,
            &mut application_items,
        );
        let mut menus = Vec::new();
        for menu in &self.menus {
            let mut items = Vec::new();
            walk(&menu.items, &mut next, &mut actions, &mut items);
            menus.push(cove_platform::NativeMenuBranch {
                title: menu.title.clone(),
                items: cove_platform::NativeMenuSnapshot { items },
            });
        }
        (
            cove_platform::NativeMenuBarSnapshot {
                app_name: self.app_name.clone(),
                application_titles: self.application_titles.clone(),
                standard_titles: self.standard_titles.clone(),
                application_items: cove_platform::NativeMenuSnapshot {
                    items: application_items,
                },
                menus,
            },
            actions,
        )
    }
}

impl From<Vec<NativeMenu>> for NativeMenuBar {
    fn from(menus: Vec<NativeMenu>) -> Self {
        Self::new("", Vec::new(), menus)
    }
}

/// Installs the menu bar through the platform shell.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod installer {
    use super::*;

    /// Installs the action table, the context-menu presenter and the AppKit
    /// application menu bar. A chosen bar item travels back as an id and runs
    /// its closure on the UI thread through the shell's deferred callbacks.
    ///
    /// Re-installing replaces the bar, which is how an application refreshes
    /// titles and enabled state: AppKit reads the snapshot's flags, so a live
    /// `is_enabled_provider` is re-evaluated here rather than inside AppKit.
    pub fn install(
        menu_bar: &NativeMenuBar,
        presenter: cove_core::scene::NativeMenuPresenterCallback,
        host: &cove_runtime::Sdl3PlatformHost,
    ) {
        let (snapshot, actions) = menu_bar.snapshot();
        native_menu_bridge::set_actions(actions);
        if cfg!(target_os = "macos") {
            cove_core::scene::NativeMenuPresenter::set_present(Some(presenter));
        }
        host.install_menu_bar(
            &snapshot,
            std::rc::Rc::new(|id| {
                native_menu_bridge::invoke(id);
            }),
        );
    }
}

/// Other platforms present Compose menus, so there is nothing native to install.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub mod installer {
    use super::*;

    pub fn install(
        _menu_bar: &NativeMenuBar,
        _presenter: cove_core::scene::NativeMenuPresenterCallback,
        _host: &cove_runtime::Sdl3PlatformHost,
    ) {
    }
}

pub use installer::install;

/// Action table the shell routes a chosen menu item back through, keyed by the
/// id the native bar reports.
pub mod native_menu_bridge {
    use super::NativeMenuAction;
    use std::{cell::RefCell, collections::BTreeMap};

    thread_local! {
        static ACTIONS: RefCell<BTreeMap<i64, NativeMenuAction>> =
            const { RefCell::new(BTreeMap::new()) };
    }

    pub fn set_actions(actions: Vec<(i64, NativeMenuAction)>) {
        ACTIONS.with(|slot| *slot.borrow_mut() = actions.into_iter().collect());
    }

    /// Runs the action with `id`, honoring its live enabled state.
    pub fn invoke(id: i64) -> bool {
        let action = ACTIONS.with(|slot| {
            slot.borrow()
                .get(&id)
                .map(|action| (action.resolved_is_enabled(), action.action.clone()))
        });
        match action {
            Some((true, action)) => {
                action();
                true
            }
            _ => false,
        }
    }

    pub fn clear() {
        ACTIONS.with(|slot| slot.borrow_mut().clear());
    }
}
