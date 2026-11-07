//! Translation of Primitives/Feedback/Alert.swift.
//!
//! Inline feedback block: a status glyph, a title, an optional explanation,
//! optional actions and an optional dismissal.
//!
//! The glyph is part of the content, not decoration — it carries the same
//! meaning as the tone's colour, so the block still reads correctly when the
//! palette is monochrome or the reader cannot distinguish the hues.

use crate::Scope;
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{EdgeInsets, FlexExt, FrameDimension, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource};
use crate::primitives::feedback::StatusTone;
use crate::primitives::layout::ActionGroupLayout;
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonRole, ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::{
    ControlSize, ControlSizeEnvironment, ControlSizeViewExt, IconMetrics, SemanticFontRef, Theme,
    theme_for_node,
};
use crate::{AnyView, BoxView, LayoutDebugExt, PrimitiveView, TupleView, View};
use cove_core::Color;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_imaging::BundleImageResource;

/// How the block relates to the surface behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AlertVariant {
    /// Tone-washed surface with a matching hairline, for a block that sits in
    /// ordinary content.
    #[default]
    Soft,
    /// Transparent with a tinted edge, for stacking inside a bordered panel.
    Outline,
    /// Fully filled with the status tone. Reserve for a single blocking message.
    Filled,
}

/// Inline feedback block with a status glyph and optional dismissal.
///
/// Density follows the inherited `ControlSize`: a compact alert shrinks the
/// glyph, the inner gaps and the card padding together.
pub struct Alert {
    pub title: String,
    pub message: String,
    pub tone: StatusTone,
    pub variant: AlertVariant,
    /// Glyph override. `None` uses the tone's glyph.
    pub icon: Option<BundleImageResource>,
    pub shows_icon: bool,
    pub is_enabled: bool,
    /// When set, a close button is rendered in the trailing slot.
    pub on_dismiss: Option<Rc<dyn Fn()>>,
    pub dismiss_label: String,
    actions: Option<AnyView>,
}

impl Alert {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: String::new(),
            tone: StatusTone::Info,
            variant: AlertVariant::Soft,
            icon: None,
            shows_icon: true,
            is_enabled: true,
            on_dismiss: None,
            dismiss_label: "Dismiss".to_string(),
            actions: None,
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn variant(mut self, variant: AlertVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn shows_icon(mut self, shows_icon: bool) -> Self {
        self.shows_icon = shows_icon;
        self
    }

    /// A disabled alert mutes the card and detaches its dismissal.
    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn on_dismiss(mut self, on_dismiss: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(on_dismiss));
        self
    }

    /// Name of the dismissal affordance for assistive technology and for its
    /// hover tip. Defaults to `Dismiss`.
    pub fn dismiss_label(mut self, label: impl Into<String>) -> Self {
        self.dismiss_label = label.into();
        self
    }

    /// Trailing or stacked action row, as an already-composed view.
    pub fn actions<Actions: View + 'static>(mut self, actions: Actions) -> Self {
        self.actions = Some(AnyView::new(actions));
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }
}

impl View for Alert {
    fn body(&self, _cx: &Scope) -> AnyView {
        AnyView::new(AlertHost {
            snapshot: AlertSnapshot {
                title: self.title.clone(),
                message: self.message.clone(),
                tone: self.tone,
                variant: self.variant,
                icon: self.icon.clone(),
                shows_icon: self.shows_icon,
                is_enabled: self.is_enabled,
                on_dismiss: self.on_dismiss.clone(),
                dismiss_label: self.dismiss_label.clone(),
                actions: self.actions.clone(),
                has_actions: self.actions.is_some(),
            },
        })
    }
}

struct AlertStyle {
    ink: Color,
    icon_ink: Color,
    detail: Color,
    dismiss_ink: Color,
    fill: Color,
    stroke: Color,
}

/// Density resolved once per pass from the inherited control size and the
/// active text scaler, so the glyph, the gaps and the padding stay in step.
struct AlertMetrics {
    glyph: f32,
    glyph_gap: f32,
    padding: f32,
    title_message_spacing: f32,
    actions_gap: f32,
    action_spacing: f32,
    title_line_height: f32,
}

