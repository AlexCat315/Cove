//! cpal output device, sample queue and audio master clock.
use crate::{AudioOutput, MediaError, MediaErrorKind, MediaResult, MediaTime};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// At most two seconds of decoded audio may wait for the device.
const QUEUE_SECONDS: f64 = 2.0;

#[derive(Default)]
struct Queue {
    /// Device-format interleaved `f32` samples awaiting playback.
    samples: VecDeque<f32>,
    /// Media time of the oldest queued sample. Contiguous chunks are assumed.
    front_pts: u64,
    /// Nanoseconds of media time per output frame.
    ns_per_frame: f64,
    /// Fractional source frames already delivered, relative to `samples[0]`.
    position: f64,
    channels: usize,
}

/// The state shared between the pump thread, the audio callback and the clock.
pub(crate) struct Bus {
    queue: Mutex<Queue>,
    playing: AtomicBool,
    volume: AtomicU64,
    rate: AtomicU64,
    clock_ns: AtomicU64,
}

impl Bus {
    fn new(channels: usize) -> Self {
        Self {
            queue: Mutex::new(Queue {
                channels,
                ..Queue::default()
            }),
            playing: AtomicBool::new(false),
            volume: AtomicU64::new(1.0f64.to_bits()),
            rate: AtomicU64::new(1.0f64.to_bits()),
            clock_ns: AtomicU64::new(u64::MAX),
        }
    }
    fn volume(&self) -> f64 {
        f64::from_bits(self.volume.load(Ordering::Acquire)).clamp(0.0, 1.0)
    }
    fn rate(&self) -> f64 {
        f64::from_bits(self.rate.load(Ordering::Acquire)).clamp(0.1, 16.0)
    }
    /// Nanoseconds of media time delivered to the device, or [`u64::MAX`] before the first sample.
    pub(crate) fn clock(&self) -> Option<MediaTime> {
        let clock = self.clock_ns.load(Ordering::Acquire);
        (clock != u64::MAX).then_some(MediaTime(clock))
    }
    pub(crate) fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Release);
    }
    pub(crate) fn set_volume(&self, volume: f64) {
        self.volume
            .store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Release);
    }
    pub(crate) fn set_rate(&self, rate: f64) {
        self.rate
            .store(rate.clamp(0.1, 16.0).to_bits(), Ordering::Release);
    }
    pub(crate) fn has_queued(&self) -> bool {
        !self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .samples
            .is_empty()
    }
    pub(crate) fn flush(&self) {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.samples.clear();
        queue.position = 0.0;
        queue.front_pts = 0;
    }
    /// Appends one decoded audio chunk. `pts` is its media time; `None` keeps continuity.
    pub(crate) fn push(&self, pts: Option<MediaTime>, samples: &[f32]) {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if queue.samples.is_empty() {
            queue.position = 0.0;
            if let Some(pts) = pts {
                queue.front_pts = pts.0;
            }
        }
        queue.samples.extend(samples.iter().copied());
    }

    /// Delivers samples to the device. Returns after filling `output`.
    fn deliver(&self, output: &mut [f32]) {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let channels = queue.channels.max(1);
        let volume = self.volume();
        let rate = self.rate();
        if !self.playing.load(Ordering::Acquire) || queue.ns_per_frame <= 0.0 {
            output.fill(0.0);
            return;
        }
        let frames = output.len() / channels;
        let mut delivered = 0usize;
        let mut position = queue.position;
        let available = queue.samples.len() / channels;
        while delivered < frames {
            let index = position.floor();
            if index < 0.0 || (index as usize) >= available {
                break;
            }
            let start = (index as usize) * channels;
            for (channel, slot) in output
                .iter_mut()
                .skip(delivered * channels)
                .take(channels)
                .enumerate()
            {
                *slot = queue.samples[start + channel].clamp(-1.0, 1.0) * volume as f32;
            }
            position += rate;
            delivered += 1;
        }
        output[delivered * channels..].fill(0.0);
        // Retire whole source frames so the clock stays aligned with the queue head.
        let retired = position.floor();
        if retired > 0.0 {
            let retire = (retired as usize).min(available);
            for _ in 0..retire {
                for _ in 0..channels {
                    queue.samples.pop_front();
                }
            }
            queue.front_pts = (queue.front_pts as f64 + retire as f64 * queue.ns_per_frame) as u64;
            position -= retire as f64;
        }
        queue.position = position;
        self.clock_ns.store(queue.front_pts, Ordering::Release);
    }
}

/// An opened output device. Dropping it stops playback.
pub(crate) struct Sink {
    /// Kept alive for the lifetime of the sink; dropping it stops the device.
    _stream: cpal::Stream,
    bus: Arc<Bus>,
    sample_rate: u32,
    channels: usize,
    /// `channels * sample_rate * QUEUE_SECONDS`, the point where the pump must wait.
    capacity: usize,
}

