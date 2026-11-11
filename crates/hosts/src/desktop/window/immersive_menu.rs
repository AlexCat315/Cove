use cove_composition::{AnyView, BoxView, Scope, TupleView, View};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_ui_compose::modifiers::layout::FlexExt;
use cove_ui_compose::primitives::controls::{
    KeyboardShortcut, KeyboardShortcutModifier, MenuButton,
};
use cove_ui_compose::primitives::overlays::ShortcutHost;
use cove_ui_compose::primitives::overlays::menu::{MenuEntry, MenuItem, MenuSubmenu};
use cove_ui_compose::primitives::overlays::portal::LayerRoot;
use cove_ui_compose::primitives::text::Text;
use cove_ui_compose::theme::styles::buttons::MenuBarItemButtonStyle;

use super::{
    ImmersiveWindowTitleBar, NativeMenu, NativeMenuAction, NativeMenuItem, NativeMenuKeyModifiers,
};

/// A Cove menu bar in the client title area. Pair it with
/// `AppWindowTitleBarStyle::HiddenInset` and mount it above the content.
pub struct ImmersiveMenuBar {
    menus: Vec<NativeMenu>,
    leading: Option<AnyView>,
    title: String,
}

impl ImmersiveMenuBar {
    pub fn new(menus: Vec<NativeMenu>) -> Self {
        Self {
            menus,
            leading: None,
            title: String::new(),
        }
    }

    pub fn leading(mut self, view: impl View + 'static) -> Self {
        self.leading = Some(AnyView::new(view));
        self
    }

    /// Displays a caption in the available title area. An empty caption hides it.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_content(self, content: impl View + 'static) -> AnyView {
        let column = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![
                AnyView::new(self),
                AnyView::new(AnyView::new(content).flex_with(1.0, 1.0, Some(0.0))),
            ])),
        );
        AnyView::new(LayerRoot::new(column))
    }
}

impl View for ImmersiveMenuBar {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut children: Vec<_> = self.leading.iter().cloned().collect();
        children.extend(self.menus.iter().map(|menu| {
            let title = menu.title.clone();
            AnyView::new(
                MenuButton::new(
                    entries(&menu.items, &mut 1),
                    AnyView::new(Text::new(&title).truncated()),
                )
                .trigger_style(MenuBarItemButtonStyle)
                .composed()
                .accessibility_label(title),
            )
        }));
        let mut shortcuts = Vec::new();
        for menu in &self.menus {
            collect_shortcuts(&menu.items, &mut shortcuts);
        }
        if !shortcuts.is_empty() {
            children.push(AnyView::new(ShortcutHost::new(move |event| {
                if event.is_repeat {
                    return false;
                }
                let Some(action) = shortcuts
                    .iter()
                    .find(|action| matches_shortcut(action, event) && action.resolved_is_enabled())
                else {
                    return false;
                };
                (action.action)();
                true
            })));
        }
        let row = BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            2.0,
            AnyView::new(TupleView::new(children)),
        );
        AnyView::new(ImmersiveWindowTitleBar::new(row).title(&self.title))
    }
}

fn entries(items: &[NativeMenuItem], next_id: &mut u64) -> Vec<MenuEntry> {
    items
        .iter()
        .map(|item| {
            let id = *next_id;
            *next_id += 1;
            match item {
                NativeMenuItem::Separator => MenuEntry::separator(id),
                NativeMenuItem::Submenu(menu) => MenuEntry::Submenu(MenuSubmenu::new(
                    id,
                    menu.title.clone(),
                    entries(&menu.items, next_id),
                )),
                NativeMenuItem::Action(action) => {
                    let callback = action.action.clone();
                    let provider = action.is_enabled_provider.clone();
                    let enabled = action.is_enabled;
                    let mut item = MenuItem::new(id, action.title.clone(), move || {
                        if provider.as_ref().map_or(enabled, |provider| provider()) {
                            callback();
                        }
                    })
                    .enabled(action.resolved_is_enabled())
                    .selected(action.is_selected);
                    if !action.key_equivalent.is_empty() {
                        let modifiers = [
                            (
                                NativeMenuKeyModifiers::COMMAND,
                                KeyboardShortcutModifier::Command,
                            ),
                            (
                                NativeMenuKeyModifiers::CONTROL,
                                KeyboardShortcutModifier::Control,
                            ),
                            (
                                NativeMenuKeyModifiers::OPTION,
                                KeyboardShortcutModifier::Option,
                            ),
                            (
                                NativeMenuKeyModifiers::SHIFT,
                                KeyboardShortcutModifier::Shift,
                            ),
                        ]
                        .into_iter()
                        .filter_map(|(flag, modifier)| {
                            action.key_modifiers.contains(flag).then_some(modifier)
                        });
                        item = item.shortcut(
                            KeyboardShortcut::new(&action.key_equivalent, modifiers)
                                .display_string(),
                        );
                    }
                    MenuEntry::Item(item)
                }
            }
        })
        .collect()
}

fn collect_shortcuts(items: &[NativeMenuItem], shortcuts: &mut Vec<NativeMenuAction>) {
    for item in items {
        match item {
            NativeMenuItem::Action(action) if !action.key_equivalent.is_empty() => {
                shortcuts.push(action.clone())
            }
            NativeMenuItem::Submenu(menu) => collect_shortcuts(&menu.items, shortcuts),
            _ => {}
        }
    }
}

fn matches_shortcut(action: &NativeMenuAction, event: &cove_platform::KeyEvent) -> bool {
    let key = action.key_modifiers.shortcut(&action.key_equivalent);
    if key.ctrl != event.modifiers.has_ctrl()
        || key.alt != event.modifiers.has_alt()
        || key.shift != event.modifiers.has_shift()
        || key.meta != event.modifiers.has_gui()
    {
        return false;
    }
    match key.key.to_ascii_uppercase().as_str() {
        "SPACE" => event.keycode == 32,
        "ENTER" => event.keycode == 13,
        "ESCAPE" => event.keycode == 27,
        "BACKSPACE" => event.keycode == 8,
        "DELETE" => event.keycode == 127,
        label if label.starts_with('F') && label.len() > 1 => {
            label[1..].parse::<u32>().ok().is_some_and(|key| {
                let scancode = match key {
                    1..=12 => 57 + key,
                    13..=24 => 91 + key,
                    _ => return false,
                };
                event.scancode == scancode
            })
        }
        label => {
            let mut chars = label.chars();
            let (Some(expected), None) = (chars.next(), chars.next()) else {
                return false;
            };
            char::from_u32(event.keycode)
                .is_some_and(|actual| expected.eq_ignore_ascii_case(&actual))
        }
    }
}
