//! Translation of Primitives/Layout/Sidebar/AdaptiveSidebar.swift.
//!
//! One navigation tree and one content tree across sidebar, rail and drawer
//! layouts. Breakpoints use this container's allocation. Optional presentation
//! belongs to the caller's session and survives a temporary return to the
//! expanded layout.
use crate::Scope;
use std::collections::HashSet;
use std::rc::Rc;

use crate::ViewExt;
use crate::adaptive::ContainerSizeReader;
use crate::adaptive::SurfaceReader;
use crate::animation::content::AnimatedVisibility;
use crate::animation::transition::Transition;
use crate::animation::transition::TransitionEdge;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{
    AbsolutePositionModifier, FlexExt, FrameDimension, LayoutFrameExt, PaddingExt,
};
use crate::modifiers::style::AppearanceExt;
use crate::primitives::interaction::horizontal_swipe::{HorizontalSwipe, HorizontalSwipeDirection};
use crate::primitives::overlays::modal::ModalDismissal;
use crate::theme::tokens::{SemanticColorRef, SemanticStyleViewExt};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::Binding;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};

use super::adaptive_hosts::{SidebarBackdrop, SidebarLayer, SidebarShellHost, SidebarTriggerHost};
use super::adaptive_options::{AdaptiveSidebarOptions, SidebarGeometry, SidebarMode};
use super::focus_handoff::SidebarFocusHandoff;
use super::items::SidebarSection;
use super::presentation::SidebarToggle;
use super::resize_handle::SidebarResizeHandle;
use super::sidebar::Sidebar;

