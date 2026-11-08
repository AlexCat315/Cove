//! cpal output device, sample queue and audio master clock.
use crate::{
    AudioDeviceInfo, AudioFormat, AudioMeter, AudioOutput, MediaError, MediaErrorKind, MediaResult,
    MediaTime,
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};

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
    meter: AudioMeter,
    format: AudioFormat,
}

impl Bus {
    fn new(format: AudioFormat, meter: AudioMeter) -> Self {
        Self {
            queue: Mutex::new(Queue {
                channels: usize::from(format.channels),
                ..Queue::default()
            }),
            playing: AtomicBool::new(false),
            volume: AtomicU64::new(1.0f64.to_bits()),
            rate: AtomicU64::new(1.0f64.to_bits()),
            clock_ns: AtomicU64::new(u64::MAX),
            meter,
            format,
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
        // The device no longer plays anything at the old time; until new samples are
        // delivered the session falls back to its anchor (the seek target or pause point).
        self.clock_ns.store(u64::MAX, Ordering::Release);
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

    /// Delivers samples to the device and meters what was played.
    fn deliver(&self, output: &mut [f32]) {
        self.fill(output);
        self.meter.record(self.format, output);
    }

    fn fill(&self, output: &mut [f32]) {
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

/// The device an `AudioOutput` names; a vanished or unknown id falls back to the default.
fn output_device(output: &AudioOutput) -> MediaResult<cpal::Device> {
    let host = cpal::default_host();
    if let AudioOutput::Device(id) = output {
        let found = id
            .parse::<cpal::DeviceId>()
            .ok()
            .and_then(|id| host.device_by_id(&id));
        if let Some(device) = found {
            return Ok(device);
        }
        warn(&format!(
            "audio device {id} is unavailable; using the default output"
        ));
    }
    host.default_output_device().ok_or_else(|| {
        MediaError::new(
            MediaErrorKind::Device,
            "no audio output device is available",
        )
    })
}

/// A floating-point stream configuration, matching `preferred` when the device supports it.
fn output_config(
    device: &cpal::Device,
    preferred: Option<AudioFormat>,
) -> MediaResult<cpal::StreamConfig> {
    let ranges: Vec<_> = device
        .supported_output_configs()
        .map_err(device_error)?
        .filter(|range| range.sample_format() == cpal::SampleFormat::F32)
        .collect();
    if let Some(format) = preferred
        && let Some(range) = ranges.iter().find(|range| {
            range.channels() == format.channels
                && (range.min_sample_rate()..=range.max_sample_rate()).contains(&format.sample_rate)
        })
    {
        return Ok(range.with_sample_rate(format.sample_rate).config());
    }
    ranges
        .into_iter()
        .max_by(|a, b| a.cmp_default_heuristics(b))
        .and_then(cpal::SupportedStreamConfigRange::try_with_standard_sample_rate)
        .map(|config| config.config())
        .ok_or_else(|| {
            MediaError::new(
                MediaErrorKind::Device,
                "no floating-point output configuration",
            )
        })
}

/// Output endpoints of the platform's default audio host.
pub(crate) fn output_devices() -> MediaResult<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();
    let default = host
        .default_output_device()
        .and_then(|device| device.id().ok());
    let mut devices = Vec::new();
    for device in host.output_devices().map_err(device_error)? {
        let Ok(id) = device.id() else { continue };
        let name = device
            .description()
            .map(|description| description.name().to_owned())
            .unwrap_or_else(|_| id.to_string());
        let mut max_channels = 0u16;
        let mut rates: Option<(u32, u32)> = None;
        if let Ok(ranges) = device.supported_output_configs() {
            for range in ranges.filter(|range| range.sample_format() == cpal::SampleFormat::F32) {
                max_channels = max_channels.max(range.channels());
                let (low, high) = (range.min_sample_rate(), range.max_sample_rate());
                rates = Some(rates.map_or((low, high), |(a, b)| (a.min(low), b.max(high))));
            }
        }
        devices.push(AudioDeviceInfo {
            is_default: default.as_ref() == Some(&id),
            id: id.to_string(),
            name,
            max_channels,
            sample_rates: rates.unwrap_or((0, 0)),
        });
    }
    Ok(devices)
}

fn warn(message: &str) {
    cove_logging::LogSystem::logger("com.cove.media").log(&cove_logging::LogEvent::new(
        cove_logging::Level::Warning,
        message.to_owned(),
    ));
}

impl Sink {
    /// Opens the selected output device with a floating-point stream.
    pub(crate) fn open(output: &AudioOutput, meter: AudioMeter) -> MediaResult<Self> {
        let device = output_device(output)?;
        let config = output_config(&device, None)?;
        let channels = usize::from(config.channels);
        let sample_rate = usize::try_from(config.sample_rate).unwrap_or(48000);
        let format = AudioFormat::new(config.sample_rate, config.channels)?;
        let bus = Arc::new(Bus::new(format, meter));
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
        let error_callback = move |error: cpal::Error| warn(&error.to_string());
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
    pub(crate) fn open(
        output: &AudioOutput,
        meter: AudioMeter,
        sample_rate: u32,
        channels: usize,
    ) -> Self {
        let sink = match output {
            AudioOutput::Silent => None,
            AudioOutput::DefaultDevice | AudioOutput::Device(_) => {
                match Sink::open(output, meter) {
                    Ok(sink) => Some(sink),
                    Err(error) => {
                        warn(&error.to_string());
                        None
                    }
                }
            }
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

/// Renders interleaved `f32` samples for an [`AudioStream`]; runs on the device thread.
pub type AudioRender = Box<dyn FnMut(&mut [f32], AudioFormat) + Send + 'static>;

/// Where and how an [`AudioStream`] plays.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioStreamOptions {
    pub output: AudioOutput,
    /// Requested format; the device default is used when it is unsupported or `None`.
    pub format: Option<AudioFormat>,
}

/// Application-rendered PCM on an output device.
///
/// The device stream lives on its own thread, so the handle is `Send + Sync` on every
/// platform. It starts paused; paused streams deliver silence. Dropping the last
/// handle closes the device.
pub struct AudioStream {
    format: AudioFormat,
    playing: Arc<AtomicBool>,
    meter: AudioMeter,
    /// Dropping the sender ends the device thread.
    _stop: mpsc::Sender<()>,
}

impl AudioStream {
    /// Opens the device and waits for it to accept the stream. Not for the UI thread.
    pub fn open(options: AudioStreamOptions, render: AudioRender) -> MediaResult<Self> {
        if options.output == AudioOutput::Silent {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "an audio stream needs an output device",
            ));
        }
        let playing = Arc::new(AtomicBool::new(false));
        let meter = AudioMeter::default();
        let (stop, stopped) = mpsc::channel::<()>();
        let (ready, opened) = mpsc::channel::<MediaResult<AudioFormat>>();
        let thread_playing = playing.clone();
        let thread_meter = meter.clone();
        std::thread::Builder::new()
            .name("cove-audio-stream".into())
            .spawn(move || {
                let built = build_stream(&options, render, thread_playing, thread_meter);
                match built {
                    Ok((stream, format)) => {
                        let _ = ready.send(Ok(format));
                        let _ = stopped.recv();
                        drop(stream);
                    }
                    Err(error) => {
                        let _ = ready.send(Err(error));
                    }
                }
            })
            .map_err(|error| MediaError::new(MediaErrorKind::Initialization, error.to_string()))?;
        let format = opened.recv().map_err(|_| {
            MediaError::new(MediaErrorKind::Device, "audio stream thread ended early")
        })??;
        Ok(Self {
            format,
            playing,
            meter,
            _stop: stop,
        })
    }

    /// The format the render callback receives.
    pub fn format(&self) -> AudioFormat {
        self.format
    }

    pub fn play(&self) {
        self.playing.store(true, Ordering::Release);
    }

    pub fn pause(&self) {
        self.playing.store(false, Ordering::Release);
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }

    /// Levels and history of the rendered output.
    pub fn meter(&self) -> AudioMeter {
        self.meter.clone()
    }
}

fn build_stream(
    options: &AudioStreamOptions,
    mut render: AudioRender,
    playing: Arc<AtomicBool>,
    meter: AudioMeter,
) -> MediaResult<(cpal::Stream, AudioFormat)> {
    let device = output_device(&options.output)?;
    let config = output_config(&device, options.format)?;
    let format = AudioFormat::new(config.sample_rate, config.channels)?;
    let callback = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        if playing.load(Ordering::Acquire) {
            render(data, format);
            for sample in data.iter_mut() {
                *sample = sample.clamp(-1.0, 1.0);
            }
        } else {
            data.fill(0.0);
        }
        meter.record(format, data);
    };
    let stream = device
        .build_output_stream(config, callback, |error| warn(&error.to_string()), None)
        .map_err(device_error)?;
    stream.play().map_err(device_error)?;
    Ok((stream, format))
}

fn device_error(error: cpal::Error) -> MediaError {
    MediaError::new(MediaErrorKind::Device, error.to_string())
}