impl AlertMetrics {
    fn resolve(node: &Node, theme: &Theme) -> Self {
        let size = node.composition_value(&ControlSizeEnvironment::key());
        let icons = IconMetrics::new(node);
        let glyph = icons.size(match size {
            ControlSize::Mini | ControlSize::Small => theme.icons.small,
            ControlSize::Regular => theme.icons.medium,
            ControlSize::Large => theme.icons.large,
        });
        let (padding, glyph_gap, actions_gap, action_spacing) = match size {
            ControlSize::Mini | ControlSize::Small => (
                theme.spacing.md,
                theme.spacing.sm,
                theme.spacing.sm,
                theme.spacing.sm,
            ),
            ControlSize::Regular => (
                theme.spacing.lg,
                theme.spacing.md,
                theme.spacing.md,
                theme.spacing.md,
            ),
            ControlSize::Large => (
                theme.spacing.xl,
                theme.spacing.lg,
                theme.spacing.lg,
                theme.spacing.lg,
            ),
        };
        Self {
            glyph,
            glyph_gap,
            padding,
            // Title and explanation read as one paragraph; only actions get air.
            title_message_spacing: theme.spacing.xs * 0.5,
            actions_gap,
            action_spacing,
            title_line_height: icons.size(theme.typography.body_strong.line_height),
        }
    }

    /// Leading inset that keeps the explanation and the actions under the
    /// title instead of under the glyph.
    fn text_inset(&self, shows_icon: bool) -> f32 {
        if shows_icon {
            self.glyph + self.glyph_gap
        } else {
            0.0
        }
    }
}

/// Everything the block needs once a theme is available. Kept as a plain
/// value so the primitive can resolve colors on the node.
struct AlertSnapshot {
    title: String,
    message: String,
    tone: StatusTone,
    variant: AlertVariant,
    icon: Option<BundleImageResource>,
    shows_icon: bool,
    is_enabled: bool,
    on_dismiss: Option<Rc<dyn Fn()>>,
    dismiss_label: String,
    actions: Option<AnyView>,
    has_actions: bool,
}

impl AlertSnapshot {
    fn style(&self, theme: &Theme) -> AlertStyle {
        let base = self.tone.color().resolve(theme);
        match self.variant {
            // The tone washes the fill and draws a hairline edge, so the block
            // holds its shape on any surface while the hue stays a hint.
            AlertVariant::Soft => AlertStyle {
                ink: theme.colors.on_surface,
                icon_ink: base,
                detail: theme.colors.on_surface_variant,
                dismiss_ink: theme.colors.on_surface_variant,
                fill: theme.colors.surface.composited(base.multiplied_alpha(0.10)),
                stroke: base.multiplied_alpha(0.18),
            },
            AlertVariant::Outline => AlertStyle {
                ink: theme.colors.on_surface,
                icon_ink: base,
                detail: theme.colors.on_surface_variant,
                dismiss_ink: theme.colors.on_surface_variant,
                fill: Color::CLEAR,
                stroke: base.multiplied_alpha(0.35),
            },
            AlertVariant::Filled => {
                let solid_ink = self.tone.ink_on_solid(theme);
                AlertStyle {
                    ink: solid_ink,
                    icon_ink: solid_ink,
                    detail: solid_ink.multiplied_alpha(0.88),
                    dismiss_ink: solid_ink.multiplied_alpha(0.80),
                    fill: base,
                    stroke: base,
                }
            }
        }
    }
}

struct AlertHost {
    snapshot: AlertSnapshot,
}

