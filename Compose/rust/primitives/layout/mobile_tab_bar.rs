//! Translation of Primitives/Layout/Tabs/MobileBar.swift.
//!
//! Bottom tab bar for touch surfaces. Up to four tabs stay visible; when there
//! are more, the fourth slot follows the selection and the rest open from a
//! "more" menu.
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityRole, Align, FlexDirection, Justify, Wrap,
};
use cove_core::{Binding, State};
use cove_composition::{AnyView, View};

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::Button;
use crate::primitives::feedback::{Badge, BadgeVariant};
use crate::primitives::layout::insets::{SafeArea, SafeAreaEdges};
use crate::primitives::layout::tab_view::TabItem;
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::media::Icon;
use crate::primitives::overlays::anchored::PopoverPlacement;
use crate::primitives::overlays::menu::{MenuEntry, MenuItem};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::overlays::roving_focus::{roving_identity, RovingItem};
use crate::primitives::layout::overlay::OverlayViewExt;
use crate::{Row, TupleView};
use crate::theme::styles::buttons::{
    button_style::{ButtonStyle, ButtonStyleConfiguration},
    ButtonStyleViewExt,
};
use crate::theme::{SemanticColorRef, SemanticFontRef};
use crate::BoxView;

const MAX_VISIBLE: usize = 4;

/// Bottom tab bar for touch surfaces.
pub struct MobileTabBar<Id: std::hash::Hash + Eq + Clone + PartialEq + 'static> {
    tabs: Vec<TabItem<Id>>,
    selection: Binding<Id>,
    is_enabled: bool,
}

impl<Id: std::hash::Hash + Eq + Clone + PartialEq + 'static> MobileTabBar<Id> {
    pub fn new(
        tabs: Vec<TabItem<Id>>,
        selection: Binding<Id>,
        is_enabled: bool,
    ) -> Self {
        Self { tabs, selection, is_enabled }
    }

    fn visible_tabs(&self) -> Vec<TabItem<Id>> {
        if self.tabs.len() <= MAX_VISIBLE + 1 {
            return self.tabs.clone();
        }
        let mut result: Vec<TabItem<Id>> = self.tabs.iter().take(MAX_VISIBLE).cloned().collect();
        if let Some(selected) = self
            .tabs
            .iter()
            .find(|tab| tab.id == self.selection.get())
            && !result.iter().any(|tab| tab.id == selected.id)
        {
            if let Some(slot) = result.get_mut(MAX_VISIBLE - 1) {
                *slot = selected.clone();
            }
        }
        result
    }
}

impl<Id: std::hash::Hash + Eq + Clone + PartialEq + Send + Sync + 'static> View
    for MobileTabBar<Id>
{
    fn body(&self) -> AnyView {
        let tabs = self.tabs.clone();
        let selection = self.selection.clone();
        let is_enabled = self.is_enabled;
        let shows_more: State<bool> = State::new(false);

        let visible = self.visible_tabs();
        let visible_ids: std::collections::HashSet<u64> =
            visible.iter().map(|tab| roving_identity(&tab.id)).collect();
        let hidden: Vec<TabItem<Id>> = tabs
            .iter()
            .filter(|tab| !visible_ids.contains(&roving_identity(&tab.id)))
            .cloned()
            .collect();

        let mut row_items: Vec<AnyView> = Vec::new();
        for tab in &visible {
            row_items.push(tab_item(
                tab.clone(),
                selection.clone(),
                is_enabled,
            ));
        }
        if !hidden.is_empty() {
            row_items.push(more_item(
                hidden,
                selection.clone(),
                is_enabled,
                shows_more.clone(),
            ));
        }

        let row = Row::new(crate::VerticalAlignment::Center, 0.0, AnyView::new(TupleView::new(row_items)))
            .padding_edges(4.0, 6.0);
        let safe = SafeArea::new(
            SafeAreaEdges(SafeAreaEdges::HORIZONTAL.0 | SafeAreaEdges::BOTTOM.0),
            row,
        );
        let column = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(safe),
        )
        .semantic_background(SemanticColorRef::surface());
        AnyView::new(column)
    }
}