impl Sink {
    /// Opens the platform default output device with a floating-point stream.
    pub(crate) fn open() -> MediaResult<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| {
            MediaError::new(
                MediaErrorKind::Device,
                "no audio output device is available",
            )
        })?;
        let config = device
            .supported_output_configs()
            .map_err(device_error)?
            .filter(|range| range.sample_format() == cpal::SampleFormat::F32)
            .max_by(|a, b| a.cmp_default_heuristics(b))
            .and_then(cpal::SupportedStreamConfigRange::try_with_standard_sample_rate)
            .ok_or_else(|| {
                MediaError::new(
                    MediaErrorKind::Device,
                    "no floating-point output configuration",
                )
            })?
            .config();
        let channels = usize::from(config.channels);
        let sample_rate = usize::try_from(config.sample_rate).unwrap_or(48000);
        let bus = Arc::new(Bus::new(channels));
        {
            let mut queue = bus
                .queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            queue.ns_per_frame = 1_000_000_000.0 / sample_rate as f64;
        }
        let callback = {
            let bus = bus.clone();
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| bus.deliver(data)
        };
        let error_callback = move |error: cpal::Error| {
            cove_logging::LogSystem::logger("com.cove.media").log(&cove_logging::LogEvent::new(
                cove_logging::Level::Warning,
                error.to_string(),
            ));
        };
        let stream = device
            .build_output_stream(config, callback, error_callback, None)
            .map_err(device_error)?;
        stream.play().map_err(device_error)?;
        Ok(Self {
            _stream: stream,
            bus,
            sample_rate: config.sample_rate,
            channels,
            capacity: sample_rate.saturating_mul(QUEUE_SECONDS as usize),
        })
    }

    pub(crate) fn bus(&self) -> &Arc<Bus> {
        &self.bus
    }
    /// The format decoded audio must be resampled into before it is queued.
    pub(crate) fn format(&self) -> (u32, usize) {
        (self.sample_rate, self.channels)
    }
    /// Whether the queue is full enough that the pump should wait before decoding more.
    pub(crate) fn is_full(&self) -> bool {
        let queue = self
            .bus
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.samples.len() / queue.channels.max(1) >= self.capacity
    }
    pub(crate) fn set_playing(&self, playing: bool) {
        self.bus.set_playing(playing);
        if !playing {
            self.bus.flush();
        }
    }
    pub(crate) fn flush(&self) {
        self.bus.flush();
    }
    /// Silences the device so a dropped sink cannot keep feeding on stale media time.
    pub(crate) fn stop(&self) {
        self.bus.set_playing(false);
        self.bus.flush();
    }
}

/// Audio delivery for one session: the platform device, or a silent sink.
///
/// A silent sink carries no clock and queues no samples; the session then drives
/// playback from its monotonic wall clock, which keeps headless and device-less
/// environments on the same control contract.
pub(crate) struct Audio {
    sink: Option<Sink>,
    format: (u32, usize),
}

impl Audio {
    pub(crate) fn open(output: AudioOutput, sample_rate: u32, channels: usize) -> Self {
        let sink = match output {
            AudioOutput::Silent => None,
            AudioOutput::DefaultDevice => match Sink::open() {
                Ok(sink) => Some(sink),
                Err(error) => {
                    cove_logging::LogSystem::logger("com.cove.media").log(
                        &cove_logging::LogEvent::new(
                            cove_logging::Level::Warning,
                            error.to_string(),
                        ),
                    );
                    None
                }
            },
        };
        let format = sink.as_ref().map_or((sample_rate, channels), Sink::format);
        Self { sink, format }
    }
    pub(crate) fn format(&self) -> (u32, usize) {
        self.format
    }
    pub(crate) fn is_full(&self) -> bool {
        self.sink.as_ref().is_some_and(Sink::is_full)
    }
    pub(crate) fn has_queued(&self) -> bool {
        self.sink
            .as_ref()
            .is_some_and(|sink| sink.bus().has_queued())
    }
    pub(crate) fn clock(&self) -> Option<MediaTime> {
        self.sink.as_ref().and_then(|sink| sink.bus().clock())
    }
    pub(crate) fn push(&self, pts: Option<MediaTime>, samples: &[f32], _span: MediaTime) {
        if let Some(sink) = &self.sink {
            sink.bus().push(pts, samples);
        }
    }
    pub(crate) fn set_playing(&self, playing: bool) {
        if let Some(sink) = &self.sink {
            sink.set_playing(playing);
        }
    }
    pub(crate) fn set_volume(&self, volume: f64) {
        if let Some(sink) = &self.sink {
            sink.bus().set_volume(volume);
        }
    }
    pub(crate) fn set_rate(&self, rate: f64) {
        if let Some(sink) = &self.sink {
            sink.bus().set_rate(rate);
        }
    }
    pub(crate) fn flush(&self) {
        if let Some(sink) = &self.sink {
            sink.flush();
        }
    }
    pub(crate) fn stop(&self) {
        if let Some(sink) = &self.sink {
            sink.stop();
        }
    }
}

fn device_error(error: cpal::Error) -> MediaError {
    MediaError::new(MediaErrorKind::Device, error.to_string())
}
