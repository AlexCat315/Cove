//! Translation of Primitives/Layout/Sidebar/AdaptiveSidebar.swift.
//!
//! One navigation tree and one content tree across sidebar, rail and drawer
//! layouts. Breakpoints use this container's allocation. Optional presentation
//! belongs to the caller's session and survives a temporary return to the
//! expanded layout.
use std::collections::HashSet;
use std::rc::Rc;

use crate::adaptive::ContainerSizeReader;
use crate::animation::content::AnimatedVisibility;
use crate::animation::transition::Transition;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{LayoutFrameExt, PaddingExt};
use crate::primitives::overlays::modal::uses_sheet;
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};

use super::adaptive_hosts::{SidebarBackdrop, SidebarLayer, SidebarShellHost, SidebarTriggerHost};
use super::adaptive_options::{AdaptiveSidebarOptions, SidebarGeometry, SidebarMode};
use super::focus_handoff::SidebarFocusHandoff;
use super::items::SidebarSection;
use super::presentation::{SidebarPresentation, SidebarToggle};
use super::sidebar::Sidebar;

const EDGE_SWIPE_WIDTH: f32 = 24.0;

/// Navigation plus content, switching between docked, rail and drawer.
pub struct AdaptiveSidebar<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    pub title: String,
    pub presentation: Option<Binding<bool>>,
    pub collapse: Option<Binding<bool>>,
    pub selection: Binding<Option<Id>>,
    pub width: Option<Binding<f32>>,
    pub expanded: Option<Binding<HashSet<Id>>>,
    pub sections: Vec<SidebarSection<Id>>,
    pub options: AdaptiveSidebarOptions,
    pub local_presentation: cove_core::State<bool>,
    pub local_collapse: cove_core::State<bool>,
    pub focus: Rc<SidebarFocusHandoff>,
    header: AnyView,
    footer: AnyView,
    content: AnyView,
}

