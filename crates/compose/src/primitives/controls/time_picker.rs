//! Translation of Primitives/Controls/TimePicker.swift.
//!
//! Inline wheel time editor: hour, minute and optional second/AM-PM columns.
//! Each column is a [`TimeWheel`] leaf whose option list, stepping and visible
//! window come from the shared [`WheelPicker`] model, so disabled rows and
//! end-stopping behave the same as a standalone wheel.
use crate::Scope;
use std::rc::Rc;
use std::sync::Arc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align,
    EventPhase, EventResult, FlexDirection, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, Justify, LayoutNode, Node, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_core::{Binding, InputCapabilities, Rect};
use cove_platform::{MouseButton, SystemCursor};

use crate::adaptive::SurfaceEnvironment;
use crate::modifiers::layout::LayoutFrameExt;
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::adaptive::{apply_touch_control_extent, supports_control_hover};
use crate::primitives::controls::date_values::{DEFAULT_MINUTE_STEP, TimeOfDay};
use crate::primitives::controls::fields::wheel_picker::{WheelPicker, WheelPickerOption};
use crate::primitives::text::Text;
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{
    AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View, impl_primitive_view,
};

/// The hour cycle a time picker shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeHourCycle {
    #[default]
    TwentyFour,
    Twelve,
}

/// Authored time-picker settings.
#[derive(Clone, Debug, PartialEq)]
pub struct TimePickerOptions {
    pub hour_cycle: TimeHourCycle,
    pub minute_step: i32,
    pub shows_seconds: bool,
    pub is_enabled: bool,
    pub minimum: TimeOfDay,
    pub maximum: TimeOfDay,
    /// Accessible name for the wheel group.
    pub label: String,
    /// Half-day wheel labels as `(am, pm)`.
    ///
    /// `None` uses `("AM", "PM")`; a Chinese interface sets `("上午", "下午")`.
    pub period_labels: Option<(String, String)>,
    /// Half-open `[start, end)` spans whose rows are disabled in every wheel.
    ///
    /// A row is disabled only when every time it can produce falls inside a
    /// span, so a wheel never offers a row that is only half selectable. Empty
    /// and reversed spans are dropped by [`TimePickerOptions::validate`].
    pub disabled_ranges: Vec<(TimeOfDay, TimeOfDay)>,
}

impl Default for TimePickerOptions {
    fn default() -> Self {
        Self {
            hour_cycle: TimeHourCycle::TwentyFour,
            minute_step: DEFAULT_MINUTE_STEP,
            shows_seconds: false,
            is_enabled: true,
            minimum: TimeOfDay::new(0, 0, 0),
            maximum: TimeOfDay::new(23, 59, 59),
            label: "Time".to_string(),
            period_labels: None,
            disabled_ranges: Vec::new(),
        }
    }
}

impl TimePickerOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self) {
        self.minute_step = self.minute_step.clamp(1, 60);
        if self.maximum < self.minimum {
            self.maximum = self.minimum;
        }
        let authored = std::mem::take(&mut self.disabled_ranges);
        self.disabled_ranges = authored
            .into_iter()
            .filter_map(|(start, end)| {
                let (start, end) = if end < start { (end, start) } else { (start, end) };
                (start.seconds() < end.seconds()).then_some((start, end))
            })
            .collect();
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
    }

    /// `time` clamped into the authored `[minimum, maximum]` range.
    ///
    /// Clamping works on the total second count, so a candidate below
    /// `minimum = 09:30` lands on `09:30` rather than on `09:00`.
    pub fn clamped(&self, time: TimeOfDay) -> TimeOfDay {
        let seconds = time
            .seconds()
            .clamp(self.minimum.seconds(), self.maximum.seconds());
        TimeOfDay::new(seconds / 3600, (seconds / 60) % 60, seconds % 60)
    }

    /// The `(am, pm)` labels the half-day wheel shows.
    pub fn period_labels(&self) -> (&str, &str) {
        match self.period_labels.as_ref() {
            Some((am, pm)) => (am.as_str(), pm.as_str()),
            None => ("AM", "PM"),
        }
    }

    /// Whether `time` falls inside an authored disabled span.
    pub fn is_time_disabled(&self, time: TimeOfDay) -> bool {
        let seconds = time.seconds();
        self.disabled_ranges
            .iter()
            .any(|(start, end)| start.seconds() <= seconds && seconds < end.seconds())
    }

    /// Whether every second in the inclusive `[start, end]` span is disabled.
    fn is_span_disabled(&self, start: i32, end: i32) -> bool {
        if start > end {
            return false;
        }
        let mut cursor = start;
        while cursor <= end {
            let next = self
                .disabled_ranges
                .iter()
                .filter_map(|(from, to)| {
                    let (from, to) = (from.seconds(), to.seconds());
                    (from <= cursor && cursor < to).then_some(to)
                })
                .max();
            match next {
                Some(to) => cursor = to,
                None => return false,
            }
        }
        true
    }
}

