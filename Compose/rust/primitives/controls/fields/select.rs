//! Shared select values and keyboard shortcut model from Fields/Select.swift.
//! The popup-backed Select view is still being ported.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyboardShortcutPlatform {
    MacOS,
    Windows,
    Linux,
    Other,
}

impl KeyboardShortcutPlatform {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyboardShortcutModifier {
    Primary,
    Command,
    Control,
    Option,
    Shift,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyboardShortcut {
    pub modifiers: Vec<KeyboardShortcutModifier>,
    pub key: String,
}

impl KeyboardShortcut {
    pub fn new(
        key: impl Into<String>,
        modifiers: impl IntoIterator<Item = KeyboardShortcutModifier>,
    ) -> Self {
        Self {
            modifiers: modifiers.into_iter().collect(),
            key: key.into(),
        }
    }

    pub fn primary(key: impl Into<String>) -> Self {
        Self::new(key, [KeyboardShortcutModifier::Primary])
    }

    pub fn primary_shift(key: impl Into<String>) -> Self {
        Self::new(
            key,
            [
                KeyboardShortcutModifier::Primary,
                KeyboardShortcutModifier::Shift,
            ],
        )
    }

    pub fn display_string(&self) -> String {
        self.display_string_for(KeyboardShortcutPlatform::current())
    }

    pub fn display_string_for(&self, platform: KeyboardShortcutPlatform) -> String {
        let caps = self.caps(platform);
        match platform {
            KeyboardShortcutPlatform::MacOS => caps.concat(),
            KeyboardShortcutPlatform::Windows
            | KeyboardShortcutPlatform::Linux
            | KeyboardShortcutPlatform::Other => caps.join("+"),
        }
    }