impl PrimitiveView for AlertHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        Some(cove_core::scene::LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let mut semantics =
            cove_core::scene::AccessibilitySemantics::new(AccessibilityRole::Status);
        semantics.label = self.snapshot.title.clone();
        semantics.help = self.snapshot.message.clone();
        semantics.state.is_enabled = self.snapshot.is_enabled;
        node.set_accessibility(Some(semantics));
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let style = self.snapshot.style(&theme);
        let metrics = AlertMetrics::resolve(node, &theme);
        let text_inset = metrics.text_inset(self.snapshot.shows_icon);

        let mut leading: Vec<AnyView> = Vec::new();
        if self.snapshot.shows_icon {
            let resource = self
                .snapshot
                .icon
                .clone()
                .unwrap_or_else(|| self.snapshot.tone.filled_icon());
            leading.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(ButtonIcon::new(
                        ButtonIconSource::resource(resource),
                        Some(metrics.glyph),
                        Some(style.icon_ink),
                    )),
                )
                .frame(
                    Some(FrameDimension::Points(metrics.glyph)),
                    Some(FrameDimension::Points(
                        metrics.glyph.max(metrics.title_line_height),
                    )),
                    None,
                    None,
                    None,
                    None,
                )
                .debug_name("alert-icon-slot")
                .accessibility_hidden(true)
                .flex(0.0)
                .shrink(0.0),
            ));
        }
        leading.push(AnyView::new(
            Text::new(self.snapshot.title.clone())
                .semantic_font(SemanticFontRef::body_strong())
                .foreground_color(style.ink)
                .flex(0.0)
                .shrink(1.0)
                .frame_min_width(0.0),
        ));

        let mut header: Vec<AnyView> = vec![AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::FlexStart,
                Justify::FlexStart,
                Wrap::NoWrap,
                metrics.glyph_gap,
                AnyView::new(TupleView::new(leading)),
            ))
            .flex(1.0)
            .shrink(1.0)
            .frame_min_width(0.0),
        )];
        if let Some(on_dismiss) = self.snapshot.on_dismiss.clone() {
            let label = self.snapshot.dismiss_label.clone();
            header.push(AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::resource(ui_icon_catalog::action::x()),
                    Some(metrics.glyph),
                    Some(style.dismiss_ink),
                    ButtonRole::Normal,
                    self.snapshot.is_enabled,
                    false,
                    false,
                    Some(label.clone()),
                    move || on_dismiss(),
                )
                .button_style(GhostButtonStyle)
                .control_size(ControlSize::Small)
                .accessibility_label(label.clone())
                .accessibility(move |semantics| semantics.label = label.clone()),
            ));
        }

        let mut column: Vec<AnyView> = vec![AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            metrics.glyph_gap,
            AnyView::new(TupleView::new(header)),
        ))];
        if !self.snapshot.message.is_empty() {
            column.push(AnyView::new(
                AnyView::new(
                    Text::new(self.snapshot.message.clone())
                        .semantic_font(SemanticFontRef::body())
                        .foreground_color(style.detail),
                )
                .padding_insets(EdgeInsets::new(0.0, text_inset, 0.0, 0.0)),
            ));
        }
        if self.snapshot.has_actions
            && let Some(actions) = self.snapshot.actions.clone()
        {
            let group = crate::LayoutContainer::<ActionGroupLayout, AnyView>::with_erased_content(
                ActionGroupLayout {
                    stacks: node
                        .composition_value(&crate::adaptive::SurfaceEnvironment::key())
                        .viewport
                        .width
                        < cove_core::Viewport::COMPACT_MAXIMUM_WIDTH,
                    alignment: crate::HorizontalAlignment::Leading,
                    spacing: metrics.action_spacing,
                },
                actions,
            );
            column.push(AnyView::new(AnyView::new(group).padding_insets(
                EdgeInsets::new(metrics.actions_gap, text_inset, 0.0, 0.0),
            )));
        }

        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                metrics.title_message_spacing,
                AnyView::new(TupleView::new(column)),
            )
            .padding(metrics.padding)
            .background_color(style.fill)
            .corner_radius(theme.radius.md)
            .border_color(style.stroke, 1.0)
            .opacity(if self.snapshot.is_enabled { 1.0 } else { 0.6 }),
        )]
    }
}

impl View for AlertHost {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
