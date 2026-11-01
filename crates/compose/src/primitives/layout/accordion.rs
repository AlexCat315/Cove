//! Translation of Primitives/Layout/Accordion.swift.
//!
//! Disclosure list with one Tab stop and wrapping arrow-key navigation. Single
//! expansion by default; `allows_multiple` leaves every open panel open.
use crate::Scope;
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::animation::content::AnimatedVisibility;
use crate::animation::transition::Transition;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource, ButtonRole};
use crate::primitives::overlays::roving_focus::{
    RovingFocusHost, RovingItem, SelectionNavigation, roving_identity,
};
use crate::primitives::text::Text;
use crate::theme::SemanticFontRef;
use crate::theme::styles::buttons::{ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt};
use crate::{AnyView, BoxView, Row, TupleView, VerticalAlignment, View, ViewExt};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::{Binding, Color};
use cove_platform::KeyEvent;

/// One collapsible section.
#[derive(Clone)]
pub struct AccordionItem<Id: PartialEq + Clone> {
    pub id: Id,
    pub title: String,
    pub is_enabled: bool,
    content: AnyView,
}

impl<Id: PartialEq + Clone> AccordionItem<Id> {
    pub fn new(id: Id, title: impl Into<String>, content: impl View + 'static) -> Self {
        Self {
            id,
            title: title.into(),
            is_enabled: true,
            content: AnyView::new(content),
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

/// Disclosure list of collapsible sections.
pub struct Accordion<Id: PartialEq + Clone + Ord + std::hash::Hash + Send + Sync + 'static> {
    pub items: Vec<AccordionItem<Id>>,
    pub expanded: Binding<BTreeSet<Id>>,
    pub allows_multiple: bool,
    pub is_enabled: bool,
    focused: cove_core::State<Option<Id>>,
}

impl<Id: PartialEq + Clone + Ord + std::hash::Hash + Send + Sync + 'static> Accordion<Id> {
    pub fn new(items: Vec<AccordionItem<Id>>, expanded: Binding<BTreeSet<Id>>) -> Self {
        assert!(
            items.iter().enumerate().all(|(index, item)| items
                .iter()
                .enumerate()
                .all(|(other, candidate)| other == index || candidate.id != item.id)),
            "Accordion IDs must be unique"
        );
        Self {
            items,
            expanded,
            allows_multiple: false,
            is_enabled: true,
            focused: cove_core::State::new(None),
        }
    }

    pub fn allows_multiple(mut self, allows_multiple: bool) -> Self {
        self.allows_multiple = allows_multiple;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// Toggle one section, respecting single-expansion mode.
    pub fn toggle(&self, id: &Id) {
        if !self.is_enabled
            || !self
                .items
                .iter()
                .any(|item| &item.id == id && item.is_enabled)
        {
            return;
        }
        let mut next = self.expanded.get();
        if next.contains(id) {
            next.remove(id);
        } else if self.allows_multiple {
            next.insert(id.clone());
        } else {
            next = BTreeSet::from([id.clone()]);
        }
        self.expanded.set(next);
    }
}

impl<Id: PartialEq + Clone + Ord + std::hash::Hash + Send + Sync + 'static> View for Accordion<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let enabled: Vec<Id> = self
            .items
            .iter()
            .filter(|item| self.is_enabled && item.is_enabled)
            .map(|item| item.id.clone())
            .collect();
        let tab_stop = self
            .focused
            .get()
            .filter(|id| enabled.contains(id))
            .or_else(|| enabled.first().cloned());
        let expanded = self.expanded.get();
        let is_enabled = self.is_enabled;
        let allows_multiple = self.allows_multiple;
        let expanded_binding = self.expanded.clone();
        let focused = self.focused.clone();
        let items_snapshot = self.items.clone();
        let item_count = self.items.len();
        let mut sections: Vec<AnyView> = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            let is_expanded = expanded.contains(&item.id);
            let item_enabled = is_enabled && item.is_enabled;
            let toggle_id = item.id.clone();
            let focused_for_button = focused.clone();
            let expanded_for_button = expanded_binding.clone();
            let items_for_toggle = items_snapshot.clone();
            let header = AnyView::new(
                Row::new(
                    VerticalAlignment::Center,
                    8.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(
                            Text::new(item.title.clone())
                                .truncated()
                                .semantic_font(SemanticFontRef::body_strong())
                                .flex(1.0)
                                .shrink(1.0)
                                .frame_min_width(0.0),
                        ),
                        AnyView::new(
                            AnyView::new(ButtonIcon::new(
                                ButtonIconSource::resource(if is_expanded {
                                    crate::primitives::media::ui_icon_catalog::navigation::chevron_down()
                                } else {
                                    crate::primitives::media::ui_icon_catalog::navigation::chevron_right()
                                }),
                                Some(12.0),
                                None,
                            ))
                            .modifier(crate::modifiers::interaction::AccessibilityModifier::new(
                                |semantics| semantics.is_hidden = true,
                            )),
                        ),
                    ])),
                )
                .padding(12.0),
            );
            let button = Button::with_options(
                header,
                move || {
                    if !items_for_toggle
                        .iter()
                        .any(|item| item.id == toggle_id && item.is_enabled)
                    {
                        return;
                    }
                    let mut next = expanded_for_button.get();
                    if next.contains(&toggle_id) {
                        next.remove(&toggle_id);
                    } else if allows_multiple {
                        next.insert(toggle_id.clone());
                    } else {
                        next = BTreeSet::from([toggle_id.clone()]);
                    }
                    expanded_for_button.set(next);
                    focused_for_button.set(Some(toggle_id.clone()));
                },
                ButtonRole::Normal,
                item_enabled,
                false,
                is_expanded,
                None,
            )
            .button_style(AccordionHeaderStyle);
            let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
            let is_tab_stop = tab_stop.as_ref() == Some(&item.id);
            let item_id_for_row = item.id.clone();
            let mut column: Vec<AnyView> = vec![AnyView::new(
                button
                    .modifier(RovingItem::new(item_id_for_row, is_tab_stop, identity))
                    .modifier(ExpandedState { is_expanded }),
            )];
            column.push(AnyView::new(AnimatedVisibility::new(
                is_expanded,
                Transition::collapse(),
                None,
                false,
                None,
                AnyView::new(item.content.clone().padding(12.0)),
            )));
            if index + 1 < item_count {
                column.push(AnyView::new(crate::primitives::text::Divider::new(
                    None,
                    1.0,
                    crate::primitives::text::DividerAxis::Horizontal,
                )));
            }
            sections.push(AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            )));
        }
        let selection = self.focused.clone();
        let selection_for_host = selection.clone();
        let enabled_for_host = enabled.clone();
        let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
        let command: Rc<dyn Fn(&KeyEvent) -> Option<SelectionNavigation>> =
            Rc::new(|event: &KeyEvent| {
                if !event.modifiers.is_empty() {
                    return None;
                }
                match event.scancode {
                    crate::Scancode::ARROW_UP => Some(SelectionNavigation::Previous),
                    crate::Scancode::ARROW_DOWN => Some(SelectionNavigation::Next),
                    crate::Scancode::HOME => Some(SelectionNavigation::First),
                    crate::Scancode::END => Some(SelectionNavigation::Last),
                    _ => None,
                }
            });
        let host = RovingFocusHost::new(
            enabled,
            move || selection_for_host.get(),
            move |id: Id| {
                if enabled_for_host.contains(&id) {
                    selection.set(Some(id));
                }
            },
            move |id: &Id| identity(id),
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(sections)),
            )),
        )
        .command(move |event: &KeyEvent| command(event));
        AnyView::new(
            host.semantic_background(crate::theme::SemanticColorRef::surface_raised())
                .semantic_border(crate::theme::SemanticColorRef::border(), 1.0)
                .corner_radius_metric(crate::theme::ThemeMetric::control_radius())
                .clipped(),
        )
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        crate::dynamic_fields!(self; focused)
    }
}

