//! Translation of Graphics/Tests/NativeRHITests/AccessTrackerTests.swift.

use cove_graphics::{
    AccessKind, AccessStage, AccessTracker, BufferRange, HazardKind, QueueClass, ResourceAccess,
    ResourceKind, ResourceRef, ShaderStage,
};

fn buffer() -> ResourceRef {
    ResourceRef::new(ResourceKind::Buffer, 1)
}

fn texture() -> ResourceRef {
    ResourceRef::new(ResourceKind::Texture, 2)
}

fn access(resource: ResourceRef, kind: AccessKind, stage: AccessStage) -> ResourceAccess {
    ResourceAccess::new(resource, kind, stage)
}

fn ranged(resource: ResourceRef, kind: AccessKind, stage: AccessStage, range: BufferRange) -> ResourceAccess {
    access(resource, kind, stage).with_range(range)
}

#[test]
fn write_after_write_is_detected() {
    let mut tracker = AccessTracker::new();
    assert!(tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics).is_empty());
    let hazards = tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics);
    assert_eq!(hazards.len(), 1);
    assert_eq!(hazards[0].kind, HazardKind::WriteAfterWrite);
    assert_eq!(hazards[0].source_queue, QueueClass::Graphics);
    assert_eq!(hazards[0].destination_queue, QueueClass::Graphics);
}

#[test]
fn write_after_read_is_detected() {
    let mut tracker = AccessTracker::new();
    assert!(tracker.observe(access(buffer(), AccessKind::Read, AccessStage::COMPUTE), QueueClass::Graphics).is_empty());
    let hazards = tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics);
    assert_eq!(hazards.len(), 1);
    assert_eq!(hazards[0].kind, HazardKind::WriteAfterRead);
}

#[test]
fn read_after_write_is_detected() {
    let mut tracker = AccessTracker::new();
    assert!(tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Compute).is_empty());
    let hazards = tracker.observe(access(buffer(), AccessKind::Read, AccessStage::COMPUTE), QueueClass::Compute);
    assert_eq!(hazards.len(), 1);
    assert_eq!(hazards[0].kind, HazardKind::ReadAfterWrite);
}

#[test]
fn independent_reads_produce_no_hazard() {
    let mut tracker = AccessTracker::new();
    assert!(tracker.observe(access(buffer(), AccessKind::Read, AccessStage::COMPUTE), QueueClass::Graphics).is_empty());
    assert!(tracker.observe(access(buffer(), AccessKind::Read, AccessStage::COMPUTE), QueueClass::Graphics).is_empty());
}

#[test]
fn different_resources_produce_no_hazard() {
    let mut tracker = AccessTracker::new();
    let other = ResourceRef::new(ResourceKind::Buffer, 9);
    tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics);
    let hazards = tracker.observe(access(other, AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics);
    assert!(hazards.is_empty());
}

#[test]
fn disjoint_ranges_produce_no_hazard() {
    let mut tracker = AccessTracker::new();
    tracker.observe(
        ranged(buffer(), AccessKind::Write, AccessStage::COMPUTE, BufferRange::new(0, 1024)),
        QueueClass::Graphics,
    );
    let hazards = tracker.observe(
        ranged(buffer(), AccessKind::Write, AccessStage::COMPUTE, BufferRange::new(2048, 1024)),
        QueueClass::Graphics,
    );
    assert!(hazards.is_empty());
}

#[test]
fn overlapping_ranges_produce_hazard() {
    let mut tracker = AccessTracker::new();
    tracker.observe(
        ranged(buffer(), AccessKind::Write, AccessStage::COMPUTE, BufferRange::new(0, 1024)),
        QueueClass::Graphics,
    );
    let hazards = tracker.observe(
        ranged(buffer(), AccessKind::Read, AccessStage::COMPUTE, BufferRange::new(512, 1024)),
        QueueClass::Graphics,
    );
    assert_eq!(hazards[0].kind, HazardKind::ReadAfterWrite);
}

#[test]
fn write_clears_prior_read_window() {
    let mut tracker = AccessTracker::new();
    tracker.observe(access(buffer(), AccessKind::Read, AccessStage::COMPUTE), QueueClass::Graphics);
    tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics);
    let hazards = tracker.observe(access(buffer(), AccessKind::Read, AccessStage::COMPUTE), QueueClass::Graphics);
    assert_eq!(hazards.len(), 1);
    assert_eq!(hazards[0].kind, HazardKind::ReadAfterWrite);
}

#[test]
fn cross_queue_hazard_attributes_queues() {
    let mut tracker = AccessTracker::new();
    tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Compute);
    let hazards = tracker.observe(access(buffer(), AccessKind::Read, AccessStage::FRAGMENT), QueueClass::Graphics);
    assert_eq!(hazards.len(), 1);
    assert_eq!(hazards[0].kind, HazardKind::ReadAfterWrite);
    assert_eq!(hazards[0].source_queue, QueueClass::Compute);
    assert_eq!(hazards[0].destination_queue, QueueClass::Graphics);
    assert_eq!(hazards[0].source_stage, AccessStage::COMPUTE);
    assert_eq!(hazards[0].destination_stage, AccessStage::FRAGMENT);
}

