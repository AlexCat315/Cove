//! Translation of CoveUICore/AnalyticClipper.swift.
use crate::rounded_clip::clip_polygon;
use crate::{DrawError, RoundedRect, Vertex};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GeometryAntialiasing {
    Analytic,
    #[default]
    Multisample,
}
unsafe extern "C" {
    fn cove_clip_feather(
        vertices: *mut Vertex,
        count: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        scale: f32,
    );
}
pub(crate) fn analytic_clip(
    triangle: &[Vertex],
    masks: &[RoundedRect],
    scale: f32,
) -> Result<Vec<Vec<Vertex>>, DrawError> {
    let scale = scale.max(1.0);
    let half = 0.5 / scale;
    let mut pieces = vec![triangle.to_vec()];
    for mask in masks {
        let segments = (mask.radius * scale * std::f32::consts::PI / 4.0)
            .ceil()
            .clamp(4.0, 64.0) as usize;
        let inside = mask.inset(half).outline(segments);
        let outside = mask.inset(-half).outline(segments);
        let mut result = Vec::new();
        for piece in pieces {
            if piece.iter().all(|v| mask.signed_distance(*v) <= -half) {
                result.push(piece);
                continue;
            }
            let core = clip_polygon(&piece, std::slice::from_ref(&inside))?;
            if core.len() >= 3 {
                result.push(core);
            }
            for index in 0..inside.len() {
                let next = (index + 1) % inside.len();
                let mut outline = vec![inside[index], outside[index], outside[next], inside[next]];
                let area: f32 = (0..outline.len())
                    .map(|edge| {
                        let a = outline[edge];
                        let b = outline[(edge + 1) % outline.len()];
                        a.x * b.y - b.x * a.y
                    })
                    .sum();
                if area.abs() <= 0.000001 {
                    continue;
                }
                if area < 0.0 {
                    outline.reverse();
                }
                let mut feather = clip_polygon(&piece, &[outline])?;
                if feather.len() < 3 {
                    continue;
                }
                let r = mask.rect;
                // SAFETY: exclusive, initialized repr(C) Vertex slice; C only updates color alpha within this slice.
                unsafe {
                    cove_clip_feather(
                        feather.as_mut_ptr(),
                        feather.len(),
                        r.x,
                        r.y,
                        r.width,
                        r.height,
                        mask.radius,
                        scale,
                    );
                }
                result.push(feather);
            }
        }
        pieces = result;
        if pieces.is_empty() {
            break;
        }
    }
    Ok(pieces)
}
