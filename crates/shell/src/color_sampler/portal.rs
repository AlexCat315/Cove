//! Wayland backend: `org.freedesktop.portal.Screenshot.PickColor`.
//!
//! Wayland deliberately offers no screen-read protocol. The desktop portal is
//! the compositor-sanctioned way to obtain a screen pixel: it presents the
//! system picker and returns an sRGB `(ddd)` colour. A missing portal, an
//! unknown method or a closed session bus is `Unavailable`, so the X11
//! screenshot backend can be tried next.

use super::Attempt;
use cove_platform::ScreenColorSample;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn pick_color(cancel: &AtomicBool) -> Attempt {
    // SAFETY: getuid has no preconditions and is async-signal-safe.
    let uid = unsafe { libc::getuid() };
    match run(&format!("{uid:x}"), cancel) {
        Ok(sample) => Attempt::Done(sample),
        Err(reason) if cancel.load(Ordering::Relaxed) => {
            let _ = reason;
            Attempt::Done(ScreenColorSample::Cancelled)
        }
        Err(reason) => Attempt::Unavailable(format!("XDG PickColor portal: {reason}")),
    }
}

fn run(uid_hex: &str, cancel: &AtomicBool) -> Result<ScreenColorSample, String> {
    let mut client = super::dbus::PortalClient::connect(uid_hex)?;
    client.add_match(cancel)?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let token = format!("cove{}_{}", std::process::id(), nonce);
    let handle = client.pick_color(&token, cancel)?;
    client.wait_response(&handle, cancel)
}
