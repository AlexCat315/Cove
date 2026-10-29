//! Translation of Primitives/Controls/Link.swift.
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, Wrap};
use crate::{AnyView, BoxView, View};
use crate::modifiers::style::{AppearanceExt, ButtonStyle, ButtonStyleConfiguration, SemanticStyleViewExt};
use crate::primitives::controls::button::{ActionControlAccessibility, Button};
use crate::primitives::media::Icon;
use crate::primitives::text::Text;
use crate::theme::SemanticMotionRef;
use crate::ViewExt;

/// When a link draws its underline.
///
/// The choice is contextual rather than decorative: in body copy the link has
/// to be identifiable before the pointer arrives, while in a dense tool panel
/// a permanent underline turns every row into visual noise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LinkUnderline {
    /// Always underlined — body copy, help text, license notices.
    #[default]
    Always,
    /// Underlined on hover and keyboard focus — tool panels, tables, toolbars.
    Hover,
    /// Never underlined; colour alone carries the affordance. Only defensible
    /// where the surrounding text is clearly not interactive.
    Never,
}

/// A navigation or command target rendered inside text.
///
/// A link is not a button with a tint: it stays in the reading flow, so it
/// inherits the surrounding type size, keeps no chrome of its own until the
/// pointer or focus arrives, and always announces itself as a link rather than
/// as a button.
pub struct Link {
    pub label: AnyView,
    pub action: std::rc::Rc<dyn Fn()>,
    pub is_enabled: bool,
    pub is_external: bool,
    pub underline: LinkUnderline,
    /// Overrides the trailing glyph. `None` falls back to the external-link
    /// arrow when `is_external` is set.
    pub icon: Option<crate::foundation::BundleImageResource>,
    pub tooltip: Option<String>,
}

impl Link {
    pub fn new(tooltip: Option<String>, action: impl Fn() + 'static, label: impl View + 'static) -> Self {
        Self {
            label: AnyView::new(label),
            action: std::rc::Rc::new(action),
            is_enabled: true,
            is_external: false,
            underline: LinkUnderline::Always,
            icon: None,
            tooltip,
        }
    }

    pub fn title(title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self::new(None, action, Text::new(title).truncating_tail())
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }
}

impl View for Link {
    fn body(&self) -> AnyView {
        let label = self.label.clone();
        let icon = self.icon.clone();
        let is_external = self.is_external;
        let action = self.action.clone();
        let mut row: Vec<AnyView> = vec![label];
        if let Some(icon) = icon {
            row.push(AnyView::new(Icon::new(icon, crate::primitives::media::IconSize::Small, None)));
        } else if is_external {
            row.push(AnyView::new(Icon::new(
                crate::theme::UIIcons::action().external_link,
                crate::primitives::media::IconSize::Small,
                None,
            )));
        }
        let content = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            4.0,
            AnyView::new(crate::TupleView::new(row)),
        ));
        let button = Button::new(content, move || action())
            .enabled(self.is_enabled)
            .tooltip(self.tooltip.clone())
            .button_style(LinkStyle { underline: self.underline })
            .composition_local(ActionControlAccessibility::role(), AccessibilityRole::Link);
        AnyView::new(button)
    }
}

struct LinkStyle {
    underline: LinkUnderline,
}

impl ButtonStyle for LinkStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration, content: AnyView) -> AnyView {
        let resting = if configuration.is_enabled { configuration.theme.colors.accent } else { configuration.theme.colors.on_surface_muted };
        let ink = if configuration.is_pressed {
            configuration.theme.colors.accent_pressed
        } else if configuration.is_hovered {
            configuration.theme.colors.accent_hover
        } else {
            resting
        };
        let shows_rule = match self.underline {
            LinkUnderline::Always => true,
            LinkUnderline::Never => false,
            // Focus draws the rule too: keyboard users get no hover to reveal it.
            LinkUnderline::Hover => configuration.is_hovered || configuration.is_focused || configuration.is_pressed,
        };
        // Reserve the same rule geometry in every interaction state.
        // Revealing an underline must never move neighbouring text.
        let rule = BoxView::new(FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::NoWrap, 0.0, AnyView::new(crate::EmptyView))
            .frame_height(1.0)
            .background_color(if shows_rule { ink } else { cove_core::Color::CLEAR });
        AnyView::new(
            BoxView::new(FlexDirection::Column, Align::Stretch, Justify::FlexStart, Wrap::NoWrap, 2.0, AnyView::new(
                crate::TupleView::new(vec![content, AnyView::new(rule)]),
            ))
            .foreground_color(ink)
            .corner_radius(configuration.theme.radius.sm)
            .border_color(
                if configuration.is_focused { configuration.theme.colors.focus_ring } else { cove_core::Color::CLEAR },
                if configuration.is_focused { 2.0 } else { 0.0 },
            )
            .animation(SemanticMotionRef::fast(), configuration.interaction_key),
        )
    }
}
