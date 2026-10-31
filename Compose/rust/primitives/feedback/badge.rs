//! Translation of Primitives/Feedback/Badge.swift.
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{ButtonIcon, ButtonIconSource};
use crate::primitives::feedback::StatusTone;
use crate::primitives::text::Text;
use crate::theme::{
    ControlSize, ControlSizeEnvironment, SemanticColorRef, SemanticFontRef, theme_for_node,
};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};
use cove_core::Color;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align, FlexDirection, Justify,
    LayoutNode, Node, Wrap,
};
use cove_imaging::BundleImageResource;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BadgeVariant {
    #[default]
    Soft,
    Solid,
    Outline,
}

/// Compact, non-interactive status label.
pub struct Badge {
    pub title: String,
    pub tone: StatusTone,
    pub variant: BadgeVariant,
    /// Leading glyph override. `None` keeps the chip text-only.
    pub icon: Option<BundleImageResource>,
    /// Leading status dot. Ignored when `icon` is set — a dot and a glyph
    /// compete for the same slot.
    pub shows_dot: bool,
    pub is_enabled: bool,
}

impl Badge {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            tone: StatusTone::Neutral,
            variant: BadgeVariant::Soft,
            icon: None,
            shows_dot: false,
            is_enabled: true,
        }
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shows_dot(mut self, shows_dot: bool) -> Self {
        self.shows_dot = shows_dot;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }

    fn resolved_style(&self, theme: &crate::theme::Theme) -> BadgeStyle {
        let base = self.tone.color().resolve(theme);
        let disabled_ink = SemanticColorRef::on_surface_disabled().resolve(theme);
        match self.variant {
            BadgeVariant::Soft => BadgeStyle {
                ink: if self.is_enabled { base } else { disabled_ink },
                // Neutral soft badges are the reference "Text at 10 %" pill.
                fill: if self.tone == StatusTone::Neutral {
                    base.multiplied_alpha(0.1)
                } else {
                    base.multiplied_alpha(0.14)
                },
                stroke: Color::CLEAR,
                stroke_width: 0.0,
            },
            BadgeVariant::Solid => BadgeStyle {
                ink: if self.is_enabled {
                    self.tone.ink_on_solid(theme)
                } else {
                    disabled_ink
                },
                fill: base.multiplied_alpha(if self.is_enabled { 1.0 } else { 0.4 }),
                stroke: Color::CLEAR,
                stroke_width: 0.0,
            },
            BadgeVariant::Outline => BadgeStyle {
                ink: if self.is_enabled { base } else { disabled_ink },
                fill: Color::CLEAR,
                stroke: base.multiplied_alpha(if self.is_enabled { 0.45 } else { 0.2 }),
                stroke_width: 1.0,
            },
        }
    }
}

struct BadgeStyle {
    ink: Color,
    fill: Color,
    stroke: Color,
    stroke_width: f32,
}

fn tone_label(tone: StatusTone) -> &'static str {
    match tone {
        StatusTone::Neutral => "Neutral",
        StatusTone::Info => "Info",
        StatusTone::Success => "Success",
        StatusTone::Warning => "Warning",
        StatusTone::Danger => "Danger",
    }
}

impl PrimitiveView for Badge {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_min_width(Some(0.0));
        layout.set_flex_shrink(1.0);
    }

    fn update_node(&self, node: &Node) {
        node.set_is_focusable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::StaticText,
            label: self.title.clone(),
            value: tone_label(self.tone).to_string(),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            combines_children: true,
            ..Default::default()
        }));
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let metrics = BadgeMetrics::new(node.composition_value(&ControlSizeEnvironment::key()));
        let style = self.resolved_style(&theme);
        let disabled_ink = SemanticColorRef::on_surface_disabled().resolve(&theme);
        let base = self.tone.color().resolve(&theme);

        // No title: a bare status dot (for activity feeds and list rows), not
        // an empty pill.
        if self.title.is_empty() && self.icon.is_none() && self.shows_dot {
            let size = metrics.dot_size + 3.0;
            let color = if self.is_enabled { base } else { disabled_ink };
            return vec![AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                )
                .frame_size(size, size)
                .background_color(color)
                .corner_radius(size / 2.0),
            )];
        }

        let mut row: Vec<AnyView> = Vec::new();
        if let Some(icon) = self.icon.clone() {
            row.push(AnyView::new(
                ButtonIcon::new(
                    ButtonIconSource::resource(icon),
                    Some(metrics.icon_size),
                    Some(style.ink),
                )
                .flex(0.0)
                .shrink(0.0),
            ));
        } else if self.shows_dot {
            // A drawn disc: the dot glyph fills only a fraction of its icon box.
            row.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(crate::EmptyView),
                )
                .frame_size(metrics.dot_size, metrics.dot_size)
                .background_color(style.ink)
                .corner_radius(metrics.dot_size / 2.0)
                .accessibility_hidden(true)
                .flex(0.0)
                .shrink(0.0),
            ));
        }
        row.push(AnyView::new(
            Text::new(self.title.clone())
                .semantic_font(metrics.font())
                .foreground_color(style.ink)
                .flex(0.0)
                .shrink(1.0)
                .frame_min_width(0.0),
        ));

        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                metrics.gap,
                AnyView::new(TupleView::new(row)),
            )
            .padding_edges(metrics.padding_x, metrics.padding_y)
            .background_color(style.fill)
            .corner_radius(999.0)
            .border_color(style.stroke, style.stroke_width)
            .opacity(if self.is_enabled { 1.0 } else { 0.6 }),
        )]
    }
}
impl_primitive_view!(Badge);

struct BadgeMetrics {
    font: SemanticFontRef,
    padding_x: f32,
    padding_y: f32,
    gap: f32,
    icon_size: f32,
    dot_size: f32,
}

impl BadgeMetrics {
    fn new(control_size: ControlSize) -> Self {
        match control_size {
            ControlSize::Mini => Self {
                font: SemanticFontRef::caption(),
                padding_x: 5.0,
                padding_y: 1.5,
                gap: 3.0,
                icon_size: 10.0,
                dot_size: 5.0,
            },
            ControlSize::Small => Self {
                font: SemanticFontRef::caption(),
                padding_x: 6.0,
                padding_y: 2.0,
                gap: 4.0,
                icon_size: 11.0,
                dot_size: 5.0,
            },
            ControlSize::Regular => Self {
                font: SemanticFontRef::label(),
                padding_x: 8.0,
                padding_y: 3.0,
                gap: 5.0,
                icon_size: 13.0,
                dot_size: 6.0,
            },
            ControlSize::Large => Self {
                font: SemanticFontRef::label(),
                padding_x: 10.0,
                padding_y: 4.5,
                gap: 6.0,
                icon_size: 15.0,
                dot_size: 7.0,
            },
        }
    }

    fn font(&self) -> SemanticFontRef {
        self.font.clone()
    }
}
