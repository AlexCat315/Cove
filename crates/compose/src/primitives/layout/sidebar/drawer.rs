//! Translation of Primitives/Layout/Sidebar/Drawer.swift.
//!
//! Modal navigation, sharing sidebar destinations and the standard modal
//! lifecycle. The application owns presentation and selection, so resizing never
//! resets them.
use std::collections::HashSet;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::PaddingExt;
use crate::modifiers::style::AppearanceExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::interaction::horizontal_swipe::{HorizontalSwipe, HorizontalSwipeDirection};
use crate::primitives::overlays::modal::{
    Modal, ModalDismissal, ModalGeometry, ModalOptions, ModalPlacement,
};
use crate::primitives::text::Text;
use crate::theme::SemanticFontRef;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use super::items::SidebarSection;
use super::presentation::SidebarToggle;
use super::sidebar::Sidebar;

/// Authored options for the modal drawer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavigationDrawerOptions {
    pub dismissal: ModalDismissal,
    pub closes_on_selection: bool,
    pub allows_swipe_dismissal: bool,
    pub close_label: String,
}

impl Default for NavigationDrawerOptions {
    fn default() -> Self {
        Self {
            dismissal: ModalDismissal::default(),
            closes_on_selection: true,
            allows_swipe_dismissal: true,
            close_label: "Collapse navigation".to_string(),
        }
    }
}

/// Modal navigation drawer over the standard modal lifecycle.
pub struct NavigationDrawer<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    pub title: String,
    pub is_presented: Binding<bool>,
    pub selection: Binding<Option<Id>>,
    pub expanded: Option<Binding<HashSet<Id>>>,
    pub sections: Vec<SidebarSection<Id>>,
    pub options: NavigationDrawerOptions,
    header: AnyView,
    footer: AnyView,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> NavigationDrawer<Id> {
    pub fn new(
        title: impl Into<String>,
        is_presented: Binding<bool>,
        selection: Binding<Option<Id>>,
        sections: Vec<SidebarSection<Id>>,
    ) -> Self {
        Self {
            title: title.into(),
            is_presented,
            selection,
            expanded: None,
            sections,
            options: NavigationDrawerOptions::default(),
            header: AnyView::new(crate::EmptyView),
            footer: AnyView::new(crate::EmptyView),
        }
    }

    pub fn expanded(mut self, expanded: Binding<HashSet<Id>>) -> Self {
        self.expanded = Some(expanded);
        self
    }

    pub fn options(mut self, options: NavigationDrawerOptions) -> Self {
        self.options = options;
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
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> View
    for NavigationDrawer<Id>
{
    fn body(&self) -> AnyView {
        let mut geometry = ModalGeometry::default();
        // A drawer is a leading sheet flush with the window edge: no inset and
        // no rounding, so it reads as navigation rather than a dialog.
        geometry.placement = ModalPlacement::Leading;
        geometry.inset = 0.0;
        geometry.corner_radius = 0.0;
        geometry.height = 560.0;
        let modal_options = ModalOptions {
            geometry,
            dismissal: self.options.dismissal,
            ..ModalOptions::default()
        };

        let title = self.title.clone();
        let close_label = self.options.close_label.clone();
        let is_presented = self.is_presented.clone();
        let closes_on_selection = self.options.closes_on_selection;
        let selection = self.selection.clone();
        let sections = self.sections.clone();
        let expanded = self.expanded.clone();
        let allows_swipe = self.options.allows_swipe_dismissal;
        let header = self.header.clone();
        let footer = self.footer.clone();

        let mut sidebar = Sidebar::new(selection, sections)
            .minimum_row_height(44.0)
            .on_activate(move |_id: Id| {
                if closes_on_selection {
                    is_presented.set(false);
                }
            })
            .header(AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(
                        BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            8.0,
                            AnyView::new(TupleView::new(vec![
                                AnyView::new(SidebarToggle::new(close_label, true, {
                                    let binding = self.is_presented.clone();
                                    move || binding.set(false)
                                })),
                                AnyView::new(DrawerTitle {
                                    title: title.clone(),
                                }),
                            ])),
                        )
                        .padding_edges(12.0, 8.0),
                    ),
                    header.clone(),
                ])),
            )))
            .footer(footer);
        if let Some(expanded) = expanded {
            sidebar = sidebar.expanded(expanded);
        }

        // Swipe-to-dismiss: the drawer is presented modally, so a leftward
        // gesture closes it.
        let swipe_presented = self.is_presented.clone();
        let swipe = HorizontalSwipe::new(
            move |direction: HorizontalSwipeDirection| {
                if direction == HorizontalSwipeDirection::Left {
                    swipe_presented.set(false);
                }
            },
            AnyView::new(sidebar),
        )
        .enabled(allows_swipe);

        let titled = AnyView::new(SwipeToggleAccessibility {
            content: AnyView::new(swipe),
            label: self.title.clone(),
            is_enabled: allows_swipe,
        });

        AnyView::new(SwipeToggleAccessibility {
            content: AnyView::new(
                Modal::new(self.is_presented.clone(), titled).options(modal_options),
            ),
            label: self.title.clone(),
            is_enabled: true,
        })
    }
}

/// Drawer heading: truncating body-strong title that absorbs the row width.
struct DrawerTitle {
    title: String,
}

impl PrimitiveView for DrawerTitle {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_min_width(Some(0.0));
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_min_width(Some(0.0));
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
    }

    fn update_node(&self, _node: &cove_core::scene::Node) {}

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![AnyView::new(
            Text::new(self.title.clone())
                .truncated()
                .semantic_font(SemanticFontRef::body_strong()),
        )]
    }
}
crate::impl_primitive_view!(DrawerTitle);

/// Applies the drawer title to the nav tree without erasing the modifier.
struct SwipeToggleAccessibility {
    content: AnyView,
    label: String,
    is_enabled: bool,
}

impl PrimitiveView for SwipeToggleAccessibility {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        let mut semantics = node.accessibility().unwrap_or_else(|| {
            cove_core::scene::AccessibilitySemantics::new(
                cove_core::scene::AccessibilityRole::Group,
            )
        });
        semantics.label = self.label.clone();
        node.set_accessibility(Some(semantics));
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        let _ = self.is_enabled;
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(SwipeToggleAccessibility);
