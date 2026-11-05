//! Colour wheel: a hue ring around a saturation/value square.
//!
//! The ring maps screen angle to hue (red at the top, clockwise); the inscribed
//! square maps left→right to saturation and top→bottom to value. Both draw from
//! the same geometry the pointer handler uses, via [`color_wheel_model`].
//!
//! Like [`Slider`](super::slider::Slider), the interaction flags and the current
//! hue live in node attachments so a pointer drag repaints without a recompose.
use std::f32::consts::TAU;
use std::rc::Rc;

use cove_core::scene::{
    AccessibilityActions, AccessibilityRole, AccessibilitySemantics, AccessibilityState,
    EventPhase, EventResult, HoverPhase, InputHandlerRoute, LayoutNode, Node, PointerCaptureHolder,
    PointerPhase, Rect,
};
use cove_core::{Binding, Color, Point, Size};
use cove_platform::{MouseButton, SystemCursor};

use crate::primitives::controls::adaptive::{apply_touch_control_extent, supports_control_hover};
use crate::primitives::controls::fields::color_field_model::hex_string;
use crate::primitives::controls::fields::color_wheel_model::{
    Hsv, angle_for_hue, color_from_hsv, disk_radius_for_saturation, disk_saturation_for_radius,
    hsv_from_color, hue_for_angle, point_for_sv, sv_for_point,
};
use crate::{PrimitiveView, impl_primitive_view};
use cove_core::scene::InteractionRegistryHolder;

/// Hue segments drawn around the ring.
const RING_SEGMENTS: usize = 120;

const PRESSED: &str = "__color_wheel_pressed";
const HOVERED: &str = "__color_wheel_hovered";
const HUE: &str = "__color_wheel_hue";
/// 0 none, 1 hue ring, 2 saturation/value square.
const REGION: &str = "__color_wheel_region";

/// How the wheel presents the colour surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorWheelStyle {
    /// Hue ring around an inset saturation/value square.
    #[default]
    Ring,
    /// A filled disk: angle is hue, radius is saturation, at full value.
    Disk,
}

/// Hue/saturation picker, as a ring-and-square or a filled disk.
pub struct ColorWheel {
    pub color: Binding<Color>,
    pub is_enabled: bool,
    pub diameter: f32,
    pub style: ColorWheelStyle,
    pub shows_value_square: bool,
}

impl ColorWheel {
    /// The default diameter, sized to sit inside a colour-field popover.
    pub const DEFAULT_DIAMETER: f32 = 132.0;

    pub fn new(color: Binding<Color>) -> Self {
        Self {
            color,
            is_enabled: true,
            diameter: Self::DEFAULT_DIAMETER,
            style: ColorWheelStyle::Ring,
            shows_value_square: true,
        }
    }

    /// Ring-and-square or filled disk.
    pub fn style(mut self, style: ColorWheelStyle) -> Self {
        self.style = style;
        self
    }

    pub fn enabled(mut self, is_enabled: bool) -> Self {
        self.is_enabled = is_enabled;
        self
    }

    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = if diameter.is_finite() {
            diameter.clamp(48.0, 320.0)
        } else {
            Self::DEFAULT_DIAMETER
        };
        self
    }

    /// Hides the square, leaving the hue ring alone.
    pub fn shows_value_square(mut self, shows_value_square: bool) -> Self {
        self.shows_value_square = shows_value_square;
        self
    }

    fn resolved_diameter(&self) -> f32 {
        if self.diameter.is_finite() {
            self.diameter.clamp(48.0, 320.0)
        } else {
            Self::DEFAULT_DIAMETER
        }
    }
}

/// Geometry resolved from the node's frame.
#[derive(Clone, Copy, Debug)]
struct WheelGeometry {
    center: Point,
    outer: f32,
    ring: f32,
    square: Rect,
}

