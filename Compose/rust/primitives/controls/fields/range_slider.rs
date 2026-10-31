//! Translation of Primitives/Controls/Fields/RangeSlider.swift.
//!
//! Two-thumb range control. The active thumb is chosen by pointer proximity,
//! then moved with pointer drag, arrow keys, Home/End or accessibility
//! increment/decrement. Values are clamped to the authored range and kept at
//! least `minimum_span` apart.
//!
//! Like the single-thumb [`Slider`](super::slider::Slider), the public struct
//! delegates to a focusable host leaf that owns hit testing, keyboard routing
//! and the draw hook.
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventPhase, EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, Node, PointerCaptureHolder, PointerPhase, Rect,
};
use cove_core::{Binding, Point};
use cove_platform::{MouseButton, SystemCursor};

use crate::primitives::controls::adaptive::{apply_touch_control_extent, supports_control_hover};
use crate::theme::styles::components::{
    AnySliderStyle, SliderStyleConfiguration, SliderStyleEnvironment,
};
use crate::theme::theme_for_node;
use crate::{AnyView, PrimitiveView, View, impl_primitive_view};

const PRESSED: &str = "__range_slider_pressed";
const HOVERED: &str = "__range_slider_hovered";
const ACTIVE: &str = "__range_slider_active";

/// Track height, matching the single-thumb slider.
const TRACK_HEIGHT: f32 = 24.0;
/// Minimum track width.
const MIN_WIDTH: f32 = 160.0;

/// Which thumb a drag or key press moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeThumb {
    Lower,
    Upper,
}

impl RangeThumb {
    fn key(self) -> &'static str {
        match self {
            Self::Lower => "lower",
            Self::Upper => "upper",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "lower" => Some(Self::Lower),
            "upper" => Some(Self::Upper),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Lower => "Range start",
            Self::Upper => "Range end",
        }
    }
}

/// A closed numeric range.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RangeValue {
    pub lower: f64,
    pub upper: f64,
}

impl RangeValue {
    pub fn new(lower: f64, upper: f64) -> Self {
        Self { lower, upper }
    }
}

/// Two-thumb range control.
pub struct RangeSlider {
    pub values: Binding<RangeValue>,
    pub range: (f64, f64),
    pub step: Option<f64>,
    pub minimum_span: f64,
    pub is_enabled: bool,
    pub on_editing_changed: Option<Rc<dyn Fn(bool)>>,
}

impl RangeSlider {
    pub fn new(values: Binding<RangeValue>) -> Self {
        Self {
            values,
            range: (0.0, 1.0),
            step: None,
            minimum_span: 0.0,
            is_enabled: true,
            on_editing_changed: None,
        }
    }

    pub fn range(mut self, range: (f64, f64)) -> Self {
        self.range = range;
        self
    }

    /// A non-finite or non-positive step is dropped, as in the source.
    pub fn step(mut self, step: f64) -> Self {
        self.step = if step.is_finite() && step > 0.0 {
            Some(step)
        } else {
            None
        };
        self
    }

    /// The gap the thumbs must keep; a non-finite value becomes zero.
    pub fn minimum_span(mut self, minimum_span: f64) -> Self {
        self.minimum_span = if minimum_span.is_finite() {
            minimum_span.max(0.0)
        } else {
            0.0
        };
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn on_editing_changed(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.on_editing_changed = Some(Rc::new(handler));
        self
    }

    fn snapshot(&self) -> RangeSlider {
        RangeSlider {
            values: self.values.clone(),
            range: self.range,
            step: self.step,
            minimum_span: self.minimum_span,
            is_enabled: self.is_enabled,
            on_editing_changed: self.on_editing_changed.clone(),
        }
    }
}

impl View for RangeSlider {
    fn body(&self) -> AnyView {
        let snapshot = self.snapshot();
        AnyView::new(RangeSliderHost {
            values: snapshot.values,
            range: snapshot.range,
            step: snapshot.step,
            minimum_span: snapshot.minimum_span,
            is_enabled: snapshot.is_enabled,
            on_editing_changed: snapshot.on_editing_changed,
        })
    }
}

/// Value arithmetic shared by pointer, keyboard and accessibility paths.
#[derive(Clone, Copy)]
struct RangeGeometry {
    range: (f64, f64),
    step: Option<f64>,
    minimum_span: f64,
}

impl RangeGeometry {
    /// Values forced into the authored range, with the lower bound winning when
    /// the pair arrives inverted.
    fn normalized(self, raw: RangeValue) -> RangeValue {
        let lower = raw.lower.clamp(self.range.0, self.range.1);
        let upper = raw.upper.clamp(self.range.0, self.range.1);
        if lower <= upper {
            RangeValue::new(lower, upper)
        } else {
            RangeValue::new(self.range.0, self.range.1)
        }
    }

