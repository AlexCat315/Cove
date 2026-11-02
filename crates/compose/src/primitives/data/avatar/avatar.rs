//! Translation of `Primitives/Data/Avatar/Avatar.swift`.
//!
//! Person image, Unicode initials or an anonymous fallback. The photograph loads
//! through `AsyncImage`; the fallback stays on screen while it is absent, loading
//! or failed. A caller may also supply an erased image or fallback view.

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::style::AppearanceExt;
use crate::primitives::media::{AsyncImage, AsyncImagePhase, Icon, ImageContentMode, ui_icons};
use crate::primitives::text::Text;
use crate::theme::theme_environment::theme_for_node;
use crate::theme::tokens::control_size::{ControlSize, ControlSizeEnvironment};
use crate::theme::tokens::icons::{IconScaling, IconScalingViewExt};
use crate::theme::tokens::text_scaler::{TextScaler, TextScalerViewExt};
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};
use cove_core::scene::{AccessibilityRole, AccessibilitySemantics, LayoutNode, Node};
use cove_core::{Font, FontWeight};

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
    /// Erased image content, which wins over `image_url`.
    pub image: Option<AnyView>,
    /// Optional caller-provided fallback. Replaces the text/icon fallback while
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
        // The avatar node already publishes image semantics.
        let visual = match (&self.image, &self.image_url) {
            (Some(image), _) => AnyView::new(image.clone().corner_radius(radius).clipped()),
            (None, Some(url)) => {
                let fallback = fallback.clone();
                AnyView::new(
                    AsyncImage::new(Some(url.clone()), inner, inner, move |phase| match phase {
                        AsyncImagePhase::Success(image) => {
                            AnyView::new(image.corner_radius(radius))
                        }
                        AsyncImagePhase::Empty
                        | AsyncImagePhase::Loading
                        | AsyncImagePhase::Failure(_) => fallback.clone(),
                    })
                    .content_mode(ImageContentMode::Fill)
                    .retry_id(appearance.retry_id as i32),
                )
            }
            (None, None) => fallback,
        };
        let mut children = vec![AnyView::new(visual.accessibility_hidden(true))];
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

/// Identity disc shown while an image is absent, loading or failed: the
/// initials, or the placeholder icon for an anonymous person.
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
        let mut semantics = AccessibilitySemantics::new(AccessibilityRole::Image);
        semantics.is_hidden = true;
        node.set_accessibility(Some(semantics));
        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(self.diameter));
            layout.set_height(Some(self.diameter));
            layout.set_flex_direction(cove_core::scene::FlexDirection::Row);
            layout.set_align_items(cove_core::scene::Align::Center);
            layout.set_justify_content(cove_core::scene::Justify::Center);
        }
    }
    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let glyph = if self.initials.is_empty() {
            match ui_icons::icon(&self.icon) {
                Some(resource) => AnyView::new(
                    Icon::with_color(resource, self.diameter * 0.6, self.colors.foreground)
                        .icon_scaling(IconScaling::Fixed),
                ),
                None => return Vec::new(),
            }
        } else {
            AnyView::new(
                Text::new(self.initials.clone())
                    .line_limit(Some(1))
                    .font(Font::system(self.diameter * 0.36, FontWeight::Semibold))
                    .foreground_color(self.colors.foreground)
                    .text_scaler(TextScaler::identity()),
            )
        };
        vec![glyph]
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
        let ll = lightness + 0.396_337_78 * a + 0.215_803_76 * b;
        let mm = lightness - 0.105_561_346 * a - 0.063_854_17 * b;
        let ss = lightness - 0.089_484_18 * a - 1.291_485_5 * b;
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
            r: channel(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
            g: channel(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
            b: channel(-0.0041960863 * l - 0.703_418_6 * m + 1.707_614_7 * s),
            a: 1.0,
        }
    }
}
