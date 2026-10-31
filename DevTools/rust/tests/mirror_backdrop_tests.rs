//! Translation of `Tests/CoveUIDevToolsTests/MirrorBackdropTests.swift`.
//!
//! Behaviour checklist:
//! - the mirror capture reuses its target while the extent is unchanged and
//!   reallocates when it changes
//! - a resized capture reports the new stride and a full-size buffer
//! - `reset` frees the target so the next capture reallocates
//! - a capture whose readback is smaller than the requested extent is rejected
//!
//! The Swift suite asserted backdrop-blur contrast on real Metal pixels, which
//! needs a live device plus the `NativeDrawListRenderer` backdrop pass. That
//! half stays device work. This target covers the capture contract the mirror
//! owns — extent accounting, reuse, and readback validation — through the same
//! host-supplied source the GPU path installs, so the assertions run on every
//! host instead of only macOS.

use std::cell::RefCell;
use std::rc::Rc;

use cove_core::DrawList;
use cove_devtools::devtools::mirror::{FrameTapError, FrameTapPixels, NativeAppFrameCapture};

/// A source that records the extents it was asked for and returns an opaque
/// buffer filled with a deterministic pattern.
fn recording_source(
    calls: Rc<RefCell<Vec<(u32, u32, f32, f32)>>>,
) -> Rc<dyn Fn(&DrawList, (u32, u32), (f32, f32)) -> Result<FrameTapPixels, FrameTapError>> {
    Rc::new(move |_list, size, logical| {
        calls.borrow_mut().push((size.0, size.1, logical.0, logical.1));
        Ok(FrameTapPixels::new(
            vec![0x7f_u8; size.0 as usize * size.1 as usize * 4],
            size.0 as usize * 4,
        ))
    })
}

#[test]
fn capture_reuses_its_target_until_the_extent_changes() {
    let calls: Rc<RefCell<Vec<(u32, u32, f32, f32)>>> = Rc::new(RefCell::new(Vec::new()));
    let capture = NativeAppFrameCapture::new(recording_source(calls.clone()));
    let list = DrawList::default();

    assert!(
        capture.requires_new_target((64, 64)),
        "the first capture has no target yet"
    );
    let first = capture.capture(&list, (64, 64), (64.0, 64.0)).expect("capture");
    assert_eq!(capture.target_extent(), Some((64, 64)));
    assert!(!capture.requires_new_target((64, 64)), "the extent is reused");
    assert_eq!(first.bytes_per_row, 256);
    assert_eq!(first.data.len(), 64 * 64 * 4);

    let second = capture.capture(&list, (64, 64), (64.0, 64.0)).expect("recapture");
    assert_eq!(second.data.len(), first.data.len());
    assert_eq!(
        calls.borrow().len(),
        2,
        "the source still ran; reuse is a target concern, not a skip"
    );

    let resized = capture
        .capture(&list, (128, 128), (64.0, 64.0))
        .expect("resized capture");
    assert!(
        capture.requires_new_target((128, 128)) == false,
        "the new extent is recorded"
    );
    assert_eq!(resized.bytes_per_row, 512, "stride follows the pixel width");
    assert_eq!(
        resized.data.len(),
        65_536,
        "a 128x128 BGRA capture is 65536 bytes"
    );
    assert_eq!(
        calls.borrow().last().copied(),
        Some((128, 128, 64.0, 64.0)),
        "the logical viewport is passed through unchanged"
    );
}

#[test]
fn reset_forces_reallocation_on_the_next_capture() {
    let calls: Rc<RefCell<Vec<(u32, u32, f32, f32)>>> = Rc::new(RefCell::new(Vec::new()));
    let capture = NativeAppFrameCapture::new(recording_source(calls));
    let list = DrawList::default();

    capture.capture(&list, (32, 32), (32.0, 32.0)).expect("capture");
    assert!(!capture.requires_new_target((32, 32)));

    capture.reset();
    assert_eq!(capture.target_extent(), None, "reset forgets the extent");
    assert!(
        capture.requires_new_target((32, 32)),
        "the next capture must allocate a fresh target"
    );
    capture.capture(&list, (32, 32), (32.0, 32.0)).expect("recapture");
    assert_eq!(capture.target_extent(), Some((32, 32)));
}

#[test]
fn capture_rejects_an_undersized_readback_and_a_zero_extent() {
    let short = NativeAppFrameCapture::new(Rc::new(|_list, size, _logical| {
        Ok(FrameTapPixels::new(vec![0_u8; 8], size.0 as usize * 4))
    }));
    let list = DrawList::default();

    let error = short
        .capture(&list, (16, 16), (16.0, 16.0))
        .expect_err("a readback smaller than the extent must be rejected");
    assert!(
        matches!(error, FrameTapError::InvalidArgument(_)),
        "under-sized readback is an invalid argument, got {error:?}"
    );

    let empty = NativeAppFrameCapture::new(Rc::new(|_list, size, _logical| {
        Ok(FrameTapPixels::new(vec![0_u8; size.0 as usize * size.1 as usize * 4], size.0 as usize * 4))
    }));
    let error = empty
        .capture(&list, (0, 16), (0.0, 16.0))
        .expect_err("a zero extent must be rejected");
    assert!(matches!(error, FrameTapError::InvalidArgument(_)));
}
