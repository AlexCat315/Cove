//! Translation of Primitives/Controls/Fields/Slider/Host.swift.
//!
//! The leaf slider: hit testing, value mapping, keyboard routing and the draw
//! hook. Interaction flags live in node attachments so a hover or press repaints
//! without scheduling a recompose.
use std::rc::Rc;

use crate::primitives::controls::adaptive::{apply_touch_control_extent, supports_control_hover};
use crate::theme::styles::components::{
    AnySliderStyle, SliderStyleConfiguration, SliderStyleEnvironment,
};
use crate::theme::theme_for_node;
use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventPhase, EventResult, FocusChainHolder, HoverPhase, InputHandlerRoute,
    InteractionRegistryHolder, LayoutNode, MeasureMode, Node, PointerCaptureHolder, PointerPhase,
};
use cove_core::{Binding, Point, Size};
use cove_platform::{MouseButton, SystemCursor};

pub use super::geometry::{SliderAxis, SliderGeometry};

const PRESSED: &str = "__slider_pressed";
const HOVERED: &str = "__slider_hovered";

#[derive(Clone, Copy, PartialEq)]
struct PaintIdentity {
    value_bits: u64,
    lower_bits: u64,
    upper_bits: u64,
    is_enabled: bool,
    axis: SliderAxis,
    step_bits: Option<u64>,
    shows_ticks: bool,
}

/// One slider surface.
pub(crate) struct SliderHost {
    pub value: Binding<f64>,
    pub range: (f64, f64),
    pub step: Option<f64>,
    pub is_enabled: bool,
    pub on_editing_changed: Option<Rc<dyn Fn(bool)>>,
    pub shows_ticks: bool,
    pub axis: SliderAxis,
}

