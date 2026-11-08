//! Backend-independent audio values: PCM buffers, waveform peaks, devices and metering.
use crate::kernel::{channel_stats, sample_stats};
use crate::{MediaError, MediaErrorKind, MediaResult, MediaTime};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Interleaved `f32` PCM layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioFormat {
    pub fn new(sample_rate: u32, channels: u16) -> MediaResult<Self> {
        if !(1..=768_000).contains(&sample_rate) || !(1..=64).contains(&channels) {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "audio format needs 1..=768000 Hz and 1..=64 channels",
            ));
        }
        Ok(Self {
            sample_rate,
            channels,
        })
    }

    /// Media time covered by `frames` sample frames.
    pub fn duration_of(&self, frames: usize) -> MediaTime {
        MediaTime(
            ((frames as u128 * 1_000_000_000) / u128::from(self.sample_rate.max(1)))
                .min(u128::from(u64::MAX)) as u64,
        )
    }
}

/// Decoded interleaved `f32` samples in `-1.0..=1.0`.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioBuffer {
    pub pts: Option<MediaTime>,
    pub format: AudioFormat,
    pub samples: Vec<f32>,
}

impl AudioBuffer {
    pub fn new(format: AudioFormat) -> Self {
        Self {
            pts: None,
            format,
            samples: Vec::new(),
        }
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.format.channels.max(1))
    }

    pub fn duration(&self) -> MediaTime {
        self.format.duration_of(self.frames())
    }

    /// Appends another buffer of the same format; the first timestamp is kept.
    pub fn append(&mut self, other: &AudioBuffer) -> MediaResult<()> {
        if other.format != self.format {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "appended audio uses a different format",
            ));
        }
        if self.samples.is_empty() {
            self.pts = other.pts;
        }
        self.samples.extend_from_slice(&other.samples);
        Ok(())
    }

    /// Envelope of the buffer split into `buckets` equal spans, all channels combined.
    pub fn peaks(&self, buckets: usize) -> Vec<AudioPeak> {
        audio_peaks(&self.samples, usize::from(self.format.channels), buckets)
    }
}

/// One waveform column.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioPeak {
    pub min: f32,
    pub max: f32,
    pub rms: f32,
}

/// Envelope of interleaved samples split into `buckets` spans of whole frames.
pub fn audio_peaks(interleaved: &[f32], channels: usize, buckets: usize) -> Vec<AudioPeak> {
    let channels = channels.max(1);
    let frames = interleaved.len() / channels;
    if buckets == 0 || frames == 0 {
        return Vec::new();
    }
    (0..buckets)
        .map(|bucket| {
            let start = bucket * frames / buckets;
            let end = ((bucket + 1) * frames / buckets).max(start + 1).min(frames);
            let stats = sample_stats(&interleaved[start * channels..end * channels]);
            AudioPeak {
                min: stats.min.clamp(-1.0, 1.0),
                max: stats.max.clamp(-1.0, 1.0),
                rms: stats.rms().min(1.0),
            }
        })
        .collect()
}

/// An audio output endpoint reported by the platform host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    /// Stable identifier accepted by `AudioOutput::Device`.
    pub id: String,
    pub name: String,
    pub is_default: bool,
    /// Largest channel count among the device's floating-point configurations.
    pub max_channels: u16,
    /// Inclusive sample-rate range among the device's floating-point configurations.
    pub sample_rates: (u32, u32),
}

/// Per-channel levels of the most recent block delivered to an output.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioLevels {
    pub peak: Vec<f32>,
    pub rms: Vec<f32>,
    /// Advances with every recorded block.
    pub revision: u64,
}

impl AudioLevels {
    /// The loudest channel's peak.
    pub fn max_peak(&self) -> f32 {
        self.peak.iter().copied().fold(0.0, f32::max)
    }
}

/// Levels and a short history of the samples an output actually played.
///
/// The audio callback records through `try_lock` and never waits for readers;
/// a contended block is skipped rather than delaying the device.
#[derive(Clone)]
pub struct AudioMeter(Arc<MeterInner>);

struct MeterInner {
    state: Mutex<MeterState>,
    revision: AtomicU64,
}

struct MeterState {
    format: Option<AudioFormat>,
    peak: Vec<f32>,
    rms: Vec<f32>,
    /// Interleaved ring of the most recent `capacity` frames.
    ring: Vec<f32>,
    head: usize,
    filled: usize,
    capacity: usize,
}

