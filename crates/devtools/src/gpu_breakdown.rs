//! Frame-local pass geometry and GPU timing attribution.

use cove_core::scene::{CompositedLayer, LayerCensus};
use cove_devtools_protocol::{GpuFrameBreakdown, GpuLayer, GpuPass};
use cove_graphics::PassReport;
use cove_graphics::rhi::{GpuSubmissionTiming, GraphicsApi};

const MAX_LAYERS: usize = 128;

pub fn assemble(
    passes: &[PassReport],
    pass_count: usize,
    census: &LayerCensus,
) -> GpuFrameBreakdown {
    let mut layers = layer_rows(census);
    // A skipped layer rendered nothing, so it can never match a pass; the join
    // below only scans the composited prefix.
    let composited = layers.len().min(census.layers.len());

    let mut rows = Vec::with_capacity(passes.len());
    for pass in passes {
        let matched = layers[..composited]
            .iter()
            .enumerate()
            .filter(|(_, layer)| {
                pass.ranges.iter().any(|range| {
                    let (first, count) = layer.index_range();
                    overlaps((range.first, range.count), (first, count))
                })
            })
            .map(|(index, _)| index as u32)
            .collect::<Vec<_>>();
        rows.push(GpuPass {
            label: pass.label.chars().take(128).collect(),
            kind: pass.kind.label().to_owned(),
            draws: pass.draws,
            indices: pass.indices,
            vertices: pass.vertices,
            layers: matched,
            gpu_ms: None,
        });
    }

    // Reverse edge: which passes touched each layer. Only the composited prefix
    // can be reached, which the loop above guarantees.
    for (pass_index, row) in rows.iter().enumerate() {
        for &layer_index in &row.layers {
            layers[layer_index as usize].passes.push(pass_index as u32);
        }
    }

    GpuFrameBreakdown {
        timing_source: "cpu-only".to_owned(),
        passes: rows,
        layers,
        pass_count,
        layer_count: census.layers.len() + census.skipped.len(),
    }
}

fn layer_rows(census: &LayerCensus) -> Vec<GpuLayer> {
    let mut rows = Vec::with_capacity((census.layers.len() + census.skipped.len()).min(MAX_LAYERS));
    for layer in census.layers.iter().take(MAX_LAYERS) {
        rows.push(composited_row(layer));
    }
    for layer in census.skipped.iter().take(MAX_LAYERS - rows.len()) {
        rows.push(GpuLayer {
            node_id: layer.node.clone(),
            first_index: 0,
            index_count: 0,
            recorded: false,
            translated: false,
            skipped: true,
            x: f64::from(layer.bounds.x),
            y: f64::from(layer.bounds.y),
            width: f64::from(layer.bounds.width),
            height: f64::from(layer.bounds.height),
            passes: Vec::new(),
        });
    }
    rows
}

fn composited_row(layer: &CompositedLayer) -> GpuLayer {
    GpuLayer {
        node_id: layer.node.clone(),
        first_index: layer.first_index,
        index_count: layer.index_count,
        recorded: layer.recorded,
        translated: layer.translated,
        skipped: false,
        x: f64::from(layer.bounds.x),
        y: f64::from(layer.bounds.y),
        width: f64::from(layer.bounds.width),
        height: f64::from(layer.bounds.height),
        passes: Vec::new(),
    }
}

fn overlaps((first, count): (u64, u64), (other_first, other_count): (u64, u64)) -> bool {
    count > 0
        && other_count > 0
        && first < other_first.saturating_add(other_count)
        && other_first < first.saturating_add(count)
}

