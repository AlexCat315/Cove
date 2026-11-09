//! List–detail navigation that adapts to the window and its folds.
//!
//! One state drives every presentation: a single pane that shows the list or
//! the detail, two panes side by side (aligned to a vertical fold or hinge
//! when one crosses the view), or detail above list across a tabletop fold.
//! The list and detail keep their slots in every presentation, so their own
//! state — scroll position, drafts, focus — survives folding and resizing.
use std::rc::Rc;

use cove_composition::Scope;
use cove_core::scene::{Align, Display, FlexDirection, LayoutNode, Node};
use cove_core::{Binding, DisplayFeatureKind, Environment, FeatureOrientation, Rect, State};

use crate::adaptive::SurfaceReader;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::layout::segments::FrameProbe;
use crate::theme::SemanticColorRef;
use crate::{AnyView, DynamicField, PrimitiveView, TupleView, View, impl_primitive_view};

/// Narrowest pane a fold may leave before the view stays single-pane.
pub const MINIMUM_PANE_WIDTH: f32 = 240.0;
/// Container width from which list and detail sit side by side.
pub const TWO_PANE_MINIMUM_WIDTH: f32 = 840.0;

/// Presentation-independent navigation state.
#[derive(Clone, Debug, PartialEq)]
pub struct ListDetailState<Item> {
    pub selection: Option<Item>,
    /// In a single pane, whether the detail covers the list. Leaving the
    /// detail keeps `selection`, so a wider window shows it again.
    pub showing_detail: bool,
}

impl<Item> Default for ListDetailState<Item> {
    fn default() -> Self {
        Self {
            selection: None,
            showing_detail: false,
        }
    }
}

impl<Item> ListDetailState<Item> {
    pub fn select(&mut self, item: Item) {
        self.selection = Some(item);
        self.showing_detail = true;
    }

    pub fn back(&mut self) {
        self.showing_detail = false;
    }
}

/// How the measured container presents list and detail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ListDetailPresentation {
    /// One pane: the list, or the detail while `showing_detail`.
    Single,
    /// List on the leading side with `list_width`, then `gap` points left
    /// empty (an occluding hinge) or a hairline, then the detail.
    SideBySide { list_width: f32, gap: f32 },
    /// Detail above a tabletop fold with `detail_height`, `gap`, list below.
    Stacked { detail_height: f32, gap: f32 },
}

impl ListDetailPresentation {
    /// `container` is the view's window frame; before it is measured the
    /// viewport stands in for it.
    pub fn resolve(environment: &Environment, container: Rect, list_width: f32) -> Self {
        Self::resolve_from(environment, container, list_width, TWO_PANE_MINIMUM_WIDTH)
    }

    /// As [`Self::resolve`], with two panes from `two_pane_width` when no
    /// fold decides.
    pub fn resolve_from(
        environment: &Environment,
        container: Rect,
        list_width: f32,
        two_pane_width: f32,
    ) -> Self {
        let container = if container.width > 0.0 && container.height > 0.0 {
            container
        } else {
            environment.viewport.bounds()
        };
        for feature in &environment.display_features {
            if feature.kind == DisplayFeatureKind::Cutout {
                continue;
            }
            let bounds = feature.bounds;
            match feature.orientation {
                FeatureOrientation::Vertical => {
                    let leading = bounds.x - container.x;
                    let trailing = container.max_x() - bounds.max_x();
                    if leading >= MINIMUM_PANE_WIDTH && trailing >= MINIMUM_PANE_WIDTH {
                        return Self::SideBySide {
                            list_width: leading,
                            gap: bounds.width,
                        };
                    }
                }
                FeatureOrientation::Horizontal if feature.separates => {
                    let top = bounds.y - container.y;
                    if top > 0.0 && bounds.max_y() < container.max_y() {
                        return Self::Stacked {
                            detail_height: top,
                            gap: bounds.height,
                        };
                    }
                }
                FeatureOrientation::Horizontal => {}
            }
        }
        if container.width >= two_pane_width {
            Self::SideBySide {
                list_width: list_width
                    .clamp(MINIMUM_PANE_WIDTH, container.width - MINIMUM_PANE_WIDTH),
                gap: 0.0,
            }
        } else {
            Self::Single
        }
    }
}

/// What the detail builder knows about its presentation.
#[derive(Clone)]
pub struct ListDetailContext {
    /// The detail covers the list; show a way back.
    pub is_single_pane: bool,
    /// How the panes are arranged around the detail.
    pub presentation: ListDetailPresentation,
    /// Returns to the list in a single pane; keeps the selection.
    pub back: Rc<dyn Fn()>,
}

