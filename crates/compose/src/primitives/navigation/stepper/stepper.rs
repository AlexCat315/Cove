//! A controlled workflow navigator. The caller owns selection, outcomes and
//! availability. Lock future steps with `StepperItem::is_enabled`; completing or
//! selecting a step never changes another step's status.
//!
//! Steps read as a sequence: a solid status marker, the title, and a connector
//! spanning the space to the next step. Selection emphasizes the marker and
//! title; it never paints a rectangle behind the whole step.
use crate::Scope;
use crate::primitives::{DuplicateId, check_ids};
use std::rc::Rc;

use crate::adaptive::ContainerSizeReader;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{EdgeInsets, FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource};
use crate::primitives::feedback::Spinner;
use crate::primitives::overlays::roving_focus::{
    RovingFocusHost, RovingItem, SelectionNavigation, roving_identity,
};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{
    ButtonInteractionKey, ButtonRole, ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt,
};
use crate::theme::{
    ControlSizeEnvironment, IconMetrics, SemanticColorRef, SemanticFontRef, Theme, theme_for_node,
};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{
    AccessibilityRole, Align, Animation, Edge, FlexDirection, Justify, LayoutNode, Node, Wrap,
    with_animation,
};
use cove_core::{Binding, Color};
use cove_platform::KeyEvent;

use super::values::{StepStatus, StepperAxis, StepperItem, StepperOptions};

/// A controlled workflow navigator.
pub struct Stepper<Id: std::hash::Hash + PartialEq + Clone + 'static> {
    pub(crate) items: Vec<StepperItem<Id>>,
    pub selection: Binding<Option<Id>>,
    pub options: StepperOptions,
}

impl<Id: std::hash::Hash + PartialEq + Clone + 'static> Stepper<Id> {
    pub fn try_new(
        items: Vec<StepperItem<Id>>,
        selection: Binding<Option<Id>>,
    ) -> Result<Self, DuplicateId>
    where
        Id: Eq,
    {
        check_ids(items.iter().map(|item| &item.id), "Stepper item")?;
        Ok(Self {
            items,
            selection,
            options: StepperOptions::default(),
        })
    }

    pub fn options(mut self, options: StepperOptions) -> Self {
        self.options = options;
        self
    }

    fn enabled(&self) -> Vec<Id> {
        if self.options.is_enabled {
            self.items
                .iter()
                .filter(|item| item.is_enabled)
                .map(|item| item.id.clone())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// A horizontal stepper only keeps its row while every step has its budget.
    /// Below that it stacks, so titles, detail text and the touch target stay
    /// readable instead of being compressed into unreadable cells.
    fn horizontal_budget(&self) -> f32 {
        let budget = self.options.minimum_horizontal_step_width.max(80.0);
        let connectors = self.items.len().saturating_sub(1) as f32 * 32.0;
        self.items.len() as f32 * budget + connectors
    }
}

/// Select `id` through the binding, honouring the controlled contract: a
/// disabled step never becomes selectable, and re-tapping the current step is
/// a no-op. Shared by the pointer and roving-keyboard paths.
fn select_step<Id: PartialEq + Clone + 'static>(
    enabled: &[Id],
    selection: &Binding<Option<Id>>,
    id: Id,
) {
    if !enabled.contains(&id) {
        return;
    }
    if selection.get().as_ref() == Some(&id) {
        return;
    }
    selection.set(Some(id));
}

