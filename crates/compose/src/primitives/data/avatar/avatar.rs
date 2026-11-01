//! Translation of `Primitives/Data/Avatar/Avatar.swift`.
//!
//! Person image, Unicode initials or an anonymous fallback. The Swift version
//! delegates the fallback glyph to `Text`/`Icon` and the photograph to
//! `AsyncImage`; those primitives have no Rust counterpart yet, so the caller
//! may supply an erased fallback/content view. Identity initials, palette
//! derivation, sizing, accessibility and status composition are preserved.
use std::rc::Rc;

use crate::theme::theme_environment::theme_for_node;
use crate::theme::tokens::control_size::{ControlSize, ControlSizeEnvironment};
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{AccessibilityRole, AccessibilitySemantics, LayoutNode, Node};

use super::status::{AvatarStatus, AvatarStatusGeometry, AvatarStatusIndicator};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AvatarShape {
    Circle,
    Square,
    RoundedRectangle(f32),
}
impl AvatarShape {
    pub fn radius(self, diameter: f32) -> f32 {
        match self {
            Self::Circle => diameter / 2.0,
            Self::Square => 0.0,
            Self::RoundedRectangle(radius) => {
                if radius.is_finite() {
                    radius.max(0.0).min(diameter / 2.0)
                } else {
                    8.0
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AvatarColors {
    pub background: cove_core::scene::Color,
    pub foreground: cove_core::scene::Color,
    pub border: cove_core::scene::Color,
}
impl AvatarColors {
    pub fn new(
        background: cove_core::scene::Color,
        foreground: cove_core::scene::Color,
        border: cove_core::scene::Color,
    ) -> Self {
        Self {
            background,
            foreground,
            border,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AvatarAppearance {
    pub size: Option<ControlSize>,
    pub diameter: Option<f32>,
    pub shape: AvatarShape,
    pub border_width: f32,
    pub colors: Option<AvatarColors>,
    /// Placeholder icon resource identifier (Swift `UIIcons.User.user`).
    pub placeholder: String,
    pub retry_id: u64,
    pub status: Option<AvatarStatus>,
    pub status_label: Option<String>,
}
impl Default for AvatarAppearance {
    fn default() -> Self {
        Self {
            size: None,
            diameter: None,
            shape: AvatarShape::Circle,
            border_width: 1.0,
            colors: None,
            placeholder: "user".to_string(),
            retry_id: 0,
            status: None,
            status_label: None,
        }
    }
}
impl AvatarAppearance {
    pub fn validate(&mut self) {
        if let Some(diameter) = self.diameter {
            self.diameter = if diameter.is_finite() {
                Some(diameter.clamp(12.0, 512.0))
            } else {
                None
            };
        }
        self.border_width = if self.border_width.is_finite() {
            self.border_width.clamp(0.0, 8.0)
        } else {
            1.0
        };
    }

    pub fn resolved_diameter(&self, node: &Node) -> f32 {
        if let Some(diameter) = self.diameter.filter(|value| value.is_finite()) {
            return diameter.clamp(12.0, 512.0);
        }
        let size = self.size.unwrap_or_else(|| {
            node.composition_value_here(&ControlSizeEnvironment::key())
                .unwrap_or(ControlSize::Regular)
        });
        match size {
            ControlSize::Mini => 16.0,
            ControlSize::Small => 24.0,
            ControlSize::Regular => 48.0,
            ControlSize::Large => 80.0,
        }
    }
}

/// Person image, Unicode initials or anonymous icon.
#[derive(Clone)]
pub struct Avatar {
    pub name: String,
    pub id: Option<String>,
    pub image_url: Option<String>,
    pub appearance: AvatarAppearance,
    /// Erased image content. When `None`, the generated initials fallback is
    /// used. `AsyncImage` has no Rust counterpart yet.
    pub image: Option<AnyView>,
    /// Optional caller-provided fallback. Replaces the crates/text/icon fallback while
    /// preserving the avatar frame, clipping and accessibility.
    pub fallback: Option<AnyView>,
}
impl Avatar {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into().trim().to_string();
        Self {
            name,
            id: None,
            image_url: None,
            appearance: AvatarAppearance::default(),
            image: None,
            fallback: None,
        }
    }
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn image_url(mut self, url: impl Into<String>) -> Self {
        self.image_url = Some(url.into());
        self
    }
    pub fn configure(mut self, configure: impl FnOnce(&mut AvatarAppearance)) -> Self {
        configure(&mut self.appearance);
        self.appearance.validate();
        self
    }
    pub fn with_image(mut self, image: AnyView) -> Self {
        self.image = Some(image);
        self
    }
    pub fn with_fallback(mut self, fallback: AnyView) -> Self {
        self.fallback = Some(fallback);
        self
    }

    pub fn initials(&self) -> String {
        AvatarIdentity::initials(&self.name)
    }
    /// Resolved presence name; empty without a status.
    pub fn status_text(&self) -> String {
        self.appearance
            .status
            .map(|status| {
                self.appearance
                    .status_label
                    .clone()
                    .unwrap_or_else(|| status.label().to_string())
            })
            .unwrap_or_default()
    }

    pub fn resolved_colors(&self, node: &Node) -> AvatarColors {
        if let Some(colors) = self.appearance.colors {
            return colors;
        }
        let initials = self.initials();
        if initials.is_empty() {
            let theme = theme_for_node(node);
            return AvatarColors::new(
                theme.colors.surface_variant,
                theme.colors.on_surface_muted,
                theme.colors.border,
            );
        }
        let theme = theme_for_node(node);
        AvatarIdentity::colors_for_initials(
            &initials,
            AvatarIdentity::is_dark(theme.colors.background),
        )
    }
}

impl PrimitiveView for Avatar {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let mut appearance = self.appearance.clone();
        appearance.validate();
        let diameter = appearance.resolved_diameter(node);
        let colors = self.resolved_colors(node);
        let theme = theme_for_node(node);
        node.set_background_color(Some(if let Some(colors) = appearance.colors {
            colors.border
        } else if self.image_url.is_none() {
            colors.border
        } else {
            theme.colors.border
        }));
        node.set_corner_radius(appearance.shape.radius(diameter));

        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Image);
        semantics.label = if self.name.is_empty() {
            "Anonymous user".to_string()
        } else {
            self.name.clone()
        };
        semantics.value = self.status_text();
        semantics.combines_children = true;
        node.set_accessibility(Some(semantics));
        node.set_attachment("avatar.initials", self.initials());
        node.set_attachment("avatar.border_width", appearance.border_width);
        node.set_attachment("avatar.image_url", self.image_url.clone());

        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(diameter));
            layout.set_height(Some(diameter));
            layout.set_align_items(cove_core::scene::Align::Center);
            layout.set_justify_content(cove_core::scene::Justify::Center);
        }
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let mut appearance = self.appearance.clone();
        appearance.validate();
        let diameter = appearance.resolved_diameter(node);
        let border = appearance.border_width.min(diameter / 4.0);
        let inner = (diameter - border * 2.0).max(0.0);
        let radius = (appearance.shape.radius(diameter) - border).max(0.0);
        let colors = self.resolved_colors(node);

        let fallback: AnyView = match &self.fallback {
            Some(view) => view.clone(),
            None => AnyView::new(AvatarFallback {
                initials: self.initials(),
                diameter: inner,
                radius,
                colors,
                icon: appearance.placeholder.clone(),
            }),
        };
        // Swift wraps the visual in `.accessibilityHidden()` because the avatar
        // node already publishes image semantics.
        let mut children = vec![fallback];
        if let Some(status) = appearance.status {
            children.push(AnyView::new(AvatarStatusIndicator::new(
                status,
                AvatarStatusGeometry::new(diameter, appearance.shape),
            )));
        }
        children
    }
}
impl_primitive_view!(Avatar);

/// Solid identity disc used while an image is absent, loading or failed.
pub struct AvatarFallback {
    pub initials: String,
    pub diameter: f32,
    pub radius: f32,
    pub colors: AvatarColors,
    pub icon: String,
}
impl PrimitiveView for AvatarFallback {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }
    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }
    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(self.colors.background));
        node.set_corner_radius(self.radius);
        node.set_clips_to_bounds(true);
        // `Text`/`Icon` are unavailable: expose the initials to a future text
        // host and keep the disc's contrast semantics local.
        node.set_attachment("avatar.fallback.initials", self.initials.clone());
        node.set_attachment("avatar.fallback.icon", self.icon.clone());
        node.set_attachment("avatar.fallback.foreground", self.colors.foreground);
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Image);
        semantics.is_hidden = true;
        node.set_accessibility(Some(semantics));
        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(self.diameter));
            layout.set_height(Some(self.diameter));
        }
    }
}
impl View for AvatarFallback {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        "AvatarFallback"
    }
}

