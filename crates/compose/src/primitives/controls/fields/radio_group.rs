//! Translation of Primitives/Controls/Fields/RadioGroup.swift.
//!
//! Controlled selection with one Tab stop and wrapping arrow-key navigation.
use std::rc::Rc;

use crate::modifiers::layout::{FlexExt, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::button::{Button, ButtonRole};
use crate::primitives::overlays::roving_focus::{RovingFocusHost, RovingItem, roving_identity};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::{ButtonStyle, ButtonStyleConfiguration, ButtonStyleViewExt};
use crate::theme::{SemanticFontRef, theme_for_node};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, ViewExt,
    impl_primitive_view,
};
use cove_core::scene::{
    AccessibilityRole, Align, FlexDirection, FocusChainHolder, Justify, LayoutNode, Node, Rect,
    Wrap,
};
use cove_core::{Binding, Color};

/// Diameter of the radio mark.
const MARK_EDGE: f32 = 16.0;

/// One labelled option.
#[derive(Clone, Debug, PartialEq)]
pub struct RadioOption<Value: PartialEq + Clone> {
    pub value: Value,
    pub title: String,
    pub detail: String,
    pub is_enabled: bool,
}

impl<Value: PartialEq + Clone> RadioOption<Value> {
    pub fn new(value: Value, title: impl Into<String>) -> Self {
        Self {
            value,
            title: title.into(),
            detail: String::new(),
            is_enabled: true,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

/// Controlled single-selection group.
pub struct RadioGroup<Value: PartialEq + Clone + std::hash::Hash + Send + Sync + 'static> {
    pub selection: Binding<Option<Value>>,
    pub options: Vec<RadioOption<Value>>,
    pub is_enabled: bool,
    pub is_horizontal: bool,
}

impl<Value: PartialEq + Clone + std::hash::Hash + Send + Sync + 'static> RadioGroup<Value> {
    pub fn new(selection: Binding<Option<Value>>, options: Vec<RadioOption<Value>>) -> Self {
        assert!(
            options.iter().enumerate().all(|(index, option)| options
                .iter()
                .enumerate()
                .all(|(other, candidate)| other == index || candidate.value != option.value)),
            "Radio option values must be unique"
        );
        Self {
            selection,
            options,
            is_enabled: true,
            is_horizontal: false,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn horizontal(mut self, is_horizontal: bool) -> Self {
        self.is_horizontal = is_horizontal;
        self
    }

    fn enabled_values(&self) -> Vec<Value> {
        self.options
            .iter()
            .filter(|option| self.is_enabled && option.is_enabled)
            .map(|option| option.value.clone())
            .collect()
    }
}

impl<Value: PartialEq + Clone + std::hash::Hash + Send + Sync + 'static> View
    for RadioGroup<Value>
{
    fn body(&self) -> AnyView {
        let enabled = self.enabled_values();
        let selection = self.selection.clone();
        let tab_stop = self
            .selection
            .get()
            .filter(|selected| enabled.contains(selected))
            .or_else(|| enabled.first().cloned());
        let mut options: Vec<AnyView> = Vec::new();
        for option in &self.options {
            let is_selected = self.selection.get().as_ref() == Some(&option.value);
            let is_option_enabled = self.is_enabled && option.is_enabled;
            let option_value = option.value.clone();
            let detail = option.detail.clone();
            let title = option.title.clone();
            let mut label: Vec<AnyView> = vec![AnyView::new(
                Text::new(option.title.clone()).semantic_font(SemanticFontRef::label()),
            )];
            if !option.detail.is_empty() {
                label.push(AnyView::new(
                    Text::new(option.detail.clone())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(crate::theme::SemanticColorRef::on_surface_muted()),
                ));
            }
            let content = AnyView::new(Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(vec![
                    AnyView::new(RadioMark {
                        is_selected,
                        is_enabled: is_option_enabled,
                    }),
                    AnyView::new(BoxView::new(
                        FlexDirection::Column,
                        Align::FlexStart,
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        3.0,
                        AnyView::new(TupleView::new(label)),
                    )),
                ])),
            ))
            .modifier(RadioSemantics { title, detail });
            let selection_for_button = selection.clone();
            let button = Button::with_options(
                content,
                move || {
                    if is_option_enabled {
                        selection_for_button.set(Some(option_value.clone()));
                    }
                },
                ButtonRole::Normal,
                is_option_enabled,
                false,
                is_selected,
                None,
            )
            .button_style(RadioItemStyle);
            let identity: Rc<dyn Fn(&Value) -> u64> =
                Rc::new(|value: &Value| roving_identity(value));
            let is_tab_stop = tab_stop.as_ref() == Some(&option.value);
            let item = AnyView::new(button.modifier(RovingItem::new(
                option.value.clone(),
                is_tab_stop,
                identity,
            )));
            options.push(AnyView::new(item.flex_with(1.0, 1.0, Some(0.0))));
        }
        let selection_for_get = selection.clone();
        let selection_for_set = selection.clone();
        let enabled_for_host = enabled.clone();
        let identity: Rc<dyn Fn(&Value) -> u64> = Rc::new(|value: &Value| roving_identity(value));
        AnyView::new(RovingFocusHost::new(
            enabled,
            move || selection_for_get.get(),
            move |value: Value| {
                if enabled_for_host.contains(&value) {
                    selection_for_set.set(Some(value));
                }
            },
            move |value: &Value| identity(value),
            AnyView::new(
                BoxView::new(
                    if self.is_horizontal {
                        FlexDirection::Row
                    } else {
                        FlexDirection::Column
                    },
                    if self.is_horizontal {
                        Align::Center
                    } else {
                        Align::Stretch
                    },
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    8.0,
                    AnyView::new(TupleView::new(options)),
                )
                .frame_min_width(0.0),
            ),
        ))
    }
}

/// Radio role and label applied to the option button.
struct RadioSemantics {
    title: String,
    detail: String,
}

impl cove_composition::ViewModifier for RadioSemantics {
    fn apply_node(&self, node: &Node) {
        let mut semantics = node.accessibility().unwrap_or_else(|| {
            cove_core::scene::AccessibilitySemantics::new(AccessibilityRole::Radio)
        });
        semantics.role = AccessibilityRole::Radio;
        semantics.label = self.title.clone();
        semantics.help = self.detail.clone();
        node.set_accessibility(Some(semantics));
    }
}

/// Row chrome for one radio option.
struct RadioItemStyle;

impl ButtonStyle for RadioItemStyle {
    fn make_body(&self, configuration: ButtonStyleConfiguration) -> AnyView {
        let theme = configuration.theme.clone();
        let fill = if configuration.is_hovered {
            theme.colors.state_layer_hover
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
                AnyView::new(configuration.label.clone()),
            )
            .flex_with(1.0, 1.0, None)
            .padding(7.0)
            .foreground_color(if configuration.is_enabled {
                theme.colors.on_surface
            } else {
                theme.colors.on_surface_muted
            })
            .background_color(fill)
            .corner_radius(theme.radius.sm)
            .border_color(
                if configuration.is_focused {
                    theme.colors.focus_ring
                } else {
                    Color::CLEAR
                },
                if configuration.is_focused { 2.0 } else { 0.0 },
            ),
        )
    }
}

