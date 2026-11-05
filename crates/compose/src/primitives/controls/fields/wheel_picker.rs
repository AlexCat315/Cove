//! Translation of `Primitives/Controls/WheelPicker.swift`.
//!
//! A five-row wheel. Pointer drags follow the finger, wheel and arrow keys move
//! one option, and the selection lands only on enabled options.
//!
//! Behaviour checklist:
//! - an option is identified by its value, not its label
//! - a disabled option stays visible but cannot be committed
//! - stepping skips disabled rows in either direction
//! - stepping stops at the ends rather than wrapping
//! - an empty wheel has no selection
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align,
    EventPhase, EventResult, FlexDirection, FocusChainHolder, InputHandlerRoute,
    InteractionRegistryHolder, Justify, LayoutNode, Node, PointerCaptureHolder, PointerPhase, Wrap,
};
use cove_core::{Binding, Environment, InputCapabilities, State};
use cove_platform::{MouseButton, SystemCursor};

use crate::Scope;
use crate::adaptive::SurfaceReader;
use crate::animation::geometry::TransformViewExt;
use crate::modifiers::layout::LayoutFrameExt;
use crate::modifiers::style::{AppearanceExt, SemanticStyleViewExt};
use crate::primitives::controls::adaptive::apply_touch_control_extent;
use crate::primitives::layout::zstack::ZStack;
use crate::primitives::text::Text;
use crate::theme::{
    SemanticColorRef, SemanticFontRef, SemanticMotionRef, semantic_animation, theme_for_node,
};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};

const POINTER_ROW_HEIGHT: f32 = 32.0;
const TOUCH_ROW_HEIGHT: f32 = 44.0;
/// Pointer travel below this is a click on a row rather than a drag.
const CLICK_SLOP: f32 = crate::primitives::interaction::DRAG_THRESHOLD;
const DRAG_START: &str = "__wheel_drag_start";
const DRAG_BASE: &str = "__wheel_drag_base";

/// One row of a wheel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WheelPickerOption<Value> {
    pub value: Value,
    pub label: String,
    pub is_enabled: bool,
}

impl<Value: PartialEq + Clone> WheelPickerOption<Value> {
    pub fn new(value: Value, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            is_enabled: true,
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }
}

/// The rows of one wheel and the value committed among them.
#[derive(Clone, Debug, PartialEq)]
pub struct WheelPickerModel<Value> {
    pub options: Vec<WheelPickerOption<Value>>,
    value: Option<Value>,
}

impl<Value: PartialEq + Clone> WheelPickerModel<Value> {
    /// A wheel starting on `value`, or on the first enabled row when `value` is
    /// not present or is disabled.
    pub fn new(options: Vec<WheelPickerOption<Value>>, value: Option<Value>) -> Self {
        let mut picker = Self {
            options,
            value: None,
        };
        let landing = value
            .filter(|candidate| {
                picker
                    .index_of(candidate)
                    .is_some_and(|index| picker.options[index].is_enabled)
            })
            .or_else(|| picker.first_enabled().cloned());
        picker.value = landing;
        picker
    }

    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    pub fn index_of(&self, value: &Value) -> Option<usize> {
        self.options
            .iter()
            .position(|option| &option.value == value)
    }

    fn first_enabled(&self) -> Option<&Value> {
        self.options
            .iter()
            .find(|option| option.is_enabled)
            .map(|option| &option.value)
    }

    /// Whether a row may be committed.
    pub fn is_selectable(&self, index: usize) -> bool {
        self.options
            .get(index)
            .is_some_and(|option| option.is_enabled)
    }

    /// Moves the selection by `delta` rows, skipping disabled ones and stopping
    /// at the ends.
    ///
    /// Returns whether the selection moved.
    pub fn step(&mut self, delta: i32) -> bool {
        let Some(current) = self.value.as_ref().and_then(|value| self.index_of(value)) else {
            return false;
        };
        let step: i64 = if delta < 0 { -1 } else { 1 };
        let mut index = current as i64 + step;
        while (0..self.options.len() as i64).contains(&index) {
            if self.options[index as usize].is_enabled {
                self.value = Some(self.options[index as usize].value.clone());
                return true;
            }
            index += step;
        }
        // A delta of zero, or a wall of disabled rows, leaves the selection alone.
        false
    }

    /// Jumps to the first enabled row.
    pub fn select_first(&mut self) -> bool {
        match self.first_enabled() {
            Some(value) => {
                let changed = self.value.as_ref() != Some(value);
                self.value = Some(value.clone());
                changed
            }
            None => false,
        }
    }