impl SliderHost {
    /// `value` forced into the authored range.
    pub fn clamped_value(&self, raw: f64) -> f64 {
        raw.min(self.range.1).max(self.range.0)
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

impl PrimitiveView for SliderHost {
    fn make_node(&self) -> Node {
        let node = Node::new();
        node.set_is_hit_testable(true);
        node.set_is_focusable(true);
        node
    }

    fn make_layout_node(&self) -> Option<LayoutNode> {
        let layout = LayoutNode::new();
        self.update_layout(&layout);
        Some(layout)
    }

    fn update_layout(&self, layout: &LayoutNode) {
        layout.set_width(if self.axis.is_horizontal() {
            None
        } else {
            Some(24.0)
        });
        layout.set_height(if self.axis.is_horizontal() {
            Some(24.0)
        } else {
            None
        });
        // A width-only frame restores Yoga's auto height. Keep the control's
        // natural size through measurement, including in center-aligned rows.
        let vertical = !self.axis.is_horizontal();
        layout.set_measure_func(Some(Rc::new(
            move |width, width_mode, height, height_mode| {
                if vertical {
                    let extent = if height_mode == MeasureMode::Undefined {
                        160.0
                    } else {
                        height.min(160.0)
                    };
                    return Size {
                        width: 24.0,
                        height: extent,
                    };
                }
                let extent = if width_mode == MeasureMode::Undefined {
                    160.0
                } else {
                    width.min(160.0)
                };
                Size {
                    width: extent,
                    height: 24.0,
                }
            },
        )));
    }

    fn update_node(&self, node: &Node) {
        apply_touch_control_extent(node, true);
        if !supports_control_hover(node) {
            node.set_attachment(HOVERED, false);
        }
        node.set_is_focusable(self.is_enabled);
        let clamped = self.clamped_value(self.value.get());
        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            value: format!("{clamped}"),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));

        let value_for_set = self.value.clone();
        let host_for_set = Snapshot {
            range: self.range,
            step: self.step,
            shows_ticks: self.shows_ticks,
        };
        let value_for_inc = self.value.clone();
        let host_for_inc = host_for_set;
        let value_for_dec = self.value.clone();
        let host_for_dec = host_for_set;
        node.set_accessibility_actions(AccessibilityActions {
            set_value: self.is_enabled.then(|| {
                Rc::new(move |text: &str| {
                    if let Ok(number) = text.parse::<f64>()
                        && number.is_finite()
                    {
                        value_for_set.set(host_for_set.snap(number));
                    }
                }) as Rc<dyn Fn(&str)>
            }),
            increment: self.is_enabled.then(|| {
                Rc::new(move || {
                    let next = host_for_inc.snap(value_for_inc.get() + host_for_inc.increment());
                    value_for_inc.set(next);
                }) as Rc<dyn Fn()>
            }),
            decrement: self.is_enabled.then(|| {
                Rc::new(move || {
                    let next = host_for_dec.snap(value_for_dec.get() - host_for_dec.increment());
                    value_for_dec.set(next);
                }) as Rc<dyn Fn()>
            }),
            ..Default::default()
        });

        if !self.is_enabled {
            if node.attachment::<bool>(PRESSED) == Some(true)
                && let Some(handler) = self.on_editing_changed.as_ref()
            {
                handler(false);
            }
            node.set_attachment(PRESSED, false);
            node.set_attachment(HOVERED, false);
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
        } else {
            if node.attachment::<bool>(PRESSED).is_none() {
                node.set_attachment(PRESSED, false);
            }
            if node.attachment::<bool>(HOVERED).is_none() {
                node.set_attachment(HOVERED, false);
            }
        }
        node.set_cursor(Some(if self.is_enabled {
            SystemCursor::Pointer
        } else {
            SystemCursor::NotAllowed
        }));

        let vertical_axis = !self.axis.is_horizontal();
        node.set_attachment("__slider_axis", if vertical_axis { 1_u8 } else { 0_u8 });
        node.set_attachment("__slider_value", self.value.get().to_bits());
        let snapshot = Snapshot {
            range: self.range,
            step: self.step,
            shows_ticks: self.shows_ticks,
        };
        let value_for_draw = self.value.clone();
        let weak = node.downgrade();
        node.update_draw(
            PaintIdentity {
                value_bits: self.value.get().to_bits(),
                lower_bits: self.range.0.to_bits(),
                upper_bits: self.range.1.to_bits(),
                is_enabled: self.is_enabled,
                axis: self.axis,
                step_bits: self.step.map(f64::to_bits),
                shows_ticks: self.shows_ticks,
            },
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let value = value_for_draw.get();
                owner.set_attachment("__slider_value", value.to_bits());
                owner.set_attachment("__slider_axis", if vertical_axis { 1_u8 } else { 0_u8 });
                render(&owner, &snapshot, list, origin);
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
                match phase {
                    HoverPhase::Enter => {
                        set_interaction(&owner, HOVERED, supports_control_hover(&owner));
                    }
                    HoverPhase::Leave => {
                        set_interaction(&owner, HOVERED, false);
                    }
                }
            }),
        );

        let pointer_host = snapshot.clone_for_events(self.value.clone());
        let pointer_enabled = self.is_enabled;
        let pointer_editing = self.on_editing_changed.clone();
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
                        // Non-transferable: a surrounding touch scroll must not
                        // take over a drag on the thumb.
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, false);
                        }
                        if set_interaction(&owner, PRESSED, true)
                            && let Some(handler) = pointer_editing.as_ref()
                        {
                            handler(true);
                        }
                        pointer_host.write_value(&owner, event.x, event.y);
                        owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                            "sliderValue",
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
                            && let Some(handler) = pointer_editing.as_ref()
                        {
                            handler(false);
                        }
                        let _ = pointer_enabled;
                        EventResult::Handled
                    }
                }
            }),
        );

        let motion_host = snapshot.clone_for_events(self.value.clone());
        let motion_node = node.downgrade();
        registry.set_motion(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, _phase| {
                let Some(owner) = Node::upgrade(&motion_node) else {
                    return EventResult::Ignored;
                };
                // Only consume motion while the thumb is being dragged. The host
                // acquires pointer capture on press, so motion outside the frame
                // still arrives here during a drag.
                if owner.attachment::<bool>(PRESSED) != Some(true) {
                    return EventResult::Ignored;
                }
                motion_host.write_value(&owner, event.x, event.y);
                owner.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
                    "sliderValue",
                ));
                EventResult::Handled
            }),
        );

        let key_host = snapshot.clone_for_events(self.value.clone());
        let key_axis = self.axis;
        let key_node = node.downgrade();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target {
                    return EventResult::Ignored;
                }
                let increment = key_host.increment();
                let current = key_host.value.get();
                // Resolve the direction from the live node: a fresh `Node` has no
                // inherited layout direction, so building the geometry from one
                // always reported LTR and the RTL mirroring below could never
                // apply. Swift reverses horizontal arrows under RTL.
                let reversed = key_axis.is_horizontal()
                    && Node::upgrade(&key_node)
                        .map(|owner| SliderGeometry::new(&owner, key_axis).is_reversed)
                        .unwrap_or(false);
                let next = match event.scancode {
                    crate::Scancode::ARROW_DOWN => current - increment,
                    crate::Scancode::ARROW_UP => current + increment,
                    crate::Scancode::ARROW_LEFT => {
                        if reversed {
                            current + increment
                        } else {
                            current - increment
                        }
                    }
                    crate::Scancode::ARROW_RIGHT => {
                        if reversed {
                            current - increment
                        } else {
                            current + increment
                        }
                    }
                    crate::Scancode::PAGE_UP => current + increment * 10.0,
                    crate::Scancode::PAGE_DOWN => current - increment * 10.0,
                    crate::Scancode::HOME => key_host.range.0,
                    crate::Scancode::END => key_host.range.1,
                    _ => return EventResult::Ignored,
                };
                key_host.value.set(key_host.snap(next));
                EventResult::Handled
            }),
        );
    }
}
impl_primitive_view!(SliderHost);

