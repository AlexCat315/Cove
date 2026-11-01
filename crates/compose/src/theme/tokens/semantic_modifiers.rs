//! Translation of Theme/Tokens/SemanticModifiers.swift.
use super::{SemanticColorRef, SemanticFontRef, ThemeMetric};
use crate::theme::ThemeReader;
use crate::{AnyView, ModifiedContent, View, ViewExt, ViewModifier};
use cove_core::scene::Node;

/// Resolves a `SemanticColorRef` against the target node's active theme and
/// writes the result into `node.background_color`.
pub struct SemanticBackgroundModifier {
    pub color: SemanticColorRef,
}
impl SemanticBackgroundModifier {
    pub fn new(color: SemanticColorRef) -> Self {
        Self { color }
    }
}
impl ViewModifier for SemanticBackgroundModifier {
    fn apply_node(&self, node: &Node) {
        let target = self.color.resolve(&crate::theme::theme_for_node(node));
        let current = node.background_color();
        let node = node.clone();
        let owner = node.clone();
        node.animatable_set_optional("backgroundColor", current, Some(target), move |value| {
            owner.set_background_color(value);
        });
    }
}

pub struct SemanticForegroundColorModifier {
    pub color: SemanticColorRef,
}
impl SemanticForegroundColorModifier {
    pub fn new(color: SemanticColorRef) -> Self {
        Self { color }
    }
}
impl ViewModifier for SemanticForegroundColorModifier {
    fn apply_node(&self, node: &Node) {
        let target = self.color.resolve(&crate::theme::theme_for_node(node));
        let current = node.foreground_color();
        let node = node.clone();
        let owner = node.clone();
        node.animatable_set_optional("foregroundColor", current, Some(target), move |value| {
            owner.set_foreground_color(value);
        });
    }
}

pub struct SemanticBorderModifier {
    pub color: SemanticColorRef,
    pub width: f32,
}
impl SemanticBorderModifier {
    pub fn new(color: SemanticColorRef, width: f32) -> Self {
        Self {
            color,
            width: width.max(0.0),
        }
    }
}
impl ViewModifier for SemanticBorderModifier {
    fn apply_node(&self, node: &Node) {
        let theme = crate::theme::theme_for_node(node);
        let target = self.color.resolve(&theme);
        let current = node.border_color();
        let owner = node.clone();
        node.animatable_set_optional("borderColor", current, Some(target), move |value| {
            owner.set_border_color(value);
        });
        let current_width = node.border_width();
        let owner = node.clone();
        node.animatable_set_with("borderWidth", current_width, self.width, move |value| {
            owner.set_border_width(value);
        });
    }
}

/// Resolves a `SemanticFontRef` and writes both the font and its companion
/// line-height into the same attachment slots used by `FontModifier` /
/// `LineHeightModifier`.
pub struct SemanticFontModifier {
    pub reference: SemanticFontRef,
}
impl SemanticFontModifier {
    pub fn new(reference: SemanticFontRef) -> Self {
        Self { reference }
    }
}
impl ViewModifier for SemanticFontModifier {
    fn apply_node(&self, node: &Node) {
        let theme = crate::theme::theme_for_node(node);
        let token = self
            .reference
            .resolve(&theme)
            .resolving_family(theme.typography.family.as_deref());
        use crate::modifiers::style::{FONT, LETTER_SPACING, LINE_HEIGHT};
        let changed = node.attachment::<cove_core::Font>(FONT) != Some(token.font.clone())
            || node.attachment::<f32>(LINE_HEIGHT) != Some(token.line_height)
            || node.attachment::<f32>(LETTER_SPACING) != Some(token.letter_spacing);
        node.set_attachment(FONT, token.font.clone());
        node.set_attachment(LINE_HEIGHT, token.line_height);
        node.set_attachment(LETTER_SPACING, token.letter_spacing);
        if let Some(layout) = node.layout_node() {
            layout.set_attachment(FONT, token.font.clone());
            layout.set_attachment(LINE_HEIGHT, token.line_height);
            layout.set_attachment(LETTER_SPACING, token.letter_spacing);
            if changed {
                layout.mark_dirty();
            }
        }
        if changed {
            crate::modifiers::style::invalidate_text_style(node);
        }
    }
}

pub trait SemanticStyleViewExt: View + Sized + 'static {
    /// Set the background fill from a semantic color slot.
    fn semantic_background(self, color: SemanticColorRef) -> ModifiedContent {
        self.modifier(SemanticBackgroundModifier::new(color))
    }
    /// Set the foreground tint from a semantic color slot.
    fn semantic_foreground(self, color: SemanticColorRef) -> ModifiedContent {
        self.modifier(SemanticForegroundColorModifier::new(color))
    }
    /// Set the border stroke from a semantic color slot.
    fn semantic_border(self, color: SemanticColorRef, width: f32) -> ModifiedContent {
        self.modifier(SemanticBorderModifier::new(color, width))
    }
    /// Set both font and line-height from a semantic typography slot.
    fn semantic_font(self, reference: SemanticFontRef) -> ModifiedContent {
        self.modifier(SemanticFontModifier::new(reference))
    }

    /// Resolve a theme radius metric at the nearest theme boundary.
    fn corner_radius_metric(self, metric: ThemeMetric) -> AnyView {
        let content = AnyView::new(self);
        AnyView::new(ThemeReader::new(move |theme| {
            use crate::modifiers::style::AppearanceExt;
            content.clone().corner_radius(metric.resolve(theme))
        }))
    }
}
impl<T: View + Sized + 'static> SemanticStyleViewExt for T {}