/// Column width for one wheel.
const WHEEL_WIDTH: f32 = 68.0;
/// Rows visible in each wheel.
const VISIBLE_ROWS: usize = 5;
/// Pointer row height on non-touch surfaces.
const AUTHORED_ROW_HEIGHT: f32 = 32.0;
/// Row height on touch surfaces.
const TOUCH_ROW_HEIGHT: f32 = 44.0;

fn row_height(environment: &cove_core::Environment) -> f32 {
    if environment.input.contains(InputCapabilities::TOUCH) {
        TOUCH_ROW_HEIGHT
    } else {
        AUTHORED_ROW_HEIGHT
    }
}

/// Inline wheel time editor.
pub struct TimePicker {
    pub selection: Binding<Option<TimeOfDay>>,
    pub options: TimePickerOptions,
    pub on_change: Option<Arc<dyn Fn(TimeOfDay) + Send + Sync>>,
}

impl TimePicker {
    pub fn new(selection: Binding<Option<TimeOfDay>>, options: TimePickerOptions) -> Self {
        Self {
            selection,
            options: options.validated(),
            on_change: None,
        }
    }

    pub fn on_change(mut self, handler: impl Fn(TimeOfDay) + Send + Sync + 'static) -> Self {
        self.on_change = Some(Arc::new(handler));
        self
    }

    fn snapshot(&self) -> TimePicker {
        TimePicker {
            selection: self.selection.clone(),
            options: self.options.clone(),
            on_change: self.on_change.clone(),
        }
    }

    /// The committed time, clamped into the authored range.
    ///
    /// The whole value is clamped, not just the hour, so the wheels always show
    /// the time `commit` would write.
    fn current(&self) -> TimeOfDay {
        let candidate = self.selection.get().unwrap_or(self.options.minimum);
        self.options.clamped(candidate)
    }

    /// Writes a time, clamping its second count into the authored range.
    fn commit(&self, time: TimeOfDay) -> TimeOfDay {
        let next = self.options.clamped(time);
        if self.selection.get() != Some(next) {
            self.selection.set(Some(next));
        }
        if let Some(handler) = self.on_change.as_ref() {
            handler(next);
        }
        next
    }

    fn hour(&self) -> Binding<i32> {
        let getter = self.snapshot();
        let setter = self.snapshot();
        Binding::new(
            move || {
                let time = getter.current();
                match getter.options.hour_cycle {
                    TimeHourCycle::Twelve => {
                        if time.hour % 12 == 0 {
                            12
                        } else {
                            time.hour % 12
                        }
                    }
                    TimeHourCycle::TwentyFour => time.hour,
                }
            },
            move |value| {
                let time = setter.current();
                let base = if setter.options.hour_cycle == TimeHourCycle::Twelve && time.hour >= 12
                {
                    12
                } else {
                    0
                };
                let hour = match setter.options.hour_cycle {
                    TimeHourCycle::Twelve => (value % 12) + base,
                    TimeHourCycle::TwentyFour => value,
                };
                setter.commit(TimeOfDay::new(hour, time.minute, time.second));
            },
        )
    }