fn tab_item<Id: std::hash::Hash + Eq + Clone + PartialEq + Send + Sync + 'static>(
    tab: TabItem<Id>,
    selection: Binding<Id>,
    bar_enabled: bool,
) -> AnyView {
    let is_selected = selection.get() == tab.id;
    let tab_enabled = bar_enabled && tab.is_enabled;
    let action_selection = selection.clone();
    let action_id = tab.id.clone();

    let mut column_items: Vec<AnyView> = Vec::new();
    if let Some(icon) = &tab.icon {
        let icon_view: AnyView =
            AnyView::new(Icon::new(icon.clone(), 24.0).accessibility_hidden(true));
        let icon_view = if let Some(badge) = &tab.badge {
            let badge_view = Badge::new(badge.clone())
                .variant(BadgeVariant::Solid)
                .accessibility_hidden(true);
            AnyView::new(icon_view.overlay(crate::Alignment::TopTrailing, badge_view))
        } else {
            icon_view
        };
        column_items.push(icon_view);
    }
    column_items.push(AnyView::new(
        crate::primitives::text::Text::new(tab.label.clone())
            .truncated()
            .semantic_font(SemanticFontRef::caption())
            .accessibility_hidden(true),
    ));
    if tab.icon.is_none()
        && let Some(badge) = &tab.badge
    {
        column_items.push(AnyView::new(
            Badge::new(badge.clone())
                .variant(BadgeVariant::Solid)
                .accessibility_hidden(true),
        ));
    }

    let label = BoxView::new(
        FlexDirection::Column,
        Align::Center,
        Justify::FlexStart,
        Wrap::NoWrap,
        4.0,
        AnyView::new(TupleView::new(column_items)),
    )
    .frame_min_width(0.0);

    let button = Button::new(label, move || action_selection.set(action_id.clone()))
        .button_style(MobileTabStyle)
        .accessibility(move |semantics| {
            semantics.role = AccessibilityRole::Tab;
            semantics.label = tab.label.clone();
            semantics.value = tab.badge.clone().unwrap_or_default();
            semantics.state.is_selected = is_selected;
            semantics.state.is_enabled = tab_enabled;
        })
        .frame_min_width(0.0)
        .flex_with(1.0, 1.0, Some(0.0));

    AnyView::new(crate::ModifiedContent::new(
        AnyView::new(button),
        RovingItem::new(tab.id.clone(), is_selected, Rc::new(|id| roving_identity(id))),
    ))
}

fn more_item<Id: std::hash::Hash + Eq + Clone + PartialEq + Send + Sync + 'static>(
    hidden: Vec<TabItem<Id>>,
    selection: Binding<Id>,
    bar_enabled: bool,
    shows_more: State<bool>,
) -> AnyView {
    let mut menu_entries: Vec<MenuEntry> = Vec::new();
    for tab in &hidden {
        let id = roving_identity(&tab.id);
        let title = tab.label.clone();
        let item_enabled = bar_enabled && tab.is_enabled;
        let action_selection = selection.clone();
        let action_id = tab.id.clone();
        let action_more = shows_more.clone();
        let item = MenuItem::new(id, title, move || {
            action_selection.set(action_id.clone());
            action_more.set(false);
        })
        .enabled(item_enabled);
        menu_entries.push(MenuEntry::Item(item));
    }

    let trigger = BoxView::new(
        FlexDirection::Column,
        Align::Center,
        Justify::FlexStart,
        Wrap::NoWrap,
        4.0,
        AnyView::new(TupleView::new(vec![
            AnyView::new(Icon::new(ui_icon_catalog::action::more_horizontal(), 24.0).accessibility_hidden(true)),
            AnyView::new(
                crate::primitives::text::Text::new("More")
                    .truncated()
                    .semantic_font(SemanticFontRef::caption())
                    .accessibility_hidden(true),
            ),
        ])),
    )
    .frame_min_width(0.0);

    let popover = Popover::new(
        trigger,
        AnyView::new(crate::primitives::overlays::menu::Menu::new(menu_entries)),
    )
    .is_presented(shows_more.binding())
    .enabled(hidden.iter().any(|tab| tab.is_enabled))
    .placement(PopoverPlacement::TopTrailing)
    .width(220.0);

    AnyView::new(
        AnyView::new(popover)
            .frame_min_width(0.0)
            .flex_with(1.0, 1.0, Some(0.0)),
    )
}

/// Button style for a mobile tab: a tall column with an icon over a caption.
#[derive(Clone, Copy, Debug, Default)]
pub struct MobileTabStyle;

impl ButtonStyle for MobileTabStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let is_enabled = configuration.is_enabled;
        let is_selected = configuration.is_selected;
        let is_pressed = configuration.is_pressed;
        let is_focused = configuration.is_focused;
        let theme = configuration.theme.clone();

        let ink = if !is_enabled {
            SemanticColorRef::on_surface_muted()
        } else if is_selected {
            SemanticColorRef::accent()
        } else {
            SemanticColorRef::on_surface_variant()
        };

        let body = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            configuration.label,
        )
        .padding_edges(4.0, 6.0)
        .frame(
            None,
            None,
            None,
            Some(52.0),
            None,
            None,
        )
        .flex_with(1.0, 1.0, None)
        .semantic_foreground(ink)
        .corner_radius(12.0)
        .opacity(if is_enabled { 1.0 } else { 0.55 });

        let body = if is_pressed {
            AnyView::new(body.semantic_background(SemanticColorRef::state_layer_pressed()))
        } else {
            AnyView::new(body)
        };
        let body = if is_focused {
            AnyView::new(
                body.border_color(theme.colors.focus_ring, 2.0),
            )
        } else {
            body
        };
        body
    }

    fn style_identity(&self) -> u64 {
        0x4D_4F_42_54
    }
}