/// The radio mark: white face, ring, and an accent dot when selected.
struct RadioMark {
    is_selected: bool,
    is_enabled: bool,
}

impl PrimitiveView for RadioMark {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_width(Some(MARK_EDGE));
        layout.set_height(Some(MARK_EDGE));
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let weak = node.downgrade();
        let is_selected = self.is_selected;
        let is_enabled = self.is_enabled;
        node.update_draw((is_selected, is_enabled), move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let colors = theme_for_node(&owner).colors;
            let rect = Rect::new(origin.x, origin.y, MARK_EDGE, MARK_EDGE);
            // White face with a thin border, like the check box.
            let fill = if is_enabled {
                colors.background
            } else {
                colors.surface
            };
            let border = if !is_enabled {
                colors.on_surface.multiplied_alpha(0.3)
            } else if is_selected {
                colors.accent
            } else {
                colors.border
            };
            let opacity = owner.opacity();
            let _ = list.add_rounded_rect(rect, MARK_EDGE * 0.5, fill.multiplied_alpha(opacity));
            // The selected ring is 2 px; the empty ring is 1 px.
            let _ = list.add_rounded_rect_stroke(
                rect,
                MARK_EDGE * 0.5,
                if is_selected { 2.0 } else { 1.0 },
                border.multiplied_alpha(opacity),
            );
            if is_selected {
                let dot = if is_enabled {
                    colors.accent
                } else {
                    colors.on_surface_muted
                };
                let diameter = 6.0;
                let _ = list.add_rounded_rect(
                    Rect::new(
                        origin.x + (MARK_EDGE - diameter) * 0.5,
                        origin.y + (MARK_EDGE - diameter) * 0.5,
                        diameter,
                        diameter,
                    ),
                    diameter * 0.5,
                    dot.multiplied_alpha(opacity),
                );
            }
        });
    }
}
impl_primitive_view!(RadioMark);