    fn minute(&self) -> Binding<i32> {
        let getter = self.snapshot();
        let setter = self.snapshot();
        let step = self.options.minute_step.max(1);
        Binding::new(
            move || getter.current().minute / step * step,
            move |value| {
                let time = setter.current();
                setter.commit(TimeOfDay::new(time.hour, value, time.second));
            },
        )
    }

    fn second(&self) -> Binding<i32> {
        let getter = self.snapshot();
        let setter = self.snapshot();
        Binding::new(
            move || getter.current().second,
            move |value| {
                let time = setter.current();
                setter.commit(TimeOfDay::new(time.hour, time.minute, value));
            },
        )
    }

    fn period(&self) -> Binding<i32> {
        let getter = self.snapshot();
        let setter = self.snapshot();
        Binding::new(
            move || if getter.current().hour >= 12 { 1 } else { 0 },
            move |value| {
                let time = setter.current();
                let hour = (time.hour % 12) + if value == 1 { 12 } else { 0 };
                setter.commit(TimeOfDay::new(hour, time.minute, time.second));
            },
        )
    }

    fn hours(&self) -> Vec<WheelPickerOption<i32>> {
        let range = match self.options.hour_cycle {
            TimeHourCycle::Twelve => 1..=12,
            TimeHourCycle::TwentyFour => 0..=23,
        };
        range
            .map(|hour| {
                let is_enabled = !self.hour_is_disabled(hour);
                WheelPickerOption::new(hour, format!("{hour:02}")).enabled(is_enabled)
            })
            .collect()
    }

    /// Whether every time this hour row can produce is disabled.
    fn hour_is_disabled(&self, hour: i32) -> bool {
        let blocked = |hour: i32| {
            let start = hour * 3600;
            self.options.is_span_disabled(start, start + 3_599)
        };
        match self.options.hour_cycle {
            TimeHourCycle::TwentyFour => blocked(hour),
            // A 12-hour row covers the same hour before and after noon, so both
            // halves have to be blocked before the row goes away.
            TimeHourCycle::Twelve => blocked(hour % 12) && blocked(hour % 12 + 12),
        }
    }

    fn minutes(&self) -> Vec<WheelPickerOption<i32>> {
        let step = self.options.minute_step.max(1);
        let time = self.current();
        (0..60)
            .step_by(step as usize)
            .map(|minute| {
                let base = time.hour * 3600 + minute * 60;
                // With seconds hidden the row commits one instant; with them
                // shown it commits the whole minute.
                let (start, end) = if self.options.shows_seconds {
                    (base, base + 59)
                } else {
                    (base + time.second, base + time.second)
                };
                WheelPickerOption::new(minute, format!("{minute:02}"))
                    .enabled(!self.options.is_span_disabled(start, end))
            })
            .collect()
    }

    fn seconds(&self) -> Vec<WheelPickerOption<i32>> {
        let time = self.current();
        (0..=59)
            .map(|second| {
                let instant = time.hour * 3600 + time.minute * 60 + second;
                WheelPickerOption::new(second, format!("{second:02}"))
                    .enabled(!self.options.is_span_disabled(instant, instant))
            })
            .collect()
    }

    fn periods(&self) -> Vec<WheelPickerOption<i32>> {
        let (am, pm) = self.options.period_labels();
        vec![
            WheelPickerOption::new(0, am.to_string())
                .enabled(!self.options.is_span_disabled(0, 43_199)),
            WheelPickerOption::new(1, pm.to_string())
                .enabled(!self.options.is_span_disabled(43_200, 86_399)),
        ]
    }

    /// The accessible name of the half-day wheel.
    fn period_label(&self) -> String {
        match self.options.period_labels.as_ref() {
            Some((am, pm)) => format!("{am}/{pm}"),
            None => "AM or PM".to_string(),
        }
    }

