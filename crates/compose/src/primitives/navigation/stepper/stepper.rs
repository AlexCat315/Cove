//! Translation of Primitives/Navigation/Stepper/Stepper.swift.
//!
//! A controlled workflow navigator. The caller owns selection, outcomes and
//! availability. Lock future steps with `StepperItem::is_enabled`; completing or
//! selecting a step never changes another step's status.
use crate::Scope;
use crate::primitives::{DuplicateId, check_ids};
use std::rc::Rc;

use crate::adaptive::ContainerSizeReader;
use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource};
use crate::primitives::feedback::Spinner;
use crate::primitives::overlays::roving_focus::{
    RovingFocusHost, RovingItem, SelectionNavigation, roving_identity,
};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{
    ButtonRole, ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt,
};
use crate::theme::{
    ControlSizeEnvironment, IconMetrics, SemanticColorRef, SemanticFontRef, Theme, ThemeReader,
    theme_for_node,
};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, ViewExt, impl_primitive_view};
use cove_core::scene::{AccessibilityRole, Align, FlexDirection, Justify, LayoutNode, Node, Wrap};
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

    #[expect(dead_code, reason = "ported from Swift but not wired in yet")]
    fn select(&self, id: &Id) {
        if !self.enabled().contains(id) {
            return;
        }
        if self.selection.get().as_ref() == Some(id) {
            return;
        }
        self.selection.set(Some(id.clone()));
    }

    fn content(&self, vertical: bool) -> AnyView {
        let enabled = self.enabled();
        let selection = self.selection.clone();
        let mut rows: Vec<AnyView> = Vec::new();
        for (index, item) in self.items.iter().enumerate() {
            let selected = self.selection.get().as_ref() == Some(&item.id);
            rows.push(self.step(item, index, vertical, selected));
            if index < self.items.len() - 1 {
                rows.push(self.connector(item, vertical));
            }
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
                if enabled_for_host.contains(&id)
                    && selection_for_select.get().as_ref() != Some(&id)
                {
                    selection_for_select.set(Some(id));
                }
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
                if vertical { 4.0 } else { 8.0 },
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
        let mut labels: Vec<AnyView> = vec![AnyView::new(
            Text::new(item.title.clone()).semantic_font(SemanticFontRef::body_strong()),
        )];
        if !item.detail.is_empty() {
            labels.push(AnyView::new(
                Text::new(item.detail.clone())
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            ));
        }
        let label = AnyView::new(BoxView::new(
            FlexDirection::Row,
            Align::Center,
            Justify::FlexStart,
            Wrap::NoWrap,
            10.0,
            AnyView::new(TupleView::new(vec![
                marker,
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        3.0,
                        AnyView::new(TupleView::new(labels)),
                    )
                    .flex_with(1.0, 1.0, Some(0.0))
                    .frame_min_width(0.0),
                ),
            ])),
        ));
        let id = item.id.clone();
        let selection = self.selection.clone();
        let enabled = self.enabled();
        let button = Button::with_options(
            label,
            move || {
                if enabled.contains(&id) && selection.get().as_ref() != Some(&id) {
                    selection.set(Some(id.clone()));
                }
            },
            ButtonRole::Normal,
            is_enabled,
            false,
            selected,
            Some(tooltip.clone()),
        )
        .button_style(StepperButtonStyle);
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

    fn connector(&self, item: &StepperItem<Id>, vertical: bool) -> AnyView {
        let status = item.status;
        AnyView::new(ThemeReader::new(move |theme: &Theme| {
            let color = if status == StepStatus::Completed {
                theme.colors.success
            } else {
                theme.colors.border
            };
            if !vertical {
                AnyView::new(
                    BoxView::new(
                        FlexDirection::Column,
                        Align::Stretch,
                        Justify::Center,
                        Wrap::NoWrap,
                        0.0,
                        AnyView::new(
                            BoxView::new(
                                FlexDirection::Row,
                                Align::Center,
                                Justify::FlexStart,
                                Wrap::NoWrap,
                                0.0,
                                AnyView::new(crate::EmptyView),
                            )
                            .frame_size(24.0, 1.0)
                            .background_color(color),
                        ),
                    )
                    .accessibility_hidden(true),
                )
            } else {
                AnyView::new(
                    AnyView::new(StepperVerticalConnector { color }).accessibility_hidden(true),
                )
            }
        }))
    }
}

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
        // Two retained branches keep each layout tree stable. Mutating one row's
        // direction in place lets Yoga keep the old row height when the narrow
        // branch activates, which collapses the step buttons.
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

struct StepperButtonStyle;

impl ButtonStyle for StepperButtonStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let colors = configuration.theme.colors;
        let fill = if configuration.is_pressed {
            colors.state_layer_pressed
        } else if configuration.is_hovered {
            colors.state_layer_hover
        } else if configuration.is_selected {
            colors.accent.multiplied_alpha(0.08)
        } else {
            Color::CLEAR
        };
        AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(configuration.label.clone().flex_with(1.0, 1.0, Some(0.0))),
            )
            .flex_with(1.0, 1.0, Some(0.0))
            .padding(8.0)
            .background_color(fill)
            .corner_radius(configuration.theme.radius.md)
            .foreground_color(if configuration.is_enabled {
                colors.on_surface
            } else {
                colors.on_surface_muted
            })
            .border_color(
                if configuration.is_focused {
                    colors.focus_ring
                } else {
                    Color::CLEAR
                },
                1.0,
            )
            .opacity(if configuration.is_enabled { 1.0 } else { 0.5 }),
        )
    }
}

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

struct StepperVerticalConnector {
    color: Color,
}

impl PrimitiveView for StepperVerticalConnector {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        Some(LayoutNode::new())
    }

    fn update_node(&self, node: &Node) {
        let size = node.composition_value(&ControlSizeEnvironment::key());
        let metrics = IconMetrics::new(node);
        if let Some(layout) = node.layout_node() {
            layout.set_height(Some(16.0));
            layout.set_flex_direction(FlexDirection::Row);
            layout.set_padding(
                9.0 + metrics.size(size.button_height()) / 2.0,
                cove_core::scene::Edge::Start,
            );
        }
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(crate::EmptyView),
            )
            .frame_size(1.0, 16.0)
            .background_color(self.color),
        )]
    }
}
impl_primitive_view!(StepperVerticalConnector);

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

    fn color(node: &Node, status: StepStatus, is_selected: bool) -> Color {
        let theme = theme_for_node(node);
        if is_selected && status == StepStatus::Pending {
            theme.colors.accent
        } else {
            status.tone().color().resolve(&theme)
        }
    }
}

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
        let color = Self::color(node, self.status, self.is_selected);
        if let Some(layout) = node.layout_node() {
            layout.set_width(Some(diameter));
            layout.set_height(Some(diameter));
            layout.set_flex_shrink(0.0);
            layout.set_align_items(Align::Center);
            layout.set_justify_content(Justify::Center);
        }
        node.set_corner_radius(diameter / 2.0);
        node.set_background_color(Some(color.multiplied_alpha(if self.is_selected {
            0.16
        } else {
            0.09
        })));
        node.set_border_color(Some(color));
        node.set_border_width(if self.is_selected { 1.5 } else { 1.0 });
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let diameter = Self::diameter(node);
        let color = Self::color(node, self.status, self.is_selected);
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