    /// Jumps to the last enabled row.
    pub fn select_last(&mut self) -> bool {
        let landing = self
            .options
            .iter()
            .rev()
            .find(|option| option.is_enabled)
            .map(|option| option.value.clone());
        match landing {
            Some(value) => {
                let changed = self.value.as_ref() != Some(&value);
                self.value = Some(value);
                changed
            }
            None => false,
        }
    }

    /// The label of the committed row, which is what a badge shows.
    pub fn selected_label(&self) -> Option<&str> {
        self.value
            .as_ref()
            .and_then(|value| self.index_of(value))
            .map(|index| self.options[index].label.as_str())
    }

    /// The rows visible around the selection, at most `count` of them, centred
    /// on the committed value where the ends allow.
    pub fn visible_window(&self, count: usize) -> std::ops::Range<usize> {
        if count == 0 || self.options.is_empty() {
            return 0..0;
        }
        let Some(current) = self.value.as_ref().and_then(|value| self.index_of(value)) else {
            return 0..count.min(self.options.len());
        };
        let total = self.options.len();
        let half = count / 2;
        let mut start = current.saturating_sub(half);
        // Slide the whole window back when it runs past the end, so it stays full
        // rather than shrinking near the last row.
        if start + count > total {
            start = total.saturating_sub(count);
        }
        start..(start + count).min(total)
    }
}

/// The bound wheel control.
pub struct WheelPicker<Value: PartialEq + Clone + 'static> {
    pub selection: Binding<Value>,
    pub options: Vec<WheelPickerOption<Value>>,
    pub is_enabled: bool,
    pub visible_rows: usize,
    pub label: String,
    pub on_change: Option<Rc<dyn Fn(&Value)>>,
    drag_offset: State<f32>,
}

impl<Value: PartialEq + Clone + 'static> WheelPicker<Value> {
    pub fn new(selection: Binding<Value>, options: Vec<WheelPickerOption<Value>>) -> Self {
        Self {
            selection,
            options,
            is_enabled: true,
            visible_rows: 5,
            label: crate::i18n::tr("Wheel picker"),
            on_change: None,
            drag_offset: State::new(0.0),
        }
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    /// An odd row count from three to nine, so one row sits on the centre band.
    pub fn visible_rows(mut self, visible_rows: usize) -> Self {
        self.visible_rows = (visible_rows | 1).clamp(3, 9);
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn on_change(mut self, handler: impl Fn(&Value) + 'static) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    fn selected_index(&self) -> usize {
        let selected = self.selection.get();
        self.options
            .iter()
            .position(|option| option.value == selected)
            .or_else(|| self.enabled_indices().first().copied())
            .unwrap_or(0)
    }

    fn enabled_indices(&self) -> Vec<usize> {
        self.options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| option.is_enabled.then_some(index))
            .collect()
    }

    fn row(&self, index: usize, distance: f32, row_height: f32) -> AnyView {
        let option = &self.options[index];
        let is_centered = distance.abs() < 0.5;
        let alpha = if option.is_enabled {
            (1.0 - distance.abs() * 0.22).max(0.18)
        } else {
            0.4
        };
        let tint = if !option.is_enabled {
            SemanticColorRef::on_surface_muted()
        } else if is_centered {
            SemanticColorRef::on_surface()
        } else {
            SemanticColorRef::on_surface_variant()
        };
        let font = if is_centered {
            SemanticFontRef::body_strong()
        } else {
            SemanticFontRef::body()
        };
        AnyView::new(
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
            .frame_height(row_height)
            .opacity(alpha)
            .offset(0.0, distance * row_height),
        )
    }

    fn rows(&self, row_height: f32) -> AnyView {
        if self.options.is_empty() {
            return AnyView::new(
                Text::new(crate::i18n::tr("No options"))
                    .semantic_font(SemanticFontRef::body())
                    .semantic_foreground(SemanticColorRef::on_surface_muted()),
            );
        }
        let selected = self.selected_index();
        let reach = self.visible_rows + 2;
        let drag = self.drag_offset.get() / row_height.max(1.0);
        let drag = if drag.is_finite() { drag } else { 0.0 };
        let lower = selected.saturating_sub(reach);
        let upper = (selected + reach).min(self.options.len() - 1);
        AnyView::new(TupleView::new(
            (lower..=upper)
                .map(|index| self.row(index, index as f32 - selected as f32 + drag, row_height))
                .collect(),
        ))
    }
}

