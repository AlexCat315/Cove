//! Translation of Primitives/Data/PropertyGrid.swift.
//!
//! Two-column inspector grid with collapsible section headers. The call site
//! owns the value controls; the primitive only handles layout.
//!
//! Behaviour checklist:
//! - a top-level section is one borderless card; child sections sit flat inside
//!   their parent card
//! - the header shows a chevron only when the section is collapsible, and a
//!   badge or a row count otherwise
//! - an adaptive row places its label beside the value only when both fit
//! - a fixed row pins the value slot to the row height; an intrinsic row lets a
//!   disclosure editor grow and shrink
//! - the controlled `collapsed_section_ids` wins over the local state, which
//!   wins over `starts_collapsed`
//! - rows are only materialised for visible rows; a collapsed section's rows
//!   are not built

use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

use crate::modifiers::interaction::AccessibilityExt;
use crate::modifiers::layout::{EdgeInsets, FlexExt, FrameDimension, LayoutFrameExt, PaddingExt};
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::Button;
use crate::primitives::controls::form::label_placement::FieldLabelPlacement;
use crate::primitives::layout::{ScrollAxis, ScrollView, ScrollbarGutter};
use crate::primitives::media::{Icon, ui_icon_catalog};
use crate::primitives::text::Text;
use crate::theme::styles::buttons::ButtonStyleViewExt;
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View};
use cove_composition::ViewExt;
use cove_core::State;
use cove_core::scene::{
    AccessibilitySemantics, Align, FlexDirection, HoverPhase, InteractionRegistryHolder, Justify,
    LayoutNode, Node, Wrap,
};

const ROW_HOVERED: &str = "__property_grid_row_hovered";

/// How a row's label and value are arranged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PropertyGridRowLayout {
    /// Label beside the value when both fit, stacked otherwise.
    #[default]
    TwoColumn,
    /// Label above the value.
    FullWidth,
}

/// How a row sizes its value slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PropertyGridRowSizing {
    /// Fix the value slot to the scaled row height and platform control
    /// minimum. An adaptive stacked label adds its own height above that slot.
    Fixed,
    /// Measure the value's content. `rowHeight` becomes a minimum, allowing
    /// disclosure editors to grow and shrink without an external formula.
    #[default]
    Intrinsic,
}

/// Which scrollbars the grid offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PropertyGridScrollAxes {
    Vertical,
    Horizontal,
    #[default]
    Both,
}

/// One inspector row: a label plus the caller's value control.
#[derive(Clone)]
pub struct PropertyGridRow {
    pub id: String,
    pub label: String,
    pub row_height: Option<f32>,
    pub layout: PropertyGridRowLayout,
    pub sizing: PropertyGridRowSizing,
    value: AnyView,
}

impl PropertyGridRow {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        value: impl View + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            row_height: None,
            layout: PropertyGridRowLayout::TwoColumn,
            sizing: PropertyGridRowSizing::Intrinsic,
            value: AnyView::new(value),
        }
    }

    pub fn row_height(mut self, row_height: Option<f32>) -> Self {
        self.row_height = row_height;
        self
    }

    pub fn layout(mut self, layout: PropertyGridRowLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn sizing(mut self, sizing: PropertyGridRowSizing) -> Self {
        self.sizing = sizing;
        self
    }

    pub fn value(&self) -> &AnyView {
        &self.value
    }
}

/// One inspector section, optionally nested and collapsible.
#[derive(Clone)]
pub struct PropertyGridSection {
    pub id: String,
    pub title: String,
    pub rows: Vec<PropertyGridRow>,
    pub children: Vec<PropertyGridSection>,
    pub header_leading: Option<AnyView>,
    pub header_trailing: Option<AnyView>,
    pub footer: Option<AnyView>,
    pub badge: Option<String>,
    pub shows_row_count: bool,
    /// When `true` the header renders a collapse chevron and rows can be
    /// hidden; `false` disables the affordance entirely.
    pub is_collapsible: bool,
    /// Initial collapse state; only relevant when `is_collapsible` is `true`.
    pub starts_collapsed: bool,
}