impl WheelGeometry {
    fn resolve(frame: Rect, origin: Point, shows_square: bool) -> Option<Self> {
        let diameter = frame.width.min(frame.height);
        if !diameter.is_finite() || diameter <= 8.0 {
            return None;
        }
        let center = Point {
            x: origin.x + frame.width / 2.0,
            y: origin.y + frame.height / 2.0,
        };
        let outer = diameter / 2.0 - 1.0;
        let ring = (diameter * 0.13).clamp(9.0, 26.0);
        let inner = (outer - ring).max(1.0);
        // The square is inscribed in the circle the ring leaves, with a small
        // clearance so the two never touch.
        let inscribed = if shows_square {
            ((inner - 3.0).max(1.0) * std::f32::consts::SQRT_2).max(1.0)
        } else {
            0.0
        };
        Some(Self {
            center,
            outer,
            ring,
            square: Rect::new(
                center.x - inscribed / 2.0,
                center.y - inscribed / 2.0,
                inscribed,
                inscribed,
            ),
        })
    }

    fn ring_radius(&self) -> f32 {
        (self.outer - self.ring / 2.0).max(self.ring / 2.0)
    }

    /// Which control a local point belongs to, and its mapped value. Region
    /// codes: 0 none, 1 hue ring, 2 saturation/value square, 3 disk.
    fn region_for(
        &self,
        point: Point,
        style: ColorWheelStyle,
        shows_square: bool,
    ) -> (u8, f32, (f32, f32)) {
        let dx = point.x - self.center.x;
        let dy = point.y - self.center.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if style == ColorWheelStyle::Disk {
            let hue = hue_for_angle(dy.atan2(dx));
            let saturation = disk_saturation_for_radius(distance, self.outer.max(1.0));
            return (3, hue, (saturation, 0.0));
        }
        let inner = self.outer - self.ring;
        if distance >= inner - 2.0 {
            return (1, hue_for_angle(dy.atan2(dx)), (0.0, 0.0));
        }
        if shows_square && self.square.width > 1.0 {
            let (s, v) = sv_for_point(
                point.x - self.square.x,
                point.y - self.square.y,
                self.square.width,
            );
            return (2, 0.0, (s, v));
        }
        (0, 0.0, (0.0, 0.0))
    }

    fn hue_marker(&self, hue: f32) -> Point {
        let angle = angle_for_hue(hue);
        let radius = self.ring_radius();
        Point {
            x: self.center.x + angle.cos() * radius,
            y: self.center.y + angle.sin() * radius,
        }
    }

    fn sv_marker(&self, s: f32, v: f32) -> Point {
        let (x, y) = point_for_sv(s, v, self.square.width);
        Point {
            x: self.square.x + x,
            y: self.square.y + y,
        }
    }

    fn disk_marker(&self, hue: f32, s: f32) -> Point {
        let angle = angle_for_hue(hue);
        let radius = disk_radius_for_saturation(s, self.outer);
        Point {
            x: self.center.x + angle.cos() * radius,
            y: self.center.y + angle.sin() * radius,
        }
    }
}

/// Snapshot carried into the interaction closures.
#[derive(Clone)]
struct WheelHost {
    color: Binding<Color>,
    style: ColorWheelStyle,
    shows_value_square: bool,
}

impl WheelHost {
    /// Writes the hue a window-space point selects, keeping saturation and value.
    fn write_hue(&self, node: &Node, point: Point) {
        let Some(geometry) = self.geometry(node) else {
            return;
        };
        let (_region, hue, _) = geometry.region_for(point, self.style, self.shows_value_square);
        node.set_attachment(HUE, hue.to_bits());
        let hsv = hsv_from_color(self.color.get());
        // A pure black or gray would make the ring look inert, so a first drag
        // lifts it to full saturation and value.
        let s = if hsv.s < 1e-4 { 1.0 } else { hsv.s };
        let v = if hsv.v < 1e-4 { 1.0 } else { hsv.v };
        self.write(Hsv::new(hue, s, v));
    }

