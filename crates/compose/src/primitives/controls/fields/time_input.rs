//! Translation of Primitives/Controls/Fields/TimeInput.swift.
//!
//! A segmented time field: hours, minutes and (in a 12-hour cycle) AM/PM each
//! take focus. Up/Down steps the focused segment, digits set it and advance to
//! the next, Left/Right move between segments, and the first letter of AM/PM
//! chooses the half of the day. All stepping and digit buffering lives in
//! [`TimeInputState`]; this module is the view half.
use crate::Scope;
use std::cell::RefCell;
use std::rc::Rc;

use cove_core::Binding;
use cove_core::scene::{
    AccessibilityRole, AccessibilitySemantics, AccessibilityState, Align, EventPhase, EventResult,
    FlexDirection, FocusChainHolder, InputHandlerRoute, InteractionRegistryHolder, Justify,
    LayoutNode, Node, Wrap,
};
use cove_platform::{MouseButton, SystemCursor};

use crate::modifiers::layout::PaddingExt;
use crate::modifiers::style::SemanticStyleViewExt;
use crate::primitives::controls::date_values::TimeOfDay;
use crate::primitives::controls::fields::time_input_model::{
    TimeInputHourCycle, TimeInputState, TimeSegment,
};
use crate::primitives::text::Text;
use crate::theme::styles::components::InputChromeState;
use crate::theme::{SemanticColorRef, SemanticFontRef, theme_for_node};
use crate::{AnyView, BoxView, PrimitiveView, TupleView, View, impl_primitive_view};

/// Segment padding.
const SEGMENT_PADDING_HORIZONTAL: f32 = 6.0;
const SEGMENT_PADDING_VERTICAL: f32 = 3.0;
/// Segment corner radius.
const SEGMENT_RADIUS: f32 = 4.0;
/// Field chrome padding.
const FIELD_PADDING_HORIZONTAL: f32 = 8.0;
const FIELD_PADDING_VERTICAL: f32 = 5.0;

/// Segmented time field.
pub struct TimeInput {
    pub selection: Binding<Option<TimeOfDay>>,
    pub hour_cycle: TimeInputHourCycle,
    pub is_enabled: bool,
    pub accessibility_label: Option<String>,
    state: Rc<RefCell<TimeInputState>>,
    segments: Rc<RefCell<Vec<Node>>>,
}