/// List–detail navigation.
///
/// `list` receives a selection binding: setting it selects an item and, in a
/// single pane, opens its detail. `detail` builds the selected item's pane;
/// `placeholder` fills the detail pane while nothing is selected.
pub struct ListDetail<Item: Clone + PartialEq + 'static> {
    state: Binding<ListDetailState<Item>>,
    list: Rc<dyn Fn(Binding<Option<Item>>) -> AnyView>,
    detail: Rc<dyn Fn(Item, ListDetailContext) -> AnyView>,
    placeholder: AnyView,
    list_width: f32,
    two_pane_width: f32,
    hairline: bool,
    frame: State<Rect>,
}

impl<Item: Clone + PartialEq + 'static> ListDetail<Item> {
    pub fn new(
        state: Binding<ListDetailState<Item>>,
        list: impl Fn(Binding<Option<Item>>) -> AnyView + 'static,
        detail: impl Fn(Item, ListDetailContext) -> AnyView + 'static,
    ) -> Self {
        Self {
            state,
            list: Rc::new(list),
            detail: Rc::new(detail),
            placeholder: AnyView::new(crate::EmptyView),
            list_width: 320.0,
            two_pane_width: TWO_PANE_MINIMUM_WIDTH,
            hairline: true,
            frame: State::new(Rect::ZERO),
        }
    }

    /// Container width from which list and detail sit side by side when no
    /// fold decides; at least the list plus one minimum pane.
    pub fn two_pane_width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.two_pane_width = width.max(self.list_width + MINIMUM_PANE_WIDTH);
        }
        self
    }

    /// Whether a hairline separates the panes when no hinge does.
    pub fn hairline(mut self, hairline: bool) -> Self {
        self.hairline = hairline;
        self
    }

    pub fn placeholder(mut self, placeholder: impl View + 'static) -> Self {
        self.placeholder = AnyView::new(placeholder);
        self
    }

    /// Preferred list width when no fold decides it.
    pub fn list_width(mut self, width: f32) -> Self {
        if width.is_finite() {
            self.list_width = width.max(MINIMUM_PANE_WIDTH);
        }
        self
    }
}