    /// Writes the saturation and value a local point selects, keeping the hue.
    fn write_sv(&self, node: &Node, point: Point) {
        let Some(geometry) = self.geometry(node) else {
            return;
        };
        let (_region, _hue, (s, v)) =
            geometry.region_for(point, self.style, self.shows_value_square);
        if geometry.square.width <= 1.0 {
            return;
        }
        let hue = node
            .attachment::<u32>(HUE)
            .map(f32::from_bits)
            .unwrap_or_else(|| hsv_from_color(self.color.get()).h);
        self.write(Hsv::new(hue, s, v));
    }

    /// Writes the hue and saturation a disk point selects, keeping value.
    fn write_disk(&self, node: &Node, point: Point) {
        let Some(geometry) = self.geometry(node) else {
            return;
        };
        let (_region, hue, (s, _v)) =
            geometry.region_for(point, self.style, self.shows_value_square);
        node.set_attachment(HUE, hue.to_bits());
        let v = hsv_from_color(self.color.get()).v;
        let v = if v < 1e-4 { 1.0 } else { v };
        self.write(Hsv::new(hue, s, v));
    }

    fn write(&self, hsv: Hsv) {
        let alpha = self.color.get().a;
        self.color.set(hsv.color(alpha));
    }

    fn geometry(&self, node: &Node) -> Option<WheelGeometry> {
        WheelGeometry::resolve(
            node.frame(),
            node.absolute_origin(),
            self.shows_value_square,
        )
    }
}

/// The pointer position in window space, the same space the geometry resolves
/// [`WheelGeometry::center`] in. Subtracting the node's origin here would mix a
/// local point with a window-space centre and rotate every derived angle.
fn window_point(x: f32, y: f32) -> Point {
    Point { x, y }
}

