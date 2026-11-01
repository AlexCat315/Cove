//! Translation of Primitives/Layout/Sidebar/AdaptiveHosts.swift.
//!
//! The shell, layer, trigger and backdrop nodes the adaptive sidebar composes.
//! Each one exists to own a single responsibility on its own node: the shell
//! prepares focus, the layer owns the modal trap, the trigger is the focus
//! return target and the backdrop owns dismissal.
use std::rc::Rc;

use crate::primitives::overlays::modal::ModalDismissal;
use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, View};
use cove_core::scene::ModalFocusResource;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, EventResult, FocusChainHolder,
    InputHandlerRole, InputHandlerRoute, InputRoutingPriority, InteractionRegistryHolder, Node,
    PointerPhase,
};
use cove_platform::MouseButton;

use super::adaptive_options::SidebarGeometry;
use super::focus_handoff::SidebarFocusHandoff;

/// Owns focus preparation and completion around a presentation change.
pub struct SidebarShellHost {
    pub geometry: SidebarGeometry,
    pub focus: Rc<SidebarFocusHandoff>,
    content: AnyView,
}

impl SidebarShellHost {
    pub fn new(
        geometry: SidebarGeometry,
        focus: Rc<SidebarFocusHandoff>,
        content: impl View + 'static,
    ) -> Self {
        Self {
            geometry,
            focus,
            content: AnyView::new(content),
        }
    }
}

impl PrimitiveView for SidebarShellHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        Some(cove_core::scene::LayoutNode::new())
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_align_items(Align::Stretch);
        layout.set_min_height(Some(0.0));
        layout.set_min_width(Some(0.0));
    }

    fn update_node(&self, node: &Node) {
        let chain = FocusChainHolder::current();
        self.focus
            .prepare(self.geometry.mode, self.geometry.is_modal, chain.clone());
        let focus = self.focus.clone();
        node.set_layout_did_update(Some(Rc::new(move |_owner: &Node| {
            focus.complete(FocusChainHolder::current());
        }) as cove_core::scene::LayoutUpdateHook));
        let _ = chain;
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(SidebarShellHost);

/// The navigation panel: modal focus trap plus Escape dismissal.
pub struct SidebarLayer {
    pub geometry: SidebarGeometry,
    pub focus: Rc<SidebarFocusHandoff>,
    pub dismissal: ModalDismissal,
    pub on_dismiss: Rc<dyn Fn()>,
    content: AnyView,
}

impl SidebarLayer {
    pub fn new(
        geometry: SidebarGeometry,
        focus: Rc<SidebarFocusHandoff>,
        dismissal: ModalDismissal,
        on_dismiss: impl Fn() + 'static,
        content: impl View + 'static,
    ) -> Self {
        Self {
            geometry,
            focus,
            dismissal,
            on_dismiss: Rc::new(on_dismiss),
            content: AnyView::new(content),
        }
    }
}

impl PrimitiveView for SidebarLayer {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        let resource: Rc<dyn cove_core::scene::NodeResource> = Rc::new(ModalFocusResource::new());
        node.add_resource(resource);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        Some(cove_core::scene::LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        self.focus.set_navigation(node);
        node.set_is_focusable(self.geometry.is_modal);
        let chain = FocusChainHolder::current();
        let resource = node.first_resource::<ModalFocusResource>();
        if self.geometry.is_modal
            && let (Some(resource), Some(chain)) = (resource.as_ref(), chain.as_ref())
        {
            resource.bind(node, Some(chain.clone()), false);
        }
        let is_modal = self.geometry.is_modal;
        let weak = node.downgrade();
        let chain_for_layout = chain.clone();
        node.set_layout_did_update(Some(Rc::new(move |owner: &Node| {
            let Some(inner) = Node::upgrade(&weak) else {
                return;
            };
            if is_modal {
                if let Some(chain) = chain_for_layout.as_ref() {
                    chain.settle_scope(&inner);
                }
            } else if let Some(resource) = inner.first_resource::<ModalFocusResource>() {
                // The rail opener lives in this retained subtree. Restore only
                // after its visibility has been updated, so it is eligible to
                // receive focus.
                cove_core::scene::NodeResource::unmount(&*resource, &inner);
            }
            let _ = owner;
        }) as cove_core::scene::LayoutUpdateHook));

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let is_modal = self.geometry.is_modal;
        let closes_on_escape = self.dismissal.closes_on_escape;
        let on_dismiss = self.on_dismiss.clone();
        registry.set_key(
            node,
            InputHandlerRoute::overlay(),
            Rc::new(move |event, _phase| {
                if !is_modal || event.scancode != crate::Scancode::ESCAPE {
                    return EventResult::Ignored;
                }
                if closes_on_escape {
                    on_dismiss();
                }
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(SidebarLayer);

/// Focus return target for the compact trigger.
pub struct SidebarTriggerHost {
    pub focus: Rc<SidebarFocusHandoff>,
    content: AnyView,
}

impl SidebarTriggerHost {
    pub fn new(focus: Rc<SidebarFocusHandoff>, content: impl View + 'static) -> Self {
        Self {
            focus,
            content: AnyView::new(content),
        }
    }
}

impl PrimitiveView for SidebarTriggerHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        self.focus.set_trigger(node);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(SidebarTriggerHost);

/// Modal backdrop: dismisses on press and swallows wheel input.
pub struct SidebarBackdrop {
    pub dismissal: ModalDismissal,
    pub on_dismiss: Rc<dyn Fn()>,
}

impl SidebarBackdrop {
    pub fn new(dismissal: ModalDismissal, on_dismiss: impl Fn() + 'static) -> Self {
        Self {
            dismissal,
            on_dismiss: Rc::new(on_dismiss),
        }
    }
}

impl PrimitiveView for SidebarBackdrop {
    fn make_node(&self) -> Node {
        Node::new()
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_flex_grow(1.0);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(theme_for_node(node).colors.overlay));
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            is_hidden: true,
            ..Default::default()
        }));
        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let closes_on_backdrop = self.dismissal.closes_on_backdrop;
        let on_dismiss = self.on_dismiss.clone();
        registry.set_pointer(
            node,
            InputHandlerRoute::new(
                InputHandlerRole::Control,
                InputRoutingPriority::MODAL,
                "sidebar.backdrop",
            ),
            Rc::new(move |event, phase, event_phase| {
                if event_phase != cove_core::scene::EventPhase::Target {
                    return EventResult::Ignored;
                }
                if event.button == MouseButton::Left
                    && phase == PointerPhase::Down
                    && closes_on_backdrop
                {
                    on_dismiss();
                }
                EventResult::Handled
            }),
        );
        registry.set_wheel(
            node,
            InputHandlerRoute::overlay(),
            Rc::new(move |_event, phase| {
                if phase == cove_core::scene::EventPhase::Capture {
                    EventResult::Ignored
                } else {
                    EventResult::Handled
                }
            }),
        );
    }
}
crate::impl_primitive_view!(SidebarBackdrop);