/// Header chrome: a solid hover face and the focus ring.
struct AccordionHeaderStyle;

impl ButtonStyle for AccordionHeaderStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let fill = if configuration.is_pressed {
            theme.colors.surface_pressed
        } else if configuration.is_hovered {
            theme.colors.surface_variant
        } else {
            Color::CLEAR
        };
        let row = BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(AnyView::new(configuration.label.clone()).flex_with(1.0, 1.0, None)),
        )
        .flex_with(1.0, 1.0, None)
        .foreground_color(if configuration.is_enabled {
            theme.colors.on_surface
        } else {
            theme.colors.on_surface_muted
        })
        .background_color(fill)
        .border_color(
            if configuration.is_focused {
                theme.colors.focus_ring
            } else {
                Color::CLEAR
            },
            2.0,
        )
        .opacity(if configuration.is_enabled { 1.0 } else { 0.55 });
        AnyView::new(row)
    }
}

/// Expanded state advertised to assistive technology on the header button.
struct ExpandedState {
    is_expanded: bool,
}

impl cove_composition::ViewModifier for ExpandedState {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node.accessibility().unwrap_or_else(|| {
            cove_core::scene::AccessibilitySemantics::new(AccessibilityRole::Button)
        });
        semantics.state.is_expanded = Some(self.is_expanded);
        node.set_accessibility(Some(semantics));
    }
}