impl PropertyGridSection {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        rows: Vec<PropertyGridRow>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            rows,
            children: Vec::new(),
            header_leading: None,
            header_trailing: None,
            footer: None,
            badge: None,
            shows_row_count: true,
            is_collapsible: false,
            starts_collapsed: false,
        }
    }

    pub fn children(mut self, children: Vec<PropertyGridSection>) -> Self {
        self.children = children;
        self
    }

    pub fn header_leading(mut self, leading: impl View + 'static) -> Self {
        self.header_leading = Some(AnyView::new(leading));
        self
    }

    pub fn header_trailing(mut self, trailing: impl View + 'static) -> Self {
        self.header_trailing = Some(AnyView::new(trailing));
        self
    }

    pub fn footer(mut self, footer: impl View + 'static) -> Self {
        self.footer = Some(AnyView::new(footer));
        self
    }

    pub fn badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn row_count(mut self, shows_row_count: bool) -> Self {
        self.shows_row_count = shows_row_count;
        self
    }

    pub fn collapsible(mut self, is_collapsible: bool) -> Self {
        self.is_collapsible = is_collapsible;
        self
    }

    pub fn starts_collapsed(mut self, starts_collapsed: bool) -> Self {
        self.starts_collapsed = starts_collapsed;
        self
    }

    /// The header's secondary text: the badge, else the row count.
    pub fn secondary_text(&self) -> Option<String> {
        match &self.badge {
            Some(badge) => Some(badge.clone()),
            None if self.shows_row_count && !self.rows.is_empty() => {
                Some(self.rows.len().to_string())
            }
            None => None,
        }
    }

    /// Whether this section draws a value for `collapsed` and the controlled
    /// set, following controlled → local → authored order.
    pub fn is_collapsed(
        &self,
        controlled: Option<&HashSet<String>>,
        local: &BTreeMap<String, bool>,
    ) -> bool {
        match controlled {
            Some(ids) => ids.contains(&self.id),
            None => local
                .get(&self.id)
                .copied()
                .unwrap_or(self.starts_collapsed),
        }
    }
}

/// Two-column inspector grid with collapsible section headers.
pub struct PropertyGrid {
    pub sections: Vec<PropertyGridSection>,
    pub label_width: f32,
    pub min_value_width: f32,
    pub label_placement: FieldLabelPlacement,
    pub row_height: f32,
    pub row_spacing: f32,
    pub section_spacing: f32,
    pub content_padding: f32,
    pub scroll_axes: PropertyGridScrollAxes,
    pub empty_text: String,
    /// When provided, expansion is controlled by the caller rather than the
    /// grid's local state.
    pub collapsed_section_ids: Option<HashSet<String>>,
    pub on_section_collapse_changed: Option<Rc<dyn Fn(&str, bool)>>,
    local_collapsed: State<BTreeMap<String, bool>>,
}

impl PropertyGrid {
    pub fn new(sections: Vec<PropertyGridSection>) -> Self {
        Self {
            sections,
            label_width: 96.0,
            min_value_width: 220.0,
            label_placement: FieldLabelPlacement::Adaptive,
            row_height: 24.0,
            row_spacing: 1.0,
            section_spacing: 10.0,
            content_padding: 8.0,
            scroll_axes: PropertyGridScrollAxes::Both,
            empty_text: "No properties".to_string(),
            collapsed_section_ids: None,
            on_section_collapse_changed: None,
            local_collapsed: State::new(BTreeMap::new()),
        }
    }

    pub fn label_width(mut self, label_width: f32) -> Self {
        self.label_width = if label_width.is_finite() {
            label_width.max(0.0)
        } else {
            96.0
        };
        self
    }

    pub fn min_value_width(mut self, min_value_width: f32) -> Self {
        self.min_value_width = if min_value_width.is_finite() {
            min_value_width.max(1.0)
        } else {
            220.0
        };
        self
    }

    pub fn label_placement(mut self, label_placement: FieldLabelPlacement) -> Self {
        self.label_placement = label_placement;
        self
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = if row_height.is_finite() {
            row_height
        } else {
            24.0
        };
        self
    }

    pub fn row_spacing(mut self, row_spacing: f32) -> Self {
        self.row_spacing = row_spacing;
        self
    }

    pub fn section_spacing(mut self, section_spacing: f32) -> Self {
        self.section_spacing = section_spacing;
        self
    }

    pub fn content_padding(mut self, content_padding: f32) -> Self {
        self.content_padding = content_padding;
        self
    }

    pub fn scroll_axes(mut self, scroll_axes: PropertyGridScrollAxes) -> Self {
        self.scroll_axes = scroll_axes;
        self
    }

