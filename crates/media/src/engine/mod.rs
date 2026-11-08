//! The FFmpeg playback engine: demux, decode, colour convert, mix and clock.
//!
//! Every raw `AV*` handle is created and destroyed on the session's pump thread,
//! so none of them is ever reachable from two threads at once. The worker thread
//! in [`crate::native`] only routes commands and enforces operation deadlines.

mod audio;
mod decoder;
mod io;
mod pump;
mod session;
mod video;

pub(crate) use audio::output_devices;
pub use audio::{AudioRender, AudioStream, AudioStreamOptions};
pub use decoder::{DecodedFrame, MediaDecoder, probe};
pub(crate) use pump::{Pump, Task};
pub use video::FrameStorage;

use crate::{MediaError, MediaErrorKind, MediaResult, MediaTime};
use ffmpeg_sys_next as ffi;
use std::ffi::{CStr, c_char, c_int};
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Cooperative abort for a session's blocking backend calls.
///
/// FFmpeg checks it between I/O reads, so a stuck `av_read_frame` returns as soon as the
/// context worker gives up on the operation. It is only ever read through atomics because
/// the callback receives a borrowed pointer while the pump owns the session.
pub(crate) struct Abort {
    stopping: AtomicBool,
    cancelled: AtomicBool,
}

impl Abort {
    pub(crate) fn new() -> Self {
        Self {
            stopping: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
        }
    }

    /// Clears a previous abort when a new source or reposition starts.
    pub(crate) fn reset(&self) {
        self.stopping.store(false, Ordering::Release);
        self.cancelled.store(false, Ordering::Release);
    }

    /// Aborts the operation in flight; the session itself stays usable.
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    /// Aborts everything; the pump thread ends once the blocking call returns.
    pub(crate) fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        self.cancel();
    }

    pub(crate) fn aborted(&self) -> bool {
        self.stopping.load(Ordering::Acquire) || self.cancelled.load(Ordering::Acquire)
    }
}

/// Pump and worker wake-up granularity.
pub(crate) const TICK: Duration = Duration::from_millis(5);
/// FFmpeg timestamps are `AV_TIME_BASE` (microseconds) fractions of a second.
const MICROSECONDS: i128 = 1_000_000;
const NANOSECONDS: i128 = 1_000_000_000;
/// Bytes read per custom-IO call; FFmpeg may grow its own copy of this buffer.
pub(crate) const IO_BUFFER: c_int = 64 * 1024;

/// Initialises FFmpeg once per process: network protocols plus silent logging,
/// because every backend failure is reported through [`MediaError`]. Unlike a
/// plugin-based backend there is no platform environment to install, on any target.
pub(crate) fn initialize() -> MediaResult<()> {
    static INITIALIZED: OnceLock<MediaResult<()>> = OnceLock::new();
    INITIALIZED
        .get_or_init(|| {
            let status = unsafe {
                // SAFETY: FFmpeg's network init is idempotent and takes no arguments.
                ffi::avformat_network_init()
            };
            if status < 0 {
                return Err(error(MediaErrorKind::Initialization, status));
            }
            unsafe {
                // SAFETY: setting the global log level is thread-safe by design.
                ffi::av_log_set_level(ffi::AV_LOG_QUIET)
            };
            Ok(())
        })
        .clone()
}

/// The FFmpeg version compiled into this build.
pub(crate) fn backend_version() -> String {
    initialize().ok();
    let version = unsafe {
        // SAFETY: `av_version_info` returns a static NUL-terminated string.
        CStr::from_ptr(ffi::av_version_info())
    };
    version.to_string_lossy().into_owned()
}

/// Whether FFmpeg can decode with a codec of this name (`h264`, `opus`, …).
pub(crate) fn has_decoder(name: &str) -> bool {
    if name.is_empty() || name.contains('\0') || initialize().is_err() {
        return false;
    }
    let Ok(c_name) = std::ffi::CString::new(name) else {
        return false;
    };
    !unsafe {
        // SAFETY: the name is NUL-free and the returned pointer is a static codec table entry.
        ffi::avcodec_find_decoder_by_name(c_name.as_ptr())
    }
    .is_null()
}

/// Turns a FFmpeg error status into a [`MediaError`] carrying the backend message.
pub(crate) fn error(kind: MediaErrorKind, status: c_int) -> MediaError {
    MediaError::new(kind, format!("{kind:?}: {}", message(status)))
}

