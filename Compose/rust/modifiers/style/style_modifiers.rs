//! Translation of Modifiers/Style/StyleModifiers.swift.
use cove_core::scene::{InvalidationSource, LayoutNode, Node};
use cove_core::{Font, TextScale};
use crate::theme::tokens::{TextScaler, TextStyleToken};
use super::style_attachment_key::{FONT, LETTER_SPACING, LINE_HEIGHT};

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