pub struct AvatarIdentity;
impl AvatarIdentity {
    pub fn is_dark(color: cove_core::scene::Color) -> bool {
        0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b < 0.5
    }

    /// First letter of each of the first two words, otherwise the first two
    /// characters. Uppercased and capped at two characters.
    pub fn initials(name: &str) -> String {
        let words: Vec<&str> = name.split_whitespace().collect();
        let raw = if words.len() > 1 {
            words
                .iter()
                .take(2)
                .filter_map(|word| word.chars().next())
                .collect::<String>()
        } else {
            words
                .first()
                .map(|word| word.chars().take(2).collect::<String>())
                .unwrap_or_default()
        };
        raw.to_uppercase().chars().take(2).collect()
    }

    /// FNV-1a-style 64-bit mix over canonical UTF-8, modulo 12 hues.
    pub fn hue_index(initials: &str) -> usize {
        let mut hash: u64 = 14_695_981_039_346_656_037;
        for byte in initials.as_bytes() {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(1_099_511_628_211);
        }
        (hash % 12) as usize
    }

    pub fn colors_for_initials(initials: &str, is_dark: bool) -> AvatarColors {
        Self::colors(Self::hue_index(initials) as f32 * 30.0, is_dark)
    }

    pub fn colors(hue: f32, is_dark: bool) -> AvatarColors {
        if is_dark {
            AvatarColors::new(
                Self::oklch(0.30, 0.05, hue),
                Self::oklch(0.82, 0.11, hue),
                Self::oklch(0.36, 0.06, hue),
            )
        } else {
            AvatarColors::new(
                Self::oklch(0.97, 0.032, hue),
                Self::oklch(0.50, 0.145, hue),
                Self::oklch(0.89, 0.05, hue),
            )
        }
    }

    pub fn oklch(lightness: f32, chroma: f32, hue: f32) -> cove_core::scene::Color {
        let radians = hue * std::f32::consts::PI / 180.0;
        let a = chroma * radians.cos();
        let b = chroma * radians.sin();
        let ll = lightness + 0.3963377774 * a + 0.2158037573 * b;
        let mm = lightness - 0.1055613458 * a - 0.0638541728 * b;
        let ss = lightness - 0.0894841775 * a - 1.2914855480 * b;
        let l = ll * ll * ll;
        let m = mm * mm * mm;
        let s = ss * ss * ss;
        let channel = |value: f32| {
            let value = value.clamp(0.0, 1.0);
            if value <= 0.0031308 {
                12.92 * value
            } else {
                1.055 * value.powf(1.0 / 2.4) - 0.055
            }
        };
        cove_core::scene::Color {
            r: channel(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
            g: channel(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
            b: channel(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
            a: 1.0,
        }
    }
}
