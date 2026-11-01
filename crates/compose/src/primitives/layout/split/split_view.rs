//! Translation of Primitives/Layout/SplitView.swift.
//!
//! Two panes sharing one axis, split by a fraction of the available space.
//! A static fraction is an application-owned layout; a bound fraction renders a
//! draggable divider that writes clamped fractions back through the binding, so
//! the application can persist the requested proportion. `compact` switches to
//! retained single panes when the container is smaller than its threshold.
use crate::Scope;
use std::rc::Rc;

use cove_core::scene::{Align, FlexDirection, FocusChainHolder, Justify, LayoutNode, Node, Wrap};
use cove_core::{Binding, Color};
use cove_platform::SystemCursor;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::primitives::layout::split_compact::{
    SplitCompactPresentation, SplitPane, SplitPaneSelector,
};
use crate::primitives::text::{Divider, DividerAxis};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};

use super::constraints::SplitConstraints;
use super::handle::SplitHandle;
use super::retention::{
    CONTAINER_MARKER, SplitFocusHandoff, SplitFocusTransfer, SplitRetainedRegion,
};

/// Orientation of a [`SplitView`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SplitAxis {
    /// Panes sit side by side; the divider is vertical.
    #[default]
    Horizontal,
    /// Panes stack; the divider is horizontal.
    Vertical,
}

impl SplitAxis {
    pub fn flex_direction(self) -> FlexDirection {
        match self {
            Self::Horizontal => FlexDirection::Row,
            Self::Vertical => FlexDirection::Column,
        }
    }

    pub fn divider_axis(self) -> DividerAxis {
        match self {
            Self::Horizontal => DividerAxis::Vertical,
            Self::Vertical => DividerAxis::Horizontal,
        }
    }

    /// Cursor shown over the divider. It follows the direction the divider
    /// travels, not the direction the panes are laid out.
    pub fn resize_cursor(self) -> SystemCursor {
        match self {
            Self::Horizontal => SystemCursor::ResizeHorizontal,
            Self::Vertical => SystemCursor::ResizeVertical,
        }
    }
}

/// Two panes split by a fraction of the available space.
pub struct SplitView {
    pub axis: SplitAxis,
    pub spacing: f32,
    pub divider_thickness: f32,
    pub divider_color: Option<Color>,
    pub constraints: SplitConstraints,
    pub compact: Option<SplitCompactPresentation>,
    /// `Some` makes the divider draggable and keyboard-adjustable.
    pub fraction: Option<Binding<f32>>,
    static_fraction: f32,
    first: AnyView,
    second: AnyView,
    measured_extent: cove_core::State<f32>,
    selected_pane: cove_core::State<SplitPane>,
    focus_transfer: Rc<SplitFocusTransfer>,
}

impl SplitView {
    /// Static split. The application drives the fraction; the divider is a
    /// plain separator.
    pub fn new(
        axis: SplitAxis,
        fraction: f32,
        first: impl View + 'static,
        second: impl View + 'static,
    ) -> Self {
        Self {
            axis,
            spacing: 0.0,
            divider_thickness: 1.0,
            divider_color: None,
            constraints: SplitConstraints::default(),
            compact: None,
            fraction: None,
            static_fraction: Self::clamp_unit(fraction),
            first: AnyView::new(first),
            second: AnyView::new(second),
            measured_extent: cove_core::State::new(0.0),
            selected_pane: cove_core::State::new(SplitPane::First),
            focus_transfer: SplitFocusTransfer::new(),
        }
    }

    /// User-resizable split. The divider becomes a drag target and keyboard
    /// control that writes clamped fractions into `fraction`.
    pub fn resizable(
        axis: SplitAxis,
        fraction: Binding<f32>,
        first: impl View + 'static,
        second: impl View + 'static,
    ) -> Self {
        let initial = Self::clamp_unit(fraction.get());
        let mut value = Self::new(axis, initial, first, second);
        value.fraction = Some(fraction);
        value
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = if spacing.is_finite() {
            spacing.max(0.0)
        } else {
            0.0
        };
        self
    }