    pub fn empty_text(mut self, empty_text: impl Into<String>) -> Self {
        self.empty_text = empty_text.into();
        self
    }

    pub fn controlled_collapsed(mut self, ids: HashSet<String>) -> Self {
        self.collapsed_section_ids = Some(ids);
        self
    }

    pub fn on_section_collapse_changed(mut self, handler: impl Fn(&str, bool) + 'static) -> Self {
        self.on_section_collapse_changed = Some(Rc::new(handler));
        self
    }

    /// Toggles `section`: the controlled handler is notified, and the local
    /// state follows when the call site does not control expansion.
    pub fn toggle_section(&self, section: &PropertyGridSection) {
        let collapsed = section.is_collapsed(
            self.collapsed_section_ids.as_ref(),
            &self.local_collapsed.get(),
        );
        let next = !collapsed;
        if let Some(handler) = &self.on_section_collapse_changed {
            handler(&section.id, next);
        }
        if self.collapsed_section_ids.is_none() {
            let mut local = self.local_collapsed.get();
            local.insert(section.id.clone(), next);
            self.local_collapsed.set(local);
        }
    }
}

impl View for PropertyGrid {
    fn body(&self) -> AnyView {
        let collapsed = PropertyGridCollapsed {
            controlled: self.collapsed_section_ids.clone(),
            local: self.local_collapsed.clone(),
        };
        let sections = self.sections.clone();
        let label_width = self.label_width;
        let min_value_width = self.min_value_width;
        let label_placement = self.label_placement;
        let row_height = self.row_height;
        let row_spacing = self.row_spacing;
        let section_spacing = self.section_spacing;
        let content_padding = self.content_padding;
        let scroll_axes = self.scroll_axes;
        let empty_text = self.empty_text.clone();
        let on_change = self.on_section_collapse_changed.clone();
        let local = self.local_collapsed.clone();
        let controlled = self.collapsed_section_ids.clone();

        let grid = PropertyGridSurface {
            sections,
            collapsed,
            label_width,
            min_value_width,
            label_placement,
            row_height,
            row_spacing,
            section_spacing,
            content_padding,
            empty_text,
            on_change,
            local,
            controlled,
        };

        let content = AnyView::new(
            AnyView::new(grid)
                .frame(None, None, None, None, None, None)
                .flex_with(1.0, 1.0, Some(0.0)),
        );

        let scroll = match scroll_axes {
            PropertyGridScrollAxes::Vertical => ScrollView::new(ScrollAxis::Vertical, content)
                .scrollbar_gutter(ScrollbarGutter::Stable),
            PropertyGridScrollAxes::Horizontal => ScrollView::new(ScrollAxis::Horizontal, content),
            PropertyGridScrollAxes::Both => {
                ScrollView::new(ScrollAxis::Both, content).scrollbar_gutter(ScrollbarGutter::Stable)
            }
        };
        AnyView::new(AnyView::new(scroll).flex_with(1.0, 1.0, Some(0.0)))
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![crate::DynamicField::state(
            "collapsed",
            &self.local_collapsed,
        )]
    }
}

/// The controlled/local collapse pair, shared with the header buttons.
#[derive(Clone)]
struct PropertyGridCollapsed {
    controlled: Option<HashSet<String>>,
    local: State<BTreeMap<String, bool>>,
}

impl PropertyGridCollapsed {
    fn is_collapsed(&self, section: &PropertyGridSection) -> bool {
        section.is_collapsed(self.controlled.as_ref(), &self.local.get())
    }
}

/// Everything the section and row builders need, without borrowing the view.
struct PropertyGridSurface {
    sections: Vec<PropertyGridSection>,
    collapsed: PropertyGridCollapsed,
    label_width: f32,
    min_value_width: f32,
    label_placement: FieldLabelPlacement,
    row_height: f32,
    row_spacing: f32,
    section_spacing: f32,
    content_padding: f32,
    empty_text: String,
    on_change: Option<Rc<dyn Fn(&str, bool)>>,
    local: State<BTreeMap<String, bool>>,
    controlled: Option<HashSet<String>>,
}

