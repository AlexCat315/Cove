//! Translation of Desktop/Sources/CoveUI/Application/Typography.swift and
//! Application/AdaptiveContent.swift.

use cove_composition::{AnyView, PrimitiveView, View, ViewKind};
use cove_core::scene::Node;
use cove_core::{Environment, State};
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
        Self {
            scale: 0.0,
            atlas: None,
            texture_id: 1,
        }
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
    pub text_scaler: Option<cove_ui_compose::theme::tokens::TextScaler>,
}

impl AdaptiveApplicationContent {
    pub fn new(content: AnyView, environment: State<Environment>) -> Self {
        Self {
            content,
            environment,
            text_scaler: None,
        }
    }

    /// Scales authored text sizes for everything below the surface root.
    pub fn text_scaler(mut self, scaler: cove_ui_compose::theme::tokens::TextScaler) -> Self {
        self.text_scaler = Some(scaler);
        self
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
        // The host root is the application's `SurfaceRoot`: publishing only the
        // environment left `SafeArea` reading zero insets under the window root,
        // because the inset composition local is installed by `SurfaceRoot`, not
        // by the bare `surface_environment` modifier. Delegating here is what
        // makes host safe-area insets reach authored content.
        let content = match &self.text_scaler {
            Some(scaler) => {
                use cove_ui_compose::theme::tokens::TextScalerViewExt;
                AnyView::new(self.content.clone().text_scaler(scaler.clone()))
            }
            None => self.content.clone(),
        };
        vec![AnyView::new(
            cove_ui_compose::primitives::layout::SurfaceRoot::new(self.environment.get(), content),
        )]
    }
}

impl View for AdaptiveApplicationContent {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    /// `children()` reads the surface environment from `State`; this
    /// dependency is what re-runs it on every window resize, keyboard, device
    /// or posture change after the first frame.
    fn dynamic_properties(&self) -> Vec<cove_composition::DynamicField> {
        vec![cove_composition::DynamicField::state(
            "environment",
            &self.environment,
        )]
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