/// Value-only slice of the host used by interaction closures.
#[derive(Clone, Copy)]
struct Snapshot {
    range: (f64, f64),
    step: Option<f64>,
    shows_ticks: bool,
}

impl Snapshot {
    fn clamped(self, raw: f64) -> f64 {
        raw.min(self.range.1).max(self.range.0)
    }

    fn snap(self, raw: f64) -> f64 {
        let Some(step) = self.step.filter(|step| *step > 0.0) else {
            return self.clamped(raw);
        };
        let low = self.range.0;
        self.clamped(low + ((raw - low) / step).round() * step)
    }

    fn increment(self) -> f64 {
        self.step
            .filter(|step| step.is_finite() && *step > 0.0)
            .unwrap_or((self.range.1 - self.range.0) / 100.0)
    }

    fn clone_for_events(self, value: Binding<f64>) -> ValueWriter {
        ValueWriter {
            value,
            range: self.range,
            step: self.step,
        }
    }
}

/// Writes a pointer position back as a snapped value.
#[derive(Clone)]
struct ValueWriter {
    value: Binding<f64>,
    range: (f64, f64),
    step: Option<f64>,
}

impl ValueWriter {
    fn clamped(&self, raw: f64) -> f64 {
        raw.min(self.range.1).max(self.range.0)
    }

    fn snap(&self, raw: f64) -> f64 {
        let Some(step) = self.step.filter(|step| *step > 0.0) else {
            return self.clamped(raw);
        };
        let low = self.range.0;
        self.clamped(low + ((raw - low) / step).round() * step)
    }

    fn increment(&self) -> f64 {
        self.step
            .filter(|step| step.is_finite() && *step > 0.0)
            .unwrap_or((self.range.1 - self.range.0) / 100.0)
    }

