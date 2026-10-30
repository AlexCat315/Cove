//! Translation of Primitives/Overlays/Anchored/Anchoring.swift.
//!
//! The shared portal boundary for popovers, confirmation bubbles and menus. The
//! anchor is the nearest preceding laid-out sibling, so the host can sit before
//! or after its trigger in the tree.
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, EventResult, LayoutNode, Node, PointerCaptureHolder, PositionType,
};
use cove_core::{Point, Rect, Size};
use crate::animation::transition::Transition;
use crate::modifiers::layout::{LayoutFrameExt, PaddingExt};
use crate::primitives::overlays::portal::portal_registry::PortalResource;
use crate::primitives::overlays::portal::{
    portal_window_bounds, PortalFitConstraints, PortalPlacementMode, PortalStoreEnvironment,
    PortalStoreHolder,
};
use crate::primitives::overlays::focus_scope::FocusScope;
use crate::{impl_primitive_view, AnyView, BoxView, PrimitiveView, View};

use super::placement::{PopoverEdge, PopoverGeometry, PopoverOverflow, PopoverPlacement, PopoverSurface};

/// Anchor description for one presented overlay.
pub struct PopoverAnchor {
    owner: Option<Node>,
    surface: RefCell<Option<Node>>,
    overflow: PopoverOverflow,
    style: PopoverSurface,
    margin: f32,
    painted: RefCell<Option<Rect>>,
}

impl PopoverAnchor {
    pub fn new(owner: &Node, overflow: PopoverOverflow, surface: PopoverSurface, margin: f32) -> Self {
        Self {
            owner: Some(owner.clone()),
            surface: RefCell::new(None),
            overflow,
            style: surface,
            margin,
            painted: RefCell::new(None),
        }
    }

    pub fn set_surface(&self, node: &Node) {
        *self.surface.borrow_mut() = Some(node.clone());
    }

    /// Repaint the bubble when its anchor moved, and mirror the direction.
    pub fn refresh_paint(&self) {
        let next = self.frame();
        if *self.painted.borrow() != Some(next) {
            *self.painted.borrow_mut() = Some(next);
            if let Some(surface) = self.surface.borrow().as_ref() {
                surface.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("popoverAnchor"));
            }
        }
        if let (Some(surface), Some(layout)) = (self.surface.borrow().as_ref(), None::<LayoutNode>) {
            let _ = (surface, layout);
        }
    }

    /// The nearest preceding sibling with a non-zero frame, walking outwards.
    pub fn frame(&self) -> Rect {
        let Some(owner) = self.owner.as_ref() else { return Rect::new(0.0, 0.0, 0.0, 0.0) };
        let mut candidate = Some(owner.clone());
        while let Some(node) = candidate {
            let Some(parent) = node.parent() else { break };
            let children = parent.children();
            if let Some(index) = children.iter().position(|child| child == &node) {
                for sibling in children[..index].iter().rev() {
                    let frame = sibling.frame();
                    if frame.width > 0.0 || frame.height > 0.0 {
                        return sibling.absolute_frame();
                    }
                    let mut pending = sibling.children();
                    while let Some(item) = pending.pop() {
                        let frame = item.frame();
                        if frame.width > 0.0 || frame.height > 0.0 {
                            return item.absolute_frame();
                        }
                        pending.extend(item.children());
                    }
                }
            }
            candidate = parent.parent();
        }
        Rect::new(0.0, 0.0, 0.0, 0.0)
    }

    pub fn geometry(&self, size: Size, placement: PopoverPlacement) -> PopoverGeometry {
        let window = self
            .owner
            .as_ref()
            .map(portal_window_bounds)
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0));
        PopoverGeometry::new(
            self.frame(),
            size,
            window,
            placement,
            self.is_rtl(),
            self.overflow,
            self.margin,
            if self.style == PopoverSurface::Bubble { PopoverGeometry::GAP } else { 0.0 },
        )
    }

    pub fn is_rtl(&self) -> bool {
        let mut candidate = self.owner.clone();
        while let Some(node) = candidate.as_ref() {
            if let Some(layout) = node.layout_node() {
                return layout.direction() == cove_core::scene::Direction::Rtl;
            }
            candidate = node.parent();
        }
        false
    }
}

