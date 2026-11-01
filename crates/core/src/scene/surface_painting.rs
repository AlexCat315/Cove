//! Translation of CoveUIScene/SurfacePainting.swift.
//!
//! How a `SurfaceShadow` becomes geometry. Both node painters share this so the
//! cheap-blur approximation and the rim geometry have exactly one
//! implementation.
use crate::{Color, DrawError, DrawList, LayeredShadow, Point, Rect, SurfaceShadow};

/// Distance between neighbouring rects in the stack. A quarter of a point is
/// half a pixel on a 2x display, so the edges blend into each other instead of
/// stacking up into ridges.
const STEP_SPACING: f32 = 0.25;

impl DrawList {
    /// Paint a layered shadow: ambient halo first, key shadow over it.
    #[allow(clippy::too_many_arguments)]
    pub fn add_shadow(
        &mut self,
        layers: LayeredShadow,
        rect: Rect,
        corner_radius: f32,
        opacity: f32,
        frosted_rect: Option<Rect>,
        occluded_rect: Option<Rect>,
    ) {
        self.add_shadow_layer(
            layers.ambient,
            rect,
            corner_radius,
            opacity,
            frosted_rect,
            occluded_rect,
        );
        self.add_shadow_layer(
            layers.key,
            rect,
            corner_radius,
            opacity,
            frosted_rect,
            occluded_rect,
        );
    }