    pub fn caps(&self, platform: KeyboardShortcutPlatform) -> Vec<String> {
        let mut resolved = Vec::new();
        for modifier in &self.modifiers {
            let modifier = if *modifier == KeyboardShortcutModifier::Primary {
                if platform == KeyboardShortcutPlatform::MacOS {
                    KeyboardShortcutModifier::Command
                } else {
                    KeyboardShortcutModifier::Control
                }
            } else {
                *modifier
            };
            if !resolved.contains(&modifier) {
                resolved.push(modifier);
            }
        }
        let mut caps: Vec<String> =
            resolved
                .into_iter()
                .map(|modifier| {
                    match platform {
                        KeyboardShortcutPlatform::MacOS => match modifier {
                            KeyboardShortcutModifier::Primary
                            | KeyboardShortcutModifier::Command => "⌘",
                            KeyboardShortcutModifier::Control => "⌃",
                            KeyboardShortcutModifier::Option => "⌥",
                            KeyboardShortcutModifier::Shift => "⇧",
                        },
                        KeyboardShortcutPlatform::Windows
                        | KeyboardShortcutPlatform::Linux
                        | KeyboardShortcutPlatform::Other => match modifier {
                            KeyboardShortcutModifier::Primary
                            | KeyboardShortcutModifier::Control => "Ctrl",
                            KeyboardShortcutModifier::Command => "Meta",
                            KeyboardShortcutModifier::Option => "Alt",
                            KeyboardShortcutModifier::Shift => "Shift",
                        },
                    }
                    .to_string()
                })
                .collect();
        caps.push(self.key.clone());
        caps
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SelectOption<Value> {
    pub value: Value,
    pub label: String,
    pub is_enabled: bool,
}

impl<Value> SelectOption<Value> {
    pub fn new(value: Value, label: impl Into<String>, is_enabled: bool) -> Self {
        Self {
            value,
            label: label.into(),
            is_enabled,
        }
    }

    pub fn id(&self) -> &Value {
        &self.value
    }
}

// MARK: - Select view

use std::cell::Cell;
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ActionControlAccessibility, ButtonIcon, ButtonIconSource};
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use crate::primitives::overlays::menu::{Menu, MenuEntry, MenuItem, MenuItemRole};
use crate::primitives::overlays::popover::{Popover, PopoverSizing, PopoverTriggerDescription};
use crate::primitives::text::Text;
use crate::theme::{SemanticFontRef, Theme, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, HoverPhase, Justify, Node, Wrap};
use cove_core::{Binding, Color};

/// Trigger height floor, matching the text fields beside it.
const TRIGGER_MIN_HEIGHT: f32 = 32.0;

/// Popup-backed single-value selector.
pub struct Select<Value: PartialEq + Clone + Send + Sync + 'static> {
    pub selection: Binding<Value>,
    pub options: Vec<SelectOption<Value>>,
    pub is_enabled: bool,
    pub width: Option<f32>,
    pub max_visible_rows: usize,
    pub placeholder: String,
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> Select<Value> {
    pub fn new(selection: Binding<Value>, options: Vec<SelectOption<Value>>) -> Self {
        Self {
            selection,
            options,
            is_enabled: true,
            width: None,
            max_visible_rows: 8,
            placeholder: "Select".to_string(),
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn max_visible_rows(mut self, rows: usize) -> Self {
        self.max_visible_rows = rows.max(1);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    fn selected_label(&self) -> String {
        self.options
            .iter()
            .find(|option| option.value == self.selection.get())
            .map(|option| option.label.clone())
            .unwrap_or_else(|| self.placeholder.clone())
    }

    fn menu_entries(&self) -> Vec<MenuEntry> {
        let current = self.selection.get();
        self.options
            .iter()
            .enumerate()
            .map(|(index, option)| {
                let selection = self.selection.clone();
                let value = option.value.clone();
                MenuEntry::Item(
                    MenuItem::new(index as u64, option.label.clone(), move || {
                        selection.set(value.clone())
                    })
                    .enabled(option.is_enabled)
                    .selected(option.value == current)
                    .role(MenuItemRole::Normal),
                )
            })
            .collect()
    }

    /// Border tone for the current trigger state. Open beats hover: the popup is
    /// open because of the trigger, so the trigger explains it even while the
    /// pointer has moved into the menu.
    fn border_color(&self, theme: &Theme, is_presented: bool, is_hovered: bool) -> Color {
        if !self.is_enabled {
            return theme.inputs.border_disabled;
        }
        if is_presented {
            return theme.inputs.border_focused;
        }
        if is_hovered {
            theme.inputs.border_hover
        } else {
            theme.inputs.border_color
        }
    }
}

impl<Value: PartialEq + Clone + Send + Sync + 'static> View for Select<Value> {
    fn body(&self) -> AnyView {
        let is_presented = Binding::new(
            {
                let state = select_presentation_state();
                move || state.get()
            },
            move |value: bool| select_presentation_state().set(value),
        );
        let selected_label = self.selected_label();
        let detail = self
            .options
            .iter()
            .find(|option| option.value == self.selection.get())
            .map(|option| option.label.clone())
            .unwrap_or_default();
        let trigger = AnyView::new(SelectTrigger {
            label: selected_label.clone(),
            is_presented: is_presented.get(),
            is_enabled: self.is_enabled,
            theme_inputs: None,
        });
        let popover = Popover::new(trigger, self.menu())
            .is_presented(is_presented)
            .enabled(self.is_enabled)
            .sizing(PopoverSizing::Anchor)
            .surface(PopoverSurface::Content)
            .overflow(PopoverOverflow::Flip)
            .trigger_description(PopoverTriggerDescription {
                label: selected_label.clone(),
                value: detail.clone(),
                help: None,
            });
        let popover = match self.width {
            Some(width) => popover.width(width),
            None => popover,
        };
        // The selected value is already the trigger's visible text, so it is the
        // control's name and value, not help. Swift provides the role and name as
        // composition locals (`Select.swift:213`, `Popover.swift:60`) so the
        // trigger `Button` picks them up in `Button::update_node`.
        AnyView::new(
            AnyView::new(popover)
                .composition_local(ActionControlAccessibility::role(), AccessibilityRole::ComboBox)
                .composition_local(ActionControlAccessibility::label(), Some(selected_label.clone()))
                .composition_local(ActionControlAccessibility::value(), detail),
        )
    }
}


impl<Value: PartialEq + Clone + Send + Sync + 'static> Select<Value> {
    fn menu(&self) -> AnyView {
        AnyView::new(Menu::new(self.menu_entries()).max_visible_rows(self.max_visible_rows))
    }
}


/// Presentation flag kept outside the view value so a toggle rebuilds it.
fn select_presentation_state() -> cove_core::State<bool> {
    thread_local! {
        static STATE: cove_core::State<bool> = cove_core::State::new(false);
    }
    STATE.with(|state| state.clone())
}

/// Trigger that reads as a text input: the same fill, border and radius the
/// text fields use, so a Select sits flush with the fields around it in a
/// property grid instead of looking like a lighter pill.
struct SelectTrigger {
    label: String,
    is_presented: bool,
    is_enabled: bool,
    theme_inputs: Option<()>,
}

impl View for SelectTrigger {
    fn body(&self) -> AnyView {
        let label = self.label.clone();
        let is_presented = self.is_presented;
        let is_enabled = self.is_enabled;
        let hovered = Rc::new(Cell::new(false));
        AnyView::new(SelectTriggerSurface {
            label,
            is_presented,
            is_enabled,
            hovered,
        })
    }
}

struct SelectTriggerSurface {
    label: String,
    is_presented: bool,
    is_enabled: bool,
    hovered: Rc<Cell<bool>>,
}

impl PrimitiveView for SelectTriggerSurface {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else {
            return;
        };
        let hovered = self.hovered.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let next = phase == HoverPhase::Enter;
                if hovered.get() != next {
                    hovered.set(next);
                }
            }),
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let border = if !self.is_enabled {
            theme.inputs.border_disabled
        } else if self.is_presented {
            theme.inputs.border_focused
        } else if self.hovered.get() {
            theme.inputs.border_hover
        } else {
            theme.inputs.border_color
        };
        let chevron = if self.is_presented {
            crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_up()
        } else {
            crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down()
        };
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        Text::new(self.label.clone())
                            .truncated()
                            .semantic_font(SemanticFontRef::body())
                            .foreground_color(if self.is_enabled {
                                theme.colors.on_surface
                            } else {
                                theme.colors.on_surface_muted
                            }),
                    ),
                    AnyView::new(ButtonIcon::new(
                        ButtonIconSource::resource(chevron),
                        Some(12.0),
                        Some(theme.colors.on_surface_muted),
                    )),
                ])),
            )
            .padding_edges(10.0, 6.0)
            .frame(None, None, None, Some(TRIGGER_MIN_HEIGHT), None, None)
            .background_color(theme.inputs.background)
            .corner_radius(theme.inputs.radius)
            .border_color(border, theme.inputs.border_width),
        )]
    }
}
crate::impl_primitive_view!(SelectTriggerSurface);
