//! Translation of Modifiers/Style/StyleModifiers.swift.
use cove_core::scene::{InvalidationSource, LayoutNode, Node};
use cove_core::{Color, Font, LinearGradient, SurfaceShadow, TextScale};
use crate::theme::tokens::{TextScaler, TextStyleToken};
use crate::{ModifiedContent, View, ViewExt, ViewModifier};
use super::style_attachment_key::{FONT, LETTER_SPACING, LINE_HEIGHT};

fn invalidate_text_measurements(layout: &LayoutNode) {
    if layout.has_measure_func() { layout.mark_dirty(); }
    for child in layout.children() { invalidate_text_measurements(&child); }
}

pub struct BackgroundModifier(pub Color);
impl ViewModifier for BackgroundModifier {
    fn apply_node(&self, node: &Node) {
        let owner = node.clone();
        node.animatable_set_optional("backgroundColor", node.background_color(), Some(self.0), move |value| {
            owner.set_background_color(value);
        });
    }
}

pub struct BackgroundGradientModifier(pub LinearGradient);
impl ViewModifier for BackgroundGradientModifier {
    fn apply_node(&self, node: &Node) {
        node.set_background_gradient(Some(self.0));
    }
}

pub struct ForegroundColorModifier(pub Color);
impl ViewModifier for ForegroundColorModifier {
    fn apply_node(&self, node: &Node) {
        let owner = node.clone();
        node.animatable_set_optional("foregroundColor", node.foreground_color(), Some(self.0), move |value| {
            owner.set_foreground_color(value);
        });
    }
}

pub struct OpacityModifier(pub f32);
impl OpacityModifier {
    pub fn new(value: f32) -> Self { Self(if value.is_finite() { value.clamp(0.0, 1.0) } else { 1.0 }) }
}
impl ViewModifier for OpacityModifier {
    fn apply_node(&self, node: &Node) {
        let owner = node.clone();
        node.animatable_set_with("opacity", node.opacity(), self.0, move |value| owner.set_opacity(value));
    }
}

pub struct ZIndexModifier(pub f32);
impl ViewModifier for ZIndexModifier {
    fn apply_node(&self, node: &Node) { node.set_z_index(self.0); }
}

pub struct ClipModifier;
impl ViewModifier for ClipModifier {
    fn apply_node(&self, node: &Node) { node.set_clips_to_bounds(true); }
}

pub struct CornerRadiusModifier(pub f32);
impl ViewModifier for CornerRadiusModifier {
    fn apply_node(&self, node: &Node) {
        let owner = node.clone();
        node.animatable_set_with("cornerRadius", node.corner_radius(), self.0.max(0.0), move |value| owner.set_corner_radius(value));
    }
}

pub struct BorderModifier { pub color: Color, pub width: f32 }
impl BorderModifier {
    pub fn new(color: Color, width: f32) -> Self { Self { color, width: width.max(0.0) } }
}
impl ViewModifier for BorderModifier {
    fn apply_node(&self, node: &Node) {
        let owner = node.clone();
        node.animatable_set_optional("borderColor", node.border_color(), Some(self.color), move |value| owner.set_border_color(value));
        let owner = node.clone();
        node.animatable_set_with("borderWidth", node.border_width(), self.width, move |value| owner.set_border_width(value));
    }
}

pub struct ShadowModifier { pub color: Color, pub offset_x: f32, pub offset_y: f32, pub blur: f32 }
impl ViewModifier for ShadowModifier {
    fn apply_node(&self, node: &Node) {
        node.set_shadow(SurfaceShadow {
            color: Some(self.color), offset_x: self.offset_x, offset_y: self.offset_y, blur: self.blur,
        });
    }
}

pub struct FontModifier(pub Font);
impl ViewModifier for FontModifier {
    fn apply_node(&self, node: &Node) {
        let font = self.0.resolving_family(crate::theme::theme_for_node(node).typography.family.as_deref());
        let changed = node.attachment::<Font>(FONT).as_ref() != Some(&font);
        node.set_attachment(FONT, font.clone());
        if let Some(layout) = node.layout_node() {
            layout.set_attachment(FONT, font);
            if changed { invalidate_text_measurements(&layout); }
        }
        if changed { invalidate_text_style(node); }
    }
}

pub struct LineHeightModifier(pub f32);
impl ViewModifier for LineHeightModifier {
    fn apply_node(&self, node: &Node) {
        let value = self.0.max(0.0);
        if node.attachment::<f32>(LINE_HEIGHT) == Some(value) { return; }
        node.set_attachment(LINE_HEIGHT, value);
        invalidate_text_style(node);
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        let value = self.0.max(0.0);
        if layout.attachment::<f32>(LINE_HEIGHT) == Some(value) { return; }
        layout.set_attachment(LINE_HEIGHT, value);
        invalidate_text_measurements(layout);
    }
}

pub struct LetterSpacingModifier(pub f32);
impl ViewModifier for LetterSpacingModifier {
    fn apply_node(&self, node: &Node) {
        let value = if self.0.is_finite() { self.0 } else { 0.0 };
        if node.attachment::<f32>(LETTER_SPACING) == Some(value) { return; }
        node.set_attachment(LETTER_SPACING, value);
        invalidate_text_style(node);
    }
    fn apply_layout(&self, layout: &LayoutNode) {
        let value = if self.0.is_finite() { self.0 } else { 0.0 };
        if layout.attachment::<f32>(LETTER_SPACING) == Some(value) { return; }
        layout.set_attachment(LETTER_SPACING, value);
        invalidate_text_measurements(layout);
    }
}

