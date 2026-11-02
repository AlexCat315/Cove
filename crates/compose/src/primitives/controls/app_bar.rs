//! Translation of Primitives/Controls/AppBar.swift and AppBar/Back.swift.
//!
//! A bounded application header. The host owns safe-area insets; the bar owns a
//! full-name heading, back action and at most one/three inline commands.
use crate::Scope;
use std::cell::RefCell;
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIconSource, ButtonRole};
use crate::primitives::controls::toolbar::{
    Toolbar, ToolbarCommandPresentation, ToolbarCommands, ToolbarDividerEdge, ToolbarItem,
    ToolbarOptions, toolbar_height,
};
use crate::primitives::media::ui_icon_catalog;
use crate::primitives::overlays::menu::MenuItem;
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, Align, FlexDirection, Justify, Node, Wrap,
};

/// Width below which the bar is compact.
pub const DEFAULT_COMPACT_MAXIMUM_WIDTH: f32 = 600.0;

/// Authored application-bar options.
#[derive(Clone, Debug, PartialEq)]
pub struct AppBarOptions {
    pub is_enabled: bool,
    pub label: String,
    pub compact_maximum_width: f32,
    pub divider: ToolbarDividerEdge,
}

impl Default for AppBarOptions {
    fn default() -> Self {
        Self {
            is_enabled: true,
            label: crate::i18n::tr("Application bar"),
            compact_maximum_width: DEFAULT_COMPACT_MAXIMUM_WIDTH,
            divider: ToolbarDividerEdge::Bottom,
        }
    }
}

impl AppBarOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.compact_maximum_width = if self.compact_maximum_width.is_finite() {
            self.compact_maximum_width.max(0.0)
        } else {
            DEFAULT_COMPACT_MAXIMUM_WIDTH
        };
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

/// A bounded application header.
pub struct AppBar {
    pub title: String,
    pub subtitle: Option<String>,
    pub back: Option<MenuItem>,
    primary: Option<ToolbarItem>,
    actions: Vec<ToolbarItem>,
    pub options: AppBarOptions,
    /// Measured width of the command row, reported after layout.
    pub command_width: Rc<RefCell<Option<f32>>>,
    /// Width this bar was given, supplied by the container-size reader.
    pub width: Rc<RefCell<f32>>,
}

impl AppBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            back: None,
            primary: None,
            actions: Vec::new(),
            options: AppBarOptions::default().validated(),
            command_width: Rc::new(RefCell::new(None)),
            width: Rc::new(RefCell::new(0.0)),
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn back(mut self, command: MenuItem) -> Self {
        self.back = Some(command);
        self
    }

    pub fn try_commands(
        mut self,
        primary: Option<ToolbarItem>,
        actions: Vec<ToolbarItem>,
    ) -> Result<Self, crate::primitives::DuplicateId> {
        crate::primitives::check_ids(
            primary.iter().chain(&actions).map(|item| item.command.id),
            "App bar command",
        )?;
        self.primary = primary;
        self.actions = actions;
        Ok(self)
    }

    pub fn options(mut self, options: AppBarOptions) -> Self {
        self.options = options.validated();
        self
    }

    /// Inline capacity: one command when compact, three otherwise, less the
    /// primary slot.
    fn items(&self, compact: bool) -> Vec<ToolbarItem> {
        let capacity = (if compact { 1 } else { 3 }) - usize::from(self.primary.is_some());
        let mut candidates: Vec<usize> = (0..self.actions.len())
            .filter(|index| {
                self.actions[*index].placement
                    != crate::primitives::controls::ToolbarItemPlacement::Overflow
            })
            .collect();
        // Higher priority inlines first; ties keep source order.
        candidates.sort_by(|lhs, rhs| {
            let a = self.actions[*lhs].placement.priority();
            let b = self.actions[*rhs].placement.priority();
            if a == b { lhs.cmp(rhs) } else { b.cmp(&a) }
        });
        let inline: Vec<usize> = candidates.into_iter().take(capacity).collect();
        let mut items: Vec<ToolbarItem> = Vec::new();
        if let Some(primary) = self.primary.as_ref() {
            items.push(
                primary
                    .clone()
                    .placement(crate::primitives::controls::ToolbarItemPlacement::Always),
            );
        }
        for (index, item) in self.actions.iter().enumerate() {
            items.push(item.clone().placement(if inline.contains(&index) {
                crate::primitives::controls::ToolbarItemPlacement::Automatic
            } else {
                crate::primitives::controls::ToolbarItemPlacement::Overflow
            }));
        }
        items
    }

    fn snapshot(&self) -> AppBar {
        AppBar {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            back: self.back.clone(),
            primary: self.primary.clone(),
            actions: self.actions.clone(),
            options: self.options.clone(),
            command_width: self.command_width.clone(),
            width: self.width.clone(),
        }
    }
}

impl View for AppBar {
    fn body(&self, _cx: &Scope) -> AnyView {
        let bar = self.snapshot();
        // Container size decides compactness, so the whole bar rebuilds when the
        // allocation crosses the breakpoint.
        AnyView::new(crate::adaptive::ContainerSizeReader::new(
            move |size: cove_core::Size| {
                let width = if size.width > 0.0 {
                    size.width
                } else {
                    *bar.width.borrow()
                };
                *bar.width.borrow_mut() = width;
                AnyView::new(AppBarHost {
                    bar: bar.snapshot(),
                    width,
                })
            },
        ))
    }
}

struct AppBarHost {
    bar: AppBar,
    width: f32,
}

