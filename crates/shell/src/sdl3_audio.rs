//! SDL3 default playback device.
//!
//! The bridge opens one stream on the default playback device and converts
//! caller-supplied `f32` sample frames into whatever the hardware wants, so a
//! client never has to know the device format up front. The stream starts
//! paused: fill it with [`put`](Sdl3AudioDevice::put) first, then
//! [`resume`](Sdl3AudioDevice::resume). Every method is safe to call from any
//! thread; SDL locks the stream internally.

use crate::native;
use crate::ShellError;
use std::ffi::c_void;

/// The sample format the device consumes, as SDL reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioSampleFormat {
    U8,
    S16,
    S32,
    F32,
    /// A format this bridge does not name yet; the raw SDL value is kept.
    Unknown(u16),
}

impl AudioSampleFormat {
    const SDL_AUDIO_U8: u16 = 0x0008;
    const SDL_AUDIO_S16: u16 = 0x8010;
    const SDL_AUDIO_S32: u16 = 0x8020;
    const SDL_AUDIO_F32: u16 = 0x8120;

    pub fn from_raw(value: u16) -> Self {
        match value {
            Self::SDL_AUDIO_U8 => Self::U8,
            Self::SDL_AUDIO_S16 => Self::S16,
            Self::SDL_AUDIO_S32 => Self::S32,
            Self::SDL_AUDIO_F32 => Self::F32,
            other => Self::Unknown(other),
        }
    }

    pub fn raw(self) -> u16 {
        match self {
            Self::U8 => Self::SDL_AUDIO_U8,
            Self::S16 => Self::SDL_AUDIO_S16,
            Self::S32 => Self::SDL_AUDIO_S32,
            Self::F32 => Self::SDL_AUDIO_F32,
            Self::Unknown(value) => value,
        }
    }

    /// Bits per sample, or `None` for a format this bridge cannot size.
    pub fn bits(self) -> Option<u16> {
        match self {
            Self::U8 => Some(8),
            Self::S16 => Some(16),
            Self::S32 | Self::F32 => Some(32),
            Self::Unknown(_) => None,
        }
    }

    pub fn is_float(self) -> bool {
        self == Self::F32
    }
}

/// The format a device really consumes, which may differ from the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioDeviceFormat {
    pub frequency: u32,
    pub channels: u8,
    pub sample_format: AudioSampleFormat,
}

impl AudioDeviceFormat {
    /// Bytes in one interleaved sample frame.
    pub fn frame_bytes(self) -> Option<usize> {
        self.sample_format
            .bits()
            .map(|bits| (bits as usize / 8) * self.channels as usize)
    }
}

/// The input side of the stream: interleaved `f32` frames at `frequency`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioStreamSpec {
    pub frequency: u32,
    pub channels: u8,
}

impl AudioStreamSpec {
    pub fn new(frequency: u32, channels: u8) -> Self {
        Self {
            frequency,
            channels,
        }
    }

    pub fn stereo(frequency: u32) -> Self {
        Self::new(frequency, 2)
    }

    pub fn is_valid(self) -> bool {
        self.frequency > 0 && self.channels > 0
    }
}

/// An open stream on the default playback device. Closing happens on drop;
/// [`close`](Self::close) consumes the handle to make that explicit.
pub struct Sdl3AudioDevice {
    stream: *mut native::AudioStream,
}

// SAFETY: the wrapper owns no Rust state besides the SDL stream pointer, and
// every SDL audio-stream call locks the stream internally. It is deliberately
// not `Sync`, so concurrent access still needs a lock chosen by the caller.
unsafe impl Send for Sdl3AudioDevice {}

impl Sdl3AudioDevice {
    /// Opens a paused stream on the default playback device that accepts
    /// interleaved `f32` frames at `spec`.
    pub fn open_default(spec: AudioStreamSpec) -> Result<Self, ShellError> {
        if !spec.is_valid() {
            return Err(ShellError::InitializationFailed(
                "an audio stream needs a frequency and at least one channel".into(),
            ));
        }
        // SAFETY: the bridge owns the SDL stream; the parameters are checked
        // above. Initializing the audio subsystem is idempotent inside the
        // bridge and happens on the calling thread.
        let stream = unsafe {
            native::cove_shell_audio_open_default(spec.frequency as i32, spec.channels as i32)
        };
        if stream.is_null() {
            return Err(native::error());
        }
        Ok(Self { stream })
    }

    /// The format the device consumes, or `None` when SDL cannot report it.
    pub fn format(&self) -> Option<AudioDeviceFormat> {
        let mut format = 0;
        let mut channels = 0;
        let mut frequency = 0;
        // SAFETY: `self.stream` is an open stream; the out pointers are valid
        // for this call.
        let reported = unsafe {
            native::cove_shell_audio_format(
                self.stream,
                &mut format,
                &mut channels,
                &mut frequency,
            )
        };
        if !reported {
            return None;
        }
        Some(AudioDeviceFormat {
            frequency: frequency.max(0) as u32,
            channels: channels.clamp(0, u8::MAX as i32) as u8,
            sample_format: AudioSampleFormat::from_raw(format.clamp(0, u16::MAX as i32) as u16),
        })
    }