    fn snap(self, raw: f64) -> f64 {
        let Some(step) = self.step.filter(|step| *step > 0.0) else {
            return raw.clamp(self.range.0, self.range.1);
        };
        let low = self.range.0;
        let count = ((raw - low) / step).round();
        (low + count * step).clamp(self.range.0, self.range.1)
    }

    /// Moves one thumb to `raw`, honouring the span and the authored range.
    fn move_thumb(self, thumb: RangeThumb, raw: f64, current: RangeValue) -> RangeValue {
        let target = self.snap(raw);
        let mut lower = current.lower;
        let mut upper = current.upper;
        match thumb {
            RangeThumb::Lower => {
                lower = target.min(upper - self.minimum_span).max(self.range.0);
            }
            RangeThumb::Upper => {
                upper = target.max(lower + self.minimum_span).min(self.range.1);
            }
        }
        if lower > upper {
            lower = upper;
        }
        RangeValue::new(lower, upper)
    }

    fn increment(self) -> f64 {
        self.step
            .filter(|step| step.is_finite() && *step > 0.0)
            .unwrap_or((self.range.1 - self.range.0) / 100.0)
    }

    fn fraction(self, value: f64) -> f64 {
        let span = self.range.1 - self.range.0;
        if span <= 0.0 {
            return 0.0;
        }
        ((value - self.range.0) / span).clamp(0.0, 1.0)
    }

    fn value_at(self, fraction: f64) -> f64 {
        self.range.0 + fraction * (self.range.1 - self.range.0)
    }
}

fn set_interaction(node: &Node, key: &str, value: bool) -> bool {
    if node.attachment::<bool>(key) == Some(value) {
        return false;
    }
    node.set_attachment(key, value);
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(key));
    true
}

fn active_thumb(node: &Node) -> RangeThumb {
    node.attachment::<&'static str>(ACTIVE)
        .and_then(RangeThumb::from_key)
        .unwrap_or(RangeThumb::Lower)
}

