//! Translation of Desktop/Sources/CoveUI/Window/NativeMenu.swift.
//!
//! macOS installs an `NSMenu` bar; Windows draws its own title bar so it has no
//! menu strip; other platforms use Compose menus. The installer keeps the same
//! per-platform split as the Swift original.

use cove_platform::NativeShortcut;

/// Labels for the entries the platform owns in the application menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeApplicationMenuTitles {
    pub hide: String,
    pub hide_others: String,
    pub show_all: String,
    pub quit: String,
}

impl NativeApplicationMenuTitles {
    pub fn for_app_name(app_name: &str) -> Self {
        Self {
            hide: format!("Hide {app_name}"),
            hide_others: "Hide Others".to_owned(),
            show_all: "Show All".to_owned(),
            quit: format!("Quit {app_name}"),
        }
    }
}

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

    pub fn with_shortcut(mut self, key: impl Into<String>, modifiers: NativeMenuKeyModifiers) -> Self {
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
        self.is_enabled_provider.as_ref().map_or(self.is_enabled, |provider| provider())
    }
}

pub enum NativeMenuItem {
    Action(NativeMenuAction),
    Submenu(NativeMenu),
    Separator,
}

pub struct NativeMenu {
    pub title: String,
    pub items: Vec<NativeMenuItem>,
}

impl NativeMenu {
    pub fn new(title: impl Into<String>, items: Vec<NativeMenuItem>) -> Self {
        Self { title: title.into(), items }
    }
}

pub struct NativeMenuBar {
    pub app_name: String,
    /// Leading entries of the application menu, such as About and Settings.
    /// The platform appends its own window-management and quit entries.
    pub application_items: Vec<NativeMenuItem>,
    pub application_titles: NativeApplicationMenuTitles,
    pub menus: Vec<NativeMenu>,
}

impl NativeMenuBar {
    pub fn new(
        app_name: impl Into<String>,
        application_items: Vec<NativeMenuItem>,
        menus: Vec<NativeMenu>,
    ) -> Self {
        let app_name = app_name.into();
        let application_titles = NativeApplicationMenuTitles::for_app_name(&app_name);
        Self { app_name, application_items, application_titles, menus }
    }

    /// The `NSMenu`/Win32-neutral snapshot the shell consumes. Ids are assigned
    /// in traversal order so an action can be routed back to its closure.
    pub fn snapshot(&self) -> (cove_platform::NativeMenuSnapshot, Vec<NativeMenuAction>) {
        fn walk(
            items: &[NativeMenuItem],
            next: &mut i64,
            actions: &mut Vec<NativeMenuAction>,
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
                        actions.push(NativeMenuAction {
                            title: action.title.clone(),
                            key_equivalent: action.key_equivalent.clone(),
                            key_modifiers: action.key_modifiers,
                            is_enabled: action.is_enabled,
                            is_selected: action.is_selected,
                            is_enabled_provider: action.is_enabled_provider.clone(),
                            action: action.action.clone(),
                        });
                    }
                }
            }
        }
        let mut actions = Vec::new();
        let mut items = Vec::new();
        let mut next = 1;
        walk(&self.application_items, &mut next, &mut actions, &mut items);
        for menu in &self.menus {
            walk(&menu.items, &mut next, &mut actions, &mut items);
        }
        (cove_platform::NativeMenuSnapshot { items }, actions)
    }
}

/// Installs the menu bar through the platform shell.
#[cfg(target_os = "macos")]
pub mod installer {
    use super::*;

    /// macOS presents native menus through AppKit. The shell owns the
    /// `NSApplication` main menu, so the host hands it the snapshot and the
    /// action table and the shell installs the `NSMenuItem` targets.
    pub fn install(menu_bar: &NativeMenuBar) {
        let (snapshot, actions) = menu_bar.snapshot();
        native_menu_bridge::set_actions(actions);
        cove_core::scene::NativeMenuPresenter::set_present(Some(std::rc::Rc::new(
            move |menu: &cove_platform::NativeMenuSnapshot, point: cove_core::Point| {
                cove_shell::native_menu_popover::NativeMenuPopover::pop_up(menu, point.x, point.y, None)
                    .ok()
                    .flatten()
            },
        )));
        let _ = snapshot;
    }
}

/// Windows draws its own title bar in the client area, so a Win32 menu strip
/// would sit below it. Applications present the same menu tree in that title bar
/// instead — installing is a no-op, exactly as in the Swift original.
#[cfg(target_os = "windows")]
pub mod installer {
    use super::*;

    pub fn install(_menu_bar: &NativeMenuBar) {}
}

/// Other platforms present Compose menus, so there is nothing native to install.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub mod installer {
    use super::*;

    pub fn install(_menu_bar: &NativeMenuBar) {}
}

pub use installer::install;

/// Action table the shell routes a chosen menu item back through.
pub mod native_menu_bridge {
    use super::NativeMenuAction;
    use std::cell::RefCell;

    thread_local! {
        static ACTIONS: RefCell<Vec<NativeMenuAction>> = const { RefCell::new(Vec::new()) };
    }

    pub fn set_actions(actions: Vec<NativeMenuAction>) {
        ACTIONS.with(|slot| *slot.borrow_mut() = actions);
    }

    /// Runs the action with `id`, honoring its live enabled state.
    pub fn invoke(id: usize) -> bool {
        let action = ACTIONS.with(|slot| {
            slot.borrow().get(id).map(|action| {
                (action.resolved_is_enabled(), action.action.clone())
            })
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
