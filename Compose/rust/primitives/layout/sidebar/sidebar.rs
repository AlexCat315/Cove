//! Translation of Primitives/Layout/Sidebar/Sidebar.swift.
//!
//! Navigation chrome with independent header/footer slots and one Tab stop among
//! enabled destinations.
use std::collections::HashSet;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityRole, Align, FlexDirection, FocusChainHolder, Justify, LayoutNode, Node, Wrap,
};
use cove_core::Binding;
use cove_platform::KeyEvent;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource, ButtonRole};
use crate::primitives::overlays::roving_focus::{
    roving_identity, RovingFocusHost, RovingItem, SelectionNavigation,
};
use crate::primitives::text::Text;
use crate::theme::SemanticFontRef;
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, TupleView, View};

use super::hierarchy::{SidebarEntry, SidebarHierarchy};
use super::items::{SidebarItem, SidebarSection};
use super::presentation::SidebarPresentation;

/// Navigation chrome with header and footer slots.
pub struct Sidebar<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    pub selection: Binding<Option<Id>>,
    pub sections: Vec<SidebarSection<Id>>,
    pub presentation: SidebarPresentation,
    pub expanded: Option<Binding<HashSet<Id>>>,
    pub minimum_row_height: f32,
    pub on_activate: Rc<dyn Fn(Id)>,
    pub local_expanded: cove_core::State<HashSet<Id>>,
    header: AnyView,
    footer: AnyView,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> Sidebar<Id> {
    pub fn new(selection: Binding<Option<Id>>, sections: Vec<SidebarSection<Id>>) -> Self {
        assert!(
            sections
                .iter()
                .enumerate()
                .all(|(index, section)| sections
                    .iter()
                    .enumerate()
                    .all(|(other, candidate)| other == index || candidate.id != section.id)),
            "Sidebar section IDs must be unique"
        );
        let hierarchy = SidebarHierarchy::new(&sections);
        let ids: Vec<Id> = hierarchy.entries().into_iter().map(|entry| entry.item.id).collect();
        assert!(
            ids.iter().enumerate().all(|(index, id)| ids
                .iter()
                .enumerate()
                .all(|(other, candidate)| other == index || candidate != id)),
            "Sidebar item IDs must be unique"
        );
        Self {
            selection,
            sections,
            presentation: SidebarPresentation::Expanded,
            expanded: None,
            minimum_row_height: 38.0,
            on_activate: Rc::new(|_| {}),
            local_expanded: cove_core::State::new(HashSet::new()),
            header: AnyView::new(crate::EmptyView),
            footer: AnyView::new(crate::EmptyView),
        }
    }

    pub fn presentation(mut self, presentation: SidebarPresentation) -> Self {
        self.presentation = presentation;
        self
    }

    pub fn expanded(mut self, expanded: Binding<HashSet<Id>>) -> Self {
        self.expanded = Some(expanded);
        self
    }

    pub fn minimum_row_height(mut self, height: f32) -> Self {
        self.minimum_row_height = if height.is_finite() { height.max(0.0) } else { 38.0 };
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(Id) + 'static) -> Self {
        self.on_activate = Rc::new(handler);
        self
    }

    pub fn header(mut self, header: impl View + 'static) -> Self {
        self.header = AnyView::new(header);
        self
    }

    pub fn footer(mut self, footer: impl View + 'static) -> Self {
        self.footer = AnyView::new(footer);
        self
    }

    fn expanded_set(&self) -> HashSet<Id> {
        match self.expanded.as_ref() {
            Some(binding) => binding.get(),
            None => self.local_expanded.get(),
        }
    }

    fn set_expanded(&self, next: HashSet<Id>) {
        match self.expanded.as_ref() {
            Some(binding) => {
                if binding.get() != next {
                    binding.set(next);
                }
            }
            None => self.local_expanded.set(next),
        }
    }

    /// One leading lane per section, even when some destinations omit an icon.
    /// Resolved against the live theme inside the row so a theme swap re-measures.
    fn icon_lane(entries: &[SidebarEntry<Id>], theme: &crate::theme::Theme) -> Option<f32> {
        entries
            .iter()
            .filter(|entry| entry.item.icon.is_some() || entry.item.slots.occupies_leading_lane())
            .map(|entry| entry.item.icon_size.resolve(theme))
            .max_by(|lhs, rhs| lhs.total_cmp(rhs))
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> View for Sidebar<Id> {
    fn body(&self) -> AnyView {
        let hierarchy = SidebarHierarchy::new(&self.sections);
        let expanded = self.expanded_set();
        let enabled = hierarchy.visible_enabled(&expanded);
        let tab_stop = self
            .selection
            .get()
            .filter(|selected| enabled.contains(selected))
            .or_else(|| enabled.first().cloned());
        let selected = self.selection.get();
        let presentation = self.presentation;
        let minimum_row_height = self.minimum_row_height;

        let mut section_views: Vec<AnyView> = Vec::new();
        for (index, section) in self.sections.iter().enumerate() {
            let entries: Vec<SidebarEntry<Id>> =
                hierarchy.groups.get(index).cloned().unwrap_or_default();
            let icon_width = Self::icon_lane(&entries);
            let mut rows: Vec<AnyView> = Vec::new();
            if !section.title.is_empty() && !presentation.is_rail() {
                rows.push(AnyView::new(
                    Text::new(section.title.clone())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted())
                        .padding_edges(8.0, 8.0),
                ));
            }
            for entry in &entries {
                if !entry.is_visible(&expanded) {
                    continue;
                }
                let is_selected = selected.as_ref() == Some(&entry.item.id);
                let is_expanded = expanded.contains(&entry.item.id);
                let item_tab_stop = tab_stop.as_ref() == Some(&entry.item.id);
                rows.push(self.row(entry, is_selected, is_expanded, item_tab_stop, icon_width, minimum_row_height));
            }
            let identity = section.id.clone();
            let column = AnyView::new(
                BoxView::new(
                    FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 3.0,
                    AnyView::new(TupleView::new(rows)),
                ),
            );
            section_views.push(AnyView::new(SectionIdentity {
                id: identity,
                content: column,
            }));
        }

        let hierarchy_for_host = hierarchy.clone();
        let selection = self.selection.clone();
        let enabled_for_host = enabled.clone();
        let expanded_for_host = expanded.clone();
        let expanded_binding = self.expanded.clone();
        let local_expanded = self.local_expanded.clone();
        let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
        let command: Rc<dyn Fn(&KeyEvent) -> Option<SelectionNavigation>> = Rc::new(|event: &KeyEvent| {
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
        let selection_for_host = selection.clone();
        let host = RovingFocusHost::new(
            enabled,
            move || selection_for_host.get(),
            {
                let hierarchy = hierarchy_for_host.clone();
                let selection = selection.clone();
                let expanded_for_host = expanded_for_host.clone();
                let expanded_binding = expanded_binding.clone();
                let local_expanded = local_expanded.clone();
                let enabled_for_host = enabled_for_host.clone();
                move |id: Id| {
                    if !enabled_for_host.contains(&id) {
                        return;
                    }
                    match hierarchy.entry(&id) {
                        Some(entry) if !entry.is_branch() => selection.set(Some(id)),
                        Some(_) => {
                            let mut next = expanded_for_host.clone();
                            if !next.remove(&id) {
                                next.insert(id);
                            }
                            match expanded_binding.as_ref() {
                                Some(binding) => binding.set(next),
                                None => local_expanded.set(next),
                            }
                        }
                        None => {}
                    }
                }
            },
            move |id: &Id| identity(id),
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 10.0,
                    AnyView::new(TupleView::new(section_views)),
                ),
            ),
        )
        .command(move |event: &KeyEvent| command(event));

        // Overlay gutter: a reserved lane insets only the trailing edge, which
        // would make every selected destination sit further from one edge.
        let scroller = crate::primitives::layout::ScrollView::vertical(AnyView::new(host));
        let body = AnyView::new(
            BoxView::new(
                FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(self.header.clone().flex_with(0.0, 0.0, None)),
                    AnyView::new(AnyView::new(scroller).flex_with(1.0, 1.0, Some(0.0))),
                    AnyView::new(self.footer.clone().flex_with(0.0, 0.0, None)),
                ])),
            ),
        );
        AnyView::new(body.accessibility(|semantics| {
            semantics.role = AccessibilityRole::List;
        }))
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> Sidebar<Id> {
    fn row(
        &self,
        entry: &SidebarEntry<Id>,
        is_selected: bool,
        is_expanded: bool,
        is_tab_stop: bool,
        icon_width: Option<f32>,
        minimum_row_height: f32,
    ) -> AnyView {
        let item = &entry.item;
        let is_enabled = entry.is_enabled;
        let is_branch = entry.is_branch();
        let depth = entry.ancestors.len();
        let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
        let row_identity = item.id.clone();

        let mut lanes: Vec<AnyView> = Vec::new();
        if let Some(width) = icon_width {
            let glyph = match item.icon.clone() {
                Some(icon) => AnyView::new(ButtonIcon::new(
                    ButtonIconSource::resource(icon),
                    Some(width),
                    None,
                )),
                None => AnyView::new(crate::EmptyView),
            };
            lanes.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row, Align::Center, Justify::Center, Wrap::NoWrap, 0.0, glyph,
                )
                .frame_width(width)
                .flex_with(0.0, 0.0, None),
            ));
        }
        lanes.push(AnyView::new(
            Text::new(item.title.clone())
                .truncated()
                .semantic_font(SemanticFontRef::label())
                .flex_with(1.0, 1.0, None)
                .frame_min_width(0.0),
        ));
        if let Some(badge) = item.badge.clone() {
            lanes.push(AnyView::new(
                crate::primitives::feedback::Badge::new(badge).enabled(is_enabled),
            ));
        }
        if let Some(trailing) = item.slots.trailing.clone() {
            lanes.push(AnyView::new(trailing.flex_with(0.0, 0.0, None)));
        }

        let label = AnyView::new(
            BoxView::new(
                FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::NoWrap, 10.0,
                AnyView::new(TupleView::new(lanes)),
            )
            .padding_insets(crate::modifiers::layout::EdgeInsets::new(
                0.0,
                10.0 + depth.min(5) as f32 * 16.0,
                0.0,
                10.0,
            ))
            .frame(None, None, None, Some(minimum_row_height), None, None),
        );

        let selection = self.selection.clone();
        let on_activate = self.on_activate.clone();
        let expanded_binding = self.expanded.clone();
        let local_expanded = self.local_expanded.clone();
        let expanded_now = self.expanded_set();
        let id = item.id.clone();
        let button = Button::with_options(
            label,
            {
                let id = id.clone();
                let selection = selection.clone();
                let on_activate = on_activate.clone();
                let expanded_binding = expanded_binding.clone();
                let local_expanded = local_expanded.clone();
                let expanded_now = expanded_now.clone();
                move || {
                    if is_branch {
                        let mut next = expanded_now.clone();
                        if !next.remove(&id) {
                            next.insert(id.clone());
                        }
                        match expanded_binding.as_ref() {
                            Some(binding) => binding.set(next),
                            None => local_expanded.set(next),
                        }
                    } else {
                        selection.set(Some(id.clone()));
                        on_activate(id.clone());
                    }
                }
            },
            ButtonRole::Normal,
            is_enabled,
            false,
            is_selected,
            None,
        )
        .button_style(SidebarItemStyle { minimum_height: minimum_row_height });

        AnyView::new(SidebarRowBody {
            button,
            row_id: row_identity,
            is_tab_stop,
            identity,
            label: item.title.clone(),
            value: item.badge.clone().unwrap_or_default(),
            is_expanded: is_branch.then_some(is_expanded),
        })
    }
}

