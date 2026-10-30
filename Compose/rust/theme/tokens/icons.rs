//! Translation of Theme/Tokens/Icons.swift.
use cove_core::scene::{CompositionLocal, LayoutNode, Node};
use cove_imaging::BundleImageResource;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use crate::{AnyView, PrimitiveView, View, ViewKind};
use super::TextScaler;
use crate::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
pub struct IconTheme {
    pub small: f32,
    pub medium: f32,
    pub large: f32,
    pub scaling: IconScaling,
    replacements: HashMap<String, BundleImageResource>,
}
impl Default for IconTheme {
    fn default() -> Self {
        Self { small: 12.0, medium: 16.0, large: 24.0, scaling: IconScaling::Text, replacements: HashMap::new() }
    }
}
impl IconTheme {
    pub fn new(configure: impl FnOnce(&mut Self)) -> Self {
        let mut value = Self::default();
        configure(&mut value);
        value
    }
    pub fn replace(&mut self, resource: &BundleImageResource, replacement: BundleImageResource) {
        self.replacements.insert(resource.to_string(), replacement);
    }
    pub fn resolve(&self, resource: &BundleImageResource) -> BundleImageResource {
        self.replacements.get(&resource.to_string()).cloned().unwrap_or_else(|| resource.clone())
    }
}

/// Icon sizes use logical points. Accessibility text scaling also enlarges
/// icons by default; fixed artwork or a custom design system can override it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconScaling {
    Text,
    Fixed,
    Factor(f32),
}

pub struct IconScalingEnvironment;
impl IconScalingEnvironment {
    pub fn key() -> Rc<CompositionLocal<Option<IconScaling>>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<Option<IconScaling>>> = Rc::new(CompositionLocal::constant(None));
        }
        KEY.with(|key| key.clone())
    }
}

/// Resolves icon and slot geometry with the same policy. Display density is
/// handled by the renderer and must never be multiplied into these values.
#[derive(Clone)]
pub struct IconMetrics {
    theme: Theme,
    pub factor: f32,
}
impl IconMetrics {
    pub fn new(node: &Node) -> Self {
        let theme = crate::theme::theme_for_node(node);
        let scaling = node.composition_value(&IconScalingEnvironment::key()).unwrap_or(theme.icons.scaling);
        let factor = match scaling {
            IconScaling::Text => crate::theme::node_text_scaler(node).factor(),
            IconScaling::Fixed => 1.0,
            IconScaling::Factor(value) => TextScaler::new(value).factor(),
        };
        Self { theme, factor }
    }
    pub fn size(&self, points: f32) -> f32 {
        let points = if points.is_finite() { points.max(1.0) } else { 16.0 };
        points * self.factor
    }
    pub fn size_role(&self, role: &SemanticIconSize) -> f32 {
        self.size(role.resolve(&self.theme))
    }
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
}

pub fn icon_metrics(node: &Node) -> IconMetrics {
    IconMetrics::new(node)
}

pub fn icon_scaling_override(node: &Node) -> Option<IconScaling> {
    node.composition_value(&IconScalingEnvironment::key())
}

pub struct IconMetricsReader<Content: View> {
    content: Rc<dyn Fn(&Node) -> Content>,
}
impl<Content: View> IconMetricsReader<Content> {
    pub fn new(content: impl Fn(&Node) -> Content + 'static) -> Self {
        Self { content: Rc::new(content) }
    }
}
impl<Content: View + 'static> PrimitiveView for IconMetricsReader<Content> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn update_node(&self, _node: &Node) {}
    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }
    fn children(&self, node: &Node) -> Vec<AnyView> {
        vec![AnyView::new((self.content)(node))]
    }
}
impl<Content: View + 'static> View for IconMetricsReader<Content> {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Custom roles can resolve application-defined `ThemeExtensions` as well.
#[derive(Clone)]
pub struct SemanticIconSize {
    resolve: Arc<dyn Fn(&Theme) -> f32>,
}
impl SemanticIconSize {
    pub fn new(resolve: impl Fn(&Theme) -> f32 + 'static) -> Self {
        Self { resolve: Arc::new(resolve) }
    }
    pub fn resolve(&self, theme: &Theme) -> f32 {
        (self.resolve)(theme)
    }
    pub fn small() -> Self { Self::new(|theme| theme.icons.small) }
    pub fn medium() -> Self { Self::new(|theme| theme.icons.medium) }
    pub fn large() -> Self { Self::new(|theme| theme.icons.large) }
}

pub trait IconScalingViewExt: View + Sized + 'static {
    /// Overrides this subtree's icon policy without changing its typography.
    fn icon_scaling(self, scaling: IconScaling) -> crate::ModifiedContent {
        use crate::ViewExt;
        self.composition_local(IconScalingEnvironment::key(), Some(scaling))
    }
}
impl<T: View + Sized + 'static> IconScalingViewExt for T {}

// `Node::iconMetrics` / `Node::iconScalingOverride` are free functions in Rust
// because inherent impls cannot be added from this crate.
pub use self::{icon_metrics as node_icon_metrics, icon_scaling_override as node_icon_scaling_override};