    fn write_value(&self, node: &Node, x: f32, y: f32) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let axis = node
            .attachment::<u8>("__slider_axis")
            .map_or(SliderAxis::Horizontal, |value| {
                if value == 0 {
                    SliderAxis::Horizontal
                } else {
                    SliderAxis::Vertical
                }
            });
        let geometry = SliderGeometry::new(node, axis);
        let origin = node.absolute_origin();
        let local = if axis.is_horizontal() {
            x - origin.x
        } else {
            y - origin.y
        };
        let fraction = geometry.value_fraction(local) as f64;
        let span = self.range.1 - self.range.0;
        let next = self.snap(self.range.0 + fraction * span);
        if next != self.value.get() {
            self.value.set(next);
        }
    }
}

fn render(node: &Node, snapshot: &Snapshot, list: &mut cove_core::DrawList, origin: Point) {
    let theme = theme_for_node(node);
    let axis = node
        .attachment::<u8>("__slider_axis")
        .map_or(SliderAxis::Horizontal, |value| {
            if value == 0 {
                SliderAxis::Horizontal
            } else {
                SliderAxis::Vertical
            }
        });
    let style: AnySliderStyle = node.composition_value(&SliderStyleEnvironment::key());
    let value = node
        .attachment::<u64>("__slider_value")
        .map_or(0.0, f64::from_bits);
    let is_enabled = node.is_focusable();
    let appearance = style.resolve(
        &SliderStyleConfiguration {
            value: snapshot.clamped(value),
            range: snapshot.range,
            is_pressed: node.attachment::<bool>(PRESSED) == Some(true),
            is_hovered: node.attachment::<bool>(HOVERED) == Some(true),
            is_focused: FocusChainHolder::current().is_some_and(|chain| {
                chain.focused().is_some_and(|focused| focused == *node) && chain.is_focus_visible()
            }),
            is_enabled,
        },
        &theme,
    );
    let geometry = SliderGeometry::new(node, axis);
    if geometry.length <= 0.0 || geometry.breadth <= 0.0 {
        return;
    }
    let _ = list.add_rounded_rect(
        geometry.rect(0.0, geometry.length, appearance.track_height, origin),
        appearance.track_corner_radius,
        appearance.track_color,
    );
    let span = snapshot.range.1 - snapshot.range.0;
    let fraction = if span > 0.0 {
        ((snapshot.clamped(value) - snapshot.range.0) / span) as f32
    } else {
        0.0
    };
    let position = geometry.position(fraction);
    let (fill_start, fill_length) = if geometry.is_reversed {
        (position, geometry.length - position)
    } else {
        (0.0, position)
    };
    if fill_length > 0.0 {
        let _ = list.add_rounded_rect(
            geometry.rect(fill_start, fill_length, appearance.track_height, origin),
            appearance.track_corner_radius,
            appearance.fill_color,
        );
    }
    if snapshot.shows_ticks
        && let Some(step) = snapshot.step
    {
        let tick_color = theme
            .colors
            .on_surface_muted
            .multiplied_alpha(0.6 * node.opacity());
        for tick in super::paint::ticks(
            &geometry,
            origin,
            appearance.track_height,
            step,
            span,
            fraction,
            appearance.thumb_length,
        ) {
            let _ = list.add_rect(tick.rect, tick_color);
        }
    }
    let thumb_center = geometry.position(fraction);
    let thumb = geometry.thumb_rect(
        thumb_center,
        appearance.thumb_length,
        appearance.thumb_diameter,
        origin,
    );
    if let Some(shadow) = appearance.thumb_shadow {
        node.set_shadow(shadow);
    }
    let _ = list.add_rounded_rect(
        thumb,
        thumb.height.min(thumb.width) / 2.0,
        appearance.thumb_color,
    );
    if let Some(border) = appearance.thumb_border_color
        && appearance.thumb_border_width > 0.0
    {
        let _ = list.add_rounded_rect_stroke(
            thumb,
            thumb.height.min(thumb.width) / 2.0,
            appearance.thumb_border_width,
            border,
        );
    }
}
