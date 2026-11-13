//! Attribution join: who rendered what, and how long the GPU spent on it.
//!
//! Three inputs meet here:
//!
//! * `PassReport`s — what the frame's command buffer recorded, pass by pass
//!   (`CommandBuffer::pass_reports()`).
//! * `LayerCensus` — what the layer-aware renderer composited, with each layer's
//!   index window in the frame draw list, plus the layers culling skipped.
//! * `GpuTimestampMarker`s — per-pass GPU durations, when the backend emits them
//!   (Vulkan does, one label per pass; Metal times submissions, not passes).
//!
//! The join is deliberately conservative: a pass is attributed to a layer only
//! when their index windows overlap, and GPU time is attached to a pass only
//! when a marker names it. Nothing is apportioned or estimated, so a missing
//! number reads as "not measured", not as zero.

use cove_core::scene::{CompositedLayer, LayerCensus};
use cove_devtools_protocol::{GpuFrameBreakdown, GpuLayer, GpuPass};
use cove_graphics::{GpuTimestampMarker, PassKind, PassReport};

/// Which clock produced the GPU numbers in a breakdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuTimingSource {
    /// One labelled timestamp pair per pass, read back from a query pool.
    VulkanTimestamps,
    /// One pair per submission: pass times are not separable on this backend.
    MetalSubmission,
    /// One number for the whole frame, from the backend's own timer.
    WebGpuFrame,
    /// Nothing was sampled; the breakdown carries geometry only.
    CpuOnly,
}

impl GpuTimingSource {
    pub fn key(self) -> &'static str {
        match self {
            Self::VulkanTimestamps => "vulkan-timestamps",
            Self::MetalSubmission => "metal-submission",
            Self::WebGpuFrame => "webgpu",
            Self::CpuOnly => "cpu-only",
        }
    }
}

/// A timestamped GPU interval, as read back from the backend.
pub struct Marker {
    pub label: String,
    pub nanoseconds: u64,
}

impl From<&GpuTimestampMarker> for Marker {
    fn from(marker: &GpuTimestampMarker) -> Self {
        Self {
            label: marker.label.clone(),
            nanoseconds: marker.nanoseconds,
        }
    }
}

/// Joins a frame's passes, layers and GPU markers into the protocol payload.
///
/// `layers` entries appear in paint order; skipped layers follow them, so an
/// index into the list is stable for both sides of the join.
pub fn assemble(
    passes: &[PassReport],
    census: &LayerCensus,
    markers: &[Marker],
    source: GpuTimingSource,
) -> GpuFrameBreakdown {
    let mut layers = layer_rows(census);
    // A skipped layer rendered nothing, so it can never match a pass; the join
    // below only scans the composited prefix.
    let composited = layers.len() - census.skipped.len();

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
            label: pass.label.clone(),
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

    if source == GpuTimingSource::VulkanTimestamps {
        attribute_labels(&mut rows, markers);
    }

    GpuFrameBreakdown {
        timing_source: source.key().to_owned(),
        passes: rows,
        layers,
    }
}

