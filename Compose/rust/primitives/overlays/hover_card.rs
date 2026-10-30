//! Translation of Primitives/Overlays/HoverCard.swift.
//!
//! Pointer detail with delayed opening and a grace period for crossing onto
//! interactive content. Touch surfaces have no hover, so a tap on the label
//! opens the detail as a sheet.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use cove_core::scene::{
    Align, AnimatorScheduler, EventResult, FlexDirection, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, Node, NodeResource, Wrap,
};
use cove_core::{Point, Size};
use cove_composition::{AnyView, PrimitiveView, View};

use crate::modifiers::interaction::{AccessibilityExt, HoverExt};
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SurfaceStyleExt};
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::overlays::modal::BottomSheet;
use crate::primitives::overlays::portal::{
    PortalPlacementMode, PortalResource, PortalStoreHolder,
};
use crate::theme::theme_for_node;
use crate::BoxView;

/// Delays, sizing and enablement for one hover card.
#[derive(Clone, Copy)]
pub struct HoverCardTiming {
    pub open_delay: f64,
    pub close_delay: f64,
    pub width: Option<f32>,
    pub is_enabled: bool,
}

impl Default for HoverCardTiming {
    fn default() -> Self {
        Self { open_delay: 0.3, close_delay: 0.15, width: Some(300.0), is_enabled: true }
    }
}

impl HoverCardTiming {
    fn validate(&mut self) {
        self.open_delay = if self.open_delay.is_finite() {
            self.open_delay.clamp(0.0, 60.0)
        } else {
            0.3
        };
        self.close_delay = if self.close_delay.is_finite() {
            self.close_delay.clamp(0.0, 60.0)
        } else {
            0.15
        };
        self.width = self.width.map(|value| {
            if value.is_finite() { value.clamp(80.0, 4096.0) } else { 300.0 }
        });
    }
}

/// Pointer detail with delayed opening; a tap opens the same detail as a sheet.
pub struct HoverCard {
    open_delay: f64,
    close_delay: f64,
    width: Option<f32>,
    is_enabled: bool,
    label: AnyView,
    content: AnyView,
}

impl HoverCard {
    pub fn new(label: AnyView, content: AnyView) -> Self {
        Self {
            open_delay: 0.3,
            close_delay: 0.15,
            width: Some(300.0),
            is_enabled: true,
            label,
            content,
        }
    }

    pub fn open_delay(mut self, delay: f64) -> Self {
        self.open_delay = delay;
        self
    }
    pub fn close_delay(mut self, delay: f64) -> Self {
        self.close_delay = delay;
        self
    }
    pub fn width(mut self, width: Option<f32>) -> Self {
        self.width = width;
        self
    }
    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    fn timing(&self) -> HoverCardTiming {
        HoverCardTiming {
            open_delay: self.open_delay,
            close_delay: self.close_delay,
            width: self.width,
            is_enabled: self.is_enabled,
        }
    }
}

impl View for HoverCard {
    fn body(&self) -> AnyView {
        let label = self.label.clone();
        let content = self.content.clone();
        let timing = self.timing();
        let enabled = self.is_enabled;
        let reader = cove_composition::adaptive::SurfaceReader::new(move |surface| {
            if crate::primitives::overlays::modal::uses_sheet(surface) {
                AnyView::new(TouchHoverCard::new(label.clone(), content.clone(), enabled))
            } else {
                AnyView::new(HoverCardHost::new(
                    label.clone(),
                    content.clone(),
                    timing,
                ))
            }
        });
        AnyView::new(reader)
    }
}

/// Pointer host: owns the portal resource and the timing session.
struct HoverCardHost {
    label: AnyView,
    content: AnyView,
    options: HoverCardTiming,
}

impl HoverCardHost {
    fn new(label: AnyView, content: AnyView, options: HoverCardTiming) -> Self {
        Self { label, content, options }
    }
}