impl<Id: std::hash::Hash + PartialEq + Clone + 'static> Stepper<Id> {
    fn content(&self, vertical: bool) -> AnyView {
        let enabled = self.enabled();
        let selection = self.selection.clone();
        let mut rows: Vec<AnyView> = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            let selected = self.selection.get().as_ref() == Some(&item.id);
            rows.push(self.step(
                item,
                index,
                vertical,
                selected,
                index == self.items.len() - 1,
            ));
        }
        let label = self.options.label.clone();
        let is_enabled = self.options.is_enabled;
        let count = self.items.len();
        let items_for_select = self.items.clone();
        let options = self.options.clone();
        let selection_for_select = self.selection.clone();
        let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
        let identity_for_host = identity.clone();
        let selection_for_host = selection.clone();
        let items_for_host = items_for_select.clone();
        let enabled_for_host = enabled.clone();
        let host = RovingFocusHost::new(
            enabled.clone(),
            move || selection_for_host.get(),
            move |id: Id| {
                select_step(&enabled_for_host, &selection_for_select, id);
            },
            move |id: &Id| identity_for_host(id),
            AnyView::new(BoxView::new(
                if vertical {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                // Each cell carries its own trailing connector, so cells sit
                // flush: the connector reaches the next marker without gaps.
                0.0,
                AnyView::new(TupleView::new(rows)),
            )),
        )
        .wraps(self.options.allows_wrap)
        .command({
            let stepper_navigation = self.navigation_snapshot();
            move |event: &KeyEvent| stepper_navigation(event, vertical)
        });
        let _ = (&items_for_host, selection, count, identity, options);
        AnyView::new(StepperListHost {
            content: AnyView::new(host),
            label,
            is_enabled,
        })
    }

    fn navigation_snapshot(&self) -> Rc<dyn Fn(&KeyEvent, bool) -> Option<SelectionNavigation>> {
        let minimum = self.options.minimum_horizontal_step_width;
        let allows_wrap = self.options.allows_wrap;
        let count = self.items.len();
        let _ = (minimum, allows_wrap, count);
        Rc::new(|event: &KeyEvent, vertical: bool| {
            if !event.modifiers.is_empty() {
                return None;
            }
            match event.scancode {
                crate::Scancode::HOME => Some(SelectionNavigation::First),
                crate::Scancode::END => Some(SelectionNavigation::Last),
                crate::Scancode::ARROW_LEFT if !vertical => Some(SelectionNavigation::Previous),
                crate::Scancode::ARROW_RIGHT if !vertical => Some(SelectionNavigation::Next),
                crate::Scancode::ARROW_UP if vertical => Some(SelectionNavigation::Previous),
                crate::Scancode::ARROW_DOWN if vertical => Some(SelectionNavigation::Next),
                _ => None,
            }
        })
    }

    fn step(
        &self,
        item: &StepperItem<Id>,
        index: usize,
        vertical: bool,
        selected: bool,
        is_last: bool,
    ) -> AnyView {
        let count = self.items.len();
        let is_enabled = self.options.is_enabled && item.is_enabled;
        let tooltip = format!(
            "Step {} of {} · {}",
            index + 1,
            count,
            item.status.description()
        );
        let marker = AnyView::new(
            AnyView::new(StepperMarker {
                number: index + 1,
                status: item.status,
                is_selected: selected,
            })
            .accessibility_hidden(true),
        );
        let title = Text::new(item.title.clone()).semantic_font(SemanticFontRef::body_strong());
        let mut labels: Vec<AnyView> = vec![if selected {
            AnyView::new(title.semantic_foreground(SemanticColorRef::accent()))
        } else {
            AnyView::new(title)
        }];
        if !item.detail.is_empty() {
            labels.push(AnyView::new(
                Text::new(item.detail.clone())
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }
        let texts = BoxView::new(
            FlexDirection::Column,
            Align::Stretch,
            Justify::FlexStart,
            Wrap::NoWrap,
            3.0,
            AnyView::new(TupleView::new(labels)),
        );
        let label = if vertical {
            // The rail stretches to the text height, so the line runs from the
            // bottom of this marker to the top of the next one.
            let mut rail_children = vec![marker];
            if !is_last {
                rail_children.push(connector_line(item.status, true));
            }
            let rail = BoxView::new(
                FlexDirection::Column,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(rail_children)),
            );
            let texts = if is_last {
                AnyView::new(texts)
            } else {
                AnyView::new(texts.padding_insets(EdgeInsets::new(0.0, 0.0, 16.0, 0.0)))
            };
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(rail),
                    AnyView::new(texts.flex_with(1.0, 1.0, Some(0.0)).frame_min_width(0.0)),
                ])),
            ))
        } else {
            let mut row_children = vec![
                marker,
                AnyView::new(texts.flex_with(0.0, 1.0, None).frame_min_width(0.0)),
            ];
            if !is_last {
                row_children.push(connector_line(item.status, false));
            }
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                8.0,
                AnyView::new(TupleView::new(row_children)),
            ))
        };
        let id = item.id.clone();
        let selection = self.selection.clone();
        let enabled = self.enabled();
        let button = Button::with_options(
            label,
            move || {
                select_step(&enabled, &selection, id.clone());
            },
            ButtonRole::Normal,
            is_enabled,
            false,
            selected,
            Some(tooltip.clone()),
        )
        .button_style(StepperButtonStyle { vertical });
        let value = format!(
            "Step {} of {} · {}",
            index + 1,
            count,
            item.status.description()
        );
        let identity: Rc<dyn Fn(&Id) -> u64> = Rc::new(|id: &Id| roving_identity(id));
        let tab_stop = self
            .selection
            .get()
            .filter(|selected| self.enabled().contains(selected))
            .or_else(|| self.enabled().first().cloned())
            .as_ref()
            .is_some_and(|first| first == &item.id);
        let item_id = item.id.clone();
        let identity_for_item = identity.clone();
        AnyView::new(
            button
                .modifier(RovingItem::new(
                    item_id.clone(),
                    tab_stop,
                    identity_for_item,
                ))
                .modifier(StepItemSemantics {
                    value: value.clone(),
                })
                .flex_with(
                    if vertical { 0.0 } else { 1.0 },
                    if vertical { 0.0 } else { 1.0 },
                    if vertical { None } else { Some(0.0) },
                ),
        )
    }
}