/// Owned copy so the container-size closure can outlive `body`.
#[derive(Clone)]
struct AdaptiveSnapshot<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    title: String,
    presentation: Option<Binding<bool>>,
    collapse: Option<Binding<bool>>,
    selection: Binding<Option<Id>>,
    width: Option<Binding<f32>>,
    expanded: Option<Binding<HashSet<Id>>>,
    sections: Vec<SidebarSection<Id>>,
    options: AdaptiveSidebarOptions,
    local_presentation: cove_core::State<bool>,
    local_collapse: cove_core::State<bool>,
    focus: Rc<SidebarFocusHandoff>,
    header: AnyView,
    footer: AnyView,
    content: AnyView,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> AdaptiveSnapshot<Id> {
    fn presented(&self) -> bool {
        match self.presentation.as_ref() {
            Some(binding) => binding.get(),
            None => self.local_presentation.get(),
        }
    }

    fn set_presented(&self, value: bool) {
        match self.presentation.as_ref() {
            Some(binding) => {
                if binding.get() != value {
                    binding.set(value);
                }
            }
            None => self.local_presentation.set(value),
        }
    }

    fn collapsed(&self) -> bool {
        match self.collapse.as_ref() {
            Some(binding) => binding.get(),
            None => self.local_collapse.get(),
        }
    }

    fn set_collapsed(&self, value: bool) {
        match self.collapse.as_ref() {
            Some(binding) => {
                if binding.get() != value {
                    binding.set(value);
                }
            }
            None => self.local_collapse.set(value),
        }
    }

    fn geometry(&self, width: f32) -> SidebarGeometry {
        SidebarGeometry::new(
            width,
            &self.options.layout,
            self.presented(),
            self.collapsed(),
            self.width.as_ref().map(|binding| binding.get()),
            &self.options.resizing,
            7.0,
        )
    }

    fn open_button(&self) -> AnyView {
        let focus = self.focus.clone();
        let label = self.options.open_label.clone();
        let is_presented = self.presented();
        let presentation = self.presentation.clone();
        let local = self.local_presentation.clone();
        let button = SidebarToggle::new(label.clone(), is_presented, {
            let focus = focus.clone();
            let label = label.clone();
            move || {
                focus.focus_opener(&label);
                match presentation.as_ref() {
                    Some(binding) => binding.set(true),
                    None => local.set(true),
                }
            }
        });
        AnyView::new(SidebarTriggerHost::new(focus, AnyView::new(button)))
    }

    fn rail_button(&self, geometry: SidebarGeometry) -> AnyView {
        if !geometry.is_collapsed {
            return self.open_button();
        }
        let collapse = self.collapse.clone();
        let local = self.local_collapse.clone();
        AnyView::new(SidebarToggle::new(
            self.options.expand_label.clone(),
            false,
            move || match collapse.as_ref() {
                Some(binding) => binding.set(false),
                None => local.set(false),
            },
        ))
    }

    fn navigation(&self, geometry: SidebarGeometry) -> AnyView {
        let is_modal = geometry.is_modal;
        let presentation = geometry.presentation(self.options.layout.shows_rail_labels);
        let row_height = if geometry.mode == SidebarMode::Expanded {
            38.0
        } else {
            44.0
        };
        let mut header_children: Vec<AnyView> = Vec::new();
        // Rail trigger inside the header so the retained subtree owns the focus
        // return target.
        if geometry.mode == SidebarMode::Rail && !is_modal {
            header_children.push(self.rail_button(geometry));
        }
        if is_modal {
            let close = {
                let focus = self.focus.clone();
                move || {
                    let _ = &focus;
                }
            };
            let _ = close;
            let presentation = self.presentation.clone();
            let local = self.local_presentation.clone();
            let title = self.title.clone();
            header_children.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(SidebarToggle::new("Collapse navigation", true, move || {
                            match presentation.as_ref() {
                                Some(binding) => binding.set(false),
                                None => local.set(false),
                            }
                        })),
                        AnyView::new(AnyView::new(
                            AnyView::new(crate::primitives::text::Text::new(title).truncated())
                                .with_modifier(SemanticBodyStrong),
                        )),
                    ])),
                )
                .padding_edges(12.0, 4.0),
            ));
        }
        header_children.push(self.header.clone());

        let selection = self.selection.clone();
        let sections = self.sections.clone();
        let presentation_binding = self.presentation.clone();
        let local_presentation = self.local_presentation.clone();
        let mut sidebar = Sidebar::new(selection, sections)
            .presentation(presentation)
            .minimum_row_height(row_height)
            .on_activate(move |_id: Id| {
                if is_modal {
                    match presentation_binding.as_ref() {
                        Some(binding) => binding.set(false),
                        None => local_presentation.set(false),
                    }
                }
            })
            .header(AnyView::new(TupleView::new(header_children)))
            .footer(self.footer.clone());
        if let Some(expanded) = self.expanded.clone() {
            sidebar = sidebar.expanded(expanded);
        }

        let mut column: Vec<AnyView> = vec![AnyView::new(sidebar)];
        if geometry.mode == SidebarMode::Expanded {
            let collapse = self.collapse.clone();
            let local = self.local_collapse.clone();
            column.push(AnyView::new(SidebarToggle::new(
                self.options.collapse_label.clone(),
                true,
                move || match collapse.as_ref() {
                    Some(binding) => binding.set(true),
                    None => local.set(true),
                },
            )));
        }
        let title = self.title.clone();
        AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            )
            .accessibility(move |semantics| {
                semantics.role = if is_modal {
                    AccessibilityRole::Dialog
                } else {
                    AccessibilityRole::Group
                };
                semantics.label = title.clone();
            }),
        )
    }
}

/// Body-strong styling as a modifier so it can be applied to an erased view.
struct SemanticBodyStrong;