impl PrimitiveView for HoverCardHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.add_resource(PortalResource::new());
        node.add_resource(HoverCardSession::new());
        node.set_attachment("hoverCard.host", true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_align_items(Align::FlexStart);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let session = node.first_resource::<HoverCardSession>().expect("session resource");
        session.configure(self.content.clone(), self.options);
        if let Some(registry) = InteractionRegistryHolder::current() {
            let hover_session = Rc::downgrade(&session);
            registry.set_hover(
                node,
                Rc::new(move |phase: HoverPhase| {
                    if let Some(session) = hover_session.upgrade() {
                        session.set_trigger_hovered(phase == HoverPhase::Enter);
                    }
                }),
            );
            let key_session = Rc::downgrade(&session);
            registry.set_key(
                node,
                InputHandlerRoute::new(
                    cove_core::scene::InputHandlerRole::Shortcut,
                    cove_core::scene::InputRoutingPriority::NORMAL,
                    "hover-card",
                ),
                Rc::new(move |event, phase| {
                    if phase == cove_core::scene::EventPhase::Capture
                        || event.scancode != crate::Scancode::ESCAPE
                    {
                        return EventResult::Ignored;
                    }
                    key_session
                        .upgrade()
                        .is_some_and(|session| session.dismiss())
                        .then(|| EventResult::Handled)
                        .unwrap_or(EventResult::Ignored)
                }),
            );
        }
        let layout_session = Rc::downgrade(&session);
        node.set_layout_did_update(Some(Rc::new(move |node| {
            if let Some(session) = layout_session.upgrade() {
                session.update_position(node);
            }
        })));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.label.clone()]
    }
}
crate::impl_primitive_view!(HoverCardHost);

/// Delays, hover intent and portal ownership share the node's lifetime.
struct HoverCardSession {
    node: RefCell<Option<Node>>,
    content: RefCell<Option<AnyView>>,
    options: RefCell<HoverCardTiming>,
    trigger_hovered: Cell<bool>,
    card_hovered: Cell<bool>,
    dismissed_until_leave: Cell<bool>,
    presented: Cell<bool>,
    pending: Cell<Option<bool>>,
    elapsed: Cell<f64>,
    is_finished: Cell<bool>,
    me: RefCell<Weak<HoverCardSession>>,
}

impl HoverCardSession {
    fn new() -> Rc<Self> {
        let session = Rc::new(Self {
            node: RefCell::new(None),
            content: RefCell::new(None),
            options: RefCell::new(HoverCardTiming::default()),
            trigger_hovered: Cell::new(false),
            card_hovered: Cell::new(false),
            dismissed_until_leave: Cell::new(false),
            presented: Cell::new(false),
            pending: Cell::new(None),
            elapsed: Cell::new(0.0),
            is_finished: Cell::new(true),
            me: RefCell::new(Weak::new()),
        });
        *session.me.borrow_mut() = Rc::downgrade(&session);
        session
    }

    fn set_trigger_hovered(&self, value: bool) {
        self.trigger_hovered.set(value);
        self.refresh();
    }

    fn set_card_hovered(&self, value: bool) {
        self.card_hovered.set(value);
        self.refresh();
    }

    fn configure(&self, content: AnyView, mut options: HoverCardTiming) {
        options.validate();
        *self.content.borrow_mut() = Some(content);
        *self.options.borrow_mut() = options;
        if !options.is_enabled {
            self.trigger_hovered.set(false);
            self.card_hovered.set(false);
            self.hide();
            return;
        }
        if self.presented.get() {
            self.show();
        }
        self.refresh();
    }

    fn refresh(&self) {
        let options = *self.options.borrow();
        let hovered = self.trigger_hovered.get() || self.card_hovered.get();
        if !hovered {
            self.dismissed_until_leave.set(false);
        }
        let wanted = options.is_enabled && hovered && !self.dismissed_until_leave.get();
        if wanted == self.presented.get() {
            self.stop_waiting();
            return;
        }
        if self.pending.get() != Some(wanted) {
            self.stop_waiting();
            self.pending.set(Some(wanted));
        }
        let delay = if wanted { options.open_delay } else { options.close_delay };
        self.elapsed.set(0.0);
        if delay == 0.0 {
            self.stop_waiting();
            if wanted { self.show() } else { self.hide() };
            return;
        }
        if self.is_finished.get() {
            self.is_finished.set(false);
            AnimatorScheduler::current().register(Rc::new(ClockRef {
                session: self.me.borrow().clone(),
            }));
        }
    }

    /// Advance the open/close timer; returns whether more ticks are needed.
    fn tick(&self, delta_time: f64) -> bool {
        if self.is_finished.get() {
            return false;
        }
        self.elapsed.set(self.elapsed.get() + delta_time.clamp(0.0, 60.0));
        let options = *self.options.borrow();
        let wanted = self.pending.get().unwrap_or(false);
        let delay = if wanted { options.open_delay } else { options.close_delay };
        if self.elapsed.get() >= delay {
            self.stop_waiting();
            if wanted { self.show() } else { self.hide() };
            return false;
        }
        true
    }

    fn stop_waiting(&self) {
        self.is_finished.set(true);
        self.pending.set(None);
        self.elapsed.set(0.0);
    }