    /// Input bytes queued but not yet consumed by the device.
    pub fn queued_bytes(&self) -> usize {
        // SAFETY: `self.stream` is an open stream.
        unsafe { native::cove_shell_audio_queued(self.stream).max(0) as usize }
    }

    /// Converted bytes already available for the device to play.
    pub fn available_bytes(&self) -> usize {
        // SAFETY: `self.stream` is an open stream.
        unsafe { native::cove_shell_audio_available(self.stream).max(0) as usize }
    }

    /// Appends interleaved `f32` frames. The device converts them to its own
    /// format, so the caller can always hand over the requested spec.
    pub fn put(&mut self, samples: &[f32]) -> Result<(), ShellError> {
        if samples.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::size_of_val(samples);
        let length = i32::try_from(bytes).map_err(|_| {
            ShellError::InitializationFailed(format!("{bytes} bytes exceed one audio write"))
        })?;
        // SAFETY: `samples` is a live, correctly aligned `f32` slice and
        // `length` counts its bytes exactly.
        let accepted =
            unsafe { native::cove_shell_audio_put(self.stream, samples.as_ptr() as *const c_void, length) };
        if accepted {
            Ok(())
        } else {
            Err(native::error())
        }
    }

    /// Drops queued input without waiting for the device to play it.
    pub fn flush(&mut self) -> Result<(), ShellError> {
        // SAFETY: `self.stream` is an open stream.
        if unsafe { native::cove_shell_audio_flush(self.stream) } {
            Ok(())
        } else {
            Err(native::error())
        }
    }

    /// Stops the device from consuming the queue; queued data is retained.
    pub fn pause(&mut self) -> Result<(), ShellError> {
        // SAFETY: `self.stream` is an open stream.
        if unsafe { native::cove_shell_audio_pause(self.stream) } {
            Ok(())
        } else {
            Err(native::error())
        }
    }

    /// Starts (or restarts) playback from the queue.
    pub fn resume(&mut self) -> Result<(), ShellError> {
        // SAFETY: `self.stream` is an open stream.
        if unsafe { native::cove_shell_audio_resume(self.stream) } {
            Ok(())
        } else {
            Err(native::error())
        }
    }

    /// Closes the device now instead of at the end of the scope.
    pub fn close(self) {}
}

impl Drop for Sdl3AudioDevice {
    fn drop(&mut self) {
        if self.stream.is_null() {
            return;
        }
        // SAFETY: this handle is the only owner of the stream, and drop runs
        // once; the pointer is cleared so a second drop cannot reuse it.
        unsafe { native::cove_shell_audio_close(self.stream) };
        self.stream = std::ptr::null_mut();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdl_sample_formats_map_to_named_values() {
        assert_eq!(AudioSampleFormat::from_raw(0x0008), AudioSampleFormat::U8);
        assert_eq!(AudioSampleFormat::from_raw(0x8010), AudioSampleFormat::S16);
        assert_eq!(AudioSampleFormat::from_raw(0x8020), AudioSampleFormat::S32);
        assert_eq!(AudioSampleFormat::from_raw(0x8120), AudioSampleFormat::F32);
        assert_eq!(AudioSampleFormat::from_raw(0x1234), AudioSampleFormat::Unknown(0x1234));
        assert_eq!(AudioSampleFormat::F32.bits(), Some(32));
        assert!(AudioSampleFormat::F32.is_float());
        assert!(!AudioSampleFormat::S16.is_float());
        assert_eq!(AudioSampleFormat::Unknown(7).bits(), None);
    }

    #[test]
    fn a_device_format_sizes_one_stereo_frame() {
        let format = AudioDeviceFormat {
            frequency: 48_000,
            channels: 2,
            sample_format: AudioSampleFormat::F32,
        };
        assert_eq!(format.frame_bytes(), Some(8));
        let unknown = AudioDeviceFormat {
            sample_format: AudioSampleFormat::Unknown(3),
            ..format
        };
        assert_eq!(unknown.frame_bytes(), None);
    }

    #[test]
    fn invalid_specs_are_rejected_before_any_device_opens() {
        assert!(AudioStreamSpec::new(0, 2).is_valid().eq(&false));
        assert!(AudioStreamSpec::new(48_000, 0).is_valid().eq(&false));
        let error = match Sdl3AudioDevice::open_default(AudioStreamSpec::new(0, 2)) {
            Ok(_) => panic!("a zero frequency must be rejected before a device opens"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("frequency"));
    }

    #[test]
    fn the_default_device_reports_a_usable_format_when_one_exists() {
        let Ok(mut device) = Sdl3AudioDevice::open_default(AudioStreamSpec::stereo(48_000))
        else {
            // Headless CI machines have no playback device; the bridge reports
            // that as an initialization error rather than pretending to play.
            eprintln!("no default playback device on this host; skipping");
            return;
        };
        let format = device.format().expect("an open stream reports its format");
        assert!(format.frequency >= 8_000, "{format:?}");
        assert!(format.channels >= 1, "{format:?}");
        assert!(format.frame_bytes().is_some(), "{format:?}");
        device.resume().expect("the stream starts paused and resumes");
        device.put(&[0.0; 960]).expect("a silent block is accepted");
        assert!(device.queued_bytes() > 0 || device.available_bytes() > 0);
        device.pause().expect("pause");
        device.flush().expect("flush");
    }
}