fn layer_rows(census: &LayerCensus) -> Vec<GpuLayer> {
    let mut rows = Vec::with_capacity(census.layers.len() + census.skipped.len());
    for layer in &census.layers {
        rows.push(composited_row(layer, false));
    }
    for layer in &census.skipped {
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

fn composited_row(layer: &CompositedLayer, skipped: bool) -> GpuLayer {
    GpuLayer {
        node_id: layer.node.clone(),
        first_index: layer.first_index,
        index_count: layer.index_count,
        recorded: layer.recorded,
        translated: layer.translated,
        skipped,
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

/// Attaches each marker to the pass it names.
///
/// A backend that labels unnamed passes by kind and ordinal (`renderPass#0`,
/// `computePass#1`) produces labels this recorder does not know, so those
/// markers are matched by ordinal among the unnamed passes of that kind. A
/// marker that names no pass, or that names two, stays unattached rather than
/// guessing.
fn attribute_labels(passes: &mut [GpuPass], markers: &[Marker]) {
    let mut unlabelled: Vec<(&'static str, Vec<usize>)> = ["render", "compute", "copy"]
        .into_iter()
        .map(|kind| {
            let indices = passes
                .iter()
                .enumerate()
                .filter(|(_, pass)| pass.kind == kind && pass.label == "<unnamed>")
                .map(|(index, _)| index)
                .collect();
            (kind, indices)
        })
        .collect();
    let mut ordinals = [0usize; 3];
    for marker in markers {
        if let Some(index) = passes
            .iter()
            .position(|pass| pass.label == marker.label && pass.gpu_ms.is_none())
        {
            passes[index].gpu_ms = Some(nanoseconds_to_ms(marker.nanoseconds));
            continue;
        }
        let Some(kind_index) = kind_from_label(&marker.label) else {
            continue;
        };
        let (_, indices) = &mut unlabelled[kind_index];
        let ordinal = ordinals[kind_index];
        ordinals[kind_index] += 1;
        if let Some(&index) = indices.get(ordinal) {
            passes[index].gpu_ms = Some(nanoseconds_to_ms(marker.nanoseconds));
        }
    }
}

fn kind_from_label(label: &str) -> Option<usize> {
    if label.starts_with("renderPass") {
        Some(0)
    } else if label.starts_with("computePass") {
        Some(1)
    } else if label.starts_with("copyPass") {
        Some(2)
    } else {
        None
    }
}

fn nanoseconds_to_ms(nanoseconds: u64) -> f64 {
    nanoseconds as f64 / 1_000_000.0
}

/// Pass kind names accepted by the protocol payload, for tests and readers that
/// only have strings.
pub fn kind_key(kind: PassKind) -> &'static str {
    kind.label()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cove_core::scene::Rect;
    use cove_core::scene::SkippedLayer;
    use cove_graphics::PassIndexRange;

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
        let breakdown = assemble(&passes, &census, &[], GpuTimingSource::CpuOnly);
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
        let breakdown = assemble(&passes, &census, &[], GpuTimingSource::CpuOnly);
        assert_eq!(breakdown.layers.len(), 2);
        assert!(breakdown.layers[1].skipped);
        assert_eq!(breakdown.layers[1].index_count, 0);
        assert!(breakdown.layers[1].passes.is_empty());
        assert_eq!(breakdown.passes[0].layers, [0]);
    }

    #[test]
    fn vulkan_labels_attach_time_to_named_and_unnamed_passes() {
        let passes = [
            pass("ui-frame", &[(0, 6)]),
            pass("<unnamed>", &[(6, 6)]),
            pass("<unnamed>", &[(12, 6)]),
        ];
        let markers = [
            Marker {
                label: "ui-frame".to_owned(),
                nanoseconds: 2_000_000,
            },
            Marker {
                label: "renderPass#0".to_owned(),
                nanoseconds: 3_000_000,
            },
            Marker {
                label: "renderPass#1".to_owned(),
                nanoseconds: 4_000_000,
            },
            // A submission-level marker names no pass and stays unattached.
            Marker {
                label: "graphics submission".to_owned(),
                nanoseconds: 9_000_000,
            },
        ];
        let breakdown = assemble(
            &passes,
            &LayerCensus::default(),
            &markers,
            GpuTimingSource::VulkanTimestamps,
        );
        assert_eq!(breakdown.passes[0].gpu_ms, Some(2.0));
        assert_eq!(breakdown.passes[1].gpu_ms, Some(3.0));
        assert_eq!(breakdown.passes[2].gpu_ms, Some(4.0));
        assert_eq!(breakdown.timing_source, "vulkan-timestamps");
    }

    #[test]
    fn metal_submission_markers_leave_pass_times_unmeasured() {
        let passes = [pass("ui-frame", &[(0, 6)])];
        let markers = [Marker {
            label: "graphics submission".to_owned(),
            nanoseconds: 5_000_000,
        }];
        let breakdown = assemble(
            &passes,
            &LayerCensus::default(),
            &markers,
            GpuTimingSource::MetalSubmission,
        );
        assert_eq!(breakdown.passes[0].gpu_ms, None);
        assert_eq!(breakdown.timing_source, "metal-submission");
    }
}
