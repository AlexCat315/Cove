//! Translation of Primitives/Layout/Sidebar/Rail.swift.
//!
//! Compact destination navigation. Applications choose it for medium/expanded
//! surfaces and use `NavigationDrawer` for compact surfaces. The rail keeps the
//! same destination nodes as the expanded sidebar, so switching presentation
//! preserves selection and drafts.
use crate::Scope;
use crate::primitives::DuplicateId;
use crate::primitives::layout::sidebar::items::SidebarSections;
use std::collections::HashSet;

use cove_core::Binding;
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

use crate::modifiers::layout::FlexExt;
use crate::modifiers::style::AppearanceExt;
use crate::primitives::layout::sidebar::{
    Sidebar, SidebarItem, SidebarPresentation, SidebarSection,
};
use crate::{AnyView, BoxView, View};

/// Compact destination navigation.
pub struct NavigationRail<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    pub selection: Binding<Option<Id>>,
    sections: SidebarSections<Id>,
    pub shows_labels: bool,
    pub expanded: Option<Binding<HashSet<Id>>>,
    pub on_activate: std::rc::Rc<dyn Fn(Id)>,
    header: AnyView,
    footer: AnyView,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> NavigationRail<Id> {
    pub fn try_new(
        selection: Binding<Option<Id>>,
        items: Vec<SidebarItem<Id>>,
    ) -> Result<Self, DuplicateId> {
        let sections = SidebarSections::new(vec![SidebarSection::new("rail", items)])?;
        Ok(Self {
            selection,
            sections,
            shows_labels: true,
            expanded: None,
            on_activate: std::rc::Rc::new(|_| {}),
            header: AnyView::new(crate::EmptyView),
            footer: AnyView::new(crate::EmptyView),
        })
    }

    pub fn shows_labels(mut self, shows_labels: bool) -> Self {
        self.shows_labels = shows_labels;
        self
    }

    pub fn expanded(mut self, expanded: Binding<HashSet<Id>>) -> Self {
        self.expanded = Some(expanded);
        self
    }

    pub fn on_activate(mut self, handler: impl Fn(Id) + 'static) -> Self {
        self.on_activate = std::rc::Rc::new(handler);
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
    for NavigationRail<Id>
{
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut sidebar = Sidebar::from_sections(self.selection.clone(), self.sections.clone())
            .presentation(SidebarPresentation::Rail {
                shows_labels: self.shows_labels,
            })
            .on_activate({
                let handler = self.on_activate.clone();
                move |id| handler(id)
            })
            .header(self.header.clone())
            .footer(self.footer.clone());
        if let Some(expanded) = self.expanded.clone() {
            sidebar = sidebar.expanded(expanded);
        }
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(sidebar),
            )
            .flex_with(0.0, 0.0, None)
            .clipped(),
        )
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
