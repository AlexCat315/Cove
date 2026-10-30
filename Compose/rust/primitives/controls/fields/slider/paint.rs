//! Translation of Primitives/Controls/Fields/Slider/Paint.swift.
//!
//! The draw hook lives with the host (`host::render`); this module keeps the
//! tick-hairline policy the paint pass uses, which the host reads when ticks are
//! requested.
use cove_core::{Point, Rect};

use super::geometry::SliderGeometry;

/// A tick hairline just outside the track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderTick {
    pub rect: Rect,
}

/// Ticks for a stepped slider. Ticks are hidden under the knob and omitted when
/// they would crowd closer than 10 points.
pub fn ticks(
    geometry: &SliderGeometry,
    origin: Point,
    track_thickness: f32,
    step: f64,
    span: f64,
    value_fraction: f32,
    thumb_length: f32,
) -> Vec<SliderTick> {
    if !span.is_finite() || span <= 0.0 || step <= 0.0 {
        return Vec::new();
    }
    let count = (span / step).floor();
    if !count.is_finite() || count < 1.0 || count > 40.0 {
        return Vec::new();
    }
    let spacing = geometry.length / count as f32;
    if spacing < 10.0 {
        return Vec::new();
    }
    let thumb_half = thumb_length / 2.0 + 2.0;
    let position = geometry.position(value_fraction);
    let track = geometry.rect(0.0, geometry.length, track_thickness, origin);
    let mut ticks = Vec::new();
    for index in 0..=count as i64 {
        let fraction = (index as f64 * step / span) as f32;
        let tick = geometry.position(fraction);
        if (tick - position).abs() <= thumb_half {
            continue;
        }
        ticks.push(SliderTick {
            rect: if geometry.axis.is_horizontal() {
                Rect::new(origin.x + tick - 0.5, track.y + track.height + 2.0, 1.0, 2.0)
            } else {
                Rect::new(track.x + track.width + 2.0, origin.y + tick - 0.5, 2.0, 1.0)
            },
        });
    }
    ticks
}