impl PrimitiveView for AppBarHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let bar = &self.bar;
        let compact = self.width < bar.options.compact_maximum_width;
        let items = bar.items(compact);

        let mut children: Vec<AnyView> = Vec::new();
        if let Some(back) = bar.back.clone() {
            children.push(AnyView::new(AppBarBack {
                command: back,
                is_enabled: bar.options.is_enabled,
            }));
        }

        // Heading: the full name stays readable, and only a clipped one shows a
        // tooltip.
        let title = bar.title.clone();
        let subtitle = bar.subtitle.clone();
        let combined_subtitle = if compact {
            subtitle.clone().unwrap_or_default()
        } else {
            String::new()
        };
        children.push(AnyView::new(AppBarHeading {
            title: title.clone(),
            subtitle: if compact { None } else { subtitle },
            value: combined_subtitle,
        }));

        // Commands take at most half the row; the heading keeps the rest.
        let gap = 8.0;
        let available =
            (self.width - 16.0 - if bar.back.is_some() { 44.0 } else { 0.0 } - gap).max(0.0);
        let primary_width: f32 = match bar.primary.as_ref() {
            None => 0.0,
            Some(item) => {
                if item.icon.is_some() {
                    44.0
                } else {
                    80.0
                }
            }
        };
        let visible_items = items.len() as f32;
        let required = primary_width
            + if visible_items > if bar.primary.is_some() { 1.0 } else { 0.0 } {
                44.0
            } else {
                0.0
            }
            + if bar.primary.is_some() && visible_items > 1.0 {
                gap
            } else {
                0.0
            };
        let budget = (available - (96.0_f32.min((available - required).max(0.0)))).max(0.0);
        let measured = *bar.command_width.borrow();
        let commands_width = measured.unwrap_or(budget).min(budget);
        let item_count = items.len();

        let commands = ToolbarCommands::from_unique_items(
            items,
            bar.options.is_enabled,
            crate::primitives::controls::ToolbarAxis::Horizontal,
        )
        // The bar's own allocation is the budget: with it supplied the overflow
        // layout measures and moves commands that do not fit.
        .available_width(budget)
        .presentation(ToolbarCommandPresentation {
            minimum_extent: Some(44.0),
            maximum_button_width: Some(
                (120.0_f32.min((budget - if item_count > 1 { 44.0 + gap } else { 0.0 }).max(0.0)))
                    .max(0.0),
            ),
            uses_overflow_icon: true,
        });
        children.push(AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(commands),
            )
            .frame(None, None, None, None, Some(commands_width), None),
        ));

        let _ = toolbar_height(crate::theme::ControlSize::Regular);
        vec![AnyView::new(
            Toolbar::new(AnyView::new(TupleView::new(children)))
                .options(ToolbarOptions {
                    divider: bar.options.divider,
                    is_enabled: bar.options.is_enabled,
                    label: bar.options.label.clone(),
                    axis: crate::primitives::controls::ToolbarAxis::Horizontal,
                })
                .minimum_height(48.0)
                .vertical_padding(2.0),
        )]
    }
}
crate::impl_primitive_view!(AppBarHost);

/// The heading block: title, and the subtitle when it is not compact.
struct AppBarHeading {
    title: String,
    subtitle: Option<String>,
    value: String,
}

impl PrimitiveView for AppBarHeading {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Heading,
            label: self.title.clone(),
            value: self.value.clone(),
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        layout.set_min_width(Some(0.0));
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        layout.set_min_width(Some(0.0));
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let mut column: Vec<AnyView> = vec![AnyView::new(
            Text::new(self.title.clone())
                .truncated()
                .semantic_font(SemanticFontRef::body_strong())
                .semantic_foreground(SemanticColorRef::on_surface()),
        )];
        if let Some(subtitle) = self.subtitle.as_ref().filter(|value| !value.is_empty()) {
            column.push(AnyView::new(
                Text::new(subtitle.clone())
                    .truncated()
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                2.0,
                AnyView::new(TupleView::new(column)),
            )
            .flex_with(1.0, 1.0, Some(0.0))
            .frame_min_width(0.0),
        )]
    }
}
crate::impl_primitive_view!(AppBarHeading);

/// The back affordance, mirrored in right-to-left layouts.
struct AppBarBack {
    command: MenuItem,
    is_enabled: bool,
}

impl PrimitiveView for AppBarBack {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, _node: &Node) {}

    fn children(&self, node: &Node) -> Vec<AnyView> {
        // The authored direction decides which way the chevron points.
        let right_to_left = node.resolves_right_to_left();
        let icon = if right_to_left {
            ui_icon_catalog::navigation::arrow_right()
        } else {
            ui_icon_catalog::navigation::arrow_left()
        };
        let command = self.command.clone();
        let enabled = self.is_enabled && command.is_enabled;
        let label = command.title.clone();
        let button = Button::icon_with_options(
            ButtonIconSource::resource(icon),
            Some(20.0),
            None,
            ButtonRole::Normal,
            enabled,
            false,
            false,
            Some(label.clone()),
            move || (command.action)(),
        );
        vec![AnyView::new(ButtonChrome {
            label: label.clone(),
            content: AnyView::new(button),
        })]
    }
}
crate::impl_primitive_view!(AppBarBack);

/// Fixed 44-point touch target carrying the back action's label.
struct ButtonChrome {
    label: String,
    content: AnyView,
}

impl PrimitiveView for ButtonChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: self.label.clone(),
            ..Default::default()
        }));
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_width(Some(44.0));
        layout.set_min_height(Some(44.0));
        layout.set_flex_shrink(0.0);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_width(Some(44.0));
        layout.set_min_height(Some(44.0));
        layout.set_flex_shrink(0.0);
    }

    fn update_node(&self, node: &Node) {
        let _ = theme_for_node(node);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(ButtonChrome);
