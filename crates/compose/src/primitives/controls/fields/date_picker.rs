//! Translation of Primitives/Controls/Fields/DatePicker.swift.
//!
//! A labelled trigger that opens a calendar in a popover. The trigger shows the
//! selected period; clearing writes `None` without opening the popover.
use std::cell::RefCell;
use std::rc::Rc;

use crate::foundation::{CalendarUnit, Date};
use crate::modifiers::layout::{FlexExt, LayoutFrameExt};
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::button::{Button, ButtonIcon, ButtonIconSource, ButtonRole};
use crate::primitives::controls::calendar::{CalendarGranularity, CalendarOptions, CalendarView};
use crate::primitives::overlays::anchored::{PopoverOverflow, PopoverSurface};
use crate::primitives::overlays::popover::Popover;
use crate::primitives::text::Text;
use crate::theme::SemanticFontRef;
use crate::theme::styles::buttons::{ButtonStyleViewExt, GhostButtonStyle};
use crate::{AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View};
use cove_core::scene::{AccessibilityRole, AccessibilitySemantics};
use cove_core::scene::{Align, FlexDirection, Justify, Wrap};

/// Trigger presentation values.
#[derive(Clone, Debug, PartialEq)]
pub struct DatePickerPresentation {
    pub placeholder: String,
    pub is_clearable: bool,
    pub is_enabled: bool,
    pub width: f32,
}

impl Default for DatePickerPresentation {
    fn default() -> Self {
        Self {
            placeholder: "Choose a date".to_string(),
            is_clearable: true,
            is_enabled: true,
            width: 240.0,
        }
    }
}

impl DatePickerPresentation {
    pub fn validate(&mut self) {
        self.width = self.width.max(140.0);
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }
}

/// Authored picker options.
#[derive(Clone, Default)]
pub struct DatePickerOptions {
    pub calendar: CalendarOptions,
    pub presentation: DatePickerPresentation,
}

impl DatePickerOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validated(mut self) -> Self {
        self.presentation.validate();
        self.calendar = self.calendar.validated();
        self
    }
}

/// A trigger that opens a calendar popover.
pub struct DatePicker {
    pub selection: Rc<dyn Fn() -> Option<Date>>,
    pub set_selection: Rc<dyn Fn(Option<Date>)>,
    pub options: DatePickerOptions,
    pub is_presented: Rc<RefCell<bool>>,
    pub cell_size: f32,
}

impl DatePicker {
    pub fn new(
        selection: Rc<dyn Fn() -> Option<Date>>,
        set_selection: Rc<dyn Fn(Option<Date>)>,
        options: DatePickerOptions,
    ) -> Self {
        let options = options.validated();
        let cell_size = options.calendar.presentation.cell_size;
        Self {
            selection,
            set_selection,
            options,
            is_presented: Rc::new(RefCell::new(false)),
            cell_size,
        }
    }

    /// Label for the selected period, or the placeholder.
    fn label(&self) -> String {
        match (self.selection)() {
            Some(date) => period_label(
                self.options.calendar.granularity,
                date,
                &self.options.calendar.calendar,
            ),
            None => self.options.presentation.placeholder.clone(),
        }
    }
}

