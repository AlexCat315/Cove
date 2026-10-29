//! Translation of Primitives/Controls/KeyCap.swift.
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};
use crate::{AnyView, BoxView, TupleView, View};
use crate::modifiers::layout::PaddingExt;
use crate::modifiers::style::{AccessibilityExt, AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::fields::select::{KeyboardShortcut, KeyboardShortcutPlatform};
use crate::primitives::text::Text;
use crate::theme::{RadiusToken, SemanticColorRef, SemanticFontRef};
use cove_core::scene::AccessibilityRole;

/// Keyboard hint drawn as one cap per key.
///
/// Splitting a shortcut into caps — instead of printing `⌘⇧K` as a run of
/// text — is what lets the glyphs stay legible at caption size and keeps the
/// hint recognisable when the locale changes modifier names.
pub struct KeyCap {
    pub keys: Vec<String>,
    pub platform: KeyboardShortcutPlatform,
    pub is_enabled: bool,
}

impl KeyCap {
    pub fn new(keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { keys: keys.into_iter().map(Into::into).collect(), platform: KeyboardShortcutPlatform::current(), is_enabled: true }
    }

    pub fn title(title: impl Into<String>) -> Self {
        Self::new([title])
    }

    pub fn shortcut(shortcut: KeyboardShortcut, platform: KeyboardShortcutPlatform) -> Self {
        let keys = shortcut.caps(platform);
        Self { keys, platform, is_enabled: true }
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }

    fn cap(&self, key: &str) -> AnyView {
        let ink = if self.is_enabled { SemanticColorRef::on_surface_variant() } else { SemanticColorRef::on_surface_disabled() };
        AnyView::new(
            Text::new(key)
                .semantic_font(SemanticFontRef::caption())
                .semantic_foreground(ink)
                .padding_edges(6.0, 2.0)
                .semantic_background(SemanticColorRef::surface_variant())
                .corner_radius_token(RadiusToken::Row)
                .semantic_border(SemanticColorRef::border(), 1.0)
                .opacity(if self.is_enabled { 1.0 } else { 0.55 })
                .flex(0.0)
                .shrink(1.0)
                .frame_min_width(0.0),
        )
    }

    /// What a screen reader should announce: the spoken modifier names, never
    /// the bare glyphs.
    pub fn spoken_form(&self) -> String {
        if self.platform != KeyboardShortcutPlatform::MacOS {
            return self.keys.join(" + ");
        }
        self.keys
            .iter()
            .map(|key| match key.as_str() {
                "⌘" => "Command".to_string(),
                "⌃" => "Control".to_string(),
                "⌥" => "Option".to_string(),
                "⇧" => "Shift".to_string(),
                _ => key.clone(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl View for KeyCap {
    fn body(&self) -> AnyView {
        let caps: Vec<AnyView> = self.keys.iter().map(|key| self.cap(key)).collect();
        AnyView::new(
            BoxView::new(FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::Wrap, 2.0, AnyView::new(TupleView::new(caps)))
                .accessibility(cove_core::scene::AccessibilitySemantics {
                    role: AccessibilityRole::StaticText,
                    label: self.spoken_form(),
                    combines_children: true,
                    ..Default::default()
                }),
        )
    }
}