/// The connector trailing a step, inside the step's own cell so it spans the
/// real gap to the next marker instead of floating as a fixed-length dash.
/// A step's outcome colors the line leaving it: success green after a completed
/// step, error red after a failed one.
fn connector_line(status: StepStatus, vertical: bool) -> AnyView {
    AnyView::new(AnyView::new(StepperConnector { status, vertical }).accessibility_hidden(true))
}

struct StepperConnector {
    status: StepStatus,
    vertical: bool,
}

impl PrimitiveView for StepperConnector {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let color = match self.status {
            StepStatus::Completed => theme.colors.success,
            StepStatus::Failed => theme.colors.error,
            _ => theme.colors.border,
        };
        node.set_background_color(Some(color));
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        layout.set_flex_basis(0.0);
        if self.vertical {
            layout.set_width(Some(1.0));
        } else {
            layout.set_height(Some(1.0));
            layout.set_min_width(Some(24.0));
        }
    }
}
impl_primitive_view!(StepperConnector);

impl<Id: std::hash::Hash + PartialEq + Clone + 'static> Stepper<Id> {
    fn snapshot(&self) -> Stepper<Id> {
        Stepper {
            items: self.items.clone(),
            selection: self.selection.clone(),
            options: self.options.clone(),
        }
    }
}

impl<Id: std::hash::Hash + PartialEq + Clone + 'static> View for Stepper<Id> {
    fn body(&self, _cx: &Scope) -> AnyView {
        if self.options.axis == StepperAxis::Vertical {
            return self.content(true);
        }
        // Two retained branches keep each layout tree stable: switching the
        // container's direction in place would restyle every cell along the
        // way and animate a layout that just changed shape.
        let budget = self.horizontal_budget();
        let wide = Rc::new(self.snapshot().content(false));
        let compact = Rc::new(self.snapshot().content(true));
        AnyView::new(
            ContainerSizeReader::new(move |size: cove_core::Size| {
                let use_wide = width_allows_horizontal(size.width, budget);
                AnyView::new(if use_wide {
                    (*wide).clone()
                } else {
                    (*compact).clone()
                })
            })
            .frame_min_width(0.0),
        )
    }
}

/// A horizontal stepper keeps its row only while the measured width covers the
/// per-step budget plus connectors.
fn width_allows_horizontal(width: f32, budget: f32) -> bool {
    !width.is_finite() || width <= 0.0 || width >= budget
}

struct StepperButtonStyle {
    vertical: bool,
}

impl ButtonStyle for StepperButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        AnyView::new(StepperChrome {
            label: configuration.label.clone(),
            theme: configuration.theme.clone(),
            vertical: self.vertical,
            is_pressed: configuration.is_pressed,
            is_hovered: configuration.is_hovered,
            is_focused: configuration.is_focused,
            is_enabled: configuration.is_enabled,
            interaction: configuration.interaction_key(),
        })
    }

    fn style_identity(&self) -> u64 {
        self.vertical as u64
    }
}