impl PropertyGridSurface {
    fn empty_state(&self) -> AnyView {
        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(
                    Text::new(self.empty_text.clone())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_variant()),
                ),
            ))
            .padding_edges(10.0, 12.0)
            .semantic_background(SemanticColorRef::surface())
            .corner_radius(6.0),
        )
    }

    fn section_view(&self, section: &PropertyGridSection, nested: bool) -> AnyView {
        let is_collapsed = self.collapsed.is_collapsed(section);
        let mut header_children = Vec::new();
        if let Some(leading) = &section.header_leading {
            header_children.push(leading.clone());
        }

        let toggle_section = section.clone();
        let toggle_local = self.local.clone();
        let toggle_controlled = self.controlled.clone();
        let toggle_change = self.on_change.clone();
        let toggle = Rc::new(move || {
            let was_collapsed =
                toggle_section.is_collapsed(toggle_controlled.as_ref(), &toggle_local.get());
            let next = !was_collapsed;
            if let Some(handler) = &toggle_change {
                handler(&toggle_section.id, next);
            }
            if toggle_controlled.is_none() {
                let mut local = toggle_local.get();
                local.insert(toggle_section.id.clone(), next);
                toggle_local.set(local);
            }
        });

        let mut label_children = Vec::new();
        if section.is_collapsible {
            let icon = if is_collapsed {
                ui_icon_catalog::navigation::chevron_right()
            } else {
                ui_icon_catalog::navigation::chevron_down()
            };
            label_children.push(AnyView::new(Icon::with_semantic(
                icon,
                12.0,
                SemanticColorRef::on_surface_variant(),
            )));
        }
        label_children.push(AnyView::new(
            AnyView::new(
                Text::new(section.title.clone())
                    .line_limit(Some(1))
                    .semantic_font(SemanticFontRef::label())
                    .semantic_foreground(SemanticColorRef::on_surface()),
            )
            .flex_with(1.0, 1.0, Some(0.0)),
        ));
        if let Some(secondary) = section.secondary_text() {
            label_children.push(AnyView::new(
                Text::new(secondary)
                    .semantic_font(SemanticFontRef::caption())
                    .semantic_foreground(SemanticColorRef::on_surface_variant()),
            ));
        }

        let header_label = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                6.0,
                AnyView::new(TupleView::new(label_children)),
            ))
            .frame(None, None, Some(0.0), None, None, None)
            .flex_with(1.0, 1.0, Some(0.0)),
        );

        let header_button = AnyView::new(
            AnyView::new(
                Button::new(header_label, {
                    let toggle = toggle.clone();
                    move || toggle()
                })
                .enabled(section.is_collapsible),
            )
            .button_style(crate::theme::styles::buttons::plain())
            .frame(None, None, Some(0.0), Some(26.0), None, None)
            .flex_with(1.0, 1.0, Some(0.0))
            .modifier(cove_composition::LayoutDebugModifier::debug_name(&format!(
                "property-section-header-{}",
                section.id
            ))),
        );
        header_children.push(header_button);
        if let Some(trailing) = &section.header_trailing {
            header_children.push(trailing.clone());
        }

        let header_height = if section.header_leading.is_none() && section.header_trailing.is_none()
        {
            26.0
        } else {
            32.0
        };
        let header_inline_padding = if nested { 7.0 } else { 8.0 };
        let header = AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Row,
                Align::Center,
                Justify::FlexStart,
                Wrap::NoWrap,
                4.0,
                AnyView::new(TupleView::new(header_children)),
            ))
            .padding_insets(EdgeInsets::new(
                3.0,
                header_inline_padding,
                3.0,
                header_inline_padding,
            ))
            .frame(None, None, Some(0.0), Some(header_height), None, None),
        );

        let body = if is_collapsed {
            None
        } else {
            Some(self.section_body(section, nested))
        };

        let mut card_children = vec![header];
        if let Some(body) = body {
            card_children.push(body);
        }

        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(card_children)),
            ))
            .semantic_background(if nested {
                SemanticColorRef::background()
            } else {
                SemanticColorRef::surface_raised()
            })
            .corner_radius(if nested { 0.0 } else { 8.0 }),
        )
    }

    fn section_body(&self, section: &PropertyGridSection, nested: bool) -> AnyView {
        let mut children: Vec<AnyView> = Vec::new();
        if section.rows.is_empty() && section.children.is_empty() && section.footer.is_none() {
            children.push(AnyView::new(
                AnyView::new(
                    Text::new(self.empty_text.clone())
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_variant()),
                )
                .padding_edges(8.0, 8.0),
            ));
        } else {
            for (index, row) in section.rows.iter().enumerate() {
                children.push(AnyView::new(self.row_view(section, row, index)));
            }
        }
        for child in &section.children {
            children.push(AnyView::new(self.section_view(child, true)));
        }
        if let Some(footer) = &section.footer {
            children.push(AnyView::new(
                AnyView::new(footer.clone()).padding_edges(7.0, 4.0),
            ));
        }

        AnyView::new(
            AnyView::new(BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                self.row_spacing,
                AnyView::new(TupleView::new(children)),
            ))
            .padding_insets(if nested {
                EdgeInsets::new(0.0, 0.0, 4.0, 0.0)
            } else {
                EdgeInsets::new(0.0, 8.0, 8.0, 8.0)
            }),
        )
    }

    fn row_view(
        &self,
        section: &PropertyGridSection,
        row: &PropertyGridRow,
        _index: usize,
    ) -> AnyView {
        let row_key = format!("{}/{}", section.id, row.id);
        let row_height = row.row_height.unwrap_or(self.row_height).max(1.0);
        let content = match row.layout {
            PropertyGridRowLayout::TwoColumn => self.two_column_row(row, row_height),
            PropertyGridRowLayout::FullWidth => self.full_width_row(row, row_height),
        };
        let _ = row_key;
        AnyView::new(PropertyGridRowHost { content })
    }

    fn two_column_row(&self, row: &PropertyGridRow, row_height: f32) -> AnyView {
        let label_width = self.label_width;
        let min_value_width = self.min_value_width;
        let placement = self.label_placement;
        let label = row.label.clone();
        let is_empty = label.is_empty();
        let fixed_height = matches!(row.sizing, PropertyGridRowSizing::Fixed).then_some(row_height);
        let value = self.sized_value(row, fixed_height);

        AnyView::new(crate::adaptive::ContainerSizeReader::new({
            let value = value.clone();
            move |size: cove_core::Size| {
                let leading = !is_empty
                    && placement.is_leading(size.width, label_width, min_value_width, 4.0);
                let label_text = if leading {
                    AnyView::new(
                        Text::new(label.clone())
                            .truncated()
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_variant()),
                    )
                } else {
                    AnyView::new(
                        Text::new(label.clone())
                            .semantic_font(SemanticFontRef::caption())
                            .semantic_foreground(SemanticColorRef::on_surface_variant()),
                    )
                };
                let label_box = AnyView::new(
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            label_text,
                        ))
                        .padding_edges(7.0, 0.0)
                        .frame(
                            if leading {
                                Some(FrameDimension::Points(label_width))
                            } else {
                                Some(FrameDimension::Auto)
                            },
                            None,
                            None,
                            if leading { Some(row_height) } else { Some(0.0) },
                            None,
                            None,
                        )
                        .clipped(),
                    )
                    .flex_with(0.0, 0.0, None),
                );

                let value_box = AnyView::new(
                    AnyView::new(
                        AnyView::new(BoxView::new(
                            FlexDirection::Row,
                            Align::Center,
                            Justify::FlexStart,
                            Wrap::NoWrap,
                            0.0,
                            AnyView::new(AnyView::new(value.clone()).flex_with(
                                1.0,
                                1.0,
                                Some(0.0),
                            )),
                        ))
                        .frame(
                            None,
                            fixed_height.map(FrameDimension::Points),
                            None,
                            Some(row_height),
                            None,
                            None,
                        )
                        .padding_insets(EdgeInsets::new(
                            2.0,
                            if leading { 0.0 } else { 7.0 },
                            2.0,
                            7.0,
                        )),
                    )
                    .flex_with(
                        if leading { 1.0 } else { 0.0 },
                        1.0,
                        if leading { Some(0.0) } else { None },
                    ),
                );

                AnyView::new(
                    AnyView::new(BoxView::new(
                        if leading {
                            FlexDirection::Row
                        } else {
                            FlexDirection::Column
                        },
                        if leading {
                            Align::Center
                        } else {
                            Align::Stretch
                        },
                        Justify::FlexStart,
                        Wrap::NoWrap,
                        if leading { 4.0 } else { 6.0 },
                        AnyView::new(TupleView::new(vec![label_box, value_box])),
                    ))
                    .frame(None, None, Some(0.0), None, None, None),
                )
            }
        }))
    }

    fn full_width_row(&self, row: &PropertyGridRow, row_height: f32) -> AnyView {
        let label = row.label.clone();
        let label_height = if label.is_empty() { 0.0 } else { 18.0 };
        let vertical_padding = 6.0;
        let label_value_spacing = if label.is_empty() { 0.0 } else { 6.0 };
        let minimum_value_height = self.row_height.max(1.0);
        let total_height = row_height.max(
            label_height + label_value_spacing + vertical_padding * 2.0 + minimum_value_height,
        );
        let value_height = match row.sizing {
            PropertyGridRowSizing::Fixed => {
                Some(total_height - label_height - label_value_spacing - vertical_padding * 2.0)
            }
            PropertyGridRowSizing::Intrinsic => None,
        };
        let value = self.sized_value(row, value_height);

        let mut children = Vec::new();
        if !label.is_empty() {
            children.push(AnyView::new(
                AnyView::new(
                    Text::new(label.clone())
                        .line_limit(Some(1))
                        .semantic_font(SemanticFontRef::caption())
                        .semantic_foreground(SemanticColorRef::on_surface_variant())
                        .padding_edges(7.0, 0.0),
                )
                .frame_height(label_height),
            ));
        }
        children.push(AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Row,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(AnyView::new(value).flex_with(1.0, 1.0, Some(0.0))),
                ))
                .frame(
                    None,
                    value_height.map(FrameDimension::Points),
                    None,
                    None,
                    None,
                    None,
                )
                .padding_insets(EdgeInsets::new(0.0, 7.0, 0.0, 7.0)),
            )
            .flex_with(0.0, 0.0, None),
        ));

        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    label_value_spacing,
                    AnyView::new(TupleView::new(children)),
                ))
                .padding_edges(0.0, vertical_padding),
            )
            .frame(
                match row.sizing {
                    PropertyGridRowSizing::Fixed => Some(FrameDimension::Points(total_height)),
                    PropertyGridRowSizing::Intrinsic => Some(FrameDimension::Auto),
                },
                None,
                None,
                None,
                None,
                None,
            ),
        )
    }

    /// The caller's value, labelled for assistive clients. A fixed row pins it
    /// to `height`; an intrinsic row leaves its own sizing untouched.
    fn sized_value(&self, row: &PropertyGridRow, height: Option<f32>) -> AnyView {
        let label = row.label.clone();
        let identifier = row.id.clone();
        let value = AnyView::new(row.value().clone()).accessibility(
            move |semantics: &mut AccessibilitySemantics| {
                semantics.label = label.clone();
                semantics.identifier = identifier.clone();
            },
        );
        match height {
            Some(height) => AnyView::new(AnyView::new(value).frame_height(height)),
            None => AnyView::new(value),
        }
    }
}

