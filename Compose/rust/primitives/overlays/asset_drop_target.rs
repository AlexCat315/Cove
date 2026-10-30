//! Translation of Primitives/Overlays/AssetDropTarget.swift.
//!
//! A drop zone that registers itself with a process-wide registry and paints
//! accept/reject chrome while an asset is dragged.
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use cove_core::scene::{
    Align, FlexDirection, LayoutNode, Node, NodeWeak,
};
use cove_core::{Binding, Point, Rect};
use cove_composition::{AnyView, PrimitiveView, View};

use crate::theme::theme_for_node;

/// Data carried by an in-flight asset drag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetDropPayload {
    pub id: String,
    pub name: String,
    pub subtitle: Option<String>,
    pub kind: String,
    pub preview_path: Option<String>,
}

impl AssetDropPayload {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        kind: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            subtitle: None,
            kind: kind.into(),
            preview_path: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn preview_path(mut self, path: impl Into<String>) -> Self {
        self.preview_path = Some(path.into());
        self
    }
}

/// One registered drop zone.
struct Zone {
    node: NodeWeak,
    accepted_kinds: HashSet<String>,
    is_enabled: bool,
    on_drop: Rc<dyn Fn(AssetDropPayload)>,
}

/// Holds every live drop zone; the ambient instance is set by the host.
#[derive(Default)]
pub struct AssetDropRegistry {
    zones: RefCell<Vec<Zone>>,
}

impl AssetDropRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &self,
        node: &Node,
        accepted_kinds: HashSet<String>,
        is_enabled: bool,
        on_drop: Rc<dyn Fn(AssetDropPayload)>,
    ) {
        let weak = node.downgrade();
        let mut zones = self.zones.borrow_mut();
        if let Some(existing) = zones
            .iter_mut()
            .find(|zone| Node::upgrade(&zone.node).is_some_and(|candidate| candidate == *node))
        {
            existing.accepted_kinds = accepted_kinds;
            existing.is_enabled = is_enabled;
            existing.on_drop = on_drop;
            return;
        }
        zones.push(Zone { node: weak, accepted_kinds, is_enabled, on_drop });
    }

    pub fn unregister(&self, node: &Node) {
        self.zones
            .borrow_mut()
            .retain(|zone| Node::upgrade(&zone.node).is_none_or(|candidate| candidate != *node));
    }

    /// Delivers the payload to the topmost zone under the point; returns whether
    /// any zone accepted it.
    pub fn drop(&self, payload: &AssetDropPayload, point: Point) -> bool {
        self.prune();
        let zones = self.zones.borrow();
        for zone in zones.iter() {
            let Some(node) = Node::upgrade(&zone.node) else { continue };
            if zone.is_enabled
                && Self::accepts(payload.kind.clone(), zone)
                && node.absolute_frame().contains(point)
            {
                (zone.on_drop)(payload.clone());
                return true;
            }
        }
        false
    }

    pub fn can_drop(&self, payload: &AssetDropPayload, node: &Node) -> bool {
        let zones = self.zones.borrow();
        zones
            .iter()
            .find(|zone| Node::upgrade(&zone.node).is_some_and(|candidate| candidate == *node))
            .is_some_and(|zone| zone.is_enabled && Self::accepts(payload.kind.clone(), zone))
    }

    fn accepts(kind: String, zone: &Zone) -> bool {
        zone.accepted_kinds.is_empty() || zone.accepted_kinds.contains(&kind)
    }

    fn prune(&self) {
        self.zones.borrow_mut().retain(|zone| Node::upgrade(&zone.node).is_some());
    }
}

thread_local! {
    static CURRENT_REGISTRY: RefCell<Option<Rc<AssetDropRegistry>>> = const { RefCell::new(None) };
}

/// Ambient access to the host-provided drop registry.
pub struct AssetDropRegistryHolder;

impl AssetDropRegistryHolder {
    pub fn current() -> Option<Rc<AssetDropRegistry>> {
        CURRENT_REGISTRY.with(|slot| slot.borrow().clone())
    }

