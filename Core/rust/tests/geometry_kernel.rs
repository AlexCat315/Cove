//! Translation of CoveUICoreTests/GeometryKernelTests.swift.

use cove_core::{Color, DrawBatch, DrawList, Point, Rect, Vertex};

fn translated_copy(count: usize) {
    let points: Vec<Vertex> = (0..count)
        .map(|index| Vertex {
            x: index as f32 * 0.25,
            y: -(index as f32),
            u: (index % 7) as f32,
            v: (index % 11) as f32,
            color: (index as u32).wrapping_mul(71),
        })
        .collect();
    let triangles: Vec<u32> = (0..count).map(|index| index as u32).collect();
    let batches = if count == 0 {
        vec![]
    } else {
        vec![DrawBatch {
            index_offset: 0,
            index_count: count as u32,
            texture_id: 42,
            scissor: Some(Rect::new(1.0, 2.0, 3.0, 4.0)),
            backdrop: None,
        }]
    };
    let mut source = DrawList::default();
    source
        .load(points.clone(), triangles.clone(), batches.clone(), Default::default())
        .unwrap();
    let mut destination = DrawList::default();
    // The first append shares the buffers; the translated append must detach.
    destination.append(&source, Point::default()).unwrap();
    destination
        .append(&source, Point { x: 3.5, y: -2.25 })
        .unwrap();
    assert_eq!(
        source.vertices().iter().map(|v| v.x).collect::<Vec<_>>(),
        points.iter().map(|v| v.x).collect::<Vec<_>>()
    );
    assert_eq!(source.indices(), triangles.as_slice());
    assert_eq!(destination.vertices().len(), count * 2);
    let expected_indices: Vec<u32> = triangles
        .iter()
        .copied()
        .chain(triangles.iter().map(|i| i + count as u32))
        .collect();
    assert_eq!(destination.indices(), expected_indices.as_slice());
    for (actual, original) in destination.vertices().iter().skip(count).zip(&points) {
        assert_eq!(actual.x, original.x + 3.5);
        assert_eq!(actual.y, original.y - 2.25);
        assert_eq!(actual.u, original.u);
        assert_eq!(actual.v, original.v);
        assert_eq!(actual.color, original.color);
    }
    assert!(
        destination
            .batches()
            .iter()
            .all(|batch| batch.scissor == batches.first().map(|b| b.scissor.unwrap())
                && batch.texture_id == 42)
    );
    if count > 0 {
        assert_eq!(destination.batches().last().unwrap().index_offset, count as u32);
    }
}

#[test]
fn translated_copies_and_rebased_indices_preserve_cow_attributes_clips_and_uneven_worker_tails() {
    for count in [0, 1, 15, 16_383, 16_384, 32_771, 131_079] {
        translated_copy(count);
    }
}

#[test]
fn parallel_opacity_matches_scalar_rounding_and_leaves_the_prefix_and_rgb_unchanged() {
    for alpha in [0.0_f32, 0.1, 0.5, 0.9999, 1.0, -1.0, 2.0] {
        let points: Vec<Vertex> = (0..32_771)
            .map(|index| Vertex {
                x: index as f32,
                y: 1.0,
                u: 0.0,
                v: 0.0,
                color: ((index % 256) as u32) << 24 | 0x0012_3456,
            })
            .collect();
        let mut list = DrawList::default();
        list.load(points.clone(), vec![], vec![], Default::default())
            .unwrap();
        list.multiply_opacity(17, alpha);
        let multiplier = alpha.clamp(0.0, 1.0);
        for (index, vertex) in list.vertices().iter().enumerate() {
            let expected = if index < 17 {
                points[index].color
            } else {
                0x0012_3456
                    | ((((points[index].color >> 24) as f32 * multiplier).round() as u32) << 24)
            };
            assert_eq!(vertex.color, expected);
            assert_eq!(vertex.x, points[index].x);
        }
        assert_eq!(points[17].color >> 24, 17);
    }
}

#[test]
fn bounds_include_overflow_geometry_and_handle_empty_lists() {
    let mut list = DrawList::default();
    assert_eq!(list.paint_bounds(), None);
    list.add_rect(Rect::new(-20.0, -8.0, 30.0, 60.0), Color::WHITE)
        .unwrap();
    assert_eq!(
        list.paint_bounds(),
        Some(Rect::new(-20.0, -8.0, 30.0, 60.0))
    );
}
