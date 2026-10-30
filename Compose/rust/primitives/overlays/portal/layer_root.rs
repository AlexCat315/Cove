//! Translation of `Primitives/Overlays/Portal/LayerRoot.swift`.
//!
//! The root of a window's layer stack: ordinary content plus a portal layer that
//! always wins hit-test priority over any sibling content (title bars, panels).
//!
//! The portal layer's z-index is 30,000, above the 20,000 ceiling a title bar
//! may use, so an overlay can never be trapped behind window chrome regardless
//! of authoring order.

use cove_composition::{impl_primitive_view, AnyView, LayoutDebugAttachmentKey, PrimitiveView, View, ViewExt};
use crate::modifiers::style::AppearanceExt;
use cove_core::scene::{Align, Display, Edge, FlexDirection, LayoutNode, Node, PositionType};

use super::portal_host::PortalHost;

/// The z-index the portal layer occupies. Above the title-bar maximum of 20,000.
pub const PORTAL_LAYER_Z_INDEX: f32 = 30_000.0;

/// Window content with a portal layer stacked above it.
pub struct LayerRoot {
    content: AnyView,
    portals: AnyView,
}

impl LayerRoot {
    /// A root whose portal layer hosts a fresh [`PortalHost`].
    pub fn new(content: impl View + 'static) -> Self {
        Self::with_portals(content, PortalHost::new())
    }

    pub fn with_portals(content: impl View + 'static, portals: impl View + 'static) -> Self {
        Self { content: AnyView::new(content), portals: AnyView::new(portals) }
    }
}

impl View for LayerRoot {
    fn body(&self) -> AnyView {
        let content = AnyView::new(LayerRootContent { content: self.content.clone() });
        let portals = AnyView::new(
            AnyView::new(PortalLayer { content: self.portals.clone() })
                .z_index(PORTAL_LAYER_Z_INDEX),
        );
        AnyView::new(cove_composition::TupleView::new(vec![content, portals]))
    }

    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// The ordinary-content half of the root: a stretching column.
struct LayerRootContent {
    content: AnyView,
}

impl PrimitiveView for LayerRootContent {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_attachment(LayoutDebugAttachmentKey::LAYOUT_ROLE, "layer-root".to_owned());
        node
    }

    fn update_node(&self, _node: &Node) {}

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl_primitive_view!(LayerRootContent);

/// The portal half: absolutely positioned over the whole root.
struct PortalLayer {
    content: AnyView,
}

impl PrimitiveView for PortalLayer {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_attachment(LayoutDebugAttachmentKey::LAYOUT_ROLE, "portal-layer".to_owned());
        node
    }

    fn update_node(&self, _node: &Node) {}

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        // Pinned to all four edges so an overlay can position anywhere in the
        // window without affecting its siblings' layout.
        layout.set_position_type(PositionType::Absolute);
        layout.set_position(0.0, Edge::Left);
        layout.set_position(0.0, Edge::Top);
        layout.set_position(0.0, Edge::Right);
        layout.set_position(0.0, Edge::Bottom);
        layout.set_display(Display::Flex);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl_primitive_view!(PortalLayer);