impl TimeInput {
    pub fn new(selection: Binding<Option<TimeOfDay>>) -> Self {
        Self {
            state: Rc::new(RefCell::new(TimeInputState::new(selection.get()))),
            selection,
            hour_cycle: TimeInputHourCycle::TwentyFour,
            is_enabled: true,
            accessibility_label: None,
            segments: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn hour_cycle(mut self, hour_cycle: TimeInputHourCycle) -> Self {
        self.hour_cycle = hour_cycle;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Mirrors the bound value into the edit state and reads the display text.
    fn state_phase(&self) -> TimeInputState {
        let mut state = self.state.borrow().clone();
        state.value = self.selection.get();
        state
    }

    fn segment(&self, kind: TimeSegment) -> AnyView {
        let state = self.state_phase();
        let text = match kind {
            TimeSegment::Hour => state.hour_text(self.hour_cycle),
            TimeSegment::Minute => state.minute_text(),
            TimeSegment::Period => state.period_text().to_string(),
        };
        AnyView::new(TimeSegmentLeaf {
            kind,
            text,
            is_enabled: self.is_enabled,
            hour_cycle: self.hour_cycle,
            state: self.state.clone(),
            selection: self.selection.clone(),
            segments: self.segments.clone(),
        })
    }
}

impl View for TimeInput {
    fn body(&self, _cx: &Scope) -> AnyView {
        let mut segments = vec![
            self.segment(TimeSegment::Hour),
            Self::separator(),
            self.segment(TimeSegment::Minute),
        ];
        if self.hour_cycle == TimeInputHourCycle::Twelve {
            segments.push(self.segment(TimeSegment::Period));
        }
        AnyView::new(TimeInputFrame {
            is_enabled: self.is_enabled,
            accessibility_label: self.accessibility_label.clone(),
            content: AnyView::new(
                BoxView::new(
                    FlexDirection::Row,
                    Align::Center,
                    Justify::FlexStart,
                    Wrap::NoWrap,
                    2.0,
                    AnyView::new(TupleView::new(segments)),
                )
                .padding_edges(FIELD_PADDING_HORIZONTAL, FIELD_PADDING_VERTICAL),
            ),
        })
    }
}

impl TimeInput {
    fn separator() -> AnyView {
        AnyView::new(
            Text::new(":")
                .semantic_font(SemanticFontRef::body())
                .semantic_foreground(SemanticColorRef::on_surface_muted()),
        )
    }
}

/// Field chrome and group semantics.
struct TimeInputFrame {
    is_enabled: bool,
    accessibility_label: Option<String>,
    content: AnyView,
}

impl PrimitiveView for TimeInputFrame {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let state = InputChromeState::new(self.is_enabled);
        node.set_background_color(Some(theme.inputs.background_for(state)));
        node.set_corner_radius(theme.inputs.radius);
        node.set_border_color(Some(theme.inputs.border_for(state)));
        node.set_border_width(theme.inputs.border_width_for(state));
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: self.accessibility_label.clone().unwrap_or_default(),
            state: AccessibilityState {
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
impl_primitive_view!(TimeInputFrame);

/// One focusable segment.
struct TimeSegmentLeaf {
    kind: TimeSegment,
    text: String,
    is_enabled: bool,
    hour_cycle: TimeInputHourCycle,
    state: Rc<RefCell<TimeInputState>>,
    selection: Binding<Option<TimeOfDay>>,
    segments: Rc<RefCell<Vec<Node>>>,
}

impl TimeSegmentLeaf {
    fn label(&self) -> &'static str {
        match self.kind {
            TimeSegment::Hour => "Hours",
            TimeSegment::Minute => "Minutes",
            TimeSegment::Period => "AM/PM",
        }
    }

    fn mutate(&self, update: impl FnOnce(&mut TimeInputState)) {
        let mut state = self.state.borrow_mut();
        state.value = self.selection.get();
        update(&mut state);
        if state.value != self.selection.get() {
            self.selection.set(state.value);
        }
    }

    /// Moves focus to the neighbouring segment of the same field.
    fn focus_neighbor(&self, node: &Node, offset: isize) -> bool {
        let Some(chain) = FocusChainHolder::current() else {
            return false;
        };
        let segments = self.segments.borrow();
        let Some(index) = segments.iter().position(|segment| segment == node) else {
            return false;
        };
        let next = index as isize + offset;
        if next < 0 || next as usize >= segments.len() {
            return false;
        }
        chain.focus(Some(&segments[next as usize]), true);
        true
    }

    fn digit(scancode: u32) -> Option<i32> {
        match scancode {
            crate::Scancode::DIGIT0 => Some(0),
            crate::Scancode::DIGIT1 => Some(1),
            crate::Scancode::DIGIT2 => Some(2),
            crate::Scancode::DIGIT3 => Some(3),
            crate::Scancode::DIGIT4 => Some(4),
            crate::Scancode::DIGIT5 => Some(5),
            crate::Scancode::DIGIT6 => Some(6),
            crate::Scancode::DIGIT7 => Some(7),
            crate::Scancode::DIGIT8 => Some(8),
            crate::Scancode::DIGIT9 => Some(9),
            _ => None,
        }
    }

    fn letter(keycode: u32) -> Option<char> {
        match keycode {
            key if key == 'a' as u32 || key == 'A' as u32 => Some('a'),
            key if key == 'p' as u32 || key == 'P' as u32 => Some('p'),
            _ => None,
        }
    }
}

impl PrimitiveView for TimeSegmentLeaf {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(self.is_enabled);
        node.set_is_focusable(self.is_enabled);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_flex_direction(FlexDirection::Row);
        layout.set_align_items(Align::Center);
        layout.set_justify_content(Justify::Center);
        Some(layout)
    }

    fn update_node(&self, node: &Node) {
        {
            let mut segments = self.segments.borrow_mut();
            if !segments.iter().any(|segment| segment == node) {
                segments.push(node.clone());
            }
        }
        node.set_is_focusable(self.is_enabled);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Button,
            label: self.label().to_string(),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            combines_children: true,
            ..Default::default()
        }));

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            return;
        }

        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, event_phase| {
                if event.button != MouseButton::Left
                    || phase != cove_core::scene::PointerPhase::Down
                    || event_phase == EventPhase::Capture
                {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                if let Some(chain) = FocusChainHolder::current() {
                    chain.focus(Some(&owner), true);
                }
                EventResult::Handled
            }),
        );

        let leaf = self.snapshot();
        let key_node = node.downgrade();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target || event.is_repeat {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    crate::Scancode::ARROW_UP => {
                        leaf.step(1);
                        EventResult::Handled
                    }
                    crate::Scancode::ARROW_DOWN => {
                        leaf.step(-1);
                        EventResult::Handled
                    }
                    crate::Scancode::ARROW_LEFT | crate::Scancode::ARROW_RIGHT => {
                        let Some(owner) = Node::upgrade(&key_node) else {
                            return EventResult::Ignored;
                        };
                        let offset = if event.scancode == crate::Scancode::ARROW_LEFT {
                            -1
                        } else {
                            1
                        };
                        if leaf.focus_neighbor(&owner, offset) {
                            EventResult::Handled
                        } else {
                            EventResult::Ignored
                        }
                    }
                    _ => {
                        if let Some(digit) = Self::digit(event.scancode)
                            && leaf.kind != TimeSegment::Period
                        {
                            let advanced = leaf.type_digit(digit);
                            if advanced && let Some(owner) = Node::upgrade(&key_node) {
                                leaf.focus_neighbor(&owner, 1);
                            }
                            return EventResult::Handled;
                        }
                        if leaf.kind == TimeSegment::Period
                            && let Some(letter) = Self::letter(event.keycode)
                        {
                            leaf.mutate(|state| {
                                state.type_period(letter == 'p');
                            });
                            return EventResult::Handled;
                        }
                        EventResult::Ignored
                    }
                }
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(SegmentLabel {
            kind: self.kind,
            text: self.text.clone(),
            is_enabled: self.is_enabled,
        })]
    }
}
impl_primitive_view!(TimeSegmentLeaf);