    pub fn divider_thickness(mut self, thickness: f32) -> Self {
        self.divider_thickness = if thickness.is_finite() {
            thickness.max(0.0)
        } else {
            1.0
        };
        self
    }

    pub fn divider_color(mut self, color: Color) -> Self {
        self.divider_color = Some(color);
        self
    }

    pub fn constraints(mut self, constraints: SplitConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn compact(mut self, compact: SplitCompactPresentation) -> Self {
        self.compact = Some(compact);
        self
    }

    fn clamp_unit(value: f32) -> f32 {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.5
        }
    }

    fn fraction(&self) -> f32 {
        match self.fraction.as_ref() {
            Some(binding) => binding.get(),
            None => self.static_fraction,
        }
    }

    fn clamped_fraction(&self, handle_thickness: f32) -> f32 {
        let available = self.measured_extent.get() - handle_thickness - 2.0 * self.spacing;
        self.constraints.clamp(self.fraction(), available)
    }

    fn selection(&self) -> SplitPane {
        self.compact
            .as_ref()
            .and_then(|presentation| presentation.selection.as_ref())
            .map_or_else(|| self.selected_pane.get(), |binding| binding.get())
    }
}

impl View for SplitView {
    fn body(&self, _cx: &Scope) -> AnyView {
        let thickness = match self.fraction {
            // A user-resizable divider needs a real grab target on touch.
            None => self.divider_thickness,
            Some(_) => self.divider_thickness.max(7.0),
        };
        let fraction = self.clamped_fraction(thickness);
        let is_compact = self.compact.as_ref().is_some_and(|presentation| {
            self.measured_extent.get() < presentation.resolved_threshold()
        });
        let selection = self.selection();
        let focus = self.focus_transfer.clone();

        let mut column: Vec<AnyView> = Vec::new();
        if let Some(compact) = self.compact.clone() {
            let presentation = compact.clone();
            let pane_selection = match compact.selection.clone() {
                Some(binding) => binding,
                None => self.selected_pane.binding(),
            };
            column.push(AnyView::new(
                SplitRetainedRegion::new(
                    is_compact,
                    focus.clone(),
                    None,
                    FlexDirection::Column,
                    SplitPaneSelector::new(presentation, pane_selection),
                )
                .flex_with(0.0, 0.0, Some(0.0)),
            ));
        }

        let first = AnyView::new(
            SplitRetainedRegion::new(
                !is_compact || selection == SplitPane::First,
                focus.clone(),
                Some(SplitPane::First),
                self.axis.flex_direction(),
                AnyView::new(self.first.clone())
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame(None, None, Some(0.0), Some(0.0), None, None),
            )
            .flex_with(if is_compact { 1.0 } else { fraction }, 1.0, Some(0.0))
            .frame(None, None, Some(0.0), Some(0.0), None, None),
        );

        let divider = AnyView::new(
            SplitRetainedRegion::new(
                !is_compact,
                focus.clone(),
                None,
                self.axis.flex_direction(),
                self.divider(thickness),
            )
            .flex_with(0.0, 0.0, Some(0.0)),
        );

        let second = AnyView::new(
            SplitRetainedRegion::new(
                !is_compact || selection == SplitPane::Second,
                focus.clone(),
                Some(SplitPane::Second),
                self.axis.flex_direction(),
                AnyView::new(self.second.clone())
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame(None, None, Some(0.0), Some(0.0), None, None),
            )
            .flex_with(
                if is_compact { 1.0 } else { 1.0 - fraction },
                1.0,
                Some(0.0),
            )
            .frame(None, None, Some(0.0), Some(0.0), None, None),
        );

        let panes = AnyView::new(BoxView::new(
            self.axis.flex_direction(),
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            self.spacing,
            AnyView::new(TupleView::new(vec![first, divider, second])),
        ));

        column.push(AnyView::new(
            AnyView::new(SplitContainerMarker { content: panes })
                .flex_with(1.0, 1.0, None)
                .frame(None, None, Some(0.0), Some(0.0), None, None),
        ));

        let extent_state = self.measured_extent.clone();
        let axis = self.axis;
        let measured = AnyView::new(SplitExtentObserver {
            axis,
            extent: extent_state.clone(),
            content: AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(column)),
            )),
        });

        AnyView::new(SplitFocusHandoffWrap {
            handoff: SplitFocusHandoff::new(focus, is_compact, selection),
            content: measured,
        })
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    /// `body` reads `measured_extent` from `State`, so the
    /// scope must declare those dependencies. Without this the writes never
    /// subscribe and the view keeps its first layout forever.
    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![crate::DynamicField::state(
            "measured_extent",
            &self.measured_extent,
        )]
    }
}

