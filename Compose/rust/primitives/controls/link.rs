//! Translation of Primitives/Controls/Link.swift.
use crate::modifiers::style::AppearanceExt;
use crate::primitives::controls::button::{
    ActionControlAccessibility, Button, ButtonIcon, ButtonIconSource,
};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt, impl_primitive_view};
use cove_core::Color;
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, LayoutNode, Node, Wrap};
use cove_imaging::BundleImageResource;

/// When a link draws its underline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LinkUnderline {
    /// Always underlined — body copy, help text, license notices.
    #[default]
    Always,
    /// Underlined on hover and keyboard focus.
    Hover,
    /// Never underlined.
    Never,
}

/// A navigation or command target rendered in the reading flow.
pub struct Link {
    pub label: AnyView,
    pub action: std::rc::Rc<dyn Fn()>,
    pub is_enabled: bool,
    pub is_external: bool,
    pub underline: LinkUnderline,
    /// Overrides the external-link glyph when present.
    pub icon: Option<BundleImageResource>,
    pub tooltip: Option<String>,
}

impl Link {
    pub fn new(label: impl View + 'static, action: impl Fn() + 'static) -> Self {
        Self {
            label: AnyView::new(label),
            action: std::rc::Rc::new(action),
            is_enabled: true,
            is_external: false,
            underline: LinkUnderline::Always,
            icon: None,
            tooltip: None,
        }
    }

    pub fn title(title: impl Into<String>, action: impl Fn() + 'static) -> Self {
        Self::new(Text::new(title).truncated(), action)
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.is_enabled = enabled;
        self
    }
    pub fn external(mut self, external: bool) -> Self {
        self.is_external = external;
        self
    }
    pub fn underline(mut self, underline: LinkUnderline) -> Self {
        self.underline = underline;
        self
    }
    pub fn icon(mut self, icon: BundleImageResource) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
    pub fn with_tooltip(mut self, tooltip: Option<String>) -> Self {
        self.tooltip = tooltip;
        self
    }
    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }
}

impl View for Link {
    fn body(&self) -> AnyView {
        let mut content = vec![self.label.clone()];
        let trailing_icon = self.icon.clone().or_else(|| {
            self.is_external
                .then(crate::primitives::media::ui_icon_catalog::action::external_link)
        });
        if let Some(resource) = trailing_icon {
            content.push(AnyView::new(ButtonIcon::new(
                ButtonIconSource::resource(resource),
                Some(12.0),
                None,
            )));
        }
        let label = BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            4.0,
            AnyView::new(TupleView::new(content)),
        );
        let action = self.action.clone();
        let button = Button::new(label, move || action())
            .enabled(self.is_enabled)
            .with_tooltip(self.tooltip.clone())
            .button_style(LinkStyle {
                underline: self.underline,
            })
            .composition_local(ActionControlAccessibility::role(), AccessibilityRole::Link);
        AnyView::new(button)
    }
}

struct LinkStyle {
    underline: LinkUnderline,
}
impl ButtonStyle for LinkStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let resting = if configuration.is_enabled {
            configuration.theme.colors.accent
        } else {
            configuration.theme.colors.on_surface_muted
        };
        let ink = if configuration.is_pressed {
            configuration.theme.colors.accent_pressed
        } else if configuration.is_hovered {
            configuration.theme.colors.accent_hover
        } else {
            resting
        };
        let show_underline = match self.underline {
            LinkUnderline::Always => true,
            LinkUnderline::Never => false,
            LinkUnderline::Hover => {
                configuration.is_hovered || configuration.is_focused || configuration.is_pressed
            }
        };
        let rule = AnyView::new(LinkUnderlineRule {
            color: if show_underline { ink } else { Color::CLEAR },
        });
        let face = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            2.0,
            AnyView::new(TupleView::new(vec![configuration.label, rule])),
        )
        .foreground_color(ink)
        .corner_radius(configuration.theme.radius.sm)
        .border_color(
            if configuration.is_focused {
                configuration.theme.colors.focus_ring
            } else {
                Color::CLEAR
            },
            if configuration.is_focused { 2.0 } else { 0.0 },
        );
        AnyView::new(face)
    }

    fn style_identity(&self) -> u64 {
        self.underline as u64
    }
}

struct LinkUnderlineRule {
    color: Color,
}
impl PrimitiveView for LinkUnderlineRule {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(1.0));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(1.0));
    }
    fn update_node(&self, node: &Node) {
        node.set_background_color(Some(self.color));
    }
}
impl_primitive_view!(LinkUnderlineRule);
