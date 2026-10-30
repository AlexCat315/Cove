//! Translation of Graphics/Tests/NativeRHITests/StateTrackerTests.swift.

use cove_graphics::{ResourceRef, ResourceState, StateTracker};

fn reference(id: u32) -> ResourceRef {
    ResourceRef::texture(id)
}

#[test]
fn first_requirement_emits_barrier_from_empty_state() {
    let mut tracker = StateTracker::new();
    tracker.require_state(reference(1), ResourceState::RENDER_TARGET);
    let barriers = tracker.commit_barriers();

    assert_eq!(barriers.len(), 1);
    assert_eq!(barriers[0].before, ResourceState::EMPTY);
    assert_eq!(barriers[0].after, ResourceState::RENDER_TARGET);
}

#[test]
fn requiring_same_state_emits_no_barrier() {
    let mut tracker = StateTracker::new();
    tracker.set_initial_state(reference(1), ResourceState::RENDER_TARGET);
    tracker.require_state(reference(1), ResourceState::RENDER_TARGET);
    assert!(tracker.commit_barriers().is_empty());
}

#[test]
fn state_change_emits_barrier_with_before_and_after() {
    let mut tracker = StateTracker::new();
    tracker.set_initial_state(reference(1), ResourceState::SHADER_RESOURCE);
    tracker.require_state(reference(1), ResourceState::RENDER_TARGET);
    let barriers = tracker.commit_barriers();

    assert_eq!(barriers.len(), 1);
    assert_eq!(barriers[0].before, ResourceState::SHADER_RESOURCE);
    assert_eq!(barriers[0].after, ResourceState::RENDER_TARGET);
}

#[test]
fn repeated_transitions_of_same_resource_merge_into_one_barrier() {
    let mut tracker = StateTracker::new();
    tracker.require_state(reference(1), ResourceState::SHADER_RESOURCE);
    tracker.require_state(reference(1), ResourceState::RENDER_TARGET);
    let barriers = tracker.commit_barriers();

    // Two transitions merge into a single barrier whose destination is the
    // union of the requested states.
    assert_eq!(barriers.len(), 1);
    assert_eq!(barriers[0].before, ResourceState::EMPTY);
    assert_eq!(
        barriers[0].after,
        ResourceState::SHADER_RESOURCE | ResourceState::RENDER_TARGET
    );
}

#[test]
fn distinct_resources_are_not_merged() {
    let mut tracker = StateTracker::new();
    tracker.require_state(ResourceRef::texture(1), ResourceState::RENDER_TARGET);
    tracker.require_state(ResourceRef::texture(2), ResourceState::RENDER_TARGET);
    assert_eq!(tracker.commit_barriers().len(), 2);
}

#[test]
fn commit_drains_pending_barriers() {
    let mut tracker = StateTracker::new();
    tracker.require_state(reference(1), ResourceState::RENDER_TARGET);
    assert_eq!(tracker.commit_barriers().len(), 1);
    assert!(tracker.commit_barriers().is_empty());
}

#[test]
fn remove_resource_stops_tracking() {
    let mut tracker = StateTracker::new();
    tracker.set_initial_state(reference(1), ResourceState::RENDER_TARGET);
    tracker.remove_resource(reference(1));
    tracker.require_state(reference(1), ResourceState::SHADER_RESOURCE);
    // After removal the resource is unknown again, so the barrier starts
    // from the empty state rather than the old renderTarget state.
    let barriers = tracker.commit_barriers();
    assert_eq!(barriers[0].before, ResourceState::EMPTY);
}