pub(crate) fn apply_timing(
    breakdown: &mut GpuFrameBreakdown,
    api: GraphicsApi,
    timing: &GpuSubmissionTiming,
) {
    let has_pass_time = timing.passes.iter().any(Option::is_some);
    if timing.nanoseconds.is_some() || has_pass_time {
        breakdown.timing_source = match api {
            GraphicsApi::Vulkan => "vulkan-timestamps",
            GraphicsApi::Metal if has_pass_time => "metal-counters",
            GraphicsApi::Metal => "metal-submission",
            GraphicsApi::Dx12 => "dx12-timestamps",
            GraphicsApi::WebGpu => "webgpu",
        }
        .to_owned();
    }
    for (pass, nanos) in breakdown.passes.iter_mut().zip(&timing.passes) {
        pass.gpu_ms = nanos.map(|nanos| nanos as f64 / 1_000_000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cove_core::scene::Rect;
    use cove_core::scene::SkippedLayer;
    use cove_graphics::{PassIndexRange, PassKind};

    fn pass(label: &str, ranges: &[(u64, u64)]) -> PassReport {
        PassReport {
            label: label.to_owned(),
            kind: PassKind::Render,
            draws: ranges.len() as u64,
            instances: ranges.len() as u64,
            vertices: 0,
            indices: ranges.iter().map(|(_, count)| *count).sum(),
            ranges: ranges
                .iter()
                .map(|(first, count)| PassIndexRange::new(*first, *count))
                .collect(),
        }
    }

    fn layer(node: &str, first: u64, count: u64, recorded: bool) -> CompositedLayer {
        CompositedLayer {
            node: Some(node.to_owned()),
            first_index: first,
            index_count: count,
            recorded,
            translated: false,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
        }
    }

    #[test]
    fn a_pass_is_attributed_to_the_layer_that_owns_its_index_window() {
        let passes = [pass("ui-frame", &[(0, 60)]), pass("backdrop", &[(600, 6)])];
        let mut census = LayerCensus::default();
        census.layers.push(layer("1", 0, 60, true));
        census.layers.push(layer("2", 60, 540, false));
        census.layers.push(layer("3", 600, 6, false));
        let breakdown = assemble(&passes, passes.len(), &census);
        // Windows that merely touch (60..600 starts where 0..60 ends) do not
        // overlap, so the first pass reaches exactly the first layer.
        assert_eq!(breakdown.passes[0].layers, [0]);
        assert_eq!(breakdown.passes[1].layers, [2]);
        assert_eq!(breakdown.layers[0].passes, [0]);
        assert_eq!(breakdown.layers[2].passes, [1]);
        assert_eq!(breakdown.timing_source, "cpu-only");
    }

    #[test]
    fn a_skipped_layer_is_reported_without_attribution() {
        let passes = [pass("ui-frame", &[(0, 6)])];
        let mut census = LayerCensus::default();
        census.layers.push(layer("1", 0, 6, true));
        census.skipped.push(SkippedLayer {
            node: Some("2".to_owned()),
            bounds: Rect::new(-200.0, 0.0, 50.0, 50.0),
        });
        let breakdown = assemble(&passes, passes.len(), &census);
        assert_eq!(breakdown.layers.len(), 2);
        assert!(breakdown.layers[1].skipped);
        assert_eq!(breakdown.layers[1].index_count, 0);
        assert!(breakdown.layers[1].passes.is_empty());
        assert_eq!(breakdown.passes[0].layers, [0]);
    }

    #[test]
    fn pass_times_keep_their_positions_with_duplicate_labels_and_missing_queries() {
        let passes = [pass("ui", &[]), pass("ui", &[]), pass("ui", &[])];
        let mut breakdown = assemble(&passes, passes.len(), &LayerCensus::default());
        apply_timing(
            &mut breakdown,
            GraphicsApi::Vulkan,
            &GpuSubmissionTiming {
                nanoseconds: Some(9_000_000),
                passes: vec![Some(2_000_000), None, Some(4_000_000)],
            },
        );
        assert_eq!(
            breakdown
                .passes
                .iter()
                .map(|pass| pass.gpu_ms)
                .collect::<Vec<_>>(),
            [Some(2.0), None, Some(4.0)]
        );
        assert_eq!(breakdown.timing_source, "vulkan-timestamps");
    }
}
