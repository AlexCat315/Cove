//! Translation of Primitives/Layout/Sidebar/Sidebar.swift.
//!
//! Navigation chrome with independent header/footer slots and one Tab stop among
//! enabled destinations.
use crate::Scope;
use crate::primitives::DuplicateId;
use crate::primitives::layout::sidebar::items::SidebarSections;
use std::collections::HashSet;
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, FrameDimension, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::overlays::roving_focus::{
    RovingFocusHost, RovingItem, SelectionNavigation, roving_identity,
};
use crate::primitives::text::Text;
use crate::theme::SemanticFontRef;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_platform::KeyEvent;

use super::hierarchy::{SidebarEntry, SidebarHierarchy};
use super::items::SidebarSection;
use super::presentation::SidebarPresentation;

/// Navigation chrome with header and footer slots.
pub struct Sidebar<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    pub selection: Binding<Option<Id>>,
    sections: SidebarSections<Id>,
    pub presentation: SidebarPresentation,
    pub expanded: Option<Binding<HashSet<Id>>>,
    pub minimum_row_height: f32,
    pub on_activate: Rc<dyn Fn(Id)>,
    /// Resolved once per body pass so section lanes and row chrome agree.
    ambient_theme: crate::theme::Theme,
    header: AnyView,
    footer: AnyView,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> Sidebar<Id> {
    pub fn try_new(
        selection: Binding<Option<Id>>,
        sections: Vec<SidebarSection<Id>>,
    ) -> Result<Self, DuplicateId> {
        Ok(Self::from_sections(
            selection,
            SidebarSections::new(sections)?,
        ))
    }

    pub(crate) fn from_sections(
        selection: Binding<Option<Id>>,
        sections: SidebarSections<Id>,
    ) -> Self {
        Self {
            selection,
            sections,
            presentation: SidebarPresentation::Expanded,
            expanded: None,
            minimum_row_height: 38.0,
            on_activate: Rc::new(|_| {}),
            ambient_theme: crate::theme::styles::default_light_theme::value(),
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
        self.minimum_row_height = if height.is_finite() {
            height.max(0.0)
        } else {
            38.0
        };
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
    fn body(&self, cx: &Scope) -> AnyView {
        let local_expanded = cx.state(HashSet::<Id>::new);
        let hierarchy = SidebarHierarchy::new(&self.sections.0);
        let expanded = self
            .expanded
            .as_ref()
            .map_or_else(|| local_expanded.get(), |value| value.get());
        let enabled = hierarchy.visible_enabled(&expanded);
        let tab_stop = self
            .selection
            .get()
            .filter(|selected| enabled.contains(selected))
            .or_else(|| enabled.first().cloned());
        let selected = self.selection.get();
        let presentation = self.presentation;
        let minimum_row_height = self.minimum_row_height;
        let gutter = presentation.gutter();
        let slot = |content: AnyView| {
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    content,
                )
                .padding_edges(gutter, 0.0)
                .flex_with(0.0, 0.0, None),
            )
        };

        let mut section_views: Vec<AnyView> = Vec::new();
        for (index, section) in self.sections.0.iter().enumerate() {
            let entries: Vec<SidebarEntry<Id>> =
                hierarchy.groups.get(index).cloned().unwrap_or_default();
            let icon_width = self.icon_lane_resolved(&entries);
            let section_has_branches = entries.iter().any(|entry| entry.is_branch());
            let mut rows: Vec<AnyView> = Vec::new();
            if !section.title.is_empty() && !presentation.is_rail() {
                rows.push(AnyView::new(
                    Text::new(section.title.clone())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted())
                        .padding_edges(10.0, 6.0),
                ));
            }
            for entry in &entries {
                if !entry.is_visible(&expanded) {
                    continue;
                }
                let is_selected = selected.as_ref() == Some(&entry.item.id);
                let is_expanded = expanded.contains(&entry.item.id);
                let item_tab_stop = tab_stop.as_ref() == Some(&entry.item.id);
                let expanded_binding = match self.expanded.clone() {
                    Some(binding) => binding,
                    None => Binding::new(
                        {
                            let local = local_expanded;
                            move || local.get()
                        },
                        {
                            let local = local_expanded;
                            move |value: HashSet<Id>| local.set(value)
                        },
                    ),
                };
                let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
                let destination = crate::primitives::layout::sidebar::SidebarDestination {
                    entry: entry.clone(),
                    selection: self.selection.clone(),
                    expanded: expanded_binding,
                    is_selected,
                    is_expanded,
                    is_tab_stop: item_tab_stop,
                    icon_width,
                    minimum_row_height,
                    on_activate: self.on_activate.clone(),
                    presentation,
                    icon_extent: icon_width.unwrap_or(20.0),
                    disclosure_lane: section_has_branches,
                    context_label: hierarchy.context(entry),
                };
                rows.push(AnyView::new(AnyView::new(destination).with_modifier(
                    RovingItem::new(entry.item.id.clone(), item_tab_stop, identity),
                )));
            }
            let identity = section.id.clone();
            let column = AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                2.0,
                AnyView::new(TupleView::new(rows)),
            ));
            section_views.push(AnyView::new(SectionIdentity {
                id: identity,
                content: column,
            }));
        }

        let hierarchy_for_host = hierarchy.clone();
        let selection = self.selection.clone();
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
        let selection_for_host = selection.clone();
        let keyboard_hierarchy = hierarchy.clone();
        let keyboard_expanded = self
            .expanded
            .clone()
            .unwrap_or_else(|| local_expanded.binding());
        let keyboard_selection = self.selection.clone();
        let host = RovingFocusHost::new(
            enabled,
            move || selection_for_host.get(),
            {
                let hierarchy = hierarchy_for_host.clone();
                let selection = selection.clone();
                let enabled_for_host = enabled_for_host.clone();
                move |id: Id| {
                    if !enabled_for_host.contains(&id) {
                        return;
                    }
                    match hierarchy.entry(&id) {
                        Some(entry) if !entry.is_branch() => selection.set(Some(id)),
                        Some(_) => {}
                        None => {}
                    }
                }
            },
            move |id: &Id| identity(id),
            AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    10.0,
                    AnyView::new(TupleView::new(section_views)),
                )
                .padding_edges(gutter, 4.0),
            ),
        )
        .command(move |event: &KeyEvent| command(event))
        .directional_command(move |event, source, owner| {
            if !event.modifiers.is_empty() {
                return None;
            }
            let disclose = match event.scancode {
                crate::Scancode::ARROW_RIGHT => !owner.resolves_right_to_left(),
                crate::Scancode::ARROW_LEFT => owner.resolves_right_to_left(),
                _ => return None,
            };
            let entry = keyboard_hierarchy.entry(source)?;
            let mut expanded = keyboard_expanded.get();
            let mut target = source.clone();
            if disclose {
                if entry.is_branch() && expanded.insert(source.clone()) {
                    keyboard_expanded.set(expanded);
                } else if entry.is_branch()
                    && let Some(child) =
                        keyboard_hierarchy.entries().into_iter().find(|candidate| {
                            candidate.ancestors.last() == Some(source)
                                && candidate.is_enabled
                                && candidate.is_visible(&expanded)
                        })
                {
                    target = child.item.id;
                }
            } else if entry.is_branch() && expanded.remove(source) {
                keyboard_expanded.set(expanded);
            } else if let Some(parent) = entry.ancestors.last() {
                target = parent.clone();
            }
            if keyboard_hierarchy
                .entry(&target)
                .is_some_and(|entry| !entry.is_branch())
            {
                keyboard_selection.set(Some(target.clone()));
            }
            Some(target)
        });

        // Overlay gutter: a reserved lane insets only the trailing edge, which
        // would make every selected destination sit further from one edge.
        let scroller = crate::primitives::layout::ScrollView::vertical(AnyView::new(host));
        let body = AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![
                slot(self.header.clone()),
                AnyView::new(AnyView::new(scroller).flex_with(1.0, 1.0, Some(0.0))),
                slot(self.footer.clone()),
            ])),
        ));
        // At rest the edge between navigation and content is the change of
        // surface; the resize handle draws a line only while grabbed.
        let rail_width = self.presentation.rail_width();
        let body = AnyView::new(
            AnyView::new(body.semantic_background(crate::theme::SemanticColorRef::surface()))
                // Always a width: the modifier leaves an unset dimension alone, so
                // leaving the rail with `None` would keep the rail's width.
                .frame(
                    Some(rail_width.map_or(FrameDimension::Percent(100.0), FrameDimension::Points)),
                    None,
                    Some(0.0),
                    Some(0.0),
                    None,
                    None,
                ),
        );
        AnyView::new(body.accessibility(|semantics| {
            semantics.role = AccessibilityRole::List;
        }))
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> Sidebar<Id> {
    /// Icon lane width for one section, resolved from the ambient theme.
    fn icon_lane_resolved(&self, entries: &[SidebarEntry<Id>]) -> Option<f32> {
        let theme = self.ambient_theme.clone();
        Self::icon_lane(entries, &theme)
    }
}