impl PrimitiveView for ColorWheel {
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
        let diameter = self.resolved_diameter();
        layout.set_width(Some(diameter));
        layout.set_height(Some(diameter));
        layout.set_flex_shrink(0.0);
        layout.set_measure_func(Some(Rc::new(move |_w, _wm, _h, _hm| Size {
            width: diameter,
            height: diameter,
        })));
    }

    fn update_node(&self, node: &Node) {
        apply_touch_control_extent(node, true);
        if !supports_control_hover(node) {
            node.set_attachment(HOVERED, false);
        }
        node.set_is_focusable(self.is_enabled);
        let color = self.color.get();
        // A live hue survives a saturation of zero; otherwise follow the colour.
        let hsv = hsv_from_color(color);
        if (hsv.s > 1e-4 && hsv.v > 1e-4) || node.attachment::<u32>(HUE).is_none() {
            node.set_attachment(HUE, hsv.h.to_bits());
        }
        let hue = node
            .attachment::<u32>(HUE)
            .map(f32::from_bits)
            .unwrap_or(hsv.h);

        node.set_accessibility(Some(AccessibilitySemantics {
            role: AccessibilityRole::Slider,
            label: crate::i18n::tr("Color wheel"),
            value: hex_string(color, true),
            state: AccessibilityState {
                is_enabled: self.is_enabled,
                ..Default::default()
            },
            ..Default::default()
        }));

        let host = WheelHost {
            color: self.color.clone(),
            style: self.style,
            shows_value_square: self.shows_value_square,
        };
        node.set_accessibility_actions(AccessibilityActions {
            set_value: None,
            increment: self.is_enabled.then(|| {
                let host = host.clone();
                Rc::new(move || {
                    let hsv = hsv_from_color(host.color.get());
                    host.write(Hsv::new(hsv.h + 1.0 / 36.0, hsv.s, hsv.v));
                }) as Rc<dyn Fn()>
            }),
            decrement: self.is_enabled.then(|| {
                let host = host.clone();
                Rc::new(move || {
                    let hsv = hsv_from_color(host.color.get());
                    host.write(Hsv::new(hsv.h - 1.0 / 36.0, hsv.s, hsv.v));
                }) as Rc<dyn Fn()>
            }),
            ..Default::default()
        });

        if !self.is_enabled {
            node.set_attachment(PRESSED, false);
            node.set_attachment(HOVERED, false);
            if PointerCaptureHolder::current()
                .and_then(|capture| capture.target())
                .is_some_and(|target| target == *node)
                && let Some(capture) = PointerCaptureHolder::current()
            {
                capture.release();
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

        let weak = node.downgrade();
        let color_for_draw = self.color.clone();
        let style = self.style;
        let shows_square = self.shows_value_square;
        let enabled = self.is_enabled;
        node.update_draw(
            PaintIdentity {
                color: color.rgba8(),
                enabled,
                hue_bits: hue.to_bits(),
                shows_square,
                style,
            },
            move |list, origin| {
                let Some(owner) = Node::upgrade(&weak) else {
                    return;
                };
                let hue = owner
                    .attachment::<u32>(HUE)
                    .map(f32::from_bits)
                    .unwrap_or(hue);
                let Some(geometry) = WheelGeometry::resolve(owner.frame(), origin, shows_square)
                else {
                    return;
                };
                draw(
                    list,
                    &geometry,
                    color_for_draw.get(),
                    hue,
                    enabled,
                    style,
                    shows_square,
                );
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
                set_flag(&owner, HOVERED, matches!(phase, HoverPhase::Enter));
            }),
        );

        let pointer_host = host.clone();
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
                        let point = window_point(event.x, event.y);
                        let region = pointer_host
                            .geometry(&owner)
                            .map(|geometry| {
                                geometry
                                    .region_for(
                                        point,
                                        pointer_host.style,
                                        pointer_host.shows_value_square,
                                    )
                                    .0
                            })
                            .unwrap_or(0);
                        if region == 0 {
                            return EventResult::Ignored;
                        }
                        if let Some(capture) = PointerCaptureHolder::current() {
                            capture.acquire(&owner, false);
                        }
                        owner.set_attachment(REGION, region);
                        set_flag(&owner, PRESSED, true);
                        apply_region(&pointer_host, &owner, region, point);
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
                        owner.set_attachment(REGION, 0_u8);
                        set_flag(&owner, PRESSED, false);
                        EventResult::Handled
                    }
                }
            }),
        );

        let motion_host = host.clone();
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
                let region = owner.attachment::<u8>(REGION).unwrap_or(0);
                if region == 0 {
                    return EventResult::Ignored;
                }
                let point = window_point(event.x, event.y);
                apply_region(&motion_host, &owner, region, point);
                EventResult::Handled
            }),
        );

        let key_host = host.clone();
        let key_node = node.downgrade();
        registry.set_key(
            node,
            InputHandlerRoute::control(),
            Rc::new(move |event, phase| {
                if phase != EventPhase::Target {
                    return EventResult::Ignored;
                }
                let Some(owner) = Node::upgrade(&key_node) else {
                    return EventResult::Ignored;
                };
                let color = key_host.color.get();
                let hsv = hsv_from_color(color);
                let hue = owner
                    .attachment::<u32>(HUE)
                    .map(f32::from_bits)
                    .unwrap_or(hsv.h);
                let next = match event.scancode {
                    crate::Scancode::ARROW_LEFT => Hsv::new(hue - 1.0 / 360.0, hsv.s, hsv.v),
                    crate::Scancode::ARROW_RIGHT => Hsv::new(hue + 1.0 / 360.0, hsv.s, hsv.v),
                    crate::Scancode::ARROW_UP => Hsv::new(hue, hsv.s, hsv.v + 0.01),
                    crate::Scancode::ARROW_DOWN => Hsv::new(hue, hsv.s, hsv.v - 0.01),
                    crate::Scancode::HOME => Hsv::new(0.0, hsv.s, hsv.v),
                    _ => return EventResult::Ignored,
                };
                key_host.write(next);
                EventResult::Handled
            }),
        );
    }
}
impl_primitive_view!(ColorWheel);

fn apply_region(host: &WheelHost, node: &Node, region: u8, point: Point) {
    match region {
        1 => host.write_hue(node, point),
        2 => host.write_sv(node, point),
        3 => host.write_disk(node, point),
        _ => {}
    }
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(
        "colorWheel",
    ));
}

