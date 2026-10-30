//! Translation of Primitives/Feedback/Alert.swift.
//!
//! Inline feedback block: a status glyph, a title, an optional explanation,
//! optional actions and an optional dismissal.
//!
//! The glyph is part of the content, not decoration — it carries the same
//! meaning as the tone's colour, so the block still reads correctly when the
//! palette is monochrome or the reader cannot distinguish the hues.

use std::rc::Rc;

use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Node, Wrap};
use cove_core::Color;
use cove_imaging::BundleImageResource;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{
    EdgeInsets, FlexExt, FrameDimension, LayoutFrameExt, PaddingExt,
};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource};
use crate::primitives::feedback::StatusTone;
use crate::primitives::layout::ActionGroupLayout;
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::theme::ControlSizeViewExt;
use crate::theme::{node_text_scaler, theme_for_node, ControlSize, SemanticColorRef, SemanticFontRef, Theme};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};

/// How the block relates to the surface behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AlertTone {
    /// Neutral grouped surface, with tone reserved for the glyph.
    #[default]
    Soft,
    /// Transparent with a tinted edge, for stacking inside a bordered panel.
    Outline,
    /// Fully filled with the status tone. Reserve for a single blocking message.
    Filled,
}

/// Inline feedback block with a status glyph and optional dismissal.
pub struct Alert {
    pub title: String,
    pub message: String,
    pub tone: StatusTone,
    pub variant: AlertTone,
    /// Glyph override. `None` uses the tone's glyph.
    pub icon: Option<BundleImageResource>,
    pub shows_icon: bool,
    /// When set, a close button is rendered in the trailing slot.
    pub on_dismiss: Option<Rc<dyn Fn()>>,
    actions: Option<AnyView>,
}

impl Alert {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: String::new(),
            tone: StatusTone::Info,
            variant: AlertTone::Soft,
            icon: None,
            shows_icon: true,
            on_dismiss: None,
            actions: None,
        }
    }

    /// Mirrors the Swift `actions:` `@ViewBuilder` overload.
    pub fn with_actions<Actions: View + 'static>(title: impl Into<String>, actions: Actions) -> Self {
        let mut value = Self::new(title);
        value.actions = Some(AnyView::new(actions));
        value
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn tone(mut self, tone: StatusTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn variant(mut self, variant: AlertTone) -> Self {
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

    pub fn on_dismiss(mut self, on_dismiss: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(on_dismiss));
        self
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }

}

impl View for Alert {
    fn body(&self) -> AnyView {
        let title = self.title.clone();
        let message = self.message.clone();
        let tone = self.tone;
        let variant = self.variant;
        let icon = self.icon.clone();
        let shows_icon = self.shows_icon;
        let on_dismiss = self.on_dismiss.clone();
        let actions = self.actions.clone();
        let has_actions = self.actions.is_some();
        AnyView::new(AlertHost {
            snapshot: AlertSnapshot { title, message, tone, variant, icon, shows_icon, on_dismiss, actions, has_actions },
        })
    }
}

struct AlertStyle {
    ink: Color,
    icon_ink: Color,
    detail: Color,
    fill: Color,
    stroke: Color,
}

/// Everything the block needs once a theme is available. Kept as a plain
/// value so the primitive can resolve colors on the node.
struct AlertSnapshot {
    title: String,
    message: String,
    tone: StatusTone,
    variant: AlertTone,
    icon: Option<BundleImageResource>,
    shows_icon: bool,
    on_dismiss: Option<Rc<dyn Fn()>>,
    actions: Option<AnyView>,
    has_actions: bool,
}

impl AlertSnapshot {
    fn style(&self, theme: &Theme) -> AlertStyle {
        let base = self.tone.color().resolve(theme);
        match self.variant {
            AlertTone::Soft => AlertStyle {
                ink: theme.colors.on_surface,
                icon_ink: base,
                detail: theme.colors.on_surface_variant,
                fill: theme.colors.surface_raised,
                stroke: theme.colors.border,
            },
            AlertTone::Outline => AlertStyle {
                ink: theme.colors.on_surface,
                icon_ink: base,
                detail: theme.colors.on_surface_variant,
                fill: Color::CLEAR,
                stroke: base.multiplied_alpha(0.45),
            },
            AlertTone::Filled => {
                let solid_ink = self.tone.ink_on_solid(theme);
                AlertStyle {
                    ink: solid_ink,
                    icon_ink: solid_ink,
                    detail: solid_ink.multiplied_alpha(0.88),
                    fill: base,
                    stroke: Color::CLEAR,
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
        let theme = theme_for_node(node);
        let style = self.snapshot.style(&theme);
        let label = self.snapshot.title.clone();
        let help = self.snapshot.message.clone();
        let mut semantics = cove_core::scene::AccessibilitySemantics::new(AccessibilityRole::Status);
        semantics.label = label;
        semantics.help = help;
        let _ = style;
        node.set_accessibility(Some(semantics));
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let scaler = node_text_scaler(node);
        let style = self.snapshot.style(&theme);
        let icon_extent = theme.icons.medium;
        let text_inset = if self.snapshot.shows_icon { icon_extent + theme.spacing.sm } else { 0.0 };

        let mut leading: Vec<AnyView> = Vec::new();
        if self.snapshot.shows_icon {
            let resource = self.snapshot.icon.clone().unwrap_or_else(|| self.snapshot.tone.icon());
            leading.push(AnyView::new(
                ButtonIcon::new(ButtonIconSource::resource(resource), Some(icon_extent), Some(style.icon_ink))
                    .frame(
                        Some(FrameDimension::Points(icon_extent)),
                        Some(FrameDimension::Points(
                            icon_extent.max(scaler.scale(theme.typography.body_strong.line_height)),
                        )),
                        None,
                        None,
                        None,
                        None,
                    )
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
                theme.spacing.sm,
                AnyView::new(TupleView::new(leading)),
            ))
            .flex(1.0)
            .shrink(1.0)
            .frame_min_width(0.0),
        )];
        if let Some(on_dismiss) = self.snapshot.on_dismiss.clone() {
            let dismiss = Button::icon(
                ButtonIconSource::resource(ui_icon_catalog::action::x()),
                14.0,
                move || on_dismiss(),
            )
            .button_style(GhostButtonStyle)
            .control_size(ControlSize::Small)
            .accessibility_label("Dismiss")
            .accessibility(move |semantics| semantics.label = "Dismiss".to_string());
            header.push(AnyView::new(dismiss));
        }

        let mut column: Vec<AnyView> = vec![AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::FlexStart,
            Justify::FlexStart,
            Wrap::NoWrap,
            theme.spacing.sm,
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
            let group = crate::LayoutContainer::with_erased_content(
                ActionGroupLayout {
                    stacks: false,
                    alignment: crate::HorizontalAlignment::Leading,
                    spacing: 8.0,
                },
                actions,
            );
            column.push(AnyView::new(
                AnyView::new(group)
                    .padding_insets(EdgeInsets::new(theme.spacing.xs, text_inset, 0.0, 0.0)),
            ));
        }

        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(column)),
            )
            .padding(theme.spacing.md)
            .background_color(style.fill)
            .corner_radius(theme.radius.md)
            .border_color(style.stroke, 1.0),
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
