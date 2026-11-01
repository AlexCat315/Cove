//! Translation of Primitives/Overlays/Toast/NotificationHost.swift.
//!
//! Toast cards anchor at the top trailing edge at every width, matching a
//! notification banner rather than a bottom snackbar. The portal fit clamps the
//! final frame inside the window.
use crate::Scope;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::animation::content::AnimatedVisibility;
use crate::animation::transition::{Transition, TransitionEdge};
use crate::modifiers::layout::{FlexExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIconSource};
use crate::primitives::overlays::portal::portal_registry::PortalResource;
use crate::primitives::overlays::portal::{
    PortalPlacementMode, PortalStoreEnvironment, PortalStoreHolder, portal_window_bounds,
};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonRole, ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{ControlSizeViewExt, SemanticFontRef, ThemeReader};
use crate::{AnyView, BoxView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use cove_core::scene::{AnimatorScheduler, AnyAnimationController, LayoutNode, Node, NodeResource};
use cove_core::{Point, Rect, Signal};

use super::controller::{ToastAction, ToastController, ToastNotification};
use super::exit_retention::{ToastPresentation, ToastPresentationEntry};

const WINDOW_BOUNDS: &str = "toast.windowBounds";
const WINDOW_MARGIN: f32 = 16.0;

/// Where toast cards are anchored inside the window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToastPosition {
    #[default]
    Automatic,
    TopLeading,
    TopTrailing,
    BottomLeading,
    BottomTrailing,
}

impl ToastPosition {
    /// `Automatic` resolves to the top trailing edge at every width.
    pub fn resolved(self) -> Self {
        match self {
            Self::Automatic => Self::TopTrailing,
            other => other,
        }
    }

    fn is_trailing(self) -> bool {
        matches!(self, Self::TopTrailing | Self::BottomTrailing)
    }

    fn is_bottom(self) -> bool {
        matches!(self, Self::BottomLeading | Self::BottomTrailing)
    }

    /// Top-left origin passed to the portal. A bottom anchor uses the window
    /// height and lets the fit pass clamp to the content's real height.
    pub fn origin(self, bounds: Rect, width: f32) -> Point {
        let x = if self.is_trailing() {
            bounds.width - width - WINDOW_MARGIN
        } else {
            WINDOW_MARGIN
        };
        let y = if self.is_bottom() {
            bounds.height
        } else {
            WINDOW_MARGIN
        };
        Point::new(x.max(WINDOW_MARGIN), y.max(WINDOW_MARGIN))
    }
}

/// Presents the controller's active queue as a stack of toast cards.
pub struct NotificationHost {
    pub controller: Rc<ToastController>,
    pub width: f32,
    pub position: ToastPosition,
}

impl NotificationHost {
    pub fn new(controller: Rc<ToastController>) -> Self {
        Self {
            controller,
            width: 360.0,
            position: ToastPosition::Automatic,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(120.0);
        self
    }

    pub fn position(mut self, position: ToastPosition) -> Self {
        self.position = position;
        self
    }
}

impl View for NotificationHost {
    fn body(&self, cx: &Scope) -> AnyView {
        // One reconciler survives every body so cards can animate in and out
        // across recompositions; a card finishing its exit invalidates the host.
        let presentation = ToastPresentationHandle {
            reconciler: cx.remember(|| Rc::new(std::cell::RefCell::new(ToastPresentation::new()))),
            changed: cx.state(|| ()),
        };
        presentation.changed.track();
        // Reading the queue subscribes this scope, so a post re-runs `body`.
        let entries = presentation.reconciler.borrow_mut().update(
            &self.controller.notifications(),
            self.controller.max_visible,
        );
        AnyView::new(ToastPresenter {
            controller: self.controller.clone(),
            entries,
            width: self.width,
            position: self.position.resolved(),
            presentation,
        })
    }
}

/// The exit reconciler plus the signal its removals raise.
#[derive(Clone)]
struct ToastPresentationHandle {
    reconciler: Rc<std::cell::RefCell<ToastPresentation>>,
    changed: Signal<()>,
}

struct ToastPresenter {
    controller: Rc<ToastController>,
    entries: Vec<ToastPresentationEntry>,
    width: f32,
    position: ToastPosition,
    presentation: ToastPresentationHandle,
}

impl PrimitiveView for ToastPresenter {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let resource: Rc<dyn NodeResource> = PortalResource::new();
        node.add_resource(resource);
        let clock: Rc<dyn NodeResource> = ToastClock::new();
        node.add_resource(clock);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        if let Some(clock) = node.first_resource::<ToastClock>() {
            clock.configure(self.controller.clone());
        }
        present(
            node,
            &self.controller,
            &self.entries,
            self.width,
            self.position,
            &self.presentation,
        );
        let controller = self.controller.clone();
        let entries = self.entries.clone();
        let width = self.width;
        let position = self.position;
        let presentation = self.presentation.clone();
        let weak = node.downgrade();
        node.set_layout_did_update(Some(Rc::new(move |_owner: &Node| {
            let Some(node) = Node::upgrade(&weak) else {
                return;
            };
            if node.attachment::<Rect>(WINDOW_BOUNDS) == Some(portal_window_bounds(&node)) {
                return;
            }
            present(&node, &controller, &entries, width, position, &presentation);
        }) as cove_core::scene::LayoutUpdateHook));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        Vec::new()
    }
}
impl_primitive_view!(ToastPresenter);