fn set_flag(node: &Node, key: &str, value: bool) {
    if node.attachment::<bool>(key) == Some(value) {
        return;
    }
    node.set_attachment(key, value);
    node.mark_render_dirty(cove_core::scene::InvalidationSource::style_set(key));
}

#[derive(Clone, Copy, PartialEq)]
struct PaintIdentity {
    color: u32,
    enabled: bool,
    hue_bits: u32,
    shows_square: bool,
    style: ColorWheelStyle,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    list: &mut cove_core::DrawList,
    geometry: &WheelGeometry,
    color: Color,
    hue: f32,
    enabled: bool,
    style: ColorWheelStyle,
    shows_square: bool,
) {
    let alpha = if enabled { 1.0 } else { 0.45 };
    let marker_radius = (geometry.outer * 0.08).clamp(4.0, 9.0);

    if style == ColorWheelStyle::Disk {
        let center = geometry.center;
        let radius = geometry.outer;
        // A shaded disk: hue by angle, saturation by radius, at full value. The
        // per-vertex shading comes from `add_disk_shaded`, which a flat fill or
        // a single linear gradient cannot express.
        let _ = list.add_disk_shaded(center, radius, move |x, y| {
            let dx = x - center.x;
            let dy = y - center.y;
            let hue = hue_for_angle(dy.atan2(dx));
            let saturation =
                disk_saturation_for_radius((dx * dx + dy * dy).sqrt(), radius.max(1.0));
            color_from_hsv(Hsv::new(hue, saturation, 1.0), alpha)
        });
        let hsv = hsv_from_color(color);
        add_marker(list, geometry.disk_marker(hue, hsv.s), marker_radius, alpha);
        return;
    }

    // Hue ring, one flat-coloured segment per step with a small overlap so no
    // seam shows between neighbours.
    let segment = TAU / RING_SEGMENTS as f32;
    for index in 0..RING_SEGMENTS {
        let segment_hue = index as f32 / RING_SEGMENTS as f32;
        let start = angle_for_hue(segment_hue) - segment * 0.1;
        let fill = color_from_hsv(Hsv::new(segment_hue, 1.0, 1.0), alpha);
        let _ = list.add_arc_stroke(
            geometry.center,
            geometry.ring_radius(),
            start,
            segment * 1.2,
            geometry.ring,
            fill,
        );
    }

    if shows_square && geometry.square.width > 1.0 {
        let hue_color = color_from_hsv(Hsv::new(hue, 1.0, 1.0), alpha);
        let rect = geometry.square;
        let _ = list.add_linear_gradient(
            rect,
            0.0,
            cove_core::LinearGradient {
                start: Color {
                    a: alpha,
                    ..Color::WHITE
                },
                end: hue_color,
                angle: 90.0,
            },
        );
        let _ = list.add_linear_gradient(
            rect,
            0.0,
            cove_core::LinearGradient {
                start: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: alpha,
                },
                end: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.0,
                },
                angle: 0.0,
            },
        );
    }

    let hsv = hsv_from_color(color);
    if shows_square && geometry.square.width > 1.0 {
        add_marker(list, geometry.sv_marker(hsv.s, hsv.v), marker_radius, alpha);
    }
    add_marker(list, geometry.hue_marker(hue), marker_radius, alpha);
}

/// A white dot with a dark halo, readable on any hue.
fn add_marker(list: &mut cove_core::DrawList, center: Point, radius: f32, alpha: f32) {
    let halo = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.45 * alpha,
    };
    let white = Color {
        a: alpha,
        ..Color::WHITE
    };
    add_disk(list, center, radius + 1.5, halo);
    add_disk(list, center, radius, white);
    let _ = list.add_arc_stroke(center, radius, 0.0, TAU, 1.0, halo);
}

/// A filled dot: an arc stroke of width `radius` on a path of half that radius
/// covers the disk `[0, radius]`.
fn add_disk(list: &mut cove_core::DrawList, center: Point, radius: f32, color: Color) {
    if radius <= 0.0 {
        return;
    }
    let _ = list.add_arc_stroke(center, radius / 2.0, 0.0, TAU, radius, color);
}