impl View for PropertyGridSurface {
    fn body(&self) -> AnyView {
        let mut children = Vec::new();
        if self.sections.is_empty() {
            children.push(self.empty_state());
        } else {
            for section in &self.sections {
                children.push(self.section_view(section, false));
            }
        }
        AnyView::new(
            AnyView::new(
                AnyView::new(BoxView::new(
                    FlexDirection::Column,
                    Align::Stretch,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    self.section_spacing,
                    AnyView::new(TupleView::new(children)),
                ))
                .padding(self.content_padding),
            )
            .frame(None, None, Some(0.0), None, None, None)
            .flex_with(1.0, 1.0, Some(0.0)),
        )
    }
}

/// Row chrome: a hover band over a transparent base.
struct PropertyGridRowHost {
    content: AnyView,
}

impl PrimitiveView for PropertyGridRowHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_corner_radius(4.0);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Column);
        layout.set_align_items(Align::Stretch);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        let paint_node = node.clone();
        let paint: Rc<dyn Fn()> = Rc::new(move || {
            let hovered = paint_node.attachment::<bool>(ROW_HOVERED) == Some(true);
            let theme = theme_for_node(&paint_node);
            paint_node.set_background_color(Some(if hovered {
                theme.colors.state_layer_hover
            } else {
                cove_core::Color::CLEAR
            }));
        });
        paint();

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let hover_node = node.downgrade();
        let hover_paint = paint.clone();
        registry.set_hover(
            node,
            Rc::new(move |phase: HoverPhase| {
                if let Some(owner) = Node::upgrade(&hover_node) {
                    owner.set_attachment(ROW_HOVERED, phase == HoverPhase::Enter);
                    hover_paint();
                }
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}

impl View for PropertyGridRowHost {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }
    fn tag(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