#[derive(Clone, Copy, PartialEq)]
struct AnchoredIdentity {
    width_bits: Option<u32>,
    placement: PopoverPlacement,
    sizing_is_anchor: bool,
    surface: PopoverSurface,
    overflow: PopoverOverflow,
}

/// The shared portal boundary for popovers, confirmation bubbles and menus.
pub struct AnchoredOverlayHost {
    pub is_presented: bool,
    pub width: Option<f32>,
    pub placement: PopoverPlacement,
    pub sizing_anchor: bool,
    pub surface: PopoverSurface,
    pub overflow: PopoverOverflow,
    pub on_dismiss: Rc<dyn Fn()>,
    pub key_handler: Option<Rc<dyn Fn(&cove_platform::KeyEvent, cove_core::scene::EventPhase) -> EventResult>>,
    pub content: AnyView,
}

impl AnchoredOverlayHost {
    pub fn new(is_presented: bool, content: impl View + 'static) -> Self {
        Self {
            is_presented,
            width: None,
            placement: PopoverPlacement::BottomLeading,
            sizing_anchor: false,
            surface: PopoverSurface::Content,
            overflow: PopoverOverflow::Flip,
            on_dismiss: Rc::new(|| {}),
            key_handler: None,
            content: AnyView::new(content),
        }
    }

    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn sizing_anchor(mut self, sizing_anchor: bool) -> Self {
        self.sizing_anchor = sizing_anchor;
        self
    }

    pub fn surface(mut self, surface: PopoverSurface) -> Self {
        self.surface = surface;
        self
    }

    pub fn overflow(mut self, overflow: PopoverOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_dismiss = Rc::new(handler);
        self
    }

    pub fn key_handler(
        mut self,
        handler: impl Fn(&cove_platform::KeyEvent, cove_core::scene::EventPhase) -> EventResult + 'static,
    ) -> Self {
        self.key_handler = Some(Rc::new(handler));
        self
    }
}

fn place(
    node: &Node,
    resource: &PortalResource,
    anchor: &PopoverAnchor,
    placement: PopoverPlacement,
    sizing_anchor: bool,
    margin: f32,
    provisional: Size,
) {
    anchor.refresh_paint();
    let size = resource.frame().map(|frame| Size { width: frame.width, height: frame.height }).unwrap_or(provisional);
    let geometry = anchor.geometry(size, placement);
    let minimum_width = if sizing_anchor { anchor.frame().width } else { 0.0 };
    resource.update_fit(PortalFitConstraints {
        minimum_width,
        viewport_margin: margin,
        maximum_width: Some(geometry.maximum_size.width),
        maximum_height: Some(geometry.maximum_size.height),
    });
    resource.update_position(Point::new(geometry.frame.x, geometry.frame.y));
}