    pub fn set_current(registry: Option<Rc<AssetDropRegistry>>) {
        CURRENT_REGISTRY.with(|slot| *slot.borrow_mut() = registry);
    }
}

/// A drop zone that paints accept/reject chrome while an asset is dragged.
pub struct AssetDropTarget {
    active_payload: Binding<Option<AssetDropPayload>>,
    accepted_kinds: HashSet<String>,
    is_enabled: bool,
    on_drop: Rc<dyn Fn(AssetDropPayload)>,
    content: AnyView,
}

impl AssetDropTarget {
    pub fn new(
        active_payload: Binding<Option<AssetDropPayload>>,
        accepted_kinds: HashSet<String>,
        is_enabled: bool,
        on_drop: Rc<dyn Fn(AssetDropPayload)>,
        content: AnyView,
    ) -> Self {
        Self { active_payload, accepted_kinds, is_enabled, on_drop, content }
    }
}

impl View for AssetDropTarget {
    fn body(&self) -> AnyView {
        AnyView::new(AssetDropTargetHost {
            active_payload: self.active_payload.clone(),
            accepted_kinds: self.accepted_kinds.clone(),
            is_enabled: self.is_enabled,
            on_drop: self.on_drop.clone(),
            content: self.content.clone(),
        })
    }
}

/// Identity compared before repainting the drag chrome.
#[derive(Clone, PartialEq)]
struct PaintIdentity {
    payload: Option<AssetDropPayload>,
    accepted_kinds: HashSet<String>,
    is_enabled: bool,
}

struct AssetDropTargetHost {
    active_payload: Binding<Option<AssetDropPayload>>,
    accepted_kinds: HashSet<String>,
    is_enabled: bool,
    on_drop: Rc<dyn Fn(AssetDropPayload)>,
    content: AnyView,
}

impl PrimitiveView for AssetDropTargetHost {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        if let Some(registry) = AssetDropRegistryHolder::current() {
            registry.register(
                node,
                self.accepted_kinds.clone(),
                self.is_enabled,
                self.on_drop.clone(),
            );
        }
        let identity = PaintIdentity {
            payload: self.active_payload.get(),
            accepted_kinds: self.accepted_kinds.clone(),
            is_enabled: self.is_enabled,
        };
        let accepted_kinds = self.accepted_kinds.clone();
        let is_enabled = self.is_enabled;
        let payload_binding = self.active_payload.clone();
        let draw_node = node.clone();
        node.update_overlay_draw(identity, move |list, origin| {
            let Some(payload) = payload_binding.get() else { return };
            if !is_enabled {
                return;
            }
            let accepts = accepted_kinds.is_empty() || accepted_kinds.contains(&payload.kind);
            let theme = theme_for_node(&draw_node);
            let frame = draw_node.frame();
            let rect = Rect::new(origin.x, origin.y, frame.width, frame.height);
            let fill = if accepts {
                theme.colors.accent.multiplied_alpha(0.10)
            } else {
                theme.colors.error.multiplied_alpha(0.08)
            };
            let stroke = if accepts {
                theme.colors.accent.multiplied_alpha(0.85)
            } else {
                theme.colors.error.multiplied_alpha(0.70)
            };
            let _ = list.add_rounded_rect(rect, theme.inputs.radius, fill);
            add_border(rect, stroke, list);
        });
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(AssetDropTargetHost);

/// Paints a one-point border as four edge rectangles.
fn add_border(rect: Rect, color: cove_core::Color, list: &mut cove_core::DrawList) {
    let thickness = 1.0;
    let _ = list.add_rect(
        Rect::new(rect.x, rect.y, rect.width, thickness),
        color,
    );
    let _ = list.add_rect(
        Rect::new(rect.x, rect.y + rect.height - thickness, rect.width, thickness),
        color,
    );
    let _ = list.add_rect(
        Rect::new(rect.x, rect.y, thickness, rect.height),
        color,
    );
    let _ = list.add_rect(
        Rect::new(rect.x + rect.width - thickness, rect.y, thickness, rect.height),
        color,
    );
}
