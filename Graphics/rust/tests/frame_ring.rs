//! Translation of Graphics/Tests/NativeRHITests/FrameRingTests.swift.

use cove_graphics::FrameRing;

#[test]
fn retire_runs_immediately_without_active_frame() {
    let ring = FrameRing::new(1);
    let ready = ring.register_retirement(1);
    assert_eq!(ready, vec![1]);
}

#[test]
fn retire_deferred_until_command_buffer_completes() {
    let ring = FrameRing::new(1);
    let (slot, _) = ring.begin();
    assert_eq!(slot, 0);

    ring.register_command_buffer(slot);
    let retired = ring.register_retirement(7);
    assert!(!retired.contains(&7));

    ring.end();
    ring.command_buffer_completed(slot);
    let ready = ring.wait_until_idle();
    assert!(ready.contains(&7));
}

#[test]
fn retire_waits_for_all_command_buffers() {
    let ring = FrameRing::new(1);
    let (slot, _) = ring.begin();

    ring.register_command_buffer(slot);
    ring.register_command_buffer(slot);
    let retired = ring.register_retirement(7);
    assert!(!retired.contains(&7));

    ring.command_buffer_completed(slot);
    assert_eq!(ring.completed_frames(), 0);
    ring.end();
    assert_eq!(ring.completed_frames(), 0);
    ring.command_buffer_completed(slot);
    assert!(ring.wait_until_idle().contains(&7));
}

#[test]
fn completion_before_end_does_not_retire_or_close_frame() {
    let ring = FrameRing::new(1);
    let (slot, _) = ring.begin();
    ring.register_command_buffer(slot);
    let retired = ring.register_retirement(7);
    ring.command_buffer_completed(slot);
    assert_eq!(ring.current_slot_index(), Some(slot));
    assert!(!retired.contains(&7));
    ring.register_command_buffer(slot);
    ring.end();
    ring.command_buffer_completed(slot);
    assert!(ring.wait_until_idle().contains(&7));
}

#[test]
fn empty_frames_can_be_reused_repeatedly() {
    let ring = FrameRing::new(1);
    for _ in 0..100 {
        ring.begin();
        ring.end();
        ring.wait_until_idle();
    }
    assert_eq!(ring.completed_frames(), 100);
}

#[test]
fn retirement_waits_for_prior_frames_on_other_queues() {
    let ring = FrameRing::new(2);
    let (prior, _) = ring.begin();
    ring.register_command_buffer(prior);
    ring.end();
    let (current, _) = ring.begin();
    ring.register_command_buffer(current);
    let retired = ring.register_retirement(7);
    ring.end();
    ring.command_buffer_completed(current);
    assert!(!retired.contains(&7));
    assert!(ring.completed_frames() < 2);
    ring.command_buffer_completed(prior);
    assert!(ring.wait_until_idle().contains(&7));
}

#[test]
fn slots_cycle_after_completion() {
    let ring = FrameRing::new(2);
    let (slot0, _) = ring.begin();
    ring.end();
    let (slot1, _) = ring.begin();
    assert_eq!(slot0, 0);
    assert_eq!(slot1, 1);

    ring.end();

    let (slot2, _) = ring.begin();
    assert_eq!(slot2, 0);
}

#[test]
fn unregister_balances_failed_registration() {
    let ring = FrameRing::new(1);
    let (slot, _) = ring.begin();
    ring.register_command_buffer(slot);
    ring.unregister_command_buffer(slot);
    ring.end();
    ring.wait_until_idle();
    // Registration count is back to zero; the slot is still in flight but
    // completing is not required. No assertion beyond not panicking.
}