#[test]
fn read_only_frames_keep_one_dependency_per_queue_and_range() {
    let mut tracker = AccessTracker::new();
    for _ in 0..5_000 {
        assert!(tracker.observe(access(buffer(), AccessKind::Read, AccessStage::VERTEX), QueueClass::Graphics).is_empty());
        assert!(tracker.observe(access(buffer(), AccessKind::Read, AccessStage::FRAGMENT), QueueClass::Graphics).is_empty());
    }
    tracker.observe(access(buffer(), AccessKind::Read, AccessStage::COMPUTE), QueueClass::Compute);
    let hazards = tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Transfer);
    assert_eq!(hazards.len(), 2);
    assert_eq!(
        hazards.iter().find(|h| h.source_queue == QueueClass::Graphics).map(|h| h.source_stage),
        Some(AccessStage::VERTEX | AccessStage::FRAGMENT)
    );
    assert_eq!(
        hazards.iter().find(|h| h.source_queue == QueueClass::Compute).map(|h| h.source_stage),
        Some(AccessStage::COMPUTE)
    );
    assert!(hazards.iter().all(|h| h.kind == HazardKind::WriteAfterRead && h.destination_queue == QueueClass::Transfer));
}

#[test]
fn covered_reads_reuse_raw_and_preserve_stage_union_for_the_next_write() {
    let mut tracker = AccessTracker::new();
    tracker.observe(access(buffer(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Compute);
    for index in 0..128 {
        let hazards = tracker.observe(access(buffer(), AccessKind::Read, AccessStage::VERTEX), QueueClass::Graphics);
        if index == 0 {
            assert_eq!(hazards.len(), 1);
            assert_eq!(hazards[0].kind, HazardKind::ReadAfterWrite);
            assert_eq!(hazards[0].source_queue, QueueClass::Compute);
            assert_eq!(hazards[0].destination_stage, AccessStage::VERTEX);
        } else {
            assert!(hazards.is_empty());
        }
    }
    let expanded = tracker.observe(access(buffer(), AccessKind::Read, AccessStage::FRAGMENT), QueueClass::Graphics);
    assert_eq!(expanded[0].kind, HazardKind::ReadAfterWrite);
    assert_eq!(expanded[0].destination_stage, AccessStage::FRAGMENT);
    assert!(
        tracker
            .observe(
                access(buffer(), AccessKind::Read, AccessStage::VERTEX | AccessStage::FRAGMENT),
                QueueClass::Graphics
            )
            .is_empty()
    );
    let writes = tracker.observe(access(buffer(), AccessKind::Write, AccessStage::TRANSFER), QueueClass::Transfer);
    assert_eq!(writes.len(), 2);
    assert_eq!(
        writes.iter().find(|h| h.kind == HazardKind::WriteAfterRead).map(|h| h.source_stage),
        Some(AccessStage::VERTEX | AccessStage::FRAGMENT)
    );
    assert_eq!(
        writes.iter().find(|h| h.kind == HazardKind::WriteAfterWrite).map(|h| h.source_queue),
        Some(QueueClass::Compute)
    );
    for queue in [QueueClass::Graphics, QueueClass::Compute] {
        let next = tracker.observe(access(buffer(), AccessKind::Read, AccessStage::FRAGMENT), queue);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].source_queue, QueueClass::Transfer);
        assert_eq!(next[0].destination_queue, queue);
        assert_eq!(next[0].kind, HazardKind::ReadAfterWrite);
    }
}

#[test]
fn reset_clears_all_state() {
    let mut tracker = AccessTracker::new();
    assert!(tracker.observe(access(texture(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics).is_empty());
    assert!(!tracker.observe(access(texture(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics).is_empty());
    tracker.reset();
    let hazards = tracker.observe(access(texture(), AccessKind::Write, AccessStage::COMPUTE), QueueClass::Graphics);
    assert!(hazards.is_empty());
}

#[test]
fn read_coverage_does_not_hide_new_ranges_queues_or_immediate_writes() {
    let mut tracker = AccessTracker::new();
    let first = BufferRange::new(0, 64);
    let second = BufferRange::new(64, 64);
    tracker.observe(access(buffer(), AccessKind::Write, AccessStage::TRANSFER), QueueClass::Graphics);
    assert_eq!(
        tracker
            .observe(ranged(buffer(), AccessKind::Read, AccessStage::VERTEX, first), QueueClass::Graphics)
            .len(),
        1
    );
    assert!(
        tracker
            .observe(ranged(buffer(), AccessKind::Read, AccessStage::VERTEX, first), QueueClass::Graphics)
            .is_empty()
    );
    assert_eq!(
        tracker
            .observe(ranged(buffer(), AccessKind::Read, AccessStage::VERTEX, second), QueueClass::Graphics)
            .len(),
        1
    );
    assert_eq!(
        tracker
            .observe(ranged(buffer(), AccessKind::Read, AccessStage::VERTEX, first), QueueClass::Compute)
            .len(),
        1
    );
    let writes = tracker.observe(access(buffer(), AccessKind::Write, AccessStage::TRANSFER), QueueClass::Graphics);
    assert_eq!(writes.iter().filter(|h| h.kind == HazardKind::WriteAfterRead).count(), 3);
    assert_eq!(
        tracker
            .observe(ranged(buffer(), AccessKind::Read, AccessStage::VERTEX, first), QueueClass::Graphics)
            .len(),
        1
    );
}

#[test]
fn shader_stage_maps_to_access_stage() {
    assert_eq!(AccessStage::of(ShaderStage::Vertex), AccessStage::VERTEX);
    assert_eq!(AccessStage::of(ShaderStage::Fragment), AccessStage::FRAGMENT);
    assert_eq!(AccessStage::of(ShaderStage::Compute), AccessStage::COMPUTE);
}
