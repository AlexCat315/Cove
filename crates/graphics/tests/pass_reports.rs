//! The pass census a frame hands to DevTools must describe what was recorded,
//! not what the renderer wished it recorded.

use cove_graphics::{
    CommandBuffer, DrawIndexedArguments, PassKind, RenderColorTarget, RenderPassDescriptor, Texture,
};

/// A handle is all the recording side holds; no device is involved in a census.
fn target() -> Texture {
    Texture::new(1)
}

#[test]
fn passes_are_reported_in_recording_order_with_their_draws() {
    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget::clearing(target(), [0.0; 4])])
            .with_label("ui-frame"),
        |encoder| {
            encoder.draw_indexed(DrawIndexedArguments::new(96));
            encoder.draw_indexed(DrawIndexedArguments::new(24));
            encoder.draw(6, 1, 0, 0);
        },
    );
    commands.render_pass(
        // No label: the census still has to name this row.
        RenderPassDescriptor::new(vec![RenderColorTarget::clearing(target(), [0.0; 4])]),
        |encoder| {
            encoder.draw_indexed(DrawIndexedArguments::new(6));
        },
    );

    let reports = commands.pass_reports();
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].label, "ui-frame");
    assert_eq!(reports[0].kind, PassKind::Render);
    assert_eq!(reports[0].draws, 3);
    assert_eq!(reports[0].indices, 120);
    assert_eq!(reports[0].vertices, 6);
    assert_eq!(reports[1].label, "<unnamed>");
    assert_eq!(reports[1].indices, 6);
}

#[test]
fn indexed_draws_keep_the_index_window_a_layer_can_be_matched_against() {
    let mut commands = CommandBuffer::new();
    commands.render_pass(
        RenderPassDescriptor::new(vec![RenderColorTarget::clearing(target(), [0.0; 4])])
            .with_label("ui-frame"),
        |encoder| {
            encoder.draw_indexed(DrawIndexedArguments {
                index_count: 60,
                instance_count: 1,
                first_index: 0,
                vertex_offset: 0,
                first_instance: 0,
            });
            encoder.draw_indexed(DrawIndexedArguments {
                index_count: 30,
                instance_count: 2,
                first_index: 60,
                vertex_offset: 0,
                first_instance: 0,
            });
        },
    );
    let reports = commands.pass_reports();
    let ranges: Vec<(u64, u64)> = reports[0]
        .ranges
        .iter()
        .map(|range| (range.first, range.count))
        .collect();
    assert_eq!(ranges, [(0, 60), (60, 30)]);
    assert_eq!(reports[0].instances, 3);
}

#[test]
fn a_buffer_without_passes_reports_nothing() {
    let commands = CommandBuffer::new();
    assert!(commands.pass_reports().is_empty());
}
