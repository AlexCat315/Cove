//! Translation of `Primitives/Overlays/Portal/PortalHost.swift`.
//!
//! Renders the registered portal entries for one window's store.
//!
//! Ownership resolution is the subtle part. The root view can be constructed
//! before `Application` enters its per-window `with_current` scope, so the store
//! is resolved from the primitive node's first update and then retained. It must
//! NOT be re-resolved on every update: later updates may run after the scoped
//! ambient has unwound, or while a different window is current, and
//! re-resolving would silently migrate the observer so this window's overlays
//! paint into another window. The process-shared fallback store is deliberately
//! not pinned — it is not ownership evidence.

use crate::Scope;
use std::cell::RefCell;
use std::rc::Rc;

use crate::modifiers::layout::LayoutFrameExt;
use crate::{ScrollAxis, ScrollView, ScrollbarGutter};
use cove_composition::{
    AnyView, LayoutDebugAttachmentKey, PrimitiveView, View, ViewExt, impl_primitive_view,
};
use cove_core::scene::{
    Edge, InputHandlerRoute, InteractionRegistryHolder, LayoutNode, Node, PointerPhase,
    PositionType, WheelRoutingAttachmentKey, WheelRoutingPriority,
};
use cove_core::{Point, Rect, Signal, Size};

use super::portal_placement::{PortalPlacement, portal_window_bounds, set_window_frame};
use super::portal_registry::{
    PortalEntry, PortalOwnership, PortalStore, PortalStoreEnvironment, PortalStoreHolder,
};

/// Node attachment carrying the store this host bound its observer to.
const BOUND_STORE: &str = "CoveUICompose.portalHost.store";
/// Node attachment carrying the observer token so it can be released.
const OBSERVER_TOKEN: &str = "CoveUICompose.portalHost.token";

/// Renders the portal entries registered in the owning window's store.
#[derive(Default)]
pub struct PortalHost;

impl PortalHost {
    pub fn new() -> Self {
        Self
    }
}

impl View for PortalHost {
    fn body(&self, cx: &Scope) -> AnyView {
        AnyView::new(PortalHostPrimitive {
            // Resolved once, then retained for the life of the host.
            store: cx.remember(|| Rc::new(RefCell::new(None))),
            // Incremented rather than mirroring the store's raw revision, so a
            // late window-store capture and its first entry (both `1`) cannot
            // collide into an equality no-op.
            revision: cx.state(|| 0),
        })
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

struct PortalHostPrimitive {
    store: Rc<RefCell<Option<Rc<PortalStore>>>>,
    revision: Signal<u64>,
}

impl PortalHostPrimitive {
    /// The store this host renders: the retained owner, else the current ambient,
    /// else the process-shared fallback.
    fn resolved_store(&self) -> Rc<PortalStore> {
        if let Some(store) = self.store.borrow().clone() {
            return store;
        }
        PortalStoreHolder::current()
    }
}

impl PrimitiveView for PortalHostPrimitive {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "portal-host".to_owned(),
        );
        node
    }

    fn update_node(&self, node: &Node) {
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "portal-host".to_owned(),
        );

        // Resolve ownership exactly once, from a real window store. The shared
        // fallback is not ownership evidence, so waiting for a scoped store
        // avoids pinning the process-wide one.
        if self.store.borrow().is_none() {
            let tree_store = node.composition_value(&PortalStoreEnvironment::key());
            if let Some(store) = tree_store {
                *self.store.borrow_mut() = Some(store);
            }
        }

        let resolved = self.resolved_store();

        // Outside clicks dismiss whatever the store considers dismissible.
        if let Some(registry) = InteractionRegistryHolder::current() {
            let store = resolved.clone();
            registry.set_pointer(
                node,
                InputHandlerRoute::overlay(),
                Rc::new(move |event: &cove_platform::MouseButtonEvent, phase, _| {
                    if phase == PointerPhase::Down {
                        store.dismiss_outside(Point {
                            x: event.x,
                            y: event.y,
                        });
                    }
                    cove_core::scene::EventResult::Ignored
                }),
            );
        }

        node.set_attachment(
            LayoutDebugAttachmentKey::DEBUG_NAME,
            format!(
                "portal-host-{:p}-entries-{}",
                Rc::as_ptr(&resolved),
                resolved.entries().len()
            ),
        );

        // (Re)bind the observer only when the store identity changes.
        let bound: Option<Rc<PortalStore>> = node.attachment(BOUND_STORE);
        let already_bound = bound
            .as_ref()
            .is_some_and(|existing| Rc::ptr_eq(existing, &resolved));
        if !already_bound {
            if let (Some(store), Some(token)) = (bound, node.attachment::<u64>(OBSERVER_TOKEN)) {
                store.remove_observer(token);
            }
            let revision = self.revision;
            let token = resolved.add_observer(Rc::new(move |_| {
                revision.update(|revision| *revision = revision.wrapping_add(1));
            }));
            node.set_attachment(BOUND_STORE, resolved);
            node.set_attachment(OBSERVER_TOKEN, token);
        }
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        // The host itself takes no space; the slots are absolutely positioned.
        None
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let _ = self.revision.get();
        let store = self.resolved_store();
        store
            .rendered_entries()
            .into_iter()
            .map(|entry| {
                AnyView::new(
                    AnyView::new(PortalEntrySlot {
                        store: store.clone(),
                        entry: entry.clone(),
                    })
                    .id(entry.id.clone()),
                )
            })
            .collect()
    }
}

