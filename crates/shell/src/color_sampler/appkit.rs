//! macOS backend: AppKit `NSColorSampler` through `objc2-app-kit`.
//!
//! `showSamplerWithSelectionHandler:` returns immediately; AppKit owns the
//! system picker and calls the handler on the main thread when the user picks
//! or cancels. Nothing here waits, polls, or blocks the platform thread.

use super::{CompletionSink, Session};
use block2::RcBlock;
use cove_platform::{ScreenColor, ScreenColorSample};
use objc2::{MainThreadMarker, rc::Retained, runtime::AnyClass};
use objc2_app_kit::{NSColor, NSColorSampler, NSColorSpace};

/// Strong references survive until the deferred completion pass drops the
/// session. AppKit also retains the sampler, but keeping our own references
/// makes the lifetime independent of a framework implementation detail.
struct AppKitSession {
    _sampler: Retained<NSColorSampler>,
    _block: RcBlock<dyn Fn(*mut NSColor)>,
}

pub(super) fn start(request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
    if AnyClass::get(c"NSColorSampler").is_none() {
        return Err(ScreenColorSample::Unsupported(
            "NSColorSampler requires macOS 10.15 or newer".into(),
        ));
    }
    // `run_with_ui_thread` keeps the platform pump on the application's main
    // thread, which is the thread AppKit requires for sampler presentation.
    if MainThreadMarker::new().is_none() {
        return Err(ScreenColorSample::Failed(
            "NSColorSampler must be started on the main thread".into(),
        ));
    }
    let sampler = NSColorSampler::new();
    let block: RcBlock<dyn Fn(*mut NSColor)> = RcBlock::new(move |color: *mut NSColor| {
        sink.deliver(request, sample(color));
    });
    // SAFETY: `block` outlives the call (held by `AppKitSession`), the sampler
    // is a live AppKit object, and AppKit invokes the handler exactly once on
    // the main thread with a retained `NSColor` or nil on cancellation.
    unsafe { sampler.showSamplerWithSelectionHandler(&block) };
    Ok(Session::native(Box::new(AppKitSession {
        _sampler: sampler,
        _block: block,
    })))
}

fn sample(color: *mut NSColor) -> ScreenColorSample {
    // SAFETY: AppKit passes nil on cancellation, otherwise a retained colour
    // valid for the duration of the handler.
    let Some(color) = (unsafe { color.as_ref() }) else {
        return ScreenColorSample::Cancelled;
    };
    let Some(rgb) = color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()) else {
        return ScreenColorSample::Failed("the sampled colour cannot be converted to sRGB".into());
    };
    ScreenColorSample::Picked(
        ScreenColor::new(
            rgb.redComponent() as f32,
            rgb.greenComponent() as f32,
            rgb.blueComponent() as f32,
            rgb.alphaComponent() as f32,
        )
        .clamped(),
    )
}
