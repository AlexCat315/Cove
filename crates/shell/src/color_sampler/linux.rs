//! Linux has no standard screen-colour API. Two backends cover the window
//! systems: the XDG desktop portal's `PickColor` (a real system picker on
//! Wayland) and X11 `XGetImage` (screenshot, then sample one pixel). Both are
//! tried, in window-system order, on a single worker thread.

use super::{Attempt, CompletionSink, Session};
use cove_platform::{ScreenColorBackend, ScreenColorSample};
use std::{
    env,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Cheap startup probe for the UI affordance. The real availability decision is
/// made on the worker thread, where a failed display or D-Bus connection cannot
/// stall the platform pump.
pub(super) fn backend() -> ScreenColorBackend {
    if env::var_os("WAYLAND_DISPLAY").is_some() {
        ScreenColorBackend::XdgPortal
    } else if env::var_os("DISPLAY").is_some() {
        ScreenColorBackend::X11
    } else {
        ScreenColorBackend::None
    }
}

pub(super) fn start(request: u64, sink: CompletionSink) -> Result<Session, ScreenColorSample> {
    let wayland = env::var_os("WAYLAND_DISPLAY").is_some();
    // On Wayland, XWayland's root image does not contain Wayland-native
    // windows, so the portal must be tried first. On X11, reading the root
    // image avoids a portal permission dialog whenever possible.
    let order = if wayland {
        [true, false]
    } else {
        [false, true]
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    std::thread::Builder::new()
        .name("cove-color-sampler".into())
        .spawn(move || {
            let mut unavailable = Vec::new();
            for use_portal in order {
                let attempt = if use_portal {
                    super::portal::pick_color(&flag)
                } else {
                    super::x11::pick_pixel(&flag)
                };
                match attempt {
                    Attempt::Done(sample) => {
                        sink.deliver(request, sample);
                        return;
                    }
                    Attempt::Unavailable(reason) => unavailable.push(reason),
                }
                if flag.load(Ordering::Relaxed) {
                    sink.deliver(request, ScreenColorSample::Cancelled);
                    return;
                }
            }
            sink.deliver(
                request,
                ScreenColorSample::Unsupported(format!(
                    "no screen-colour backend is available: {}",
                    unavailable.join("; ")
                )),
            );
        })
        .map_err(|error| {
            ScreenColorSample::Failed(format!("the colour sampler thread failed: {error}"))
        })?;
    Ok(Session::worker(cancel))
}