pub trait AppearanceExt: View + Sized + 'static {
    fn background_color(self, color: Color) -> ModifiedContent { self.modifier(BackgroundModifier(color)) }
    fn background_gradient(self, gradient: LinearGradient) -> ModifiedContent { self.modifier(BackgroundGradientModifier(gradient)) }
    fn foreground_color(self, color: Color) -> ModifiedContent { self.modifier(ForegroundColorModifier(color)) }
    fn opacity(self, value: f32) -> ModifiedContent { self.modifier(OpacityModifier::new(value)) }
    fn z_index(self, value: f32) -> ModifiedContent { self.modifier(ZIndexModifier(value)) }
    fn clipped(self) -> ModifiedContent { self.modifier(ClipModifier) }
    fn corner_radius(self, radius: f32) -> ModifiedContent { self.modifier(CornerRadiusModifier(radius)) }
    fn border_color(self, color: Color, width: f32) -> ModifiedContent { self.modifier(BorderModifier::new(color, width)) }
    fn shadow(self, color: Color, offset_x: f32, offset_y: f32, blur: f32) -> ModifiedContent {
        self.modifier(ShadowModifier { color, offset_x, offset_y, blur })
    }
    fn font(self, font: Font) -> ModifiedContent { self.modifier(FontModifier(font)) }
    fn line_height(self, value: f32) -> ModifiedContent { self.modifier(LineHeightModifier(value)) }
    fn letter_spacing(self, value: f32) -> ModifiedContent { self.modifier(LetterSpacingModifier(value)) }
}
impl<T: View + Sized + 'static> AppearanceExt for T {}

fn raw_font(node: &Node) -> Option<Font> {
    let mut cursor = Some(node.clone());
    while let Some(current) = cursor {
        if let Some(font) = current.attachment::<Font>(FONT) {
            return Some(font);
        }
        cursor = current.parent();
    }
    None
}
fn raw_line_height(node: &Node) -> Option<f32> {
    let mut cursor = Some(node.clone());
    while let Some(current) = cursor {
        if let Some(value) = current.attachment::<f32>(LINE_HEIGHT) {
            return Some(value);
        }
        cursor = current.parent();
    }
    None
}
fn raw_letter_spacing(node: &Node) -> Option<f32> {
    let mut cursor = Some(node.clone());
    while let Some(current) = cursor {
        if let Some(value) = current.attachment::<f32>(LETTER_SPACING) {
            return Some(value);
        }
        cursor = current.parent();
    }
    None
}

/// Font resolved through explicit attachments, the ambient body style, then
/// the ambient text scaler.
pub fn text_style_font(node: &Node) -> Option<Font> {
    let scaler = crate::theme::node_text_scaler(node);
    if let Some(font) = raw_font(node) {
        return Some(scaler.scale_font(&font));
    }
    let body = node.composition_value(&crate::theme::ThemeEnvironment::body_style());
    body.map(|token: TextStyleToken| scaler.scale_font(&token.font))
}
pub fn text_style_line_height(node: &Node) -> Option<f32> {
    let scaler = crate::theme::node_text_scaler(node);
    let font = raw_font(node);
    let value = raw_line_height(node).or_else(|| {
        node.composition_value(&crate::theme::ThemeEnvironment::body_style())
            .map(|token: TextStyleToken| token.line_height_for(font.as_ref()))
    });
    value.map(|points| scaler.scale(points))
}
pub fn text_style_letter_spacing(node: &Node) -> Option<f32> {
    let scaler = crate::theme::node_text_scaler(node);
    let value = raw_letter_spacing(node).or_else(|| {
        node.composition_value(&crate::theme::ThemeEnvironment::body_style()).map(|token: TextStyleToken| token.letter_spacing)
    });
    value.map(|points| scaler.scale(points))
}

pub fn invalidate_text_style(node: &Node) {
    node.mark_render_dirty(InvalidationSource::style_set("textStyle"));
    if let Some(layout) = node.layout_node()
        && layout.has_measure_func()
    {
        layout.mark_dirty();
    }
    for child in node.children() {
        invalidate_text_style(&child);
    }
}

fn raw_font_on_layout(layout: &LayoutNode) -> Option<Font> {
    let mut cursor = Some(layout.clone());
    while let Some(current) = cursor {
        if let Some(font) = current.attachment::<Font>(FONT) {
            return Some(font);
        }
        cursor = current.parent();
    }
    None
}
fn raw_scaler(layout: &LayoutNode) -> TextScaler {
    layout.attachment::<TextScaler>(crate::modifiers::style::TEXT_STYLE_SCALER).unwrap_or_default()
}
pub fn layout_text_style_font(layout: &LayoutNode) -> Option<Font> {
    raw_font_on_layout(layout).map(|font| raw_scaler(layout).scale_font(&font))
}
pub fn layout_text_style_line_height(layout: &LayoutNode) -> Option<f32> {
    let mut cursor = Some(layout.clone());
    while let Some(current) = cursor {
        if let Some(value) = current.attachment::<f32>(LINE_HEIGHT) {
            return Some(raw_scaler(current).scale(value));
        }
        cursor = current.parent();
    }
    None
}

pub fn apply_system_text_scale(layout: &LayoutNode, scale: TextScale) {
    let mut changed = false;
    if let Some(token) = layout.attachment::<TextStyleToken>(crate::theme::ThemeEnvironment::BODY_STYLE_ATTACHMENT) {
        let resolved = TextScaler::identity().applying_system_scale(scale.clone());
        layout.set_attachment(crate::modifiers::style::TEXT_STYLE_SCALER, resolved.clone());
        layout.set_attachment(FONT, resolved.scale_font(&token.font));
        layout.set_attachment(LINE_HEIGHT, resolved.scale(token.line_height));
        layout.set_attachment(LETTER_SPACING, resolved.scale(token.letter_spacing));
        changed = true;
    }
    if changed {
        layout.mark_dirty();
    }
}
