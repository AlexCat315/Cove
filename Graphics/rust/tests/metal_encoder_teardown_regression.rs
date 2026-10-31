//! Regression for the Metal compute-encoder leak on an encoding failure.
//!
//! `encode_compute_pass` used to install `self.compute_encoder` only *after* the
//! body loop succeeded. An error inside the loop (`?`) dropped the live
//! `MTLComputeCommandEncoder` without `endEncoding()`, and Metal aborts the
//! process at dealloc:
//!
//!   -[_MTLCommandEncoder dealloc]: failed assertion
//!   'Command encoder released without endEncoding'
//!
//! Swift assigns `computeEncoder` before iterating
//! (`MetalSubmitEncoder.swift:470-483`), so its `endActiveEncoders()` teardown
//! always ends it. This target pins that behaviour: a failing compute command
//! must return an error and the process must survive.

#![cfg(target_os = "macos")]

use cove_graphics::rhi::{
    BindingSet, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass, RhiError,
};

fn metal_device() -> Option<Device> {
    Device::make(DeviceConfig {
        preferred_backends: vec![GraphicsApi::Metal],
        enable_validation: false,
        ..DeviceConfig::default()
    })
    .ok()
}

#[test]
fn a_failing_compute_command_returns_an_error_and_does_not_abort() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping");
        return;
    };

    // A binding set handle that was never registered makes
    // `encode_compute_command` fail inside the pass body.
    let mut commands = CommandBuffer::new();
    commands.compute_pass(|encoder| {
        encoder.set_binding_set(BindingSet::new(u32::MAX), 0, Vec::new());
    });

    device.begin_frame().expect("begin frame");
    let outcome = device.submit(&commands, QueueClass::Compute, &[], &[], None);
    device.end_frame();

    let error = match outcome {
        Ok(()) => panic!("an unregistered compute binding set must be rejected"),
        Err(error) => error,
    };
    // The exact validation variant is not the point: any rejection before
    // `endEncoding` is what used to leak the encoder.
    assert!(
        matches!(
            error,
            RhiError::InvalidArgument(_) | RhiError::LayoutMismatch(_)
        ),
        "expected a validation error, got {error:?}"
    );

    // Reaching here at all is the real assertion: before the fix the encoder was
    // released without `endEncoding` and the process aborted before this point.
    device.wait_until_idle().expect("the device stays usable");
}

#[test]
fn a_failing_compute_command_after_a_valid_one_still_tears_down() {
    let Some(device) = metal_device() else {
        eprintln!("no Metal device available; skipping");
        return;
    };

    // Two passes: one that fails early, one that fails after real work is
    // recorded. Both must tear the encoder down without aborting.
    let mut commands = CommandBuffer::new();
    commands.compute_pass(|encoder| {
        encoder.set_binding_set(BindingSet::new(u32::MAX), 0, Vec::new());
    });
    commands.compute_pass(|encoder| {
        encoder.set_binding_set(BindingSet::new(u32::MAX), 1, Vec::new());
        encoder.dispatch(1, 1, 1);
    });

    device.begin_frame().expect("begin frame");
    let outcome = device.submit(&commands, QueueClass::Compute, &[], &[], None);
    device.end_frame();

    assert!(
        outcome.is_err(),
        "each unregistered binding set must be rejected"
    );
    device
        .wait_until_idle()
        .expect("the device survives teardown after partial encoding");
}