impl<Value: PartialEq + Clone + 'static> View for WheelPicker<Value> {
    fn body(&self, _cx: &Scope) -> AnyView {
        let wheel = self.snapshot();
        AnyView::new(SurfaceReader::new(move |surface: &Environment| {
            let row_height = if surface.input.contains(InputCapabilities::TOUCH) {
                TOUCH_ROW_HEIGHT
            } else {
                POINTER_ROW_HEIGHT
            };
            let total = row_height * wheel.visible_rows as f32;
            let gesture = WheelGestureLayer {
                wheel: wheel.snapshot(),
                row_height,
            };
            AnyView::new(
                ZStack::new(TupleView::new(vec![
                    AnyView::new(WheelBand { row_height, total }),
                    AnyView::new(ZStack::new(wheel.rows(row_height))),
                    AnyView::new(gesture),
                ]))
                .frame_height(total)
                .clipped(),
            )
        }))
    }

    fn dynamic_properties(&self) -> Vec<crate::DynamicField> {
        vec![crate::DynamicField::state("drag_offset", &self.drag_offset)]
    }

    fn tag(&self) -> &'static str {
        "WheelPicker"
    }
}

impl<Value: PartialEq + Clone + 'static> WheelPicker<Value> {
    fn snapshot(&self) -> Self {
        Self {
            selection: self.selection.clone(),
            options: self.options.clone(),
            is_enabled: self.is_enabled,
            visible_rows: self.visible_rows,
            label: self.label.clone(),
            on_change: self.on_change.clone(),
            drag_offset: self.drag_offset.clone(),
        }
    }

    fn settle(&self, node: &Node, body: impl FnOnce()) {
        let theme = theme_for_node(node);
        let animation = (!node.prefers_reduced_motion())
            .then(|| semantic_animation(&SemanticMotionRef::fast(), &theme));
        cove_core::scene::with_animation(animation, body);
    }

    /// Commits `index` when it is enabled; otherwise the wheel springs back.
    fn choose(&self, index: i64, node: &Node) {
        if self.options.is_empty() {
            return;
        }
        let target = index.clamp(0, self.options.len() as i64 - 1) as usize;
        let current = self.selected_index();
        self.settle(node, || {
            if target != current && self.options[target].is_enabled {
                let value = self.options[target].value.clone();
                self.selection.set(value.clone());
                if let Some(handler) = self.on_change.as_ref() {
                    handler(&value);
                }
            }
            self.drag_offset.set(0.0);
        });
    }

    /// Moves by `delta` enabled rows, stopping at the ends.
    fn step(&self, delta: i64, node: &Node) {
        let enabled = self.enabled_indices();
        let Some(position) = enabled
            .iter()
            .position(|index| *index == self.selected_index())
        else {
            return;
        };
        let target = (position as i64 + delta).clamp(0, enabled.len() as i64 - 1) as usize;
        self.choose(enabled[target] as i64, node);
    }
}

/// The centre band marking the committed row.
struct WheelBand {
    row_height: f32,
    total: f32,
}

impl PrimitiveView for WheelBand {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        self.update_layout(&layout);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.total));
        layout.set_align_self(Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        let band_height = (self.row_height - 4.0).max(0.0);
        let total = self.total;
        let weak = node.downgrade();
        node.update_draw(
            (band_height.to_bits(), total.to_bits()),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let theme = theme_for_node(&owner);
                let rect = cove_core::Rect::new(
                    origin.x,
                    origin.y + (total - band_height) * 0.5,
                    owner.frame().width,
                    band_height,
                );
                let _ = list.add_rounded_rect(
                    rect,
                    theme.radius.sm,
                    theme
                        .colors
                        .surface_variant
                        .multiplied_alpha(owner.opacity()),
                );
            },
        );
    }
}
impl_primitive_view!(WheelBand);

/// Owns the wheel's hit testing, focus and semantics; the rows beneath it are
/// decorative and must not be read twice.
struct WheelGestureLayer<Value: PartialEq + Clone + 'static> {
    wheel: WheelPicker<Value>,
    row_height: f32,
}

