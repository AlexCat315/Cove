//! Translation of CoveUIScene/Rendering/ScaledPainting.swift.
use crate::{Backdrop, DrawList, Point, Rect, Vertex, with_deferred_evaluation};
use std::rc::Rc;

pub type Painter = Rc<dyn Fn(&mut DrawList, Point)>;

/// Custom painters use layout lengths. Resolve their geometry and local clips
/// into the same scaled space as the node's surface, without changing layout.
pub fn paint_scaled(painter: &Painter, list: &mut DrawList, origin: Point, scale: f32) {
    if scale == 1.0 {
        with_deferred_evaluation(|| painter(list, origin));
        return;
    }
    if !(scale > 0.0 && scale.is_finite()) {
        return;
    }
    let (x, y) = (origin.x, origin.y);
    let resolve = |rect: Rect, factor: f32| {
        Rect::new(
            x + (rect.x - x) * factor,
            y + (rect.y - y) * factor,
            rect.width * factor,
            rect.height * factor,
        )
    };
    let mut local = DrawList::default();
    local.pixel_scale = list.pixel_scale * scale;
    let viewport = list.viewport_bounds;
    local.set_viewport_bounds(viewport.map(|rect| resolve(rect, 1.0 / scale)));
    if let Some(clip) = list.current_clip() {
        local.push_clip_rect(resolve(clip, 1.0 / scale));
    }
    with_deferred_evaluation(|| painter(&mut local, origin));
    let vertices: Vec<Vertex> = local
        .vertices()
        .iter()
        .map(|vertex| Vertex { x: x + (vertex.x - x) * scale, y: y + (vertex.y - y) * scale, ..*vertex })
        .collect();
    let batches = local
        .batches()
        .iter()
        .map(|batch| crate::DrawBatch {
            scissor: batch.scissor.map(|rect| resolve(rect, scale)),
            backdrop: batch.backdrop.map(|backdrop| Backdrop {
                rect: resolve(backdrop.rect, scale),
                corner_radius: backdrop.corner_radius * scale,
                blur_radius: backdrop.blur_radius * scale,
                opacity: backdrop.opacity,
            }),
            ..batch.clone()
        })
        .collect::<Vec<_>>();
    let indices = local.indices().to_vec();
    let resources = local.resources.clone();
    if local.load(vertices, indices, batches, resources).is_err() {
        return;
    }
    let _ = list.append(&local, Point::default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;

    #[test]
    fn identity_scale_paints_straight_through() {
        let mut list = DrawList::default();
        let painter: Painter = Rc::new(|list, origin| {
            let _ = list.add_rect(Rect::new(origin.x, origin.y, 10.0, 10.0), Color::BLACK);
        });
        paint_scaled(&painter, &mut list, Point::new(4.0, 6.0), 1.0);
        assert_eq!(list.vertices().len(), 4);
        assert_eq!(list.vertices()[0].x, 4.0);
    }

    #[test]
    fn scale_maps_geometry_about_the_origin() {
        let mut list = DrawList::default();
        let painter: Painter = Rc::new(|list, origin| {
            let _ = list.add_rect(Rect::new(origin.x + 10.0, origin.y, 10.0, 10.0), Color::BLACK);
        });
        paint_scaled(&painter, &mut list, Point::new(0.0, 0.0), 2.0);
        let bounds = list.paint_bounds().unwrap();
        assert_eq!(bounds, Rect::new(20.0, 0.0, 20.0, 20.0));
    }

    #[test]
    fn collapsed_scale_paints_nothing() {
        let mut list = DrawList::default();
        let painter: Painter = Rc::new(|list, _| {
            let _ = list.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK);
        });
        paint_scaled(&painter, &mut list, Point::default(), 0.0);
        assert!(list.vertices().is_empty());
    }
}