fn present(
    node: &Node,
    controller: &Rc<ToastController>,
    entries: &[ToastPresentationEntry],
    width: f32,
    position: ToastPosition,
    presentation: &ToastPresentationHandle,
) {
    let bounds = portal_window_bounds(node);
    node.set_attachment(WINDOW_BOUNDS, bounds);
    let resource = node.first_resource::<PortalResource>();
    if entries.is_empty() {
        if let Some(resource) = resource.as_ref() {
            cove_core::scene::NodeResource::unmount(&**resource, node);
        }
        return;
    }
    let actual_width = width.min((bounds.width - 32.0).max(120.0));
    let store = Some(
        node.composition_value(&PortalStoreEnvironment::key())
            .unwrap_or_else(PortalStoreHolder::current),
    );
    let edge = if position.is_bottom() {
        TransitionEdge::Bottom
    } else {
        TransitionEdge::Top
    };
    let mut cards: Vec<AnyView> = Vec::new();
    for entry in entries {
        let presentation = presentation.clone();
        let controller = controller.clone();
        let id = entry.notification.id.clone();
        let revision = entry.revision;
        let card = AnyView::new(AnimatedVisibility::new(
            entry.is_visible,
            Transition::opacity()
                .combined(Transition::move_edge(edge, 12.0))
                .combined(Transition::collapse()),
            None,
            true,
            Some(Rc::new(move |visible| {
                if !visible {
                    presentation.reconciler.borrow_mut().remove(&id, revision);
                    presentation.changed.notify();
                }
            })),
            AnyView::new(ToastCard {
                notification: entry.notification.clone(),
                controller: controller.clone(),
            }),
        ));
        cards.push(card);
    }
    let stack = AnyView::new(BoxView::new(
        FlexDirection::Column,
        Align::Stretch,
        Justify::FlexStart,
        Wrap::NoWrap,
        10.0,
        AnyView::new(crate::TupleView::new(cards)),
    ));
    if let Some(resource) = resource.as_ref() {
        resource.present(
            store,
            position.origin(bounds, actual_width),
            Some(actual_width),
            stack,
            false,
            PortalPlacementMode::Fixed,
            None,
            None,
        );
    }
}

/// One toast card: inverted chip, title, message and up to two actions.
struct ToastCard {
    notification: ToastNotification,
    controller: Rc<ToastController>,
}

impl ToastCard {
    fn action_button(&self, action: &ToastAction) -> AnyView {
        let controller = self.controller.clone();
        let id = self.notification.id.clone();
        let dismisses = action.dismisses_notification;
        let handler = action.action.clone();
        AnyView::new(
            Button::with_options(
                Text::new(action.title.clone()),
                move || {
                    handler();
                    if dismisses {
                        controller.dismiss(&id);
                    }
                },
                action.role,
                true,
                false,
                false,
                None,
            )
            .button_style(GhostButtonStyle)
            .control_size(crate::theme::ControlSize::Small),
        )
    }
}