/// Trigger and accessibility text for a selected period.
fn period_label(
    granularity: CalendarGranularity,
    date: Date,
    calendar: &crate::foundation::Calendar,
) -> String {
    let parts = calendar.parts(date);
    match granularity {
        CalendarGranularity::Day => {
            format!("{} {}, {}", month_name(parts.month), parts.day, parts.year)
        }
        CalendarGranularity::Week => {
            let (year, week) = crate::foundation::iso_week_of(
                crate::foundation::DateParts::new(parts.year, parts.month, parts.day).epoch_day(),
            );
            format!("Week {week}, {year}")
        }
        CalendarGranularity::Month => format!("{} {}", month_name(parts.month), parts.year),
        CalendarGranularity::Year => parts.year.to_string(),
    }
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

impl View for DatePicker {
    fn body(&self) -> AnyView {
        AnyView::new(DatePickerBody {
            picker: self.snapshot(),
        })
    }
}

impl DatePicker {
    fn snapshot(&self) -> DatePicker {
        DatePicker {
            selection: self.selection.clone(),
            set_selection: self.set_selection.clone(),
            options: self.options.clone(),
            is_presented: self.is_presented.clone(),
            cell_size: self.cell_size,
        }
    }
}

struct DatePickerBody {
    picker: DatePicker,
}

impl PrimitiveView for DatePickerBody {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        None
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        let picker = &self.picker;
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: picker.options.presentation.placeholder.clone(),
            value: match (picker.selection)() {
                Some(date) => period_label(
                    picker.options.calendar.granularity,
                    date,
                    &picker.options.calendar.calendar,
                ),
                None => String::new(),
            },
            state: cove_core::scene::AccessibilityState {
                is_expanded: Some(*picker.is_presented.borrow()),
                is_enabled: picker.options.presentation.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        let picker = self.picker.snapshot();
        let label = picker.label();
        let has_selection = (picker.selection)().is_some();
        let is_presented = *picker.is_presented.borrow();

        let mut trigger: Vec<AnyView> = vec![AnyView::new(
            Text::new(label.clone())
                .truncated()
                .semantic_font(SemanticFontRef::label())
                .flex_with(1.0, 1.0, Some(0.0)),
        )];
        if picker.options.presentation.is_clearable && has_selection {
            let set_selection = picker.set_selection.clone();
            trigger.push(AnyView::new(
                Button::icon_with_options(
                    ButtonIconSource::resource(
                        crate::primitives::media::ui_icon_catalog::action::x(),
                    ),
                    Some(12.0),
                    None,
                    ButtonRole::Normal,
                    picker.options.presentation.is_enabled,
                    false,
                    false,
                    Some("Clear date".to_string()),
                    move || set_selection(None),
                )
                .button_style(GhostButtonStyle),
            ));
        }
        trigger.push(AnyView::new(ButtonIcon::new(
            ButtonIconSource::resource(
                crate::primitives::media::ui_icon_catalog::navigation::dropdown_chevron_down(),
            ),
            Some(12.0),
            None,
        )));
        let trigger_view = AnyView::new(TriggerFace {
            content: AnyView::new(Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(trigger)),
            )),
        });

        let calendar_options = picker.options.calendar.clone();
        let set_selection = picker.set_selection.clone();
        let presented = picker.is_presented.clone();
        let calendar = CalendarView::new(
            picker.selection.clone(),
            {
                let set_selection = set_selection.clone();
                let presented = presented.clone();
                Rc::new(move |value: Option<Date>| {
                    set_selection(value);
                    // A pick closes the popover.
                    *presented.borrow_mut() = false;
                })
            },
            calendar_options,
        );
        let popover = Popover::new(AnyView::new(trigger_view), AnyView::new(calendar))
            .width(CalendarView::width(
                picker.cell_size,
                &picker.options.calendar,
            ))
            .surface(PopoverSurface::Content)
            .overflow(PopoverOverflow::Flip)
            .enabled(picker.options.presentation.is_enabled);
        let _ = is_presented;
        let _ = CalendarUnit::Day;
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::FlexStart,
                Wrap::NoWrap,
                0.0,
                AnyView::new(popover),
            )
            .frame_width(picker.options.presentation.width),
        )]
    }
}
crate::impl_primitive_view!(DatePickerBody);

/// The trigger's surface: field chrome around the label, clear and chevron.
struct TriggerFace {
    content: AnyView,
}

impl PrimitiveView for TriggerFace {
    fn make_node(&self) -> cove_core::scene::Node {
        let node = cove_core::scene::Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<cove_core::scene::LayoutNode> {
        let layout = cove_core::scene::LayoutNode::new();
        layout.set_min_height(Some(34.0));
        layout.set_padding(10.0, cove_core::scene::Edge::Start);
        layout.set_padding(6.0, cove_core::scene::Edge::Top);
        layout.set_padding(10.0, cove_core::scene::Edge::End);
        layout.set_padding(6.0, cove_core::scene::Edge::Bottom);
        Some(layout)
    }

    fn update_layout(&self, layout: &cove_core::scene::LayoutNode) {
        layout.set_min_height(Some(34.0));
        layout.set_padding(10.0, cove_core::scene::Edge::Start);
        layout.set_padding(6.0, cove_core::scene::Edge::Top);
        layout.set_padding(10.0, cove_core::scene::Edge::End);
        layout.set_padding(6.0, cove_core::scene::Edge::Bottom);
    }

    fn update_node(&self, node: &cove_core::scene::Node) {
        let theme = crate::theme::theme_for_node(node);
        node.set_background_color(Some(theme.colors.surface));
        node.set_corner_radius(theme.inputs.radius);
        node.set_border_color(Some(theme.colors.border));
        node.set_border_width(1.0);
    }

    fn children(&self, _node: &cove_core::scene::Node) -> Vec<AnyView> {
        vec![self.content.clone()]
    }
}
crate::impl_primitive_view!(TriggerFace);
