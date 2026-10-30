//! Translation of Primitives/Controls/MenuButton.swift.
//!
//! A button that opens a menu anchored to itself. It uses the regular popover
//! trigger contract, so the menu escapes clipping, closes on Escape or an
//! outside click, and exposes expanded state to accessibility.
use std::sync::{Arc, Mutex};

use cove_core::scene::{Align, FlexDirection, Justify, Node, Wrap};
use cove_core::Binding;

use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::overlays::anchored::PopoverPlacement;
use crate::primitives::overlays::menu::{Menu, MenuEntry};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, SecondaryButtonStyle};
use crate::theme::SemanticFontRef;
use crate::modifiers::layout::LayoutFrameExt;
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, TupleView, View};

/// A button that opens a menu.
pub struct MenuButton<Label: View + Clone + 'static> {
    pub entries: Vec<MenuEntry>,
    pub is_enabled: bool,
    pub width: Option<f32>,
    pub placement: PopoverPlacement,
    pub accessibility_label: Option<String>,
    pub tooltip: Option<String>,
    pub label: Label,
    presented: Arc<Mutex<bool>>,
}

impl<Label: View + Clone + 'static> MenuButton<Label> {
    pub fn new(entries: Vec<MenuEntry>, label: Label) -> Self {
        Self {
            entries,
            is_enabled: true,
            width: None,
            placement: PopoverPlacement::BottomLeading,
            accessibility_label: None,
            tooltip: None,
            label,
            presented: Arc::new(Mutex::new(false)),
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

    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The popover trigger, exposed for hosts that need the chrome alone.
    pub fn menu(&self) -> Menu {
        let menu = Menu::new(self.entries.clone());
        let menu = match self.width {
            Some(width) => menu.width(width),
            None => menu,
        };
        let presented = self.presented.clone();
        menu.on_item_activated(move || {
            *presented.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = false;
        })
    }
}

impl<Label: View + Clone + 'static> View for MenuButton<Label> {
    fn body(&self) -> AnyView {
        let presented = Binding::new(
            {
                let state = self.presented.clone();
                move || *state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
            },
            {
                let state = self.presented.clone();
                move |value: bool| {
                    *state.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
                }
            },
        );
        let label = AnyView::new(
            Button::with_options(
                self.label.clone(),
                || {},
                ButtonRole::Normal,
                self.is_enabled,
                false,
                false,
                self.tooltip.clone(),
            )
            .button_style(SecondaryButtonStyle),
        );
        let mut popover = Popover::new(label, AnyView::new(self.menu()))
            .is_presented(presented)
            .enabled(self.is_enabled)
            .placement(self.placement);
        if let Some(width) = self.width {
            popover = popover.width(width);
        }
        if let Some(label) = self.accessibility_label.as_ref() {
            popover = popover.trigger_description(crate::primitives::overlays::popover::PopoverTriggerDescription {
                label: label.clone(),
                value: String::new(),
                help: self.tooltip.clone(),
            });
        }
        AnyView::new(popover)
    }
}

/// A text-only menu button: the label and the dropdown arrow drawn together, so
/// it reads as a menu trigger.
pub struct MenuButtonTitle {
    pub title: String,
    pub entries: Vec<MenuEntry>,
    pub is_enabled: bool,
    pub width: Option<f32>,
    pub placement: PopoverPlacement,
    pub tooltip: Option<String>,
    presented: Arc<Mutex<bool>>,
}

impl MenuButtonTitle {
    pub fn new(title: impl Into<String>, entries: Vec<MenuEntry>) -> Self {
        Self {
            title: title.into(),
            entries,
            is_enabled: true,
            width: None,
            placement: PopoverPlacement::BottomLeading,
            tooltip: None,
            presented: Arc::new(Mutex::new(false)),
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

    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl View for MenuButtonTitle {
    fn body(&self) -> AnyView {
        let title = self.title.clone();
        let label = MenuButton::new(
            self.entries.clone(),
            AnyView::new(MenuButtonLabel { title: title.clone() }),
        )
        .enabled(self.is_enabled)
        .placement(self.placement)
        .accessibility_label(title);
        let label = match self.width {
            Some(width) => label.width(width),
            None => label,
        };
        let label = match self.tooltip.clone() {
            Some(tooltip) => label.tooltip(tooltip),
            None => label,
        };
        AnyView::new(label)
    }
}

/// The title and its dropdown chevron.
struct MenuButtonLabel {
    title: String,
}

impl PrimitiveView for MenuButtonLabel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::NoWrap, 6.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(Text::new(self.title.clone()).truncated()),
                    AnyView::new(crate::primitives::controls::button::ButtonIcon::new(
                        crate::primitives::controls::button::ButtonIconSource::resource(
                            crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down(),
                        ),
                        Some(12.0),
                        None,
                    )),
                ])),
            )
            .frame_min_width(0.0),
        )]
    }
}
impl_primitive_view!(MenuButtonLabel);

impl MenuButtonTitle {
    /// The label font used by the text-only menu button.
    pub fn font() -> SemanticFontRef {
        SemanticFontRef::label()
    }
}

impl<Label: View + Clone + 'static> MenuButton<Label> {
    /// Whether the menu is currently open.
    pub fn is_presented(&self) -> bool {
        *self.presented.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl MenuButtonTitle {
    /// Whether the menu is currently open.
    pub fn is_presented(&self) -> bool {
        *self.presented.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Clone for MenuButtonTitle {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            entries: self.entries.clone(),
            is_enabled: self.is_enabled,
            width: self.width,
            placement: self.placement,
            tooltip: self.tooltip.clone(),
            presented: self.presented.clone(),
        }
    }
}

impl<Label: View + Clone + 'static> Clone for MenuButton<Label> {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            is_enabled: self.is_enabled,
            width: self.width,
            placement: self.placement,
            accessibility_label: self.accessibility_label.clone(),
            tooltip: self.tooltip.clone(),
            label: self.label.clone(),
            presented: self.presented.clone(),
        }
    }
}