    /// Paint one shadow layer as a stack of expanded translucent rounded rects.
    ///
    /// The renderer has no blurred primitive, so the gradient is built from
    /// concentric rects: the outermost is the faint fringe, and each one on top
    /// is smaller and denser, which accumulates into a falloff near the
    /// surface.
    ///
    /// `frosted_rect` is the panel a backdrop blur will sample, when it has one.
    /// The shadow is then clipped to the band outside it: a panel's own shadow
    /// falls on what is *behind* the panel, so letting the frost sample it
    /// greys the panel from the inside — measured at −17% luma on white.
    ///
    /// `occluded_rect` is a surface that will be painted opaquely over this node
    /// straight after, so the shadow's interior can never be seen. Skipping it
    /// keeps a `blur: 8` shadow from costing 33 full-surface fills, which
    /// dominated GPU time on card-dense layouts. Corners stay rounded there;
    /// clipping to plain bands would square the card's outline.
    pub fn add_shadow_layer(
        &mut self,
        layer: SurfaceShadow,
        rect: Rect,
        corner_radius: f32,
        opacity: f32,
        frosted_rect: Option<Rect>,
        occluded_rect: Option<Rect>,
    ) {
        if layer.is_empty() || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let frost = frosted_rect.filter(|rect| rect.width > 0.0 && rect.height > 0.0);
        // A frosted panel samples what is behind it, so its shadow has to stay
        // outside even when nothing opaque covers it — that clip wins.
        // A rounded surface does not cover its own box's corners: skipping the
        // whole box left a notch in the shadow there. Only the part inside the
        // corner arcs is certainly covered.
        let arc_inset = corner_radius.max(0.0) * (1.0 - 0.5f32.sqrt());
        let occluded = if frost.is_some() {
            None
        } else {
            occluded_rect.and_then(|box_| {
                let covered = Rect::new(
                    box_.x + arc_inset,
                    box_.y + arc_inset,
                    box_.width - arc_inset * 2.0,
                    box_.height - arc_inset * 2.0,
                );
                (covered.width > 0.0 && covered.height > 0.0).then_some(covered)
            })
        };
        self.paint_steps(
            layer,
            rect,
            corner_radius,
            opacity,
            step_count(layer),
            frost,
            occluded,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_steps(
        &mut self,
        layer: SurfaceShadow,
        rect: Rect,
        corner_radius: f32,
        opacity: f32,
        count: usize,
        frost: Option<Rect>,
        occluded: Option<Rect>,
    ) {
        let Some(color) = layer.color else { return };
        let blur = layer.blur.max(0.0);
        let radius = corner_radius.max(0.0);
        let count = count.max(1);
        for index in 0..count {
            let t = index as f32 / (count - 1).max(1) as f32;
            let inset = -blur * (1.0 - t);
            // Largest rect is the faint outer edge; smaller rects build density
            // towards the surface, not the other way around.
            let weight = if blur > 0.0 {
                (t + 0.15) * (t + 0.15)
            } else {
                1.0
            };
            let alpha = color.a * weight / count as f32;
            let step_color = Color {
                r: color.r,
                g: color.g,
                b: color.b,
                a: alpha.min(1.0) * opacity,
            };
            // Vertices store 8-bit alpha. Many faint blur steps round to zero;
            // they cannot affect compositing but still cost a full rounded
            // mesh, its upload and validation if recorded.
            if step_color.rgba8() >> 24 == 0 {
                continue;
            }
            let step_rect = Rect::new(
                rect.x + layer.offset_x + inset,
                rect.y + layer.offset_y + inset,
                rect.width - 2.0 * inset,
                rect.height - 2.0 * inset,
            );
            let step_radius = radius + (-inset).max(0.0);
            if let Some(frost) = frost {
                for band in bands_around(frost, step_rect) {
                    let _ = self.add_rect(band, step_color);
                }
            } else if let Some(occluded) = occluded {
                if step_radius > 0.0 {
                    // Only the fringe outside the surface can be seen; a `blur: 8`
                    // shadow is 33 nested rects, and repainting the covered
                    // interior each time dominated GPU time on card layouts.
                    for edge in fringe_edges(occluded, step_rect, step_radius) {
                        let _ = self.add_rect(edge, step_color);
                    }
                    self.add_corner_fringe(step_rect, step_radius, step_color);
                } else {
                    for band in bands_around(occluded, step_rect) {
                        let _ = self.add_rect(band, step_color);
                    }
                }
            } else if step_radius > 0.0 {
                let _ = self.add_rounded_rect(step_rect, step_radius, step_color);
            } else {
                let _ = self.add_rect(step_rect, step_color);
            }
        }
    }

    /// The visible part of each corner: the step's own quarter disc.
    ///
    /// The surface is painted opaque over this shadow, so anything drawn inside
    /// it is never seen. That makes the inner bound free — the corner only has
    /// to reach as far as the step's arc, and the part that spills under the
    /// surface costs nothing visually. Fanning from the step's own arc centre
    /// therefore lands the outer edge exactly on the rounded-rect boundary,
    /// whatever the shadow's offset is.
    fn add_corner_fringe(&mut self, step: Rect, step_radius: f32, color: Color) {
        let segments = self.fan_segments(step_radius);
        // Corner arc centres, paired with the two bounding edges their arc
        // sweeps between, as unit vectors.
        let corners: [(f32, f32, (f32, f32, f32, f32)); 4] = [
            (
                step.x + step_radius,
                step.y + step_radius,
                (-1.0, 0.0, 0.0, -1.0),
            ),
            (
                step.max_x() - step_radius,
                step.y + step_radius,
                (0.0, -1.0, 1.0, 0.0),
            ),
            (
                step.x + step_radius,
                step.max_y() - step_radius,
                (-1.0, 0.0, 0.0, 1.0),
            ),
            (
                step.max_x() - step_radius,
                step.max_y() - step_radius,
                (0.0, 1.0, 1.0, 0.0),
            ),
        ];
        for (cx, cy, (ax, ay, bx, by)) in corners {
            let mut previous = Point::new(cx + ax * step_radius, cy + ay * step_radius);
            for index in 1..=segments {
                let angle = index as f32 / segments as f32 * (std::f32::consts::PI / 2.0);
                let (sine, cosine) = angle.sin_cos();
                let point = Point::new(
                    cx + (ax * cosine + bx * sine) * step_radius,
                    cy + (ay * cosine + by * sine) * step_radius,
                );
                let _ = self.add_triangle(Point::new(cx, cy), previous, point, color);
                previous = point;
            }
        }
    }

    /// Triangles per corner fan.
    ///
    /// A chord cuts inside the arc by `r * (1 - cos(step / 2))`, which for a
    /// small step is `r * step^2 / 8`. Holding that under `tolerance` bounds the
    /// step at `sqrt(8 * tolerance / r)`, so a quarter arc needs
    /// `(pi / 2) / sqrt(8 * tolerance / r)` segments.
    fn fan_segments(&self, radius: f32) -> usize {
        if radius <= 0.0 {
            return 1;
        }
        let tolerance = 0.05 / self.pixel_scale.max(1.0);
        let count = (std::f32::consts::PI / (4.0 * 2.0f32.sqrt())) * (radius / tolerance).sqrt();
        (count.ceil() as usize).clamp(2, 32)
    }
}

/// Steps spaced evenly in *space*, one every `STEP_SPACING` points.
///
/// What shows as banding is where consecutive rects land, not how much alpha a
/// single one adds. Space is also what bounds the count: a 24pt halo wants ~96
/// rects, a 2pt one wants 8, and neither pays for the other's resolution.
fn step_count(layer: SurfaceShadow) -> usize {
    let blur = layer.blur.max(0.0);
    if layer.color.is_none_or(|color| color.a <= 0.0) || blur <= 0.0 {
        return 1;
    }
    ((blur / STEP_SPACING).ceil() as usize + 1).clamp(2, 128)
}

/// Straight parts of `outer` outside `hole`, shortened so the corner arcs — not
/// these quads — own the corners.
///
/// The corners are shortened by exactly one `radius`, because that is where a
/// step's corner arc reaches: `outer.y + radius` is the arc's centre, and the
/// arc sweeps from there back to the step's edge. Shortening by more leaves a
/// gap along the straight edges, and by less leaves the band painting square
/// corners where the arc has already curved in.
fn fringe_edges(hole: Rect, outer: Rect, radius: f32) -> Vec<Rect> {
    let reach = radius;
    let top = outer.y.max(hole.y);
    let bottom = outer.max_y().min(hole.max_y());
    let left = outer.x.max(hole.x);
    let right = outer.max_x().min(hole.max_x());
    if top >= bottom && left >= right {
        return Vec::new();
    }
    let mid_x0 = (outer.x + reach).min(outer.max_x());
    let mid_x1 = (outer.max_x() - reach).max(mid_x0);
    let mid_y0 = (outer.y + reach).min(outer.max_y());
    let mid_y1 = (outer.max_y() - reach).max(mid_y0);
    let mut edges: Vec<Rect> = Vec::new();
    if top < bottom {
        edges.push(Rect::new(mid_x0, outer.y, mid_x1 - mid_x0, top - outer.y));
        edges.push(Rect::new(
            mid_x0,
            bottom,
            mid_x1 - mid_x0,
            outer.max_y() - bottom,
        ));
    }
    if left < right {
        edges.push(Rect::new(outer.x, mid_y0, left - outer.x, mid_y1 - mid_y0));
        edges.push(Rect::new(
            right,
            mid_y0,
            outer.max_x() - right,
            mid_y1 - mid_y0,
        ));
    }
    edges.retain(|rect| rect.width > 0.0 && rect.height > 0.0);
    edges
}

/// The four bands of `outer` that lie outside `hole`.
fn bands_around(hole: Rect, outer: Rect) -> Vec<Rect> {
    let top = outer.y.max(outer.max_y().min(hole.y));
    let bottom = top.max(outer.max_y().min(hole.max_y()));
    let bands = [
        Rect::new(outer.x, outer.y, outer.width, (top - outer.y).max(0.0)),
        Rect::new(
            outer.x,
            bottom,
            outer.width,
            (outer.max_y() - bottom).max(0.0),
        ),
        Rect::new(
            outer.x,
            top,
            (hole.x - outer.x).max(0.0),
            (bottom - top).max(0.0),
        ),
        Rect::new(
            hole.max_x(),
            top,
            (outer.max_x() - hole.max_x()).max(0.0),
            (bottom - top).max(0.0),
        ),
    ];
    bands
        .into_iter()
        .filter(|rect| rect.width > 0.0 && rect.height > 0.0)
        .collect()
}

/// Errors from shadow geometry are dropped: a full draw buffer already surfaces
/// on the next primitive that fits, and a shadow must never abort a frame.
#[allow(dead_code)]
fn ignore(_: Result<(), DrawError>) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn shadow(color: Color, blur: f32) -> SurfaceShadow {
        SurfaceShadow {
            color: Some(color),
            offset_x: 0.0,
            offset_y: 2.0,
            blur,
        }
    }

    #[test]
    fn step_count_scales_with_blur_and_bounds_the_range() {
        assert_eq!(step_count(shadow(Color::BLACK, 0.0)), 1);
        assert_eq!(step_count(shadow(Color::BLACK, 1.0)), 5);
        assert_eq!(step_count(shadow(Color::BLACK, 24.0)), 97);
        assert_eq!(step_count(shadow(Color::BLACK, 400.0)), 128);
    }

    #[test]
    fn empty_layers_emit_nothing() {
        let mut list = DrawList::default();
        list.add_shadow_layer(
            SurfaceShadow::NONE,
            Rect::new(0.0, 0.0, 10.0, 10.0),
            0.0,
            1.0,
            None,
            None,
        );
        assert!(list.vertices().is_empty());
    }

    #[test]
    fn opaque_surface_clips_the_shadow_interior() {
        let mut unclipped = DrawList::default();
        unclipped.add_shadow_layer(
            shadow(Color::BLACK, 8.0),
            Rect::new(0.0, 0.0, 40.0, 40.0),
            6.0,
            1.0,
            None,
            None,
        );
        let mut clipped = DrawList::default();
        clipped.add_shadow_layer(
            shadow(Color::BLACK, 8.0),
            Rect::new(0.0, 0.0, 40.0, 40.0),
            6.0,
            1.0,
            None,
            Some(Rect::new(0.0, 0.0, 40.0, 40.0)),
        );
        assert!(clipped.vertices().len() < unclipped.vertices().len());
    }
}
