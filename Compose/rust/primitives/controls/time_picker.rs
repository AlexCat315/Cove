//! Translation of Primitives/Controls/TimePicker.swift.
//!
//! Inline wheel time editor: hour, minute and optional second/AM-PM columns.
//! Each column is a [`TimeWheel`] leaf whose option list, stepping and visible
//! window come from the shared [`WheelPicker`] model, so disabled rows and
//! end-stopping behave the same as a standalone wheel.
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
use crate::primitives::controls::date_values::TimeOfDay;
use crate::primitives::controls::fields::wheel_picker::{WheelPicker, WheelPickerOption};
use crate::primitives::text::Text;
use crate::theme::{theme_for_node, SemanticColorRef, SemanticFontRef};
use crate::{
    impl_primitive_view, AnyView, BoxView, PrimitiveView, Row, TupleView, VerticalAlignment, View,
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
    pub label: String,
}

impl Default for TimePickerOptions {
    fn default() -> Self {
        Self {
            hour_cycle: TimeHourCycle::TwentyFour,
            minute_step: 5,
            shows_seconds: false,
            is_enabled: true,
            minimum: TimeOfDay::new(0, 0, 0),
            maximum: TimeOfDay::new(23, 59, 59),
            label: "Time".to_string(),
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
    }

    pub fn validated(mut self) -> Self {
        self.validate();
        self
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
        Self { selection, options: options.validated(), on_change: None }
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

    /// The committed time, with its hour clamped into the authored range.
    fn current(&self) -> TimeOfDay {
        let candidate = self.selection.get().unwrap_or(self.options.minimum);
        TimeOfDay::new(
            candidate.hour.clamp(self.options.minimum.hour, self.options.maximum.hour),
            candidate.minute,
            candidate.second,
        )
    }

    /// Writes a time, clamping its second count into the authored range.
    fn commit(&self, time: TimeOfDay) -> TimeOfDay {
        let seconds = time
            .seconds()
            .clamp(self.options.minimum.seconds(), self.options.maximum.seconds());
        let next = TimeOfDay::new(seconds / 3600, (seconds / 60) % 60, seconds % 60);
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
                        if time.hour % 12 == 0 { 12 } else { time.hour % 12 }
                    }
                    TimeHourCycle::TwentyFour => time.hour,
                }
            },
            move |value| {
                let time = setter.current();
                let base = if setter.options.hour_cycle == TimeHourCycle::Twelve && time.hour >= 12 {
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
        range.map(|hour| WheelPickerOption::new(hour, format!("{hour:02}"))).collect()
    }

    fn minutes(&self) -> Vec<WheelPickerOption<i32>> {
        (0..60)
            .step_by(self.options.minute_step.max(1) as usize)
            .map(|minute| WheelPickerOption::new(minute, format!("{minute:02}")))
            .collect()
    }

    fn seconds(&self) -> Vec<WheelPickerOption<i32>> {
        (0..=59).map(|second| WheelPickerOption::new(second, format!("{second:02}"))).collect()
    }

    fn periods(&self) -> Vec<WheelPickerOption<i32>> {
        vec![WheelPickerOption::new(0, "AM"), WheelPickerOption::new(1, "PM")]
    }

    fn wheel(&self, label: &str, options: Vec<WheelPickerOption<i32>>, value: Binding<i32>) -> AnyView {
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
    fn body(&self) -> AnyView {
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
            columns.push(self.wheel("AM or PM", self.periods(), self.period()));
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
        let Some(option) = self.options.get(index) else { return };
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
            state: AccessibilityState { is_enabled: self.is_enabled, ..Default::default() },
            ..Default::default()
        }));

        let hosted = self.snapshot();
        let increment = hosted.clone();
        let decrement = hosted.clone();
        let set_value = hosted.clone();
        node.set_accessibility_actions(AccessibilityActions {
            increment: self.is_enabled.then(|| Rc::new(move || increment.step(1)) as Rc<dyn Fn()>),
            decrement: self.is_enabled.then(|| Rc::new(move || decrement.step(-1)) as Rc<dyn Fn()>),
            set_value: self.is_enabled.then(|| {
                Rc::new(move |text: &str| {
                    if let Some(index) = set_value.options.iter().position(|option| option.label == text) {
                        set_value.choose(index);
                    }
                }) as Rc<dyn Fn(&str)>
            }),
            ..Default::default()
        });

        let weak = node.downgrade();
        node.update_draw(height.to_bits(), move |list, origin| {
            let Some(owner) = Node::upgrade(&weak) else { return };
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

        let Some(registry) = InteractionRegistryHolder::current() else { return };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }

        let hover_node = node.downgrade();
        registry.set_hover(node, Rc::new(move |phase| {
            let Some(owner) = Node::upgrade(&hover_node) else { return };
            let next = phase == HoverPhase::Enter && supports_control_hover(&owner);
            if owner.attachment::<bool>(WHEEL_HOVERED) != Some(next) {
                owner.set_attachment(WHEEL_HOVERED, next);
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set("timeWheelHover"));
            }
        }));

        let pointer = self.snapshot();
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != MouseButton::Left || event_phase == EventPhase::Capture {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else { return EventResult::Ignored };
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
            let Some(option) = picker.options.get(index) else { continue };
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
                    FlexDirection::Row, Align::Center, Justify::Center, Wrap::NoWrap, 0.0,
                    AnyView::new(Text::new(option.label.clone()).semantic_font(font).semantic_foreground(tint)),
                )
                .opacity(alpha)
                .frame_height(height),
            ));
        }
        vec![AnyView::new(
            BoxView::new(
                FlexDirection::Column, Align::Stretch, Justify::Center, Wrap::NoWrap, 0.0,
                AnyView::new(TupleView::new(rows)),
            )
            .frame_width(WHEEL_WIDTH),
        )]
    }
}
impl_primitive_view!(TimeWheel);