    fn wheel(
        &self,
        label: &str,
        options: Vec<WheelPickerOption<i32>>,
        value: Binding<i32>,
    ) -> AnyView {
        AnyView::new(
            TimeWheel {
                label: label.to_string(),
                options,
                value,
                is_enabled: self.options.is_enabled,
            }
            .frame_width(WHEEL_WIDTH),
        )
    }

    fn separator() -> AnyView {
        AnyView::new(
            Text::new(":")
                .semantic_font(SemanticFontRef::title())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
        )
    }
}

impl View for TimePicker {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut columns: Vec<AnyView> = vec![
            self.wheel("Hour", self.hours(), self.hour()),
            Self::separator(),
            self.wheel("Minute", self.minutes(), self.minute()),
        ];
        if self.options.shows_seconds {
            columns.push(Self::separator());
            columns.push(self.wheel("Second", self.seconds(), self.second()));
        }
        if self.options.hour_cycle == TimeHourCycle::Twelve {
            columns.push(self.wheel(&self.period_label(), self.periods(), self.period()));
        }
        AnyView::new(TimePickerBody {
            label: self.options.label.clone(),
            row: AnyView::new(Row::new(
                VerticalAlignment::Center,
                8.0,
                AnyView::new(TupleView::new(columns)),
            )),
        })
    }
}

/// Group semantics for the whole row of wheels.
struct TimePickerBody {
    label: String,
    row: AnyView,
}

impl PrimitiveView for TimePickerBody {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: self.label.clone(),
            ..Default::default()
        }));
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![self.row.clone()]
    }
}
impl_primitive_view!(TimePickerBody);

const WHEEL_HOVERED: &str = "__time_wheel_hovered";
const WHEEL_ROW_HEIGHT: &str = "__time_wheel_row_height";

/// One focusable wheel column.
struct TimeWheel {
    label: String,
    options: Vec<WheelPickerOption<i32>>,
    value: Binding<i32>,
    is_enabled: bool,
}

impl Clone for TimeWheel {
    fn clone(&self) -> Self {
        self.snapshot()
    }
}

impl TimeWheel {
    fn snapshot(&self) -> TimeWheel {
        TimeWheel {
            label: self.label.clone(),
            options: self.options.clone(),
            value: self.value.clone(),
            is_enabled: self.is_enabled,
        }
    }

    fn picker(&self) -> WheelPicker<i32> {
        WheelPicker::new(self.options.clone(), Some(self.value.get()))
    }

    fn choose(&self, index: usize) {
        let Some(option) = self.options.get(index) else {
            return;
        };
        if !option.is_enabled || option.value == self.value.get() {
            return;
        }
        self.value.set(option.value);
    }

    fn step(&self, delta: i32) {
        let mut picker = self.picker();
        if picker.step(delta)
            && let Some(value) = picker.value().copied()
        {
            self.value.set(value);
        }
    }

    fn choose_first(&self) {
        let mut picker = self.picker();
        if picker.select_first()
            && let Some(value) = picker.value().copied()
        {
            self.value.set(value);
        }
    }

    fn choose_last(&self) {
        let mut picker = self.picker();
        if picker.select_last()
            && let Some(value) = picker.value().copied()
        {
            self.value.set(value);
        }
    }

    fn value_label(&self, picker: &WheelPicker<i32>) -> String {
        picker.selected_label().unwrap_or_default().to_string()
    }

    fn row_height_for(node: &Node) -> f32 {
        node.attachment::<f32>(WHEEL_ROW_HEIGHT)
            .unwrap_or(AUTHORED_ROW_HEIGHT)
    }
}