/// Gives a section column a stable identity across recompositions.
struct SectionIdentity {
    id: String,
    content: AnyView,
}

impl PrimitiveView for SectionIdentity {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment("__sidebar.section", self.id.clone());
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SectionIdentity);

/// The painted row, built in one pass so the modifier chain never needs erasing.
struct SidebarChromeRow {
    label: AnyView,
    minimum_height: f32,
    fill: cove_core::Color,
    ink: cove_core::Color,
    radius: f32,
}

impl PrimitiveView for SidebarChromeRow {
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
        let row = BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(self.label.clone().with_modifier(FlexGrowOne)),
        )
        .foreground_color(self.ink)
        .background_color(self.fill)
        .corner_radius(self.radius)
        .flex_with(1.0, 1.0, None)
        .frame(None, None, Some(0.0), Some(self.minimum_height), None, None);
        vec![AnyView::new(row)]
    }
}
crate::impl_primitive_view!(SidebarChromeRow);

/// Lets the label absorb the remaining row width.
struct FlexGrowOne;

impl cove_composition::ViewModifier for FlexGrowOne {
    fn apply_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
    }
}

/// Destination row chrome: a selection band that fills the row and does not move
/// the text.
pub struct SidebarItemStyle {
    pub minimum_height: f32,
}

impl crate::theme::styles::buttons::ButtonStyle for SidebarItemStyle {
    fn make_body(
        &self,
        configuration: crate::theme::styles::buttons::ButtonStyleConfiguration,
    ) -> AnyView {
        let theme = configuration.theme.clone();
        let appearance =
            crate::theme::styles::components::selection_appearance::SelectionAppearance::resolve(
                crate::theme::styles::components::selection_appearance::SelectionRole::Sidebar,
                &theme,
            );
        let fill = appearance.fill(
            configuration.is_selected,
            configuration.is_hovered,
            configuration.is_pressed,
        );
        let ink = if !configuration.is_enabled {
            theme.colors.on_surface_muted
        } else if configuration.is_selected {
            theme.colors.on_surface
        } else {
            theme.colors.on_surface_variant
        };
        AnyView::new(SidebarChromeRow {
            label: configuration.label.clone(),
            minimum_height: self.minimum_height,
            fill,
            ink,
            radius: appearance.corner_radius,
        })
    }
}
