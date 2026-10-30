//! Translation of Desktop/Sources/CoveUI/Application/Typography.swift and
//! Application/AdaptiveContent.swift.

use cove_core::scene::Node;
use cove_core::{Environment, State};
use cove_composition::adaptive::SurfaceEnvironmentExt;
use cove_composition::{AnyView, PrimitiveView, View, ViewKind};
use cove_text::FontAtlas;
use std::rc::Rc;

/// Per-application text raster state: the scale the atlas was built for and the
/// atlas texture identifier the draw list reserves.
pub struct ApplicationTypography {
    pub scale: f32,
    pub atlas: Option<FontAtlas>,
    pub texture_id: cove_core::TextureId,
}

impl Default for ApplicationTypography {
    fn default() -> Self {
        Self { scale: 0.0, atlas: None, texture_id: 1 }
    }
}

/// Root wrapper that publishes the window's [`Environment`] to adaptive
/// content.
///
/// `SurfaceRoot` in the Compose vocabulary is the same idea; the host needs it
/// here because the environment lives in host state that changes as the window
/// resizes or the safe area moves.
pub struct AdaptiveApplicationContent {
    pub content: AnyView,
    pub environment: State<Environment>,
}

impl AdaptiveApplicationContent {
    pub fn new(content: AnyView, environment: State<Environment>) -> Self {
        Self { content, environment }
    }
}

impl PrimitiveView for AdaptiveApplicationContent {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, _node: &Node) {}

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let environment = self.environment.get();
        // `surface_environment` is a `View` extension, so the modifier is applied
        // to the erased view and the result re-erased once more.
        vec![AnyView::new(self.content.clone().surface_environment(environment))]
    }
}

impl View for AdaptiveApplicationContent {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Thread-local access to the shared adaptive content factory, so a host that
/// has no `AnyView` handy can still build the root.
pub fn adaptive_root(
    content: Rc<dyn Fn() -> AnyView>,
    environment: State<Environment>,
) -> AdaptiveApplicationContent {
    AdaptiveApplicationContent::new(content(), environment)
}