fn format_value(value: f64) -> String {
    if value == value.round() {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

/// The leaf range slider: one hit-testable node owning both thumbs.
pub struct RangeSliderHost {
    pub values: Binding<RangeValue>,
    pub range: (f64, f64),
    pub step: Option<f64>,
    pub minimum_span: f64,
    pub is_enabled: bool,
    pub on_editing_changed: Option<Rc<dyn Fn(bool)>>,
}

impl RangeSliderHost {
    fn geometry(&self) -> RangeGeometry {
        RangeGeometry {
            range: self.range,
            step: self.step,
            minimum_span: self.minimum_span,
        }
    }

    fn current(&self) -> RangeValue {
        self.geometry().normalized(self.values.get())
    }

    fn write(&self, next: RangeValue) {
        if next != self.current() {
            self.values.set(next);
        }
    }

    fn snapshot(&self) -> RangeSliderHost {
        RangeSliderHost {
            values: self.values.clone(),
            range: self.range,
            step: self.step,
            minimum_span: self.minimum_span,
            is_enabled: self.is_enabled,
            on_editing_changed: self.on_editing_changed.clone(),
        }
    }
}

impl PrimitiveView for RangeSliderHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        layout.set_height(Some(TRACK_HEIGHT));
        layout.set_min_width(Some(MIN_WIDTH));
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_height(Some(TRACK_HEIGHT));
    }

    fn update_node(&self, node: &Node) {
        apply_touch_control_extent(node, true);
        if !supports_control_hover(node) {
            node.set_attachment(HOVERED, false);
        }
        node.set_is_focusable(false);
        let current = self.current();
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Group,
            label: "Range".to_string(),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            combines_children: false,
            ..Default::default()
        }));
        node.set_accessibility_actions(AccessibilityActions::default());

        if !self.is_enabled {
            if PointerCaptureHolder::current()
                .and_then(|capture| capture.target())
                .is_some_and(|target| target == *node)
                && let Some(capture) = PointerCaptureHolder::current()
            {
                capture.release();
            }
            if let Some(chain) = FocusChainHolder::current()
                && chain.focused().is_some_and(|focused| focused == *node)
            {
                chain.clear();
            }
            node.set_attachment(PRESSED, false);
            node.set_attachment(HOVERED, false);
        } else {
            if node.attachment::<bool>(PRESSED).is_none() {
                node.set_attachment(PRESSED, false);
            }
            if node.attachment::<bool>(HOVERED).is_none() {
                node.set_attachment(HOVERED, false);
            }
        }
        if node.attachment::<&'static str>(ACTIVE).is_none() {
            node.set_attachment(ACTIVE, RangeThumb::Lower.key());
        }
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));

        let snapshot = self.snapshot();
        let weak = node.downgrade();
        node.update_draw(
            (
                current.lower.to_bits(),
                current.upper.to_bits(),
                self.range.0.to_bits(),
                self.range.1.to_bits(),
                self.is_enabled,
                active_thumb(node),
            ),
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                snapshot.render(&owner, list, origin);
            },
        );

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
                set_interaction(&owner, HOVERED, next);
            }),
        );

        let pointer = self.snapshot();
        let pointer_node = node.downgrade();
        registry.set_pointer(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase, _event_phase| {
                if event.button != MouseButton::Left {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&pointer_node) else {
                    return EventResult::Ignored;
                };
                match phase {
                    PointerPhase::Down => {
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, true);
                        }
                        if set_interaction(&owner, PRESSED, true)
                            && let Some(handler) = pointer.on_editing_changed.as_ref()
                        {
                            handler(true);
                        }
                        pointer.choose_thumb(&owner, event.x);
                        pointer.write_at(&owner, event.x);
                        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                            "rangeSliderValue",
                        ));
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
                        if set_interaction(&owner, PRESSED, false)
                            && let Some(handler) = pointer.on_editing_changed.as_ref()
                        {
                            handler(false);
                        }
                        EventResult::Handled
                    }
                }
            }),
        );

        let motion = self.snapshot();
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _phase| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                if owner.attachment::<bool>(PRESSED) != Some(true) {
                    return EventResult::Ignored;
                }
                motion.write_at(&owner, event.x);
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "rangeSliderValue",
                ));
                EventResult::Handled
            }),
        );
    }

    fn children(&self, _node: &Node) -> Vec<AnyView> {
        let current = self.current();
        [RangeThumb::Lower, RangeThumb::Upper]
            .into_iter()
            .map(|thumb| {
                AnyView::new(RangeThumbStop {
                    thumb,
                    current,
                    is_enabled: self.is_enabled,
                    values: self.values.clone(),
                    range: self.range,
                    step: self.step,
                    minimum_span: self.minimum_span,
                })
            })
            .collect()
    }
}
impl_primitive_view!(RangeSliderHost);