impl PrimitiveView for AnchoredOverlayHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let resource: Rc<dyn cove_core::scene::NodeResource> = PortalResource::new();
        node.add_resource(resource);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(0.0, cove_core::scene::Edge::Start);
        layout.set_position_percent(100.0, cove_core::scene::Edge::Top);
        layout.set_width(Some(0.0));
        layout.set_height(Some(0.0));
    }

    fn update_node(&self, node: &Node) {
        let Some(resource) = node.first_resource::<PortalResource>() else { return };
        if !self.is_presented {
            resource.dismiss(node);
            return;
        }
        if !node.accepts_subtree_input() {
            cove_core::scene::NodeResource::unmount(&*resource, node);
            (self.on_dismiss)();
            return;
        }
        let margin = if self.sizing_anchor {
            12.0
        } else if self.surface == PopoverSurface::Bubble {
            8.0
        } else {
            6.0
        };
        let anchor = PopoverAnchor::new(node, self.overflow, self.surface, margin);
        let minimum_width = if self.sizing_anchor { anchor.frame().width } else { 0.0 };
        let desired_width = self.width.map(|width| minimum_width.max(width));
        let provisional = Size { width: desired_width.unwrap_or(300.0), height: 160.0 };
        let geometry = anchor.geometry(resource.frame().map(|frame| Size { width: frame.width, height: frame.height }).unwrap_or(provisional), self.placement);
        let on_dismiss = self.on_dismiss.clone();
        let key_handler = self.key_handler.clone();
        let surface = self.surface;
        let placement = self.placement;
        let content = self.content.clone();
        let margin_for_key = margin;
        let dismissed = on_dismiss.clone();
        let inner = AnyView::new(FocusScope::new(AnyView::new(MenuKeyHost {
            on_key: Rc::new(move |event: &cove_platform::KeyEvent| {
                if event.scancode == crate::Scancode::ESCAPE {
                    dismissed();
                    return true;
                }
                key_handler
                    .as_ref()
                    .is_some_and(|handler| handler(event, cove_core::scene::EventPhase::Target) == EventResult::Handled)
            }),
            content: match surface {
                PopoverSurface::Bubble => AnyView::new(PopoverSurfaceHost {
                    anchor: Rc::new(PopoverAnchor::new(node, self.overflow, surface, margin)),
                    placement,
                    content: content.clone(),
                }),
                PopoverSurface::Content => AnyView::new(
                    BoxView::new(
                        cove_core::scene::FlexDirection::Column,
                        cove_core::scene::Align::Stretch,
                        cove_core::scene::Justify::FlexStart,
                        cove_core::scene::Wrap::NoWrap,
                        0.0,
                        content.clone(),
                    )
                    .padding_insets(crate::modifiers::layout::EdgeInsets::new(4.0, 0.0, 0.0, 0.0)),
                ),
            },
        })));
        let store = Some(
            node.composition_value(&PortalStoreEnvironment::key())
                .unwrap_or_else(PortalStoreHolder::current),
        );
        let placement_anchor = Rc::new(PopoverAnchor::new(node, self.overflow, surface, margin));
        let placement_for_size = placement_anchor.clone();
        resource.present(
            store,
            Point::new(geometry.frame.x, geometry.frame.y),
            desired_width,
            inner,
            false,
            PortalPlacementMode::Fixed,
            Some(Rc::new(move |size: Size| {
                let frame = placement_for_size.geometry(size, placement).frame;
                Point::new(frame.x, frame.y)
            })),
            Some(
                Transition::opacity()
                    .combined(Transition::offset(0.0, if surface == PopoverSurface::Content { -6.0 } else { 0.0 })),
            ),
        );
        let dismissal_anchor = Rc::new(PopoverAnchor::new(node, self.overflow, surface, margin));
        let dismissal_anchor_inner = dismissal_anchor.clone();
        let dismiss = self.on_dismiss.clone();
        resource.set_dismissal(
            Rc::new(move || dismissal_anchor_inner.frame()),
            Rc::new(move || dismiss()),
        );
        place(node, &resource, &anchor, self.placement, self.sizing_anchor, margin_for_key, provisional);
        let weak = node.downgrade();
        let placement_for_layout = self.placement;
        let sizing = self.sizing_anchor;
        node.set_layout_did_update(Some(Rc::new(move |_owner: &Node| {
            let Some(node) = Node::upgrade(&weak) else { return };
            let Some(resource) = node.first_resource::<PortalResource>() else { return };
            let margin = if sizing { 12.0 } else { 8.0 };
            let anchor = PopoverAnchor::new(&node, PopoverOverflow::Flip, PopoverSurface::Bubble, margin);
            place(&node, &resource, &anchor, placement_for_layout, sizing, margin, provisional);
        }) as cove_core::scene::LayoutUpdateHook));
        let identity = AnchoredIdentity {
            width_bits: self.width.map(f32::to_bits),
            placement: self.placement,
            sizing_is_anchor: self.sizing_anchor,
            surface: self.surface,
            overflow: self.overflow,
        };
        let weak = node.downgrade();
        node.update_overlay_draw(identity, move |_list, _origin| {
            let Some(owner) = Node::upgrade(&weak) else { return };
            let Some(resource) = owner.first_resource::<PortalResource>() else { return };
            let margin = if sizing { 12.0 } else { 8.0 };
            let anchor = PopoverAnchor::new(&owner, PopoverOverflow::Flip, PopoverSurface::Bubble, margin);
            place(&owner, &resource, &anchor, placement_for_layout, sizing, margin, provisional);
        });
        let _ = (AccessibilityActions::default(), PointerCaptureHolder::current(), dismissal_anchor, on_dismiss);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(AnchoredOverlayHost);

/// Key host that owns Escape dismissal for an anchored overlay.
struct MenuKeyHost {
    on_key: Rc<dyn Fn(&cove_platform::KeyEvent) -> bool>,
    content: AnyView,
}

impl PrimitiveView for MenuKeyHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let Some(registry) = cove_core::scene::InteractionRegistryHolder::current() else { return };
        let on_key = self.on_key.clone();
        registry.set_key(node, cove_core::scene::InputHandlerRoute::overlay(), Rc::new(move |event, phase| {
            if phase == cove_core::scene::EventPhase::Capture {
                return EventResult::Ignored;
            }
            if on_key(event) { EventResult::Handled } else { EventResult::Ignored }
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(MenuKeyHost);

/// The bubble painter: card, pointer triangle and border.
pub struct PopoverSurfaceHost {
    pub anchor: Rc<PopoverAnchor>,
    pub placement: PopoverPlacement,
    pub content: AnyView,
}

impl PrimitiveView for PopoverSurfaceHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(cove_core::scene::FlexDirection::Column);
        layout.set_align_items(cove_core::scene::Align::Stretch);
        layout.set_flex_shrink(0.0);
        layout.set_direction(if self.anchor.is_rtl() {
            cove_core::scene::Direction::Rtl
        } else {
            cove_core::scene::Direction::Ltr
        });
        let maximum = self.anchor.geometry(Size::default(), self.placement).maximum_size.height;
        layout.set_max_height(Some(maximum));
    }

    fn update_node(&self, node: &Node) {
        self.anchor.set_surface(node);
        node.set_clips_to_bounds(true);
        let weak = node.downgrade();
        let placement = self.placement;
        node.update_draw(placement, move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else { return };
            let frame = owner.frame();
            if frame.width <= 20.0 || frame.height <= 20.0 {
                return;
            }
            let theme = crate::theme::theme_for_node(&owner);
            let inset = PopoverGeometry::INSET;
            let body = Rect::new(
                origin.x + inset,
                origin.y + inset,
                frame.width - inset * 2.0,
                frame.height - inset * 2.0,
            );
            // Panels use the tighter radius with a 20-pt lift shadow.
            let radius = theme.radius.lg.min(body.width.min(body.height) / 2.0);
            let fill = theme.colors.surface_floating.multiplied_alpha(owner.opacity());
            let border = theme.colors.border.multiplied_alpha(owner.opacity());
            let _ = list.add_rounded_rect(body, radius, fill);
            let _ = list.add_rounded_rect_stroke(body, radius, 1.0, border);
            let geometry = PopoverAnchor::new(&owner, super::placement::PopoverOverflow::Flip, super::placement::PopoverSurface::Bubble, 8.0)
                .geometry(Size { width: frame.width, height: frame.height }, placement);
            let half = 7.0_f32;
            let absolute = owner.absolute_frame();
            let anchor_frame = super::placement::PopoverGeometry::MARGIN;
            let _ = anchor_frame;
            let (a, b, tip) = if geometry.edge.is_vertical() {
                let center = (body.x + radius + half).max(
                    (origin.x + (anchor_frame - absolute.x)).min(body.x + body.width - radius - half),
                );
                let base = if geometry.edge == PopoverEdge::Top { body.y + body.height } else { body.y };
                (
                    Point::new(center - half, base),
                    Point::new(center + half, base),
                    Point::new(center, base + if geometry.edge == PopoverEdge::Top { inset } else { -inset }),
                )
            } else {
                let center = (body.y + radius + half).max(
                    (origin.y + (anchor_frame - absolute.y)).min(body.y + body.height - radius - half),
                );
                let base = if geometry.edge == PopoverEdge::Left { body.x + body.width } else { body.x };
                (
                    Point::new(base, center - half),
                    Point::new(base, center + half),
                    Point::new(base + if geometry.edge == PopoverEdge::Left { inset } else { -inset }, center),
                )
            };
            let _ = list.add_triangle(a, b, tip, fill);
            let _ = list.add_line(a, tip, 1.0, border);
            let _ = list.add_line(b, tip, 1.0, border);
        });
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let maximum = self.anchor.geometry(Size::default(), self.placement).maximum_size.height;
        let scroll = crate::primitives::layout::ScrollView::new(
            crate::primitives::layout::ScrollAxis::Both,
            self.content.clone(),
        )
        .scrollbar_gutter(crate::primitives::layout::ScrollbarGutter::Overlay);
        vec![AnyView::new(
            AnyView::new(scroll)
                .frame(None, None, None, None, None, Some((maximum - 48.0).max(0.0)))
                .padding(24.0),
        )]
    }
}
impl_primitive_view!(PopoverSurfaceHost);