/// Row semantics: label, badge value and branch expanded state.
struct SidebarRowSemantics {
    label: String,
    value: String,
    is_expanded: Option<bool>,
}

impl cove_composition::ViewModifier for SidebarRowSemantics {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node.accessibility().unwrap_or_else(|| {
            cove_core::scene::AccessibilitySemantics::new(AccessibilityRole::Button)
        });
        semantics.label = self.label.clone();
        semantics.value = self.value.clone();
        semantics.state.is_expanded = self.is_expanded;
        node.set_accessibility(Some(semantics));
    }
}

/// Destination row chrome: a solid selection band that does not move the text.
struct SidebarItemStyle {
    minimum_height: f32,
}

impl crate::theme::styles::buttons::ButtonStyle for SidebarItemStyle {
    fn make_body(&self, configuration: crate::theme::styles::buttons::ButtonStyleConfiguration) -> AnyView {
        let theme = for_style(&configuration);
        let appearance = crate::theme::styles::components::selection_appearance::SelectionAppearance::resolve(
            crate::theme::styles::components::selection_appearance::SelectionRole::Sidebar,
            &theme,
        );
        let fill = if configuration.is_pressed {
            appearance.pressed_fill
        } else if configuration.is_selected {
            appearance.fill
        } else if configuration.is_hovered {
            appearance.hover_fill
        } else {
            cove_core::Color::CLEAR
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::NoWrap, 0.0,
                AnyView::new(configuration.label.clone()).flex_with(1.0, 1.0, None),
            )
            .flex_with(1.0, 1.0, None)
            .frame(None, None, Some(0.0), Some(self.minimum_height), None, None)
            .foreground_color(if !configuration.is_enabled {
                theme.colors.on_surface_muted
            } else if configuration.is_selected {
                theme.colors.on_surface
            } else {
                theme.colors.on_surface_variant
            })
            .background_color(fill)
            .corner_radius(appearance.corner_radius),
        )
    }
}

fn for_style(configuration: &crate::theme::styles::buttons::ButtonStyleConfiguration) -> crate::theme::Theme {
    configuration.theme.clone()
}