impl View for ToastCard {
    fn body(&self, _cx: &Scope) -> AnyView {
        let notification = self.notification.clone();
        let controller = self.controller.clone();
        AnyView::new(ThemeReader::new(move |theme: &crate::theme::Theme| {
            let on_chip = theme.colors.background;
            let on_chip_soft = theme.colors.background.multiplied_alpha(0.82);
            let mut lines: Vec<AnyView> = vec![AnyView::new(
                Text::new(notification.title.clone())
                    .truncated()
                    .semantic_font(SemanticFontRef::body_strong())
                    .foreground_color(on_chip),
            )];
            if !notification.message.is_empty() {
                lines.push(AnyView::new(
                    Text::new(notification.message.clone())
                        .semantic_font(SemanticFontRef::body())
                        .foreground_color(on_chip_soft),
                ));
            }
            if !notification.actions.is_empty() {
                let card = ToastCard {
                    notification: notification.clone(),
                    controller: controller.clone(),
                };
                let mut actions: Vec<AnyView> = vec![card.action_button(&notification.actions[0])];
                if notification.actions.len() > 1 {
                    actions.push(card.action_button(&notification.actions[1]));
                }
                lines.push(AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::Wrap,
                    8.0,
                    AnyView::new(crate::TupleView::new(actions)),
                )));
            }
            let mut row: Vec<AnyView> = vec![AnyView::new(
                BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    5.0,
                    AnyView::new(crate::TupleView::new(lines)),
                )
                .flex_with(1.0, 1.0, Some(0.0)),
            )];
            if notification.is_dismissible {
                let controller = controller.clone();
                let id = notification.id.clone();
                row.push(AnyView::new(
                    Button::icon_with_options(
                        ButtonIconSource::resource(
                            crate::primitives::media::ui_icon_catalog::action::x(),
                        ),
                        Some(12.0),
                        None,
                        ButtonRole::Normal,
                        true,
                        false,
                        false,
                        Some("Dismiss notification".to_string()),
                        move || controller.dismiss(&id),
                    )
                    .button_style(GhostButtonStyle)
                    .control_size(crate::theme::ControlSize::Small)
                    .foreground_color(on_chip_soft),
                ));
            }
            // Inverted toast: the primary text color fills the chip, the window
            // background color carries the label, and a wide lift shadow floats
            // it.
            AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::FlexStart,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    10.0,
                    AnyView::new(crate::TupleView::new(row)),
                )
                .foreground_color(on_chip)
                .padding_edges(14.0, 10.0)
                .background_color(theme.colors.on_surface)
                .corner_radius(theme.radius.lg)
                .shadow(cove_core::Color::from_rgba8(0, 0, 0, 64), 0.0, 6.0, 20.0),
            )
        }))
    }
}

/// Advances toast lifetimes while any visible card is counting down.
struct ToastClock {
    state: Rc<ToastClockState>,
    scheduler: RefCell<Option<AnimatorScheduler>>,
    controller: RefCell<Option<Rc<dyn AnyAnimationController>>>,
}

struct ToastClockState {
    controller: RefCell<Option<Rc<ToastController>>>,
    is_finished: Cell<bool>,
}

impl ToastClockState {
    fn cancel(&self) {
        self.is_finished.set(true);
    }
}

impl AnyAnimationController for ToastClockState {
    fn is_finished(&self) -> bool {
        self.is_finished.get()
    }

    fn tick(&self, delta_time: f64) {
        if self.is_finished.get() {
            return;
        }
        let Some(controller) = self.controller.borrow().clone() else {
            return;
        };
        controller.advance(delta_time);
        if !controller.needs_clock() {
            self.is_finished.set(true);
        }
    }

    fn finish_immediately(&self) {
        self.is_finished.set(true);
    }

    fn cancel(&self) {
        self.is_finished.set(true);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ToastClock {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            state: Rc::new(ToastClockState {
                controller: RefCell::new(None),
                is_finished: Cell::new(true),
            }),
            scheduler: RefCell::new(None),
            controller: RefCell::new(None),
        })
    }

    fn configure(&self, controller: Rc<ToastController>) {
        self.state.is_finished.set(!controller.needs_clock());
        *self.state.controller.borrow_mut() = Some(controller.clone());
        if self.state.is_finished.get() {
            if let (Some(scheduler), Some(existing)) = (
                self.scheduler.borrow().as_ref(),
                self.controller.borrow().as_ref(),
            ) {
                scheduler.unregister(existing);
            }
            self.controller.borrow_mut().take();
            return;
        }
        if self.controller.borrow().is_some() {
            return;
        }
        let any: Rc<dyn AnyAnimationController> = self.state.clone();
        *self.controller.borrow_mut() = Some(any.clone());
        if let Some(scheduler) = self.scheduler.borrow().as_ref() {
            scheduler.register(any);
        }
    }
}

impl NodeResource for ToastClock {
    fn mount(&self, _node: &Node) {
        *self.scheduler.borrow_mut() = Some(AnimatorScheduler::current());
    }

    fn unmount(&self, _node: &Node) {
        self.state.cancel();
        *self.scheduler.borrow_mut() = None;
        *self.state.controller.borrow_mut() = None;
    }

    fn as_any_rc(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}
