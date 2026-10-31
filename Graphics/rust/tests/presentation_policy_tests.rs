//! Translation of `Graphics/Tests/NativeRHITests/Vulkan/PresentationPolicyTests.swift`.
//!
//! Behaviour checklist:
//! - VSync selects FIFO (ordered refresh) even when other modes exist, and FIFO
//!   synchronizes to the display
//! - disabling VSync prefers IMMEDIATE regardless of the driver's ordering
//! - when IMMEDIATE is unavailable it falls back to MAILBOX, then FIFO, both of
//!   which still synchronize; COMPOSITOR always reports as synchronized
//! - an empty or incompatible mode list is rejected
//!
//! `choose_presentation_mode` is deliberately independent of Vulkan handles, so
//! this target runs on every host instead of only Windows/Linux.

use cove_graphics::rhi::PresentationMode;
use cove_graphics::vulkan::choose_presentation_mode;

#[test]
fn vsync_uses_ordered_refresh_even_when_other_modes_are_available() {
    let selected = choose_presentation_mode(
        true,
        &[
            PresentationMode::Immediate,
            PresentationMode::Mailbox,
            PresentationMode::Fifo,
        ],
    )
    .expect("FIFO is available");

    assert_eq!(selected, PresentationMode::Fifo);
    assert!(
        selected.synchronizes_to_display(),
        "VSync must present in step with the display"
    );
}

#[test]
fn disabling_vsync_prefers_immediate_regardless_of_driver_order() {
    let lists = [
        vec![
            PresentationMode::Fifo,
            PresentationMode::Mailbox,
            PresentationMode::Immediate,
        ],
        vec![
            PresentationMode::Immediate,
            PresentationMode::Fifo,
            PresentationMode::Mailbox,
        ],
    ];
    for supported in lists {
        let selected =
            choose_presentation_mode(false, &supported).expect("IMMEDIATE is available");
        assert_eq!(
            selected,
            PresentationMode::Immediate,
            "driver ordering must not change the choice"
        );
        assert!(!selected.synchronizes_to_display());
    }
}

#[test]
fn unavailable_immediate_reports_a_synchronized_fallback() {
    let mailbox = choose_presentation_mode(false, &[PresentationMode::Fifo, PresentationMode::Mailbox])
        .expect("MAILBOX is available");
    assert_eq!(mailbox, PresentationMode::Mailbox);
    assert!(
        mailbox.synchronizes_to_display(),
        "MAILBOX paces to the compositor, so it is a synchronized fallback"
    );

    let fifo = choose_presentation_mode(false, &[PresentationMode::Fifo])
        .expect("FIFO is available");
    assert_eq!(fifo, PresentationMode::Fifo);
    assert!(fifo.synchronizes_to_display());

    assert!(
        PresentationMode::Compositor.synchronizes_to_display(),
        "a compositor-owned mode always synchronizes"
    );
}

#[test]
fn invalid_mode_lists_are_rejected() {
    assert!(
        choose_presentation_mode(false, &[]).is_err(),
        "an empty list has no usable mode"
    );
    assert!(
        choose_presentation_mode(false, &[PresentationMode::Compositor]).is_err(),
        "the Vulkan surface never supports a compositor-owned mode"
    );
    assert!(
        choose_presentation_mode(true, &[PresentationMode::Immediate]).is_err(),
        "VSync cannot be honoured by IMMEDIATE alone"
    );
}