impl PrimitiveView for TimeWheel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_enabled);
        node.set_is_focusable(self.is_enabled);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        self.update_layout(&layout);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(AUTHORED_ROW_HEIGHT * VISIBLE_ROWS as f32));
    }

    fn update_node(&self, node: &Node) {
        apply_touch_control_extent(node, true);
        if !supports_control_hover(node) {
            node.set_attachment(WHEEL_HOVERED, false);
        }
        node.set_is_focusable(self.is_enabled);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));

        let environment = node.composition_value(&SurfaceEnvironment::key());
        let height = row_height(&environment);
        node.set_attachment(WHEEL_ROW_HEIGHT, height);
        if let Some(layout) = node.layout_node() {
            layout.set_height(Some(height * VISIBLE_ROWS as f32));
        }

        let picker = self.picker();
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: self.label.clone(),
            value: self.value_label(&picker),
            help: "Use arrow keys, Home or End to choose a value".to_string(),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));

        let hosted = self.snapshot();
        let increment = hosted.clone();
        let decrement = hosted.clone();
        let set_value = hosted.clone();
        node.set_accessibility_actions(AccessibilityActions {
            increment: self
                .is_enabled
                .then(|| Rc::new(move || increment.step(1)) as Rc<dyn Fn()>),
            decrement: self
                .is_enabled
                .then(|| Rc::new(move || decrement.step(-1)) as Rc<dyn Fn()>),
            set_value: self.is_enabled.then(|| {
                Rc::new(move |text: &str| {
                    if let Some(index) = set_value
                        .options
                        .iter()
                        .position(|option| option.label == text)
                    {
                        set_value.choose(index);
                    }
                }) as Rc<dyn Fn(&str)>
            }),
            ..Default::default()
        });

        let weak = node.downgrade();
        node.update_draw(height.to_bits(), move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else {
                return;
            };
            let theme = theme_for_node(&owner);
            let height = Self::row_height_for(&owner);
            let band_height = (height - 4.0).max(0.0);
            let width = owner.frame().width.max(1.0);
            let y = origin.y + (height * VISIBLE_ROWS as f32 - band_height) * 0.5;
            let tint = if owner.is_focusable() {
                theme.colors.surface_variant
            } else {
                theme.colors.surface
            };
            let _ = list.add_rounded_rect(
                Rect::new(origin.x, y, width, band_height),
                theme.radius.sm,
                tint.multiplied_alpha(owner.opacity()),
            );
        });

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }

        let hover_node = node.downgrade();
        registry.set_hover(
            node,
            Rc::new(move |phase| {
                let Some(owner) = Node::upgrade(&hover_node) else {
                    return;
                };
                let next = phase == HoverPhase::Enter && supports_control_hover(&owner);
                if owner.attachment::<bool>(WHEEL_HOVERED) != Some(next) {
                    owner.set_attachment(WHEEL_HOVERED, next);
                    owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                        "timeWheelHover",
                    ));
                }
            }),
        );

        let pointer = self.snapshot();
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != MouseButton::Left || event_phase == EventPhase::Capture {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        if let Some(chain) = FocusChainHolder::current() {
                            chain.focus(Some(&owner), false);
                        }
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        let height = TimeWheel::row_height_for(&owner);
                        let origin = owner.absolute_origin();
                        let centre = origin.y + owner.frame().height * 0.5;
                        let steps = ((event.y - centre) / height.max(1.0)).round() as i64;
                        let picker = pointer.picker();
                        let current = picker
                            .value()
                            .and_then(|value| picker.index_of(value))
                            .unwrap_or(0) as i64;
                        let target = (current + steps).clamp(0, pointer.options.len() as i64 - 1);
                        pointer.choose(target as usize);
                        EventResult::Handled
                    }
                }
            }),
        );

        let wheel = self.snapshot();
        registry.set_wheel(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target || event.y == 0.0 {
                    return EventResult::Ignored;
                }
                wheel.step(if event.y > 0.0 { -1 } else { 1 });
                EventResult::Handled
            }),
        );

        let key = self.snapshot();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target || event.is_repeat {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    crate::Scancode::ARROW_UP | crate::Scancode::ARROW_LEFT => {
                        key.step(-1);
                        EventResult::Handled
                    }
                    crate::Scancode::ARROW_DOWN | crate::Scancode::ARROW_RIGHT => {
                        key.step(1);
                        EventResult::Handled
                    }
                    crate::Scancode::HOME => {
                        key.choose_first();
                        EventResult::Handled
                    }
                    crate::Scancode::END => {
                        key.choose_last();
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }),
        );
    }

    fn children(&self, node: &Node) -> Vec<AnyView> {
        let height = Self::row_height_for(node);
        let picker = self.picker();
        let window = picker.visible_window(VISIBLE_ROWS);
        let selected = picker.value().and_then(|value| picker.index_of(value));
        let mut rows: Vec<AnyView> = Vec::new();
        for index in window {
            let Some(option) = picker.options.get(index) else {
                continue;
            };
            let is_selected = Some(&option.value) == picker.value();
            let distance = selected.map_or(0.0, |selected| (index as f32 - selected as f32).abs());
            let alpha = if option.is_enabled {
                (1.0 - distance * 0.22).max(0.18)
            } else {
                0.4
            };
            let tint = if !option.is_enabled {
                SemanticColorRef::on_surface_muted()
            } else if is_selected {
                SemanticColorRef::on_surface()
            } else {
                SemanticColorRef::on_surface_variant()
            };
            let font = if is_selected {
                SemanticFontRef::body_strong()
            } else {
                SemanticFontRef::body()
            };
            rows.push(AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::Center,
                    Wrap::NoWrap,
                    0.0,
                    AnyView::new(
                        Text::new(option.label.clone())
                            .semantic_font(font)
                            .semantic_foreground(tint),
                    ),
                )
                .opacity(alpha)
                .frame_height(height),
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column,
                Align::Stretch,
                Justify::Center,
                Wrap::NoWrap,
                0.0,
                AnyView::new(TupleView::new(rows)),
            )
            .frame_width(WHEEL_WIDTH),
        )]
    }
}
impl_primitive_view!(TimeWheel);