impl TimeSegmentLeaf {
    fn snapshot(&self) -> TimeSegmentLeaf {
        TimeSegmentLeaf {
            kind: self.kind,
            text: self.text.clone(),
            is_enabled: self.is_enabled,
            hour_cycle: self.hour_cycle,
            state: self.state.clone(),
            selection: self.selection.clone(),
            segments: self.segments.clone(),
        }
    }

    fn step(&self, delta: i32) {
        match self.kind {
            TimeSegment::Hour => {
                let cycle = self.hour_cycle;
                self.mutate(|state| state.step_hour(delta, cycle));
            }
            TimeSegment::Minute => self.mutate(|state| state.step_minute(delta)),
            TimeSegment::Period => {
                self.mutate(|state| {
                    state.toggle_period();
                });
            }
        }
    }

    fn type_digit(&self, digit: i32) -> bool {
        let cycle = self.hour_cycle;
        match self.kind {
            TimeSegment::Hour => {
                let mut advanced = false;
                self.mutate(|state| {
                    advanced = state.type_hour(digit, cycle);
                });
                advanced
            }
            TimeSegment::Minute => {
                let mut advanced = false;
                self.mutate(|state| {
                    advanced = state.type_minute(digit);
                });
                advanced
            }
            TimeSegment::Period => false,
        }
    }
}

/// The styled label of one segment, tinted while focused.
struct SegmentLabel {
    kind: TimeSegment,
    text: String,
    is_enabled: bool,
}

impl PrimitiveView for SegmentLabel {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        let theme = theme_for_node(node);
        let is_focused = FocusChainHolder::current()
            .and_then(|chain| chain.focused())
            .is_some_and(|focused| focused == node.parent().unwrap_or_else(|| node.clone()));
        let (fill, ink) = if is_focused {
            (Some(theme.colors.accent), theme.colors.on_accent)
        } else {
            (None, theme.colors.on_surface)
        };
        node.set_background_color(fill);
        node.set_corner_radius(SEGMENT_RADIUS);
        node.set_foreground_color(Some(if self.is_enabled {
            ink
        } else {
            theme.colors.on_surface_muted
        }));
        let _ = self.kind;
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        vec![AnyView::new(
            Text::new(self.text.clone())
                .semantic_font(SemanticFontRef::body())
                .padding_edges(SEGMENT_PADDING_HORIZONTAL, SEGMENT_PADDING_VERTICAL),
        )]
    }
}
impl_primitive_view!(SegmentLabel);