impl_primitive_view!(PortalHostPrimitive);

/// One positioned overlay in the portal layer.
struct PortalEntrySlot {
    store: Rc<PortalStore>,
    entry: PortalEntry,
}

impl PortalEntrySlot {
    /// Fit the slot into the window when it does not fill it.
    fn apply_fit(&self, layout: &LayoutNode, window: Rect) {
        if self.entry.fills_window {
            layout.set_min_width(None);
            layout.set_max_width(None);
            layout.set_max_height(None);
            return;
        }
        let (minimum, maximum) = self.entry.fit.width_range(window);
        layout.set_min_width(Some(minimum));
        layout.set_max_width(Some(maximum));
        layout.set_max_height(Some(self.entry.fit.height_limit(window)));
    }
}

impl PrimitiveView for PortalEntrySlot {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_attachment(
            LayoutDebugAttachmentKey::LAYOUT_ROLE,
            "portal-entry".to_owned(),
        );
        node.set_attachment(PortalOwnership::ENTRY_KEY, self.entry.id.clone());
        node.set_attachment(LayoutDebugAttachmentKey::DEBUG_NAME, self.entry.id.clone());
        node.set_attachment(
            WheelRoutingAttachmentKey::PRIORITY,
            WheelRoutingPriority::PreferHit,
        );
        node
    }

    fn update_node(&self, node: &Node) {
        self.store.attach_slot_node(&self.entry.id, node);
        let interactive = !self.entry.is_exiting;
        node.set_allows_hit_testing(interactive);
        node.set_is_interaction_enabled(interactive);
        // The entry never clips: a fallback scroller clips only on overflow, so a
        // floating panel keeps the shadow that lifts it.
        node.set_clips_to_bounds(false);

        if let Some(layout) = node.layout_node() {
            self.apply_fit(&layout, portal_window_bounds(node));
        }

        // Re-place on every layout pass: the content's measured size feeds the
        // position, so a panel that grows must be re-fitted.
        if !self.entry.fills_window {
            let entry = self.entry.clone();
            let store = self.store.clone();
            node.set_layout_did_update(Some(Rc::new(move |node: &Node| {
                let window = portal_window_bounds(node);
                let size: Size = {
                    let frame = node.frame();
                    Size {
                        width: frame.width,
                        height: frame.height,
                    }
                };
                let position = match &entry.position_for_size {
                    Some(resolve) => resolve(size),
                    None => entry.position,
                };
                let fitted = PortalPlacement::fit(
                    position,
                    size,
                    window,
                    store.anchor(&entry.id),
                    entry.fit.viewport_margin,
                    entry.placement,
                );
                set_window_frame(node, fitted);
                if let Some(layout) = node.layout_node() {
                    let (minimum, maximum) = entry.fit.width_range(window);
                    layout.set_min_width(Some(minimum));
                    layout.set_max_width(Some(maximum));
                    layout.set_max_height(Some(entry.fit.height_limit(window)));
                }
            })));
        }
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(self.entry.position.x, Edge::Left);
        layout.set_position(self.entry.position.y, Edge::Top);
        if self.entry.fills_window {
            layout.set_position(0.0, Edge::Left);
            layout.set_position(0.0, Edge::Top);
            layout.set_position(0.0, Edge::Right);
            layout.set_position(0.0, Edge::Bottom);
        } else if let Some(width) = self.entry.width {
            layout.set_width(Some(width));
        }
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        if self.entry.fills_window {
            return vec![self.entry.content.clone()];
        }
        let window = portal_window_bounds(node);
        let (minimum, maximum) = self.entry.fit.width_range(window);
        let limit = self.entry.fit.height_limit(window);
        // A non-filling entry scrolls only when its content overflows, so short
        // panels do not gain a scrollbar or lose their shadow.
        let mut scroller = ScrollView::new(ScrollAxis::Vertical, self.entry.content.clone())
            .scrollbar_gutter(ScrollbarGutter::Overlay);
        scroller.clips_only_overflow = true;
        // `LayoutFrameExt` exposes only the min-width shortcut, so the full
        // frame call carries the max bounds.
        vec![AnyView::new(AnyView::new(scroller).frame(
            None,
            None,
            Some(minimum),
            None,
            Some(maximum),
            Some(limit),
        ))]
    }
}

impl_primitive_view!(PortalEntrySlot);