impl<Value: PartialEq + Clone + 'static> PrimitiveView for WheelGestureLayer<Value> {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(self.wheel.is_enabled);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        self.update_layout(&layout);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(self.row_height * self.wheel.visible_rows as f32));
        layout.set_align_self(Align::Stretch);
    }

    fn update_node(&self, node: &Node) {
        apply_touch_control_extent(node, true);
        let wheel = &self.wheel;
        node.set_is_focusable(wheel.is_enabled);
        node.set_cursor(Some(if wheel.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));
        let value = wheel
            .options
            .get(wheel.selected_index())
            .map(|option| option.label.clone())
            .unwrap_or_default();
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: wheel.label.clone(),
            value,
            help: crate::i18n::tr("Use arrow keys, Home or End to choose a value"),
            state: AccessibilityState {
                is_enabled: wheel.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));
        if !wheel.is_enabled {
            node.set_accessibility_actions(AccessibilityActions::default());
            if let Some(registry) = InteractionRegistryHolder::current() {
                registry.remove(node);
            }
            return;
        }

        let action = |delta: i64| {
            let hosted = wheel.snapshot();
            let weak = node.downgrade();
            Rc::new(move || {
                if let Some(owner) = Node::upgrade(&weak) {
                    hosted.step(delta, &owner);
                }
            }) as Rc<dyn Fn()>
        };
        let set_value = {
            let hosted = wheel.snapshot();
            let weak = node.downgrade();
            Rc::new(move |raw: &str| {
                if let (Some(owner), Some(index)) = (
                    Node::upgrade(&weak),
                    hosted.options.iter().position(|option| option.label == raw),
                ) {
                    hosted.choose(index as i64, &owner);
                }
            }) as Rc<dyn Fn(&str)>
        };
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(action(1)),
            decrement: Some(action(-1)),
            set_value: Some(set_value),
            ..Default::default()
        });

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        let row_height = self.row_height;

        let pointer = wheel.snapshot();
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
                        owner.set_attachment(DRAG_START, event.y);
                        owner.set_attachment(DRAG_BASE, pointer.drag_offset.get());
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        EventResult::Handled
                    }
                    PointerPhase::Up => {
                        let start = owner.attachment::<f32>(DRAG_START);
                        owner.remove_attachment(DRAG_START);
                        owner.remove_attachment(DRAG_BASE);
                        if PointerCaptureHolder::current()
                            .and_then(|capture| capture.target())
                            .is_some_and(|target| target == owner)
                            && let Some(capture) = PointerCaptureHolder::current()
                        {
                            capture.release();
                        }
                        let Some(start) = start else {
                            return EventResult::Ignored;
                        };
                        let current = pointer.selected_index() as i64;
                        let steps = if (event.y - start).abs() < CLICK_SLOP {
                            let frame = owner.absolute_frame();
                            ((event.y - (frame.y + frame.height * 0.5)) / row_height.max(1.0))
                                .round() as i64
                        } else {
                            (-pointer.drag_offset.get() / row_height.max(1.0)).round() as i64
                        };
                        pointer.choose(current + steps, &owner);
                        EventResult::Handled
                    }
                }
            }),
        );

        let motion = wheel.snapshot();
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _phase| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                let (Some(start), Some(base)) = (
                    owner.attachment::<f32>(DRAG_START),
                    owner.attachment::<f32>(DRAG_BASE),
                ) else {
                    return EventResult::Ignored;
                };
                motion.drag_offset.set(base + (event.y - start));
                EventResult::Handled
            }),
        );

        let scroll = wheel.snapshot();
        let scroll_node = node.downgrade();
        registry.set_wheel(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target || event.y == 0.0 {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&scroll_node) else {
                    return EventResult::Ignored;
                };
                scroll.step(if event.y > 0.0 { -1 } else { 1 }, &owner);
                EventResult::Handled
            }),
        );

        let key = wheel.snapshot();
        let key_node = node.downgrade();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target || event.is_repeat {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&key_node) else {
                    return EventResult::Ignored;
                };
                match event.scancode {
                    crate::Scancode::ARROW_UP | crate::Scancode::ARROW_LEFT => {
                        key.step(-1, &owner);
                    }
                    crate::Scancode::ARROW_DOWN | crate::Scancode::ARROW_RIGHT => {
                        key.step(1, &owner);
                    }
                    crate::Scancode::HOME => {
                        let first = key.enabled_indices().first().copied().unwrap_or(0);
                        key.choose(first as i64, &owner);
                    }
                    crate::Scancode::END => {
                        let last = key.enabled_indices().last().copied().unwrap_or(0);
                        key.choose(last as i64, &owner);
                    }
                    _ => return EventResult::Ignored,
                }
                EventResult::Handled
            }),
        );
    }
}

impl<Value: PartialEq + Clone + 'static> View for WheelGestureLayer<Value> {
    fn kind(&self) -> crate::ViewKind<'_> {
        crate::ViewKind::Primitive(self)
    }

    fn tag(&self) -> &'static str {
        "WheelGestureLayer"
    }
}