impl cove_composition::ViewModifier for SemanticBodyStrong {
    fn apply_node(&self, node: &Node) {
        let token = crate::theme::SemanticFontRef::body_strong()
            .resolve(&crate::theme::theme_for_node(node));
        node.set_attachment(crate::modifiers::style::FONT, token.font.clone());
        node.set_attachment(crate::modifiers::style::LINE_HEIGHT, token.line_height);
        if let Some(layout) = node.layout_node() {
            layout.set_attachment(crate::modifiers::style::FONT, token.font.clone());
            layout.set_attachment(crate::modifiers::style::LINE_HEIGHT, token.line_height);
            layout.mark_dirty();
        }
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> AdaptiveSidebar<Id> {
    /// Navigation plus the content it owns.
    ///
    /// The header and footer default to empty views; callers add them with the
    /// builders below. Section IDs must be unique, matching `Sidebar`.
    pub fn new(
        title: impl Into<String>,
        selection: Binding<Option<Id>>,
        sections: Vec<SidebarSection<Id>>,
    ) -> Self {
        assert!(
            sections.iter().enumerate().all(|(index, section)| sections
                .iter()
                .enumerate()
                .all(|(other, candidate)| other == index || candidate.id != section.id)),
            "AdaptiveSidebar section IDs must be unique"
        );
        Self {
            title: title.into(),
            presentation: None,
            collapse: None,
            selection,
            width: None,
            expanded: None,
            sections,
            options: AdaptiveSidebarOptions::default(),
            local_presentation: cove_core::State::new(false),
            local_collapse: cove_core::State::new(false),
            focus: Rc::new(SidebarFocusHandoff::default()),
            header: AnyView::new(cove_composition::EmptyView),
            footer: AnyView::new(cove_composition::EmptyView),
            content: AnyView::new(cove_composition::EmptyView),
        }
    }

    /// The pane the sidebar navigates.
    pub fn content(mut self, content: impl View + 'static) -> Self {
        self.content = AnyView::new(content);
        self
    }

    /// Binds the drawer's presented state, so a caller can open it.
    pub fn presentation(mut self, is_presented: Binding<bool>) -> Self {
        self.presentation = Some(is_presented);
        self
    }

    /// Binds the requested docked width.
    pub fn width(mut self, width: Binding<f32>) -> Self {
        self.width = Some(width);
        self
    }

    /// Binds which groups are expanded.
    pub fn expanded(mut self, expanded: Binding<std::collections::HashSet<Id>>) -> Self {
        self.expanded = Some(expanded);
        self
    }

    pub fn options(mut self, options: AdaptiveSidebarOptions) -> Self {
        self.options = options;
        self
    }

    fn snapshot(&self) -> AdaptiveSnapshot<Id> {
        AdaptiveSnapshot {
            title: self.title.clone(),
            presentation: self.presentation.clone(),
            collapse: self.collapse.clone(),
            selection: self.selection.clone(),
            width: self.width.clone(),
            expanded: self.expanded.clone(),
            sections: self.sections.clone(),
            options: self.options.clone(),
            local_presentation: self.local_presentation.clone(),
            local_collapse: self.local_collapse.clone(),
            focus: self.focus.clone(),
            header: self.header.clone(),
            footer: self.footer.clone(),
            content: self.content.clone(),
        }
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> View
    for AdaptiveSidebar<Id>
{
    fn body(&self) -> AnyView {
        let snapshot = self.snapshot();
        AnyView::new(AdaptiveShell { snapshot })
    }
}

struct AdaptiveShell<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> {
    snapshot: AdaptiveSnapshot<Id>,
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> PrimitiveView
    for AdaptiveShell<Id>
{
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
        let snapshot = self.snapshot.clone();
        vec![AnyView::new(ContainerSizeReader::new(
            move |size: cove_core::Size| {
                let width = if size.width > 0.0 { size.width } else { 0.0 };
                let geometry = snapshot.geometry(width);
                let navigation = snapshot.navigation(geometry);
                let dismiss = {
                    let snapshot = snapshot.clone();
                    move || snapshot.set_presented(false)
                };
                let layer = SidebarLayer::new(
                    geometry,
                    snapshot.focus.clone(),
                    crate::primitives::overlays::modal::ModalDismissal::default(),
                    dismiss.clone(),
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(AnimatedVisibility::new(
                            geometry.is_modal,
                            Transition::opacity(),
                            None,
                            false,
                            None,
                            AnyView::new(SidebarBackdrop::new(
                                crate::primitives::overlays::modal::ModalDismissal::default(),
                                dismiss,
                            )),
                        )),
                        AnyView::new(navigation),
                    ])),
                );
                AnyView::new(SidebarShellHost::new(
                    geometry,
                    snapshot.focus.clone(),
                    AnyView::new(TupleView::new(vec![
                        AnyView::new(snapshot.content.clone()),
                        AnyView::new(layer),
                    ])),
                ))
            },
        ))]
    }
}
impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> View
    for AdaptiveShell<Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}
