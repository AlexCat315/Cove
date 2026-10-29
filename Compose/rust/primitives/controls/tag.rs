//! Translation of Primitives/Controls/Tag.swift.
use cove_core::scene::{AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, LayoutNode, Node, Wrap};
use cove_core::Color;
use crate::{AnyView, BoxView, PrimitiveView, View};
use crate::impl_primitive_view;
use crate::modifiers::layout::PaddingExt;
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::Button;
use crate::primitives::media::Icon;
use crate::primitives::text::Text;
use crate::theme::{ControlSize, ControlSizeEnvironment, SemanticColorRef, SemanticFontRef, StatusTone, Theme, theme_for_node};

/// Compact labelled chip, optionally removable.
///
/// `Tag` carries content (a filter, a category, an assignee); `Badge` carries
/// status. They look similar on purpose but are separate types so a tag can own
/// a remove action and a badge can never be mistaken for one.
pub struct Tag {
    pub title: String,
    pub tone: StatusTone,
    pub variant: TagVariant,
    /// Leading glyph. `None` keeps the chip text-only.
    pub icon: Option<crate::foundation::BundleImageResource>,
    pub shows_dot: bool,
    pub is_enabled: bool,
    /// When set, a remove button is rendered in the trailing slot.
    pub on_remove: Option<std::rc::Rc<dyn Fn()>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TagVariant {
    #[default]
    Soft,
    Solid,
    Outline,
}

impl Tag {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            tone: StatusTone::Neutral,
            variant: TagVariant::Soft,
            icon: None,
            shows_dot: false,
            is_enabled: true,
            on_remove: None,
        }
    }

    pub fn configure(mut self, configure: impl FnOnce(&mut Self)) -> Self {
        configure(&mut self);
        self
    }

    fn resolved_style(&self, base: Color, disabled_ink: Color, theme: &Theme) -> TagStyle {
        let ink = if self.is_enabled { base } else { disabled_ink };
        match self.variant {
            TagVariant::Soft => TagStyle { ink, fill: base.multiplied_alpha(0.14), stroke: Color::CLEAR, stroke_width: 0.0 },
            TagVariant::Solid => TagStyle {
                ink: if self.is_enabled { self.tone.ink_on_solid(theme) } else { disabled_ink },
                fill: base.multiplied_alpha(if self.is_enabled { 1.0 } else { 0.4 }),
                stroke: Color::CLEAR,
                stroke_width: 0.0,
            },
            TagVariant::Outline => TagStyle {
                ink,
                fill: Color::CLEAR,
                stroke: base.multiplied_alpha(if self.is_enabled { 0.45 } else { 0.2 }),
                stroke_width: 1.0,
            },
        }
    }
}

impl PrimitiveView for Tag {
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
            role: AccessibilityRole::Group,
            label: self.title.clone(),
            help: if self.on_remove.is_none() { String::new() } else { format!("Remove {}", self.title) },
            state: cove_core::scene::AccessibilityState { is_enabled: self.is_enabled, ..Default::default() },
            combines_children: true,
            ..Default::default()
        }));
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let theme = theme_for_node(node);
        let metrics = TagMetrics::new(node.composition_value(&ControlSizeEnvironment::key()));
        let base = self.tone.color().resolve(&theme);
        let disabled_ink = SemanticColorRef::on_surface_disabled().resolve(&theme);
        let style = self.resolved_style(base, disabled_ink, &theme);
        let mut row: Vec<AnyView> = Vec::new();
        if let Some(icon) = self.icon.clone() {
            row.push(AnyView::new(
                Icon::new(icon, crate::primitives::media::IconSize::Points(metrics.icon_size), Some(style.ink)).flex(0.0).shrink(0.0),
            ));
        } else if self.shows_dot {
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
                .accessibility_hidden()
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
        if let Some(on_remove) = self.on_remove.clone() {
            let title = self.title.clone();
            row.push(AnyView::new(
                Button::icon(
                    crate::primitives::controls::button::ButtonIcon::resource(crate::theme::UIIcons::action().x),
                    metrics.remove_size,
                )
                .enabled(self.is_enabled)
                .tooltip(Some(format!("Remove {title}")))
                .action(move || on_remove())
                .button_style(crate::primitives::controls::button::BuiltinButtonStyle::Ghost)
                .control_size(ControlSize::Mini)
                .accessibility_label(format!("Remove {title}"))
                .flex(0.0)
                .shrink(0.0),
            ));
        }
        vec![AnyView::new(
            BoxView::new(FlexDirection::Row, Align::Center, Justify::FlexStart, Wrap::NoWrap, metrics.gap, AnyView::new(crate::TupleView::new(row)))
                .padding_edges(metrics.padding_x, metrics.padding_y)
                .background_color(style.fill)
                .corner_radius(theme.radius.sm)
                .border_color(style.stroke, style.stroke_width)
                .opacity(if self.is_enabled { 1.0 } else { 0.6 }),
        )]
    }
}
impl_primitive_view!(Tag);

struct TagStyle {
    ink: Color,
    fill: Color,
    stroke: Color,
    stroke_width: f32,
}

struct TagMetrics {
    font: SemanticFontRef,
    padding_x: f32,
    padding_y: f32,
    gap: f32,
    icon_size: f32,
    dot_size: f32,
    remove_size: f32,
}

impl TagMetrics {
    fn new(control_size: ControlSize) -> Self {
        match control_size {
            ControlSize::Mini => Self { font: SemanticFontRef::caption(), padding_x: 5.0, padding_y: 1.5, gap: 3.0, icon_size: 10.0, dot_size: 5.0, remove_size: 8.0 },
            ControlSize::Small => Self { font: SemanticFontRef::caption(), padding_x: 7.0, padding_y: 2.0, gap: 4.0, icon_size: 11.0, dot_size: 5.0, remove_size: 9.0 },
            ControlSize::Regular => Self { font: SemanticFontRef::label(), padding_x: 9.0, padding_y: 3.0, gap: 5.0, icon_size: 13.0, dot_size: 6.0, remove_size: 10.0 },
            ControlSize::Large => Self { font: SemanticFontRef::label(), padding_x: 11.0, padding_y: 4.5, gap: 6.0, icon_size: 15.0, dot_size: 7.0, remove_size: 12.0 },
        }
    }

    fn font(&self) -> SemanticFontRef {
        self.font.clone()
    }
}