impl<Item: Clone + PartialEq + 'static> View for ListDetail<Item> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let frame = self.frame.get();
        let state = self.state.clone();
        let list = self.list.clone();
        let detail = self.detail.clone();
        let placeholder = self.placeholder.clone();
        let list_width = self.list_width;
        let two_pane_width = self.two_pane_width;
        let hairline = self.hairline;
        let content = AnyView::new(SurfaceReader::new(move |environment: &Environment| {
            let presentation = ListDetailPresentation::resolve_from(
                environment,
                frame,
                list_width,
                two_pane_width,
            );
            let current = state.get();
            let selection = {
                let state = state.clone();
                Binding::new(
                    {
                        let state = state.clone();
                        move || state.get().selection
                    },
                    move |value: Option<Item>| {
                        let showing = value.is_some();
                        state.set(ListDetailState {
                            selection: value,
                            showing_detail: showing,
                        });
                    },
                )
            };
            let context = ListDetailContext {
                is_single_pane: presentation == ListDetailPresentation::Single,
                presentation,
                back: {
                    let state = state.clone();
                    Rc::new(move || {
                        let mut next = state.get();
                        next.back();
                        state.set(next);
                    })
                },
            };
            let detail_view = match current.selection.clone() {
                Some(item) => detail(item, context),
                None => placeholder.clone(),
            };
            let single = presentation == ListDetailPresentation::Single;
            let detail_shown = !single || current.showing_detail && current.selection.is_some();
            let list_shown = !single || !detail_shown;
            let (direction, list_pane, separator, detail_pane) = match presentation {
                ListDetailPresentation::Single => (
                    FlexDirection::Row,
                    PaneLayout::fill(list_shown),
                    PaneLayout::hidden(),
                    PaneLayout::fill(detail_shown),
                ),
                ListDetailPresentation::SideBySide { list_width, gap } => (
                    FlexDirection::Row,
                    PaneLayout::fixed(list_width),
                    PaneLayout::separator(gap, hairline),
                    PaneLayout::fill(true),
                ),
                ListDetailPresentation::Stacked { detail_height, gap } => (
                    FlexDirection::ColumnReverse,
                    PaneLayout::fill(true),
                    PaneLayout::separator(gap, hairline),
                    PaneLayout::fixed(detail_height),
                ),
            };
            AnyView::new(PaneContainer {
                direction,
                content: AnyView::new(TupleView::new(vec![
                    AnyView::new(Pane {
                        layout: list_pane,
                        role: "list-detail-list",
                        content: list(selection),
                    }),
                    AnyView::new(
                        AnyView::new(Pane {
                            layout: separator,
                            role: "list-detail-separator",
                            content: AnyView::new(crate::EmptyView),
                        })
                        .semantic_background(
                            if separator.extent > 0.0 && separator.extent < 1.5 {
                                SemanticColorRef::divider()
                            } else {
                                SemanticColorRef::clear()
                            },
                        ),
                    ),
                    AnyView::new(Pane {
                        layout: detail_pane,
                        role: "list-detail-detail",
                        content: detail_view,
                    }),
                ])),
            })
        }));
        AnyView::new(FrameProbe {
            frame: self.frame.clone(),
            content,
        })
    }

    fn dynamic_properties(&self) -> Vec<DynamicField> {
        vec![DynamicField::state("frame", &self.frame)]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PaneLayout {
    visible: bool,
    /// Main-axis extent; `None` fills the remaining space, `Some(NaN)` sizes
    /// to the content.
    fixed: Option<f32>,
    extent: f32,
}

impl PaneLayout {
    pub(crate) fn fill(visible: bool) -> Self {
        Self {
            visible,
            fixed: None,
            extent: 0.0,
        }
    }

    pub(crate) fn fixed(extent: f32) -> Self {
        Self {
            visible: true,
            fixed: Some(extent.max(0.0)),
            extent: extent.max(0.0),
        }
    }

    pub(crate) fn hidden() -> Self {
        Self {
            visible: false,
            fixed: Some(0.0),
            extent: 0.0,
        }
    }

    /// Sizes to its content along the main axis.
    pub(crate) fn intrinsic(visible: bool) -> Self {
        Self {
            visible,
            fixed: Some(f32::NAN),
            extent: 0.0,
        }
    }

    /// A hinge leaves its width empty; a crease or plain split gets a hairline
    /// unless the caller separates the panes itself.
    fn separator(gap: f32, hairline: bool) -> Self {
        Self::fixed(if gap > 0.0 {
            gap
        } else if hairline {
            1.0
        } else {
            0.0
        })
    }
}

/// The row or column that holds the three slots; it never changes identity.
pub(crate) struct PaneContainer {
    pub(crate) direction: FlexDirection,
    pub(crate) content: AnyView,
}

impl PrimitiveView for PaneContainer {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(self.direction);
        layout.set_align_items(Align::Stretch);
        layout.set_width_percent(100.0);
        layout.set_height_percent(100.0);
        layout.set_flex_grow(1.0);
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(PaneContainer);

/// One slot. Hidden slots leave layout and hit testing but keep their subtree.
pub(crate) struct Pane {
    pub(crate) layout: PaneLayout,
    pub(crate) role: &'static str,
    pub(crate) content: AnyView,
}

impl PrimitiveView for Pane {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_attachment(
            cove_composition::LayoutDebugAttachmentKey::DEBUG_NAME,
            self.role.to_owned(),
        );
        node
    }
    fn update_node(&self, node: &Node) {
        node.set_allows_hit_testing(self.layout.visible);
        node.set_is_interaction_enabled(self.layout.visible);
        node.set_clips_to_bounds(true);
        // A collapsed pane keeps its subtree; it must also paint nothing, since
        // content overflowing a zero-size frame is not clipped away.
        node.set_opacity(if self.layout.visible { 1.0 } else { 0.0 });
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_display(if self.layout.visible {
            Display::Flex
        } else {
            Display::None
        });
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        match self.layout.fixed {
            Some(extent) if extent.is_nan() => {
                layout.set_flex_grow(0.0);
                layout.set_flex_shrink(0.0);
                layout.set_flex_basis_auto();
            }
            Some(extent) => {
                layout.set_flex_grow(0.0);
                layout.set_flex_shrink(0.0);
                layout.set_flex_basis(extent);
            }
            None => {
                layout.set_flex_grow(1.0);
                layout.set_flex_shrink(1.0);
                layout.set_flex_basis(0.0);
            }
        }
        layout.set_min_width(Some(0.0));
        layout.set_min_height(Some(0.0));
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(Pane);