const STEPPER_KEY: &str = "stepper.interaction";

/// Interaction chrome only: hover/press state layers and the keyboard focus
/// ring, animated with the theme's fast motion. The resting selected step has
/// no fill of its own — the marker and title carry the emphasis.
struct StepperChrome {
    label: AnyView,
    theme: Theme,
    vertical: bool,
    is_pressed: bool,
    is_hovered: bool,
    is_focused: bool,
    is_enabled: bool,
    interaction: ButtonInteractionKey,
}

impl PrimitiveView for StepperChrome {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn update_node(&self, node: &Node) {
        let colors = &self.theme.colors;
        let fill = if self.is_pressed {
            colors.state_layer_pressed
        } else if self.is_hovered {
            colors.state_layer_hover
        } else {
            Color::CLEAR
        };
        let border = if self.is_focused {
            colors.focus_ring
        } else {
            Color::CLEAR
        };
        let border_width = if self.is_focused { 1.0 } else { 0.0 };
        let animate = node
            .attachment::<ButtonInteractionKey>(STEPPER_KEY)
            .is_some_and(|previous| previous != self.interaction);

        let apply = || {
            let owner = node.clone();
            let current = node.background_color();
            node.animatable_set_optional(
                "stepper.backgroundColor",
                current,
                Some(fill),
                move |value| {
                    owner.set_background_color(value);
                },
            );
            let owner = node.clone();
            let current = node.border_color();
            node.animatable_set_optional(
                "stepper.borderColor",
                current,
                Some(border),
                move |value| {
                    owner.set_border_color(value);
                },
            );
            let owner = node.clone();
            let current = node.border_width();
            node.animatable_set_with("stepper.borderWidth", current, border_width, move |value| {
                owner.set_border_width(value);
            });
            node.set_corner_radius(self.theme.radius.md);
            node.set_opacity(if self.is_enabled { 1.0 } else { 0.5 });
        };

        if animate {
            with_animation(
                Some(Animation::new(
                    self.theme.motion.fast,
                    self.theme.motion.standard_easing.as_animation_curve(),
                )),
                apply,
            );
        } else {
            apply();
        }
        node.set_attachment(STEPPER_KEY, self.interaction);
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        layout.set_flex_grow(1.0);
        layout.set_flex_shrink(1.0);
        // Auto basis: a stacked cell's height comes from its content, so a zero
        // basis would collapse the step onto the ones below it.
        layout.set_flex_basis_auto();
        layout.set_min_width(Some(0.0));
        // Vertical cells pad only horizontally: the rail must reach the cell
        // edges so consecutive markers connect without a break.
        let insets = if self.vertical {
            EdgeInsets::horizontal_vertical(8.0, 0.0)
        } else {
            EdgeInsets::all(8.0)
        };
        layout.set_padding(insets.top, Edge::Top);
        layout.set_padding(insets.leading, Edge::Start);
        layout.set_padding(insets.bottom, Edge::Bottom);
        layout.set_padding(insets.trailing, Edge::End);
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let foreground = if self.is_enabled {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_disabled()
        };
        vec![AnyView::new(
            self.label.clone().semantic_foreground(foreground),
        )]
    }
}

impl_primitive_view!(StepperChrome);

/// Row semantics applied after the roving modifier, so the item identity and
/// the list-item role both land on the button node.
struct StepItemSemantics {
    value: String,
}

impl cove_composition::ViewModifier for StepItemSemantics {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node.accessibility().unwrap_or_else(|| {
            cove_core::scene::AccessibilitySemantics::new(AccessibilityRole::ListItem)
        });
        semantics.role = AccessibilityRole::ListItem;
        semantics.value = self.value.clone();
        node.set_accessibility(Some(semantics));
    }
}

/// Pass-through container that owns the list semantics for the roving host.
/// Wrapping an `AnyView` in another `AnyView` would drop the modifier, so the
/// semantics live on this node instead.
struct StepperListHost {
    content: AnyView,
    label: String,
    is_enabled: bool,
}