#[cfg(test)]
mod checks {
    use super::*;
    use cove_core::State;

    fn picker_at(selection: TimeOfDay, options: TimePickerOptions) -> TimePicker {
        let state = State::new(Some(selection));
        TimePicker::new(state.binding(), options)
    }

    #[test]
    fn clamping_uses_the_whole_time_not_just_the_hour() {
        let mut options = TimePickerOptions::new();
        options.minimum = TimeOfDay::new(9, 30, 0);
        options.maximum = TimeOfDay::new(17, 0, 0);
        assert_eq!(
            options.clamped(TimeOfDay::new(9, 0, 0)),
            TimeOfDay::new(9, 30, 0),
            "a candidate below the minimum lands on the minimum"
        );
        assert_eq!(
            options.clamped(TimeOfDay::new(18, 45, 0)),
            TimeOfDay::new(17, 0, 0),
            "a candidate above the maximum lands on the maximum"
        );
        assert_eq!(
            options.clamped(TimeOfDay::new(12, 15, 30)),
            TimeOfDay::new(12, 15, 30),
            "a candidate inside the range is untouched"
        );
    }

    #[test]
    fn the_wheels_show_the_clamped_time() {
        let mut options = TimePickerOptions::new();
        options.minimum = TimeOfDay::new(9, 30, 0);
        let picker = picker_at(TimeOfDay::new(9, 0, 0), options);
        assert_eq!(
            picker.current(),
            TimeOfDay::new(9, 30, 0),
            "the wheel position matches what commit would write"
        );
    }

    #[test]
    fn a_fully_blocked_hour_is_disabled() {
        let mut options = TimePickerOptions::new();
        options.disabled_ranges = vec![(TimeOfDay::new(12, 0, 0), TimeOfDay::new(13, 0, 0))];
        let picker = picker_at(TimeOfDay::new(9, 0, 0), options);
        let hours = picker.hours();
        assert!(!hours[12].is_enabled, "the lunch hour cannot be picked");
        assert!(hours[11].is_enabled);
        assert!(hours[13].is_enabled);
    }

