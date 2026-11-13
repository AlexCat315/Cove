//! Translation of Primitives/Overlays/Menu/ContextMenu.swift.
//!
//! Right-click (or Ctrl-click) presents a menu at the pointer. A native backend
//! shows the system menu and runs the chosen command; otherwise the self-drawn
//! panel is presented through the portal. Touch surfaces have no secondary
//! click, so a long press opens the same menu as a sheet.
use crate::Scope;
use std::rc::Rc;

use cove_core::scene::{
    Align, EventPhase, EventResult, FlexDirection, InputHandlerRole, InputHandlerRoute,
    InputRoutingPriority, InteractionRegistryHolder, Justify, LayoutNode, NativeMenuPresenter,
    Node, PointerCaptureAttachmentKey, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_core::{Binding, Environment, InputCapabilities, Point, Rect};
use cove_platform::MouseButton;

use crate::adaptive::{SurfaceEnvironment, SurfaceReader};
use crate::animation::transition::{Transition, TransitionEdge};
use crate::primitives::controls::adaptive::long_press::TouchLongPress;
use crate::primitives::overlays::focus_scope::FocusScope;
use crate::primitives::overlays::menu::{Menu, MenuEntry, NativeMenuBuilder};
use crate::primitives::overlays::modal::uses_sheet;
use crate::primitives::overlays::popover::TouchOverlay;
use crate::primitives::overlays::portal::{
    PortalPlacementMode, PortalResource, PortalStoreEnvironment, PortalStoreHolder,
};
use crate::theme::theme_for_node;
use crate::{
    AnyView, BoxView, LayoutDebugAttachmentKey, PrimitiveView, TupleView, View, impl_primitive_view,
};

const HOST_MARKER: &str = "context-menu-anchor";

/// Right-click menu attached to its content.
pub struct ContextMenu {
    pub width: f32,
    pub entries: Rc<dyn Fn() -> Vec<MenuEntry>>,
    pub on_open: Rc<dyn Fn()>,
    /// Whether a press at this window point should open the menu. A host that
    /// covers a whole document uses it to stay out of the way of presses that
    /// belong to the control underneath.
    pub is_interested: Rc<dyn Fn(Point) -> bool>,
    content: AnyView,
}

impl ContextMenu {
    pub fn new(
        entries: impl Fn() -> Vec<MenuEntry> + 'static,
        content: impl View + 'static,
    ) -> Self {
        Self {
            width: 220.0,
            entries: Rc::new(entries),
            on_open: Rc::new(|| {}),
            is_interested: Rc::new(|_| true),
            content: AnyView::new(content),
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = if width.is_finite() {
            width.max(80.0)
        } else {
            220.0
        };
        self
    }

    pub fn on_open(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_open = Rc::new(handler);
        self
    }

    pub fn is_interested(mut self, predicate: impl Fn(Point) -> bool + 'static) -> Self {
        self.is_interested = Rc::new(predicate);
        self
    }
}

impl View for ContextMenu {
    fn body(&self, cx: &Scope) -> AnyView {
        let point = cx.state(|| None::<Point>);
        let content = self.content.clone();
        let entries = self.entries.clone();
        let width = self.width;
        let on_open = self.on_open.clone();
        let interested = self.is_interested.clone();
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            let sheet = uses_sheet(surface);
            let current = point.get();
            let open = {
                let entries = entries.clone();
                let on_open = on_open.clone();
                Rc::new(move |position: Point| {
                    on_open();
                    // A native backend shows the system menu and runs the chosen
                    // command; otherwise the self-drawn panel is presented.
                    if !sheet && NativeMenuPresenter::is_installed() {
                        let built = NativeMenuBuilder::build(&entries());
                        let snapshot = built.snapshot.clone();
                        NativeMenuPresenter::present(
                            &snapshot,
                            position,
                            Box::new(move |chosen| {
                                if let Some(chosen) = chosen {
                                    built.run(chosen);
                                }
                            }),
                        );
                    } else {
                        point.set(Some(position));
                    }
                })
            };
            let dismiss: Rc<dyn Fn()> = Rc::new(move || point.set(None));
            let mut children = vec![AnyView::new(ContextMenuHost {
                content: content.clone(),
                entries: if current.is_some() {
                    entries()
                } else {
                    Vec::new()
                },
                width,
                point: current,
                interested: interested.clone(),
                on_open: open,
                on_dismiss: dismiss.clone(),
            })];
            if sheet {
                let presented = Binding::new(
                    move || point.get().is_some(),
                    move |value| {
                        if !value {
                            point.set(None);
                        }
                    },
                );
                let activated = dismiss.clone();
                children.push(AnyView::new(TouchOverlay {
                    is_presented: presented,
                    content: AnyView::new(
                        Menu::new(if current.is_some() {
                            entries()
                        } else {
                            Vec::new()
                        })
                        .on_item_activated(move || activated())
                        .on_dismiss({
                            let dismiss = dismiss.clone();
                            move || dismiss()
                        }),
                    ),
                    on_key: None,
                }));
            }
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(children)),
            ))
        }))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Attaches a context menu to any view.