/// Width of the leading strip that starts an opening edge swipe.
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

    #[expect(dead_code, reason = "ported from Swift but not wired in yet")]
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

    fn geometry(&self, width: f32, handle_extent: f32) -> SidebarGeometry {
        SidebarGeometry::new(
            width,
            &self.options.layout,
            self.presented(),
            self.collapsed(),
            self.width.as_ref().map(|binding| binding.get()),
            &self.options.resizing,
            handle_extent,
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

        let mut column: Vec<AnyView> = vec![AnyView::new(
            AnyView::new(sidebar).flex_with(1.0, 1.0, Some(0.0)).frame(
                None,
                None,
                Some(0.0),
                Some(0.0),
                None,
                None,
            ),
        )];
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

    /// Leading navigation chrome above the sections.
    pub fn header(mut self, header: impl View + 'static) -> Self {
        self.header = AnyView::new(header);
        self
    }

    /// Trailing navigation chrome below the sections.
    pub fn footer(mut self, footer: impl View + 'static) -> Self {
        self.footer = AnyView::new(footer);
        self
    }

    /// Binds the docked sidebar's collapsed (rail) state.
    pub fn collapse(mut self, is_collapsed: Binding<bool>) -> Self {
        self.collapse = Some(is_collapsed);
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
    fn body(&self, _cx: &Scope) -> AnyView {
        let snapshot = self.snapshot();
        AnyView::new(AdaptiveShell { snapshot })
    }

    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![
            cove_composition::DynamicField::state("local_presentation", &self.local_presentation),
            cove_composition::DynamicField::state("local_collapse", &self.local_collapse),
        ]
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
        let shell = SurfaceReader::new(move |surface: &cove_core::Environment| {
            let snapshot = snapshot.clone();
            let surface = surface.clone();
            AnyView::new(ContainerSizeReader::new(move |size: cove_core::Size| {
                snapshot.shell(size, &surface)
            }))
            .flex_with(1.0, 1.0, None)
        });
        vec![AnyView::new(AnyView::new(shell).flex_with(1.0, 1.0, None))]
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> AdaptiveSnapshot<Id> {
    /// Content beside a docked lane, with the navigation layer absolutely
    /// positioned inside the lane so a drawer can extend past it.
    fn shell(&self, size: cove_core::Size, surface: &cove_core::Environment) -> AnyView {
        let touch = surface.input.contains(cove_core::InputCapabilities::TOUCH);
        let width = if size.width > 0.0 {
            size.width
        } else {
            surface.viewport.width
        };
        let handle = if touch {
            surface.minimum_control_extent()
        } else {
            7.0
        };
        let geometry = self.geometry(width, handle);
        let fill = |view: AnyView| {
            AnyView::new(view.flex_with(1.0, 1.0, Some(0.0)).frame(
                None,
                None,
                Some(0.0),
                Some(0.0),
                None,
                None,
            ))
        };

        let compact_bar = AnyView::new(
            AnyView::new(AnimatedVisibility::new(
                geometry.mode == SidebarMode::Compact,
                Transition::identity(),
                None,
                false,
                None,
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Row,
                        Align::Center,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(SidebarTriggerHost::new(
                            self.focus.clone(),
                            self.open_button(),
                        )),
                    )
                    .padding_edges(12.0, 4.0)
                    .semantic_background(SemanticColorRef::surface()),
                ),
            ))
            .flex_with(0.0, 0.0, None),
        );
        let main = fill(AnyView::new(BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![
                compact_bar,
                fill(self.content.clone()),
            ])),
        )));

        let mut handle_view = SidebarResizeHandle::new(
            geometry.resize_range,
            geometry.viewport_width,
            geometry.docked_width,
        )
        .extent(geometry.resize_handle_width)
        .label(self.options.resizing.label.clone());
        if let Some(width) = self.width.clone() {
            handle_view = handle_view.value(width);
        }

        let dismiss = {
            let snapshot = self.clone();
            move || snapshot.set_presented(false)
        };
        let pinned = |view: AnyView, width: f32| {
            AnyView::new(
                view.frame(
                    Some(FrameDimension::Points(width)),
                    Some(FrameDimension::Percent(100.0)),
                    None,
                    None,
                    None,
                    None,
                )
                .modifier(AbsolutePositionModifier::new(
                    Some(0.0),
                    Some(0.0),
                    None,
                    None,
                )),
            )
        };
        let backdrop = pinned(
            AnyView::new(AnimatedVisibility::new(
                geometry.is_modal,
                Transition::opacity(),
                None,
                false,
                None,
                AnyView::new(SidebarBackdrop::new(
                    ModalDismissal::default(),
                    dismiss.clone(),
                )),
            )),
            geometry.viewport_width,
        );
        let panel = pinned(
            AnyView::new(AnimatedVisibility::new(
                geometry.is_visible(),
                if geometry.mode == SidebarMode::Expanded {
                    Transition::identity()
                } else {
                    Transition::move_edge(TransitionEdge::Leading, geometry.panel_width)
                },
                None,
                false,
                None,
                fill(self.navigation(geometry)),
            )),
            geometry.panel_width,
        );
        let layer = AnyView::new(
            AnyView::new(SidebarLayer::new(
                geometry,
                self.focus.clone(),
                ModalDismissal::default(),
                dismiss,
                AnyView::new(TupleView::new(vec![backdrop, panel])),
            ))
            .frame(
                Some(FrameDimension::Points(geometry.docked_width)),
                None,
                None,
                None,
                None,
                None,
            )
            .flex_with(0.0, 0.0, None)
            .z_index(100.0),
        );

        let row = fill(AnyView::new(BoxView::new(
            FlexDirection::RowReverse,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(TupleView::new(vec![main, AnyView::new(handle_view), layer])),
        )));

        let presentation = self.presentation.clone();
        let local = self.local_presentation.clone();
        let swipe = HorizontalSwipe::new(
            move |direction: HorizontalSwipeDirection| {
                if direction == HorizontalSwipeDirection::Right {
                    match presentation.as_ref() {
                        Some(binding) => binding.set(true),
                        None => local.set(true),
                    }
                }
            },
            row,
        )
        .enabled(
            self.options.allows_edge_swipe
                && touch
                && geometry.mode == SidebarMode::Compact
                && !geometry.is_modal,
        )
        .should_begin(|event, node| {
            let x = event.x - node.absolute_frame().x;
            (0.0..EDGE_SWIPE_WIDTH).contains(&x)
        });

        AnyView::new(
            AnyView::new(SidebarShellHost::new(
                geometry,
                self.focus.clone(),
                AnyView::new(swipe).flex_with(1.0, 1.0, Some(0.0)),
            ))
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }
}

impl<Id: PartialEq + Eq + std::hash::Hash + Clone + Send + Sync + 'static> View
    for AdaptiveShell<Id>
{
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
}