impl Default for AudioMeter {
    fn default() -> Self {
        Self::new(4096)
    }
}

impl AudioMeter {
    /// `history_frames` bounds the sample history kept for scopes and spectra.
    pub fn new(history_frames: usize) -> Self {
        Self(Arc::new(MeterInner {
            state: Mutex::new(MeterState {
                format: None,
                peak: Vec::new(),
                rms: Vec::new(),
                ring: Vec::new(),
                head: 0,
                filled: 0,
                capacity: history_frames.clamp(1, 1 << 20),
            }),
            revision: AtomicU64::new(0),
        }))
    }

    /// Records one delivered block. Safe to call from a realtime audio callback.
    pub fn record(&self, format: AudioFormat, interleaved: &[f32]) {
        let Ok(mut state) = self.0.state.try_lock() else {
            return;
        };
        let channels = usize::from(format.channels.max(1));
        if state.format != Some(format) {
            state.format = Some(format);
            let length = state.capacity * channels;
            state.ring.clear();
            state.ring.resize(length, 0.0);
            state.head = 0;
            state.filled = 0;
        }
        let stats = channel_stats(interleaved, channels);
        state.peak.clear();
        state.rms.clear();
        for channel in &stats {
            state.peak.push(channel.peak().min(1.0));
            state.rms.push(channel.rms().min(1.0));
        }
        let capacity = state.capacity;
        let frames = interleaved.len() / channels;
        let skip = frames.saturating_sub(capacity);
        for frame in interleaved.chunks_exact(channels).skip(skip) {
            let at = state.head * channels;
            state.ring[at..at + channels].copy_from_slice(frame);
            state.head = (state.head + 1) % capacity;
        }
        state.filled = (state.filled + frames - skip).min(capacity);
        drop(state);
        self.0.revision.fetch_add(1, Ordering::Release);
    }

    pub fn revision(&self) -> u64 {
        self.0.revision.load(Ordering::Acquire)
    }

    pub fn levels(&self) -> AudioLevels {
        let state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        AudioLevels {
            peak: state.peak.clone(),
            rms: state.rms.clone(),
            revision: self.revision(),
        }
    }

    /// Copies the recorded history, oldest first, and returns its format.
    pub fn history(&self, output: &mut Vec<f32>) -> Option<AudioFormat> {
        let state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        output.clear();
        let format = state.format?;
        let channels = usize::from(format.channels.max(1));
        let start = (state.head + state.capacity - state.filled) % state.capacity;
        for index in 0..state.filled {
            let at = ((start + index) % state.capacity) * channels;
            output.extend_from_slice(&state.ring[at..at + channels]);
        }
        Some(format)
    }

    pub fn reset(&self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.peak.iter_mut().for_each(|value| *value = 0.0);
        state.rms.iter_mut().for_each(|value| *value = 0.0);
        state.filled = 0;
        state.head = 0;
        drop(state);
        self.0.revision.fetch_add(1, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_cover_every_frame_once() {
        let format = AudioFormat::new(8, 1).expect("format");
        let buffer = AudioBuffer {
            pts: None,
            format,
            samples: vec![0.1, -0.2, 0.9, -0.9, 0.0, 0.5, -0.5, 0.25],
        };
        let peaks = buffer.peaks(4);
        assert_eq!(peaks.len(), 4);
        assert_eq!(peaks[1].max, 0.9);
        assert_eq!(peaks[1].min, -0.9);
        assert_eq!(buffer.duration(), MediaTime(1_000_000_000));
    }

    #[test]
    fn meter_keeps_bounded_ordered_history() {
        let meter = AudioMeter::new(3);
        let format = AudioFormat::new(48_000, 2).expect("format");
        meter.record(format, &[0.1, 0.2, 0.3, 0.4]);
        meter.record(format, &[0.5, 0.6, 0.7, 0.8]);
        let mut history = Vec::new();
        assert_eq!(meter.history(&mut history), Some(format));
        assert_eq!(history, vec![0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);
        let levels = meter.levels();
        assert_eq!(levels.peak, vec![0.7, 0.8]);
        assert_eq!(levels.revision, 2);
        meter.reset();
        assert_eq!(meter.levels().max_peak(), 0.0);
    }
}