impl RangeSliderHost {
    /// Picks the thumb nearest the pointer, so a press grabs the obvious one.
    fn choose_thumb(&self, node: &Node, x: f32) {
        let geometry = self.geometry();
        let local = local_fraction(node, x);
        let current = self.current();
        let lower = geometry.fraction(current.lower);
        let upper = geometry.fraction(current.upper);
        let thumb = if (local - lower).abs() <= (local - upper).abs() {
            RangeThumb::Lower
        } else {
            RangeThumb::Upper
        };
        node.set_attachment(ACTIVE, thumb.key());
    }

    /// Writes the active thumb to the pointer's position.
    fn write_at(&self, node: &Node, x: f32) {
        let geometry = self.geometry();
        let thumb = active_thumb(node);
        let raw = geometry.value_at(local_fraction(node, x));
        let next = geometry.move_thumb(thumb, raw, self.current());
        self.write(next);
        self.refresh_accessibility(node);
    }

    /// Keeps the two thumb handles' values in step without a recompose.
    fn refresh_accessibility(&self, node: &Node) {
        let current = self.current();
        for child in node.children() {
            let Some(thumb) = child
                .attachment::<&'static str>("range.thumb")
                .and_then(RangeThumb::from_key)
            else {
                continue;
            };
            let mut semantics = child
                .accessibility()
                .unwrap_or_else(|| AccessibilitySemantics::new(AccessibilityRole::Slider));
            semantics.role = AccessibilityRole::Slider;
            semantics.label = thumb.label().to_string();
            semantics.state.is_enabled = self.is_enabled;
            semantics.value = format_value(match thumb {
                RangeThumb::Lower => current.lower,
                RangeThumb::Upper => current.upper,
            });
            child.set_accessibility(Some(semantics));
        }
    }

    fn render(&self, node: &Node, list: &mut cove_core::DrawList, origin: Point) {
        let theme = theme_for_node(node);
        let style: AnySliderStyle = node.composition_value(&SliderStyleEnvironment::key());
        let current = self.current();
        let active = active_thumb(node);
        let appearance = style.resolve(
            &SliderStyleConfiguration {
                value: match active {
                    RangeThumb::Lower => current.lower,
                    RangeThumb::Upper => current.upper,
                },
                range: self.range,
                is_pressed: node.attachment::<bool>(PRESSED) == Some(true),
                is_hovered: node.attachment::<bool>(HOVERED) == Some(true),
                is_focused: FocusChainHolder::current().is_some_and(|chain| {
                    chain.focused().is_some_and(|focused| focused == *node)
                        && chain.is_focus_visible()
                }),
                is_enabled: self.is_enabled,
            },
            &theme,
        );
        let width = node.frame().width;
        let height = node.frame().height;
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let geometry = RangeGeometry {
            range: self.range,
            step: self.step,
            minimum_span: self.minimum_span,
        };
        let track_y = origin.y + (height - appearance.track_height) * 0.5;
        let _ = list.add_rounded_rect(
            Rect::new(origin.x, track_y, width, appearance.track_height),
            appearance.track_corner_radius,
            appearance.track_color,
        );
        let lower_x = origin.x + geometry.fraction(current.lower) as f32 * width;
        let upper_x = origin.x + geometry.fraction(current.upper) as f32 * width;
        let fill_width = (upper_x - lower_x).max(0.0);
        if fill_width > 0.0 {
            let _ = list.add_rounded_rect(
                Rect::new(lower_x, track_y, fill_width, appearance.track_height),
                appearance.track_corner_radius,
                appearance.fill_color,
            );
        }
        let half = appearance.thumb_length * 0.5;
        for centre in [lower_x, upper_x] {
            let clamped = centre.clamp(origin.x + half, origin.x + width - half);
            let rect = Rect::new(
                clamped - half,
                origin.y + (height - appearance.thumb_diameter) * 0.5,
                appearance.thumb_length,
                appearance.thumb_diameter,
            );
            let corner = rect.width.min(rect.height) * 0.5;
            if let Some(border) = appearance.thumb_border_color
                && appearance.thumb_border_width > 0.0
            {
                let stroke = appearance.thumb_border_width;
                let _ = list.add_rounded_rect(
                    Rect::new(
                        rect.x - stroke,
                        rect.y - stroke,
                        rect.width + 2.0 * stroke,
                        rect.height + 2.0 * stroke,
                    ),
                    corner + stroke,
                    border,
                );
            }
            let _ = list.add_rounded_rect(rect, corner, appearance.thumb_color);
        }
    }
}

