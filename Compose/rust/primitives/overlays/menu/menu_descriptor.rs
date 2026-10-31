//! Data-only context-menu descriptors independent of the menu renderer.
use crate::primitives::controls::KeyboardShortcut;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct MenuDescriptor {
    pub items: Vec<MenuItemDescriptor>,
}

impl MenuDescriptor {
    pub fn new(items: Vec<MenuItemDescriptor>) -> Self {
        Self { items }
    }
}

#[derive(Clone)]
pub enum MenuItemDescriptor {
    Separator,
    Label(String),
    Submenu {
        title: String,
        is_enabled: bool,
        items: Vec<MenuItemDescriptor>,
    },
    Action {
        title: String,
        shortcut: Option<KeyboardShortcut>,
        is_enabled: bool,
        action: Arc<dyn Fn() + Send + Sync>,
    },
}

impl MenuItemDescriptor {
    pub fn submenu(title: impl Into<String>, is_enabled: bool, items: Vec<Self>) -> Self {
        Self::Submenu {
            title: title.into(),
            is_enabled,
            items,
        }
    }

    pub fn action(
        title: impl Into<String>,
        shortcut: Option<KeyboardShortcut>,
        is_enabled: bool,
        action: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::Action {
            title: title.into(),
            shortcut,
            is_enabled,
            action: Arc::new(action),
        }
    }

    pub fn invoke(&self) -> bool {
        match self {
            Self::Action {
                is_enabled: true,
                action,
                ..
            } => {
                action();
                true
            }
            _ => false,
        }
    }
}
