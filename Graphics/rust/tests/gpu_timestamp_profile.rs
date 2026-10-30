//! Translation of Graphics/Tests/NativeRHITests/GPUTimestampProfileTests.swift.

use cove_graphics::{GpuTimestampMarker, GpuTimestampProfile, RenderPassDescriptor};
use std::sync::Arc;
use std::thread;

fn marker(label: &str, nanoseconds: u64) -> GpuTimestampMarker {
    GpuTimestampMarker::new(label, nanoseconds)
}

#[test]
fn markers_keep_completion_order_and_count() {
    let profile = GpuTimestampProfile::default();
    profile.append(marker("ui-segment", 1_000));
    profile.append(marker("backdrop-blit", 2_000));
    profile.append(marker("backdrop-blur-v", 3_000));
    assert_eq!(profile.sample_count(), 3);
    assert_eq!(
        profile.recorded_markers().iter().map(|m| m.label.clone()).collect::<Vec<_>>(),
        ["ui-segment", "backdrop-blit", "backdrop-blur-v"]
    );
    assert_eq!(
        profile.recorded_markers().iter().map(|m| m.nanoseconds).collect::<Vec<_>>(),
        [1_000, 2_000, 3_000]
    );
}

#[test]
fn sampling_stops_at_the_marker_limit_but_still_counts() {
    let profile = GpuTimestampProfile::new(2);
    for index in 0..5 {
        profile.append(marker("pass", index));
    }
    assert_eq!(profile.sample_count(), 5, "every completion should still be counted");
    assert_eq!(profile.recorded_markers().len(), 2, "only the limit worth of markers is retained");
    assert_eq!(
        profile.recorded_markers().iter().map(|m| m.nanoseconds).collect::<Vec<_>>(),
        [0, 1]
    );
}

#[test]
fn reset_clears_counters_and_retained_markers() {
    let profile = GpuTimestampProfile::default();
    profile.append(marker("ui-segment", 1_000));
    profile.reset();
    assert_eq!(profile.sample_count(), 0);
    assert!(profile.recorded_markers().is_empty());
}

#[test]
fn concurrent_appends_are_serialised() {
    let profile = Arc::new(GpuTimestampProfile::new(1_000));
    let writers = 4;
    let per_writer = 50;
    let mut handles = Vec::new();
    for writer in 0..writers {
        let profile = profile.clone();
        handles.push(thread::spawn(move || {
            for index in 0..per_writer {
                profile.append(marker(&format!("pass-{writer}"), index));
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(profile.sample_count(), (writers * per_writer) as usize);
    assert_eq!(profile.recorded_markers().len(), (writers * per_writer) as usize);
}

#[test]
fn zero_limit_retains_nothing_but_still_counts() {
    let profile = GpuTimestampProfile::new(0);
    profile.append(marker("ui-segment", 1_000));
    assert_eq!(profile.sample_count(), 1);
    assert!(profile.recorded_markers().is_empty());
}

#[test]
fn descriptor_label_defaults_to_none_so_unlabelled_passes_stay_unchanged() {
    assert_eq!(RenderPassDescriptor::new(vec![]).label, None);
    let mut labelled = RenderPassDescriptor::new(vec![]);
    labelled.label = Some("ui-segment");
    assert_eq!(labelled.label, Some("ui-segment"));
}
