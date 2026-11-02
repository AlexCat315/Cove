//! Translation of Theme/Tokens/TextScaler.swift.
use super::semantic_modifiers::SemanticFontModifier;
use crate::{AnyView, PrimitiveView, View, ViewExt, ViewKind, ViewModifier};
use cove_core::scene::{CompositionLocal, LayoutNode, Node};
use cove_core::{Font, TextScale};
use std::rc::Rc;

/// Text scaling is independent of display density and the application's
/// typography ramp. Hosts supply their accessibility setting once at the root;
/// pages can override it. No rounding occurs in logical coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct TextScaler {
    authored_factor: f32,
    system_scale: TextScale,
}
impl Default for TextScaler {
    fn default() -> Self {
        Self::new(1.0)
    }
}
impl TextScaler {
    pub fn new(factor: f32) -> Self {
        Self {
            authored_factor: if factor.is_finite() && factor > 0.0 {
                factor
            } else {
                1.0
            },
            system_scale: TextScale::default(),
        }
    }
    pub fn identity() -> Self {
        Self::default()
    }
    pub fn factor(&self) -> f32 {
        self.authored_factor * self.system_scale.factor()
    }
    /// Apply authored scaling without flattening a native accessibility ramp.
    pub fn multiplied(&self, factor: f32) -> Self {
        if !factor.is_finite() || factor <= 0.0 || !(self.authored_factor * factor).is_finite() {
            return self.clone();
        }
        let mut result = self.clone();
        result.authored_factor *= factor;
        result
    }
    pub fn applying_system_scale(&self, scale: TextScale) -> Self {
        let mut result = self.clone();
        result.system_scale = scale;
        result
    }
    pub fn system_scale(&self) -> &TextScale {
        &self.system_scale
    }
    pub fn scale(&self, value: f32) -> f32 {
        self.system_scale.scale(value) * self.authored_factor
    }
    pub fn scale_font(&self, font: &Font) -> Font {
        font.with_size(self.scale(font.size()))
    }
}

pub struct TextScalerEnvironment;
impl TextScalerEnvironment {
    pub fn key() -> Rc<CompositionLocal<TextScaler>> {
        thread_local! {
            static KEY: Rc<CompositionLocal<TextScaler>> = Rc::new(CompositionLocal::constant(TextScaler::identity()));
        }
        KEY.with(|key| key.clone())
    }
}

pub const TEXT_SCALER_ATTACHMENT: &str = "__text_scaler";

#[derive(Clone)]
pub struct TextScalerModifier {
    pub scaler: TextScaler,
}
impl ViewModifier for TextScalerModifier {
    fn scope_apply(&self, node: &Node) -> Option<bool> {
        if node.composition_value(&TextScalerEnvironment::key()) == self.scaler {
            return Some(false);
        }
        node.set_composition_value(&TextScalerEnvironment::key(), self.scaler.clone());
        crate::modifiers::style::invalidate_text_style(node);
        Some(true)
    }
    fn is_scope_applying(&self) -> bool {
        true
    }
}

pub fn node_text_scaler(node: &Node) -> TextScaler {
    node.composition_value(&TextScalerEnvironment::key())
}

/// Copy composition defaults to the measured leaf. Explicit font/style
/// attachments anywhere in its ancestry retain priority over this fallback.
pub fn sync_text_style_environment(node: &Node) {
    let Some(layout) = node.layout_node() else {
        return;
    };
    let body: Option<super::TextStyleToken> =
        node.composition_value(&crate::theme::ThemeEnvironment::body_style());
    let scaler = node_text_scaler(node);
    let changed = layout.attachment::<TextScaler>(TEXT_SCALER_ATTACHMENT) != Some(scaler.clone())
        || layout.attachment::<super::TextStyleToken>(
            crate::theme::ThemeEnvironment::BODY_STYLE_ATTACHMENT,
        ) != body;
    if !changed {
        return;
    }
    layout.set_attachment(TEXT_SCALER_ATTACHMENT, scaler);
    if let Some(body) = body {
        layout.set_attachment(crate::theme::ThemeEnvironment::BODY_STYLE_ATTACHMENT, body);
    } else {
        layout.remove_attachment(crate::theme::ThemeEnvironment::BODY_STYLE_ATTACHMENT);
    }
    crate::modifiers::style::invalidate_text_measurements(&layout);
}

/// For layout with explicit row heights, resolve the same scale as text.
pub struct TextScalerReader<Content: View> {
    content: Rc<dyn Fn(TextScaler) -> Content>,
}
impl<Content: View> TextScalerReader<Content> {
    pub fn new(content: impl Fn(TextScaler) -> Content + 'static) -> Self {
        Self {
            content: Rc::new(content),
        }
    }
}
impl<Content: View + 'static> PrimitiveView for TextScalerReader<Content> {
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
        vec![AnyView::new((self.content)(node_text_scaler(node)))]
    }
}
impl<Content: View + 'static> View for TextScalerReader<Content> {
    fn kind(&self) -> ViewKind<'_> {
        ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

pub trait TextScalerViewExt: View + Sized + 'static {
    fn text_scaler(self, scaler: TextScaler) -> crate::ModifiedContent {
        self.modifier(TextScalerModifier { scaler })
    }
    fn text_style(self, token: super::TextStyleToken) -> crate::ModifiedContent {
        self.modifier(SemanticFontModifier::new(super::SemanticFontRef::new(
            move |_| token.clone(),
        )))
    }
}
impl<T: View + Sized + 'static> TextScalerViewExt for T {}