    fn show(&self) {
        let Some(node) = self.node.borrow().clone() else { return };
        if !node.accepts_subtree_input() {
            return;
        }
        let Some(content) = self.content.borrow().clone() else { return };
        let options = *self.options.borrow();
        if !options.is_enabled {
            return;
        }
        self.presented.set(true);
        let weak = self.me.borrow().clone();
        let card = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            cove_core::scene::Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            content,
        )
        .padding_edges(theme_for_node(&node).spacing.md, 0.0)
        .floating_surface(theme_for_node(&node).radius.md)
        .on_hover(move |hovered| {
            if let Some(session) = weak.upgrade() {
                session.set_card_hovered(hovered);
            }
        });
        let position = self.position(&node);
        if let Some(resource) = node.first_resource::<PortalResource>() {
            resource.present(
                Some(PortalStoreHolder::current()),
                position,
                options.width,
                AnyView::new(card),
                false,
                PortalPlacementMode::Fixed,
                None,
                None,
            );
            let anchor_node = node.clone();
            let weak = self.me.borrow().clone();
            resource.set_dismissal(
                Rc::new(move || anchor_node.absolute_frame()),
                Rc::new(move || {
                    if let Some(session) = weak.upgrade() {
                        session.dismiss();
                    }
                }),
            );
        }
    }

    fn position(&self, node: &Node) -> Point {
        let frame = node.absolute_frame();
        Point::new(frame.x, frame.y + frame.height + 6.0)
    }

    fn update_position(&self, node: &Node) {
        if !node.accepts_subtree_input() {
            self.dismiss();
            return;
        }
        if !self.presented.get() {
            return;
        }
        if let Some(resource) = node.first_resource::<PortalResource>() {
            resource.update_position(self.position(node));
        }
    }

    fn dismiss(&self) -> bool {
        let was_presented = self.presented.get();
        self.dismissed_until_leave.set(true);
        self.hide();
        was_presented
    }

    fn hide(&self) {
        self.stop_waiting();
        self.presented.set(false);
        self.card_hovered.set(false);
        if let Some(node) = self.node.borrow().clone()
            && let Some(resource) = node.first_resource::<PortalResource>()
        {
            resource.unmount(&node);
        }
    }
}

impl NodeResource for HoverCardSession {
    fn mount(&self, node: &Node) {
        *self.node.borrow_mut() = Some(node.clone());
    }

    fn unmount(&self, node: &Node) {
        self.hide();
        *self.node.borrow_mut() = None;
        *self.content.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// A scheduled open/close tick, so the timer only needs the scheduler.
struct ClockRef {
    session: Weak<HoverCardSession>,
}

impl cove_core::scene::AnyAnimationController for ClockRef {
    fn is_finished(&self) -> bool {
        self.session
            .upgrade()
            .is_none_or(|session| !session.tick(0.0) && session.is_finished.get())
    }

    fn tick(&self, delta_time: f64) {
        if let Some(session) = self.session.upgrade() {
            session.tick(delta_time);
        }
    }

    fn finish_immediately(&self) {
        if let Some(session) = self.session.upgrade() {
            session.hide();
        }
    }

    fn cancel(&self) {
        if let Some(session) = self.session.upgrade() {
            session.hide();
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Touch path: a plain button opens the detail in a bottom sheet.
struct TouchHoverCard {
    label: AnyView,
    content: AnyView,
    is_enabled: bool,
    is_presented: cove_core::State<bool>,
}

impl TouchHoverCard {
    fn new(label: AnyView, content: AnyView, is_enabled: bool) -> Self {
        Self {
            label,
            content,
            is_enabled,
            is_presented: cove_core::State::new(false),
        }
    }
}

impl View for TouchHoverCard {
    fn body(&self) -> AnyView {
        if !self.is_enabled && self.is_presented.get() {
            self.is_presented.set(false);
        }
        let presented = self.is_presented.clone();
        let trigger = Button::with_options(
            self.label.clone(),
            move || presented.set(true),
            ButtonRole::Normal,
            self.is_enabled,
            false,
            false,
            None,
        )
        .button_style(crate::theme::styles::buttons::plain());
        let sheet = BottomSheet::new(
            self.is_presented.binding(),
            self.content
                .clone()
                .padding_edges(8.0, 0.0),
        );
        let column = BoxView::new(
            FlexDirection::Column,
            Align::FlexStart,
            cove_core::scene::Justify::FlexStart,
            Wrap::NoWrap,
            0.0,
            AnyView::new(crate::TupleView::new(vec![AnyView::new(trigger), AnyView::new(sheet)])),
        );
        AnyView::new(column)
    }
}