impl PrimitiveView for StepperListHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(cove_core::scene::AccessibilitySemantics {
            role: AccessibilityRole::List,
            label: self.label.clone(),
            state: cove_core::scene::AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
impl_primitive_view!(StepperListHost);

struct StepperMarker {
    number: usize,
    status: StepStatus,
    is_selected: bool,
}

impl StepperMarker {
    fn diameter(node: &Node) -> f32 {
        let size = node.composition_value(&ControlSizeEnvironment::key());
        IconMetrics::new(node).size(size.button_height())
    }

    /// (fill, border, glyph). Finished outcomes and the current step are solid
    /// discs with a light glyph; waiting steps are quiet outlined circles.
    fn palette(
        node: &Node,
        status: StepStatus,
        is_selected: bool,
    ) -> (Color, Option<Color>, Color) {
        let theme = theme_for_node(node);
        let colors = &theme.colors;
        match status {
            StepStatus::Completed => (colors.success, None, colors.on_accent),
            StepStatus::Failed => (colors.error, None, colors.on_accent),
            StepStatus::InProgress => (colors.accent, None, colors.on_accent),
            StepStatus::Pending if is_selected => (colors.accent, None, colors.on_accent),
            StepStatus::Pending => (
                Color::CLEAR,
                Some(colors.border_strong),
                colors.on_surface_muted,
            ),
        }
    }
}

const MARKER_KEY: &str = "stepper.marker";

impl PrimitiveView for StepperMarker {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let diameter = Self::diameter(node);
        let (fill, border, _) = Self::palette(node, self.status, self.is_selected);
        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(diameter));
            layout.set_height(Some(diameter));
            layout.set_flex_shrink(0.0);
            layout.set_align_items(Align::Center);
            layout.set_justify_content(Justify::Center);
        }
        node.set_corner_radius(diameter / 2.0);
        let state_key = ((self.status as u64) << 1) | self.is_selected as u64;
        let animate = node
            .attachment::<u64>(MARKER_KEY)
            .is_some_and(|previous| previous != state_key);
        let border_width = if border.is_some() { 1.0 } else { 0.0 };
        let apply = || {
            let owner = node.clone();
            let current = node.background_color();
            node.animatable_set_optional(
                "stepper.marker.backgroundColor",
                current,
                Some(fill),
                move |value| {
                    owner.set_background_color(value);
                },
            );
            let owner = node.clone();
            let current = node.border_color();
            node.animatable_set_optional(
                "stepper.marker.borderColor",
                current,
                border,
                move |value| {
                    owner.set_border_color(value);
                },
            );
            let owner = node.clone();
            let current = node.border_width();
            node.animatable_set_with(
                "stepper.marker.borderWidth",
                current,
                border_width,
                move |value| {
                    owner.set_border_width(value);
                },
            );
        };
        if animate {
            let theme = theme_for_node(node);
            with_animation(
                Some(Animation::new(
                    theme.motion.fast,
                    theme.motion.standard_easing.as_animation_curve(),
                )),
                apply,
            );
        } else {
            apply();
        }
        node.set_attachment(MARKER_KEY, state_key);
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let diameter = Self::diameter(node);
        let (_, _, color) = Self::palette(node, self.status, self.is_selected);
        let content = match self.status {
            StepStatus::Completed => AnyView::new(ButtonIcon::new(
                ButtonIconSource::resource(
                    crate::primitives::media::ui_icon_catalog::action::check(),
                ),
                Some(diameter * 0.5),
                Some(color),
            )),
            StepStatus::Failed => AnyView::new(ButtonIcon::new(
                ButtonIconSource::resource(crate::primitives::media::ui_icon_catalog::action::x()),
                Some(diameter * 0.5),
                Some(color),
            )),
            StepStatus::InProgress => AnyView::new(
                AnyView::new(Spinner::new().size(diameter * 0.55)).foreground_color(color),
            ),
            StepStatus::Pending => AnyView::new(
                Text::new(self.number.to_string())
                    .semantic_font(SemanticFontRef::label())
                    .foreground_color(color),
            ),
        };
        vec![content]
    }
}
impl_primitive_view!(StepperMarker);