/// The pointer's position as a fraction of the track width.
fn local_fraction(node: &Node, x: f32) -> f64 {
    let origin = node.absolute_origin();
    let width = node.frame().width.max(1.0);
    let local = x - origin.x;
    (local / width).clamp(0.0, 1.0) as f64
}

/// One thumb's focus stop.
struct RangeThumbStop {
    thumb: RangeThumb,
    current: RangeValue,
    is_enabled: bool,
    values: Binding<RangeValue>,
    range: (f64, f64),
    step: Option<f64>,
    minimum_span: f64,
}

impl RangeThumbStop {
    fn geometry(&self) -> RangeGeometry {
        RangeGeometry {
            range: self.range,
            step: self.step,
            minimum_span: self.minimum_span,
        }
    }

    fn move_to(&self, raw: f64) {
        let geometry = self.geometry();
        let current = geometry.normalized(self.values.get());
        let next = geometry.move_thumb(self.thumb, raw, current);
        if next != current {
            self.values.set(next);
        }
    }

    fn move_by(&self, delta: f64) {
        let current = self.geometry().normalized(self.values.get());
        let base = match self.thumb {
            RangeThumb::Lower => current.lower,
            RangeThumb::Upper => current.upper,
        };
        self.move_to(base + delta);
    }

    fn value(&self) -> f64 {
        match self.thumb {
            RangeThumb::Lower => self.current.lower,
            RangeThumb::Upper => self.current.upper,
        }
    }
}

impl PrimitiveView for RangeThumbStop {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_attachment("range.thumb", self.thumb.key());
        node.set_is_focusable(self.is_enabled);
        node.set_is_tab_stop(self.is_enabled);
        node.set_is_hit_testable(false);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        None
    }

    fn update_node(&self, node: &Node) {
        node.set_is_focusable(self.is_enabled);
        node.set_is_tab_stop(self.is_enabled);
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: self.thumb.label().to_string(),
            value: format_value(self.value()),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));

        let Some(registry) = InteractionRegistryHolder::current() else {
            return;
        };
        if !self.is_enabled {
            registry.remove(node);
            node.set_accessibility_actions(AccessibilityActions::default());
            return;
        }
        let increment = self.geometry().increment();
        let increment_up = self.snapshot();
        let decrement_up = self.snapshot();
        node.set_accessibility_actions(AccessibilityActions {
            increment: Some(Rc::new(move || increment_up.move_by(increment)) as Rc<dyn Fn()>),
            decrement: Some(Rc::new(move || decrement_up.move_by(-increment)) as Rc<dyn Fn()>),
            ..Default::default()
        });

        let key = self.snapshot();
        let increment = key.geometry().increment();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target {
                    return EventResult::Ignored;
                }
                match event.scancode {
                    crate::Scancode::ARROW_LEFT | crate::Scancode::ARROW_DOWN => {
                        key.move_by(-increment)
                    }
                    crate::Scancode::ARROW_RIGHT | crate::Scancode::ARROW_UP => {
                        key.move_by(increment)
                    }
                    crate::Scancode::HOME => key.move_to(key.range.0),
                    crate::Scancode::END => key.move_to(key.range.1),
                    _ => return EventResult::Ignored,
                }
                EventResult::Handled
            }),
        );
    }
}

impl RangeThumbStop {
    fn snapshot(&self) -> RangeThumbStop {
        RangeThumbStop {
            thumb: self.thumb,
            current: self.current,
            is_enabled: self.is_enabled,
            values: self.values.clone(),
            range: self.range,
            step: self.step,
            minimum_span: self.minimum_span,
        }
    }
}
impl_primitive_view!(RangeThumbStop);