    #[test]
    fn a_partly_blocked_hour_keeps_its_row_and_blocks_minutes() {
        let mut options = TimePickerOptions::new();
        options.minute_step = 1;
        options.disabled_ranges = vec![(TimeOfDay::new(12, 0, 0), TimeOfDay::new(12, 30, 0))];
        let picker = picker_at(TimeOfDay::new(12, 0, 0), options);
        assert!(
            picker.hours()[12].is_enabled,
            "half of the hour is still selectable"
        );
        let minutes = picker.minutes();
        assert!(!minutes[0].is_enabled, "12:00 is blocked");
        assert!(!minutes[25].is_enabled, "12:25 is blocked");
        assert!(minutes[30].is_enabled, "12:30 is the first open minute");
    }

    /// Whether the hour row showing `hour` is selectable.
    fn hour_row(picker: &TimePicker, hour: i32) -> bool {
        picker
            .hours()
            .into_iter()
            .find(|option| option.value == hour)
            .expect("the hour row exists")
            .is_enabled
    }

    #[test]
    fn a_twelve_hour_row_disappears_only_when_both_halves_are_blocked() {
        let mut options = TimePickerOptions::new();
        options.hour_cycle = TimeHourCycle::Twelve;
        options.disabled_ranges = vec![(TimeOfDay::new(9, 0, 0), TimeOfDay::new(10, 0, 0))];
        let picker = picker_at(TimeOfDay::new(9, 0, 0), options);
        assert!(
            hour_row(&picker, 9),
            "09:00 PM is still open, so the shared row stays"
        );
        let mut options = TimePickerOptions::new();
        options.hour_cycle = TimeHourCycle::Twelve;
        options.disabled_ranges = vec![
            (TimeOfDay::new(9, 0, 0), TimeOfDay::new(10, 0, 0)),
            (TimeOfDay::new(21, 0, 0), TimeOfDay::new(22, 0, 0)),
        ];
        let picker = picker_at(TimeOfDay::new(9, 0, 0), options);
        assert!(!hour_row(&picker, 9), "both halves are blocked");
    }

    #[test]
    fn period_labels_reach_the_half_day_wheel() {
        let mut options = TimePickerOptions::new();
        options.hour_cycle = TimeHourCycle::Twelve;
        options.period_labels = Some(("上午".to_string(), "下午".to_string()));
        let picker = picker_at(TimeOfDay::new(9, 0, 0), options);
        let periods = picker.periods();
        assert_eq!(periods[0].label, "上午");
        assert_eq!(periods[1].label, "下午");
        assert_eq!(picker.period_label(), "上午/下午");
    }

    #[test]
    fn empty_and_reversed_spans_are_dropped() {
        let mut options = TimePickerOptions::new();
        options.disabled_ranges = vec![
            (TimeOfDay::new(13, 0, 0), TimeOfDay::new(12, 0, 0)),
            (TimeOfDay::new(15, 0, 0), TimeOfDay::new(15, 0, 0)),
        ];
        options.validate();
        assert_eq!(
            options.disabled_ranges,
            vec![(TimeOfDay::new(12, 0, 0), TimeOfDay::new(13, 0, 0))],
            "a reversed span is normalised and an empty one is dropped"
        );
    }

    #[test]
    fn a_disabled_instant_is_reported() {
        let mut options = TimePickerOptions::new();
        options.disabled_ranges = vec![(TimeOfDay::new(12, 0, 0), TimeOfDay::new(13, 0, 0))];
        assert!(options.is_time_disabled(TimeOfDay::new(12, 30, 0)));
        assert!(
            !options.is_time_disabled(TimeOfDay::new(13, 0, 0)),
            "the end of the span is open"
        );
        assert!(!options.is_time_disabled(TimeOfDay::new(11, 59, 59)));
    }
}