impl SplitView {
    fn divider(&self, thickness: f32) -> AnyView {
        match self.fraction.clone() {
            Some(fraction) => AnyView::new(SplitHandle {
                axis: self.axis,
                thickness,
                color: self.divider_color,
                fraction,
                constraints: self.constraints,
                spacing: self.spacing,
            }),
            None => AnyView::new(Divider::new(
                self.divider_color,
                self.divider_thickness,
                self.axis.divider_axis(),
            )),
        }
    }
}

/// Marks the flex container whose extent the divider measures itself against.
struct SplitContainerMarker {
    content: AnyView,
}

impl PrimitiveView for SplitContainerMarker {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment(CONTAINER_MARKER, true);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SplitContainerMarker);

/// Writes the container extent into state so the fraction can be clamped.
struct SplitExtentObserver {
    axis: SplitAxis,
    extent: cove_core::State<f32>,
    content: AnyView,
}

impl PrimitiveView for SplitExtentObserver {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let extent = self.extent.clone();
        let axis = self.axis;
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |node: &Node| {
            let next = match axis {
                SplitAxis::Horizontal => node.frame().width,
                SplitAxis::Vertical => node.frame().height,
            };
            if (extent.get() - next).abs() > f32::EPSILON {
                extent.set(next);
            }
            let _ = &weak;
        })));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SplitExtentObserver);

/// Applies the focus handoff around the measured split.
struct SplitFocusHandoffWrap {
    handoff: SplitFocusHandoff,
    content: AnyView,
}

impl PrimitiveView for SplitFocusHandoffWrap {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(SplitFocusHandoffHost {
            focus: self.handoff.focus.clone(),
            is_compact: self.handoff.is_compact,
            selection: self.handoff.selection,
            content: self.content.clone(),
        })]
    }
}
impl_primitive_view!(SplitFocusHandoffWrap);

struct SplitFocusHandoffHost {
    focus: Rc<SplitFocusTransfer>,
    is_compact: bool,
    selection: SplitPane,
    content: AnyView,
}

impl PrimitiveView for SplitFocusHandoffHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let focus = self.focus.clone();
        let is_compact = self.is_compact;
        let selection = self.selection;
        node.set_layout_did_update(Some(Rc::new(move |node: &Node| {
            let Some(visible) = *focus.pending_visibility.borrow() else {
                return;
            };
            *focus.pending_visibility.borrow_mut() = None;
            let handoff = SplitFocusHandoff::new(focus.clone(), is_compact, selection);
            let target = handoff
                .locate_in(node, true)
                .or_else(|| handoff.locate_in(node, false));
            if let Some(target) = target {
                if let Some(chain) = FocusChainHolder::current() {
                    chain.focus(Some(&target), visible);
                }
                crate::primitives::layout::ScrollReveal::reveal(&target, false);
            }
        })));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(SplitFocusHandoffHost);