pub trait ContextMenuExt: View + Sized + 'static {
    fn context_menu(self, entries: Vec<MenuEntry>) -> ContextMenu {
        ContextMenu::new(move || entries.clone(), self).width(240.0)
    }

    fn context_menu_with(
        self,
        entries: impl Fn() -> Vec<MenuEntry> + 'static,
        on_open: impl Fn() + 'static,
    ) -> ContextMenu {
        ContextMenu::new(entries, self).on_open(on_open)
    }
}
impl<T: View + Sized + 'static> ContextMenuExt for T {}

struct ContextMenuHost {
    content: AnyView,
    entries: Vec<MenuEntry>,
    width: f32,
    point: Option<Point>,
    interested: Rc<dyn Fn(Point) -> bool>,
    on_open: Rc<dyn Fn(Point)>,
    on_dismiss: Rc<dyn Fn()>,
}

impl PrimitiveView for ContextMenuHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.add_resource(PortalResource::new());
        node.set_attachment(HOST_MARKER, true);
        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            HOST_MARKER.to_string(),
        );
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let hold = node.first_resource::<TouchLongPress>().unwrap_or_else(|| {
            let hold = TouchLongPress::new();
            node.add_resource(hold.clone());
            hold
        });
        *hold.on_recognized.borrow_mut() = self.on_open.clone();
        {
            let hold = Rc::downgrade(&hold);
            let lost: Rc<dyn Fn()> = Rc::new(move || {
                if let Some(hold) = hold.upgrade() {
                    hold.cancel();
                }
            });
            node.set_attachment(PointerCaptureAttachmentKey::DID_LOSE_CAPTURE, lost);
        }
        let surface = node.composition_value(&SurfaceEnvironment::key());
        let touch = surface.input.contains(InputCapabilities::TOUCH);
        if !touch {
            hold.cancel();
        }
        if let Some(registry) = InteractionRegistryHolder::current() {
            {
                let hold = hold.clone();
                registry.set_motion(
                    node,
                    InputHandlerRoute::scroll_chrome(),
                    Rc::new(move |event, _| {
                        hold.move_to(Point::new(event.x, event.y));
                        EventResult::Ignored
                    }),
                );
            }
            {
                let hold = hold.clone();
                registry.set_wheel(
                    node,
                    InputHandlerRoute::default(),
                    Rc::new(move |_, _| {
                        hold.cancel();
                        EventResult::Ignored
                    }),
                );
            }
            let interested = self.interested.clone();
            let on_open = self.on_open.clone();
            let weak = node.downgrade();
            registry.set_pointer(
                node,
                InputHandlerRoute::new(
                    InputHandlerRole::Control,
                    InputRoutingPriority::CHROME,
                    "context-menu",
                ),
                Rc::new(move |event, phase, event_phase| {
                    let here = Point::new(event.x, event.y);
                    if touch && event.button == MouseButton::Left && !event.modifiers.has_ctrl() {
                        if phase == PointerPhase::Down {
                            // Only a press on this host's own content arms the
                            // long press, so a document-wide region never takes
                            // the gesture from a control inside it.
                            if !interested(here) {
                                return EventResult::Ignored;
                            }
                            hold.begin(here);
                            if event_phase != EventPhase::Capture
                                && let Some(capture) = PointerCaptureHolder::current()
                                && capture.target().is_none()
                                && let Some(node) = Node::upgrade(&weak)
                            {
                                capture.acquire(&node, true);
                            }
                        } else {
                            hold.cancel();
                            if let Some(capture) = PointerCaptureHolder::current()
                                && let Some(node) = Node::upgrade(&weak)
                                && capture
                                    .target()
                                    .is_some_and(|target| target.id() == node.id())
                            {
                                capture.release();
                            }
                        }
                        return EventResult::Ignored;
                    }
                    let secondary = event.button == MouseButton::Right
                        || (event.button == MouseButton::Left && event.modifiers.has_ctrl());
                    if !secondary || !interested(here) {
                        return EventResult::Ignored;
                    }
                    // The chrome capture sweep runs before target delivery. A
                    // secondary press must reach the target so it can open the
                    // menu; only the delivered press is consumed.
                    if event_phase == EventPhase::Capture {
                        return EventResult::Ignored;
                    }
                    if phase == PointerPhase::Down {
                        on_open(here);
                    }
                    EventResult::Handled
                }),
            );
            // Keyboard equivalent of a secondary click: the Application key or
            // Shift+F10 opens the menu at the focused region's top-left, so the
            // menu is reachable without a pointer (Swift has no keyboard opener).
            let on_open = self.on_open.clone();
            let weak = node.downgrade();
            registry.set_key(
                node,
                InputHandlerRoute::control(),
                Rc::new(move |event, phase| {
                    if phase == EventPhase::Capture {
                        return EventResult::Ignored;
                    }
                    let menu_key = event.scancode == crate::Scancode::APPLICATION
                        || (event.scancode == crate::Scancode::F10 && event.modifiers.has_shift());
                    if !menu_key {
                        return EventResult::Ignored;
                    }
                    let Some(node) = Node::upgrade(&weak) else {
                        return EventResult::Ignored;
                    };
                    let frame = node.absolute_frame();
                    on_open(Point::new(frame.x, frame.y));
                    EventResult::Handled
                }),
            );
        }
        let Some(resource) = node.first_resource::<PortalResource>() else {
            return;
        };
        let point = match self.point {
            Some(point) if !uses_sheet(&surface) => point,
            _ => {
                resource.dismiss(node);
                return;
            }
        };
        let dismiss = self.on_dismiss.clone();
        let content = AnyView::new(FocusScope::with_restoring_commands(
            true,
            AnyView::new(
                Menu::new(self.entries.clone())
                    .width(self.width)
                    .max_visible_rows(12)
                    .on_item_activated({
                        let dismiss = dismiss.clone();
                        move || dismiss()
                    })
                    .on_dismiss(move || dismiss()),
            ),
        ));
        // Fall back to the ambient store like every other overlay: a host above
        // the window's composition local would otherwise never find a store.
        let store = node
            .composition_value(&PortalStoreEnvironment::key())
            .unwrap_or_else(PortalStoreHolder::current);
        // Opens at the pointer and extends right and down, so the top-left
        // corner is the one nearest the trigger.
        let transition = Transition::opacity()
            .combined(Transition::move_edge(TransitionEdge::Top, 6.0))
            .combined(PortalResource::surface_reveal(
                Point::ZERO,
                &theme_for_node(node),
            ));
        resource.present(
            Some(store),
            point,
            Some(self.width),
            content,
            false,
            PortalPlacementMode::Fixed,
            None,
            Some(transition),
        );
        resource.set_dismissal(Rc::new(|| Rect::ZERO), self.on_dismiss.clone());
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(ContextMenuHost);