pub(crate) fn message(status: c_int) -> String {
    let mut buffer = [0 as c_char; 256];
    let ok = unsafe {
        // SAFETY: the buffer is live for the call and its length is passed alongside it.
        ffi::av_strerror(status, buffer.as_mut_ptr(), buffer.len())
    };
    if ok >= 0 {
        let text = unsafe {
            // SAFETY: `av_strerror` NUL-terminates the buffer it was given.
            CStr::from_ptr(buffer.as_ptr())
        };
        return text.to_string_lossy().into_owned();
    }
    format!("FFmpeg error {status}")
}

/// Classifies a demuxer or protocol failure by its underlying errno.
pub(crate) fn source_error(status: c_int) -> MediaError {
    let raw = -status;
    let kind = if raw == ffi::AVERROR_INVALIDDATA || raw == ffi::AVERROR(libc::EINVAL) {
        MediaErrorKind::Decode
    } else if raw == ffi::AVERROR(libc::ENOMEM) {
        MediaErrorKind::Initialization
    } else {
        MediaErrorKind::Source
    };
    error(kind, status)
}

/// A stream time base into integer nanoseconds. Negative and unknown stamps collapse to zero.
pub(crate) fn to_ns(timestamp: i64, time_base: ffi::AVRational) -> Option<MediaTime> {
    if timestamp == ffi::AV_NOPTS_VALUE || time_base.den <= 0 || time_base.num <= 0 {
        return None;
    }
    let den = i128::from(time_base.den);
    let nanos =
        (i128::from(timestamp) * i128::from(time_base.num) * NANOSECONDS * 2 + den) / (den * 2);
    Some(MediaTime(nanos.clamp(0, u64::MAX as i128) as u64))
}

/// Nanoseconds into a stream (or [`ffi::AV_TIME_BASE`]) timestamp.
pub(crate) fn from_ns(position: MediaTime, time_base: ffi::AVRational) -> i64 {
    if time_base.den <= 0 || time_base.num <= 0 {
        return 0;
    }
    let scale = i128::from(time_base.num) * NANOSECONDS;
    let ticks = (position.0 as i128 * i128::from(time_base.den) * 2 + scale) / (scale * 2);
    ticks.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

/// Microseconds into nanoseconds. Unknown or negative stamps collapse to zero.
pub(crate) fn from_micros(timestamp: i64) -> MediaTime {
    MediaTime(
        timestamp
            .max(0)
            .unsigned_abs()
            .saturating_mul((NANOSECONDS / MICROSECONDS) as u64),
    )
}

/// Gives a frame whose channel layout only carries a count the native default layout.
///
/// The resampler stores the default layout for such input, so leaving it unspecified
/// makes every following frame look like a format change.
///
/// # Safety
/// `frame` must be a live decoded audio frame owned by the caller.
pub(crate) unsafe fn normalize_layout(frame: *mut ffi::AVFrame) {
    // SAFETY: guaranteed by the caller; the layout is reset before it is rebuilt.
    unsafe {
        let layout = &mut (*frame).ch_layout;
        if layout.order == ffi::AVChannelOrder::AV_CHANNEL_ORDER_UNSPEC {
            let channels = layout.nb_channels.max(1);
            ffi::av_channel_layout_uninit(layout);
            ffi::av_channel_layout_default(layout, channels);
        }
    }
}

/// A UTF-8 path as a NUL-terminated C string for the FFmpeg file protocol.
pub(crate) fn c_path(path: &Path) -> MediaResult<std::ffi::CString> {
    let text = path.to_str().ok_or_else(|| {
        MediaError::new(
            MediaErrorKind::InvalidInput,
            "media path is not valid UTF-8",
        )
    })?;
    std::ffi::CString::new(text).map_err(|_| {
        MediaError::new(
            MediaErrorKind::InvalidInput,
            "media path contains a NUL byte",
        )
    })
}

pub(crate) fn c_str(text: &str) -> MediaResult<std::ffi::CString> {
    std::ffi::CString::new(text).map_err(|_| {
        MediaError::new(
            MediaErrorKind::InvalidInput,
            "media URI contains a NUL byte",
        )
    })
}

/// A negative FFmpeg status built from a platform errno.
pub(crate) fn averror(errno: c_int) -> c_int {
    ffi::AVERROR(errno)
}

#[cfg(test)]
mod tests {
    #[test]
    fn microseconds_become_nanoseconds() {
        assert_eq!(super::from_micros(1_500_000).0, 1_500_000_000);
        assert_eq!(super::from_micros(-4).0, 0);
    }
}
