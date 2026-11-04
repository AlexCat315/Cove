//! Advanced entry point: the official bindings and a retained video endpoint.
pub use gstreamer::*;
pub use gstreamer_app as app;
pub use gstreamer_audio as audio;
pub use gstreamer_video as video;
#[cfg(any(target_os = "android", test))]
pub mod android;

use crate::{
    MediaError, MediaErrorKind, MediaResult, MediaTime, PixelFormat, PlaneLayout,
    VideoDynamicRange, VideoFormat, VideoFrame, VideoOutput, VideoStorage,
};
use gstreamer::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

pub fn initialize() -> MediaResult<()> {
    #[cfg(target_os = "android")]
    if !android::is_installed() {
        return Err(MediaError::new(
            MediaErrorKind::Initialization,
            "install gst::android::install_environment with the host JavaVM and global references before media initialization",
        ));
    }
    static INITIALIZED: OnceLock<MediaResult<()>> = OnceLock::new();
    INITIALIZED
        .get_or_init(|| {
            gstreamer::init()
                .map_err(|error| MediaError::new(MediaErrorKind::Initialization, error.to_string()))
        })
        .clone()
}

/// The sample owns the decoder's backing memory even after the pipeline stops.
pub struct SampleStorage {
    sample: Sample,
    info: video::VideoInfo,
}
impl SampleStorage {
    pub fn sample(&self) -> &Sample {
        &self.sample
    }
}
impl VideoStorage for SampleStorage {
    fn copy_planes(&self) -> MediaResult<Vec<Vec<u8>>> {
        let buffer = self
            .sample
            .buffer_owned()
            .ok_or_else(|| MediaError::new(MediaErrorKind::Decode, "sample has no buffer"))?;
        let frame = video::VideoFrame::from_buffer_readable(buffer, &self.info).map_err(|e| {
            MediaError::new(
                MediaErrorKind::Decode,
                format!("failed to map video memory: {e:?}"),
            )
        })?;
        (0..self.info.n_planes())
            .map(|plane| {
                frame
                    .plane_data(plane)
                    .map(<[u8]>::to_vec)
                    .map_err(|e| MediaError::new(MediaErrorKind::Decode, e.to_string()))
            })
            .collect()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub(crate) struct Epoch {
    current: AtomicU64,
    pending: Mutex<Option<(Seqnum, u64)>>,
    video: AtomicU64,
    audio: AtomicU64,
    audio_buffer: Mutex<Option<(u64, Option<MediaTime>)>>,
}
impl Epoch {
    fn new(generation: u64) -> Self {
        Self {
            current: AtomicU64::new(generation),
            pending: Mutex::new(None),
            video: AtomicU64::new(generation),
            audio: AtomicU64::new(generation),
            audio_buffer: Mutex::new(None),
        }
    }
    fn stream_generation(&self, video: bool) -> &AtomicU64 {
        if video { &self.video } else { &self.audio }
    }
    fn segment(&self, seqnum: Seqnum, video: bool) {
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let generation = match *pending {
            Some((expected, generation)) if seqnum == expected => generation,
            Some(_) => 0,
            None => self.current.load(Ordering::Acquire),
        };
        self.stream_generation(video)
            .store(generation, Ordering::Release);
    }
}

/// Add element() to an application-owned pipeline. This object does not own that pipeline.
#[derive(Clone)]
pub struct GstVideoEndpoint {
    sink: app::AppSink,
    output: VideoOutput,
    epoch: Arc<Epoch>,
    error: Arc<Mutex<Option<MediaError>>>,
}
impl GstVideoEndpoint {
    pub fn new(output: VideoOutput) -> MediaResult<Self> {
        initialize()?;
        Self::with_caps(
            output,
            Caps::builder("video/x-raw").field("format", "RGBA").build(),
        )
    }
    /// Use explicit caps for advanced pipelines, including backend memory features.
    pub fn with_caps(output: VideoOutput, caps: Caps) -> MediaResult<Self> {
        initialize()?;
        let generation = output.begin_generation()?;
        let epoch = Arc::new(Epoch::new(generation));
        let error = Arc::new(Mutex::new(None));
        let sink = app::AppSink::builder()
            .caps(&caps)
            .max_buffers(3)
            .max_bytes(256 * 1024 * 1024)
            // 1.28.8 can report nonzero queued time with an empty queue after
            // a nonzero seek. Bound bytes/frames; max-time would block forever.
            .enable_last_sample(false)
            .sync(true)
            .build();
        let sequence = Arc::new(AtomicU64::new(1));
        let publish = {
            let output = output.clone();
            let epoch = epoch.clone();
            let error = error.clone();
            move |sample: Sample| -> Result<FlowSuccess, FlowError> {
                let generation = epoch.video.load(Ordering::Acquire);
                if generation == 0 || output.is_closed() {
                    return Ok(FlowSuccess::Ok);
                }
                let result =
                    make_frame(sample, generation, sequence.fetch_add(1, Ordering::Relaxed));
                match result {
                    Ok(frame) => {
                        output.publish(frame);
                        Ok(FlowSuccess::Ok)
                    }
                    Err(failure) => {
                        *error
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(failure);
                        Err(FlowError::Error)
                    }
                }
            }
        };
        let publish = Arc::new(publish);
        let preroll = publish.clone();
        sink.set_callbacks(
            app::AppSinkCallbacks::builder()
                .new_sample(move |sink| publish(sink.pull_sample().map_err(|_| FlowError::Eos)?))
                .new_preroll(move |sink| preroll(sink.pull_preroll().map_err(|_| FlowError::Eos)?))
                .build(),
        );
        let pad = sink.static_pad("sink").ok_or_else(|| {
            MediaError::new(MediaErrorKind::Initialization, "appsink has no sink pad")
        })?;
        attach_epoch_probe(&pad, epoch.clone(), true);
        Ok(Self {
            sink,
            output,
            epoch,
            error,
        })
    }
    pub fn element(&self) -> &Element {
        self.sink.upcast_ref()
    }
    pub fn output(&self) -> &VideoOutput {
        &self.output
    }
    pub fn appsink(&self) -> &app::AppSink {
        &self.sink
    }
    pub fn take_error(&self) -> Option<MediaError> {
        self.error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
    pub(crate) fn generation(&self) -> u64 {
        self.epoch.current.load(Ordering::Acquire)
    }
    pub(crate) fn prepare_seek(&self, seqnum: Seqnum) -> MediaResult<u64> {
        let generation = self.output.begin_generation()?;
        *self
            .epoch
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((seqnum, generation));
        self.epoch.current.store(generation, Ordering::Release);
        Ok(generation)
    }
    pub(crate) fn acknowledge_seek(&self) {
        self.epoch
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
    pub(crate) fn reject_seek(&self) {
        self.acknowledge_seek();
        let generation = self.generation();
        self.epoch.video.store(generation, Ordering::Release);
        self.epoch.audio.store(generation, Ordering::Release);
        *self
            .epoch
            .audio_buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }
    pub(crate) fn attach_audio(&self, sink: &Element) -> MediaResult<()> {
        let pad = sink.static_pad("sink").ok_or_else(|| {
            MediaError::new(MediaErrorKind::Initialization, "audio sink has no sink pad")
        })?;
        attach_epoch_probe(&pad, self.epoch.clone(), false);
        Ok(())
    }
    // Outer None means no post-seek buffer; inner None is a genuine unknown PTS.
    pub(crate) fn seek_witness(
        &self,
        has_video: bool,
        generation: u64,
    ) -> Option<Option<MediaTime>> {
        if has_video {
            self.output
                .latest()
                .filter(|frame| frame.generation == generation)
                .map(|f| f.pts)
        } else {
            self.epoch
                .audio_buffer
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .filter(|(epoch, _)| *epoch == generation)
                .map(|(_, pts)| pts)
        }
    }
}

fn attach_epoch_probe(pad: &Pad, epoch: Arc<Epoch>, video: bool) {
    pad.add_probe(
        PadProbeType::EVENT_DOWNSTREAM | PadProbeType::EVENT_FLUSH | PadProbeType::BUFFER,
        move |_, info| {
            if let Some(event) = info.event() {
                match event.view() {
                    EventView::FlushStart(_) => {
                        epoch.stream_generation(video).store(0, Ordering::Release);
                    }
                    EventView::Segment(_) => epoch.segment(event.seqnum(), video),
                    _ => {}
                }
            } else if !video && let Some(buffer) = info.buffer() {
                let generation = epoch.audio.load(Ordering::Acquire);
                *epoch
                    .audio_buffer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some((generation, buffer.pts().map(|t| MediaTime(t.nseconds()))));
            }
            PadProbeReturn::Ok
        },
    );
}

fn make_frame(sample: Sample, generation: u64, sequence: u64) -> MediaResult<VideoFrame> {
    let caps = sample
        .caps()
        .ok_or_else(|| MediaError::new(MediaErrorKind::Negotiation, "sample has no caps"))?;
    let info = video::VideoInfo::from_caps(caps)
        .map_err(|e| MediaError::new(MediaErrorKind::Negotiation, e.to_string()))?;
    let buffer = sample
        .buffer()
        .ok_or_else(|| MediaError::new(MediaErrorKind::Decode, "sample has no buffer"))?;
    let meta = buffer.meta::<video::VideoMeta>();
    let strides = meta
        .as_ref()
        .map_or_else(|| info.stride(), |meta| meta.stride());
    let offsets = meta
        .as_ref()
        .map_or_else(|| info.offset(), |meta| meta.offset());
    if strides.len() < info.n_planes() as usize
        || offsets.len() < info.n_planes() as usize
        || meta.as_ref().is_some_and(|meta| {
            meta.format() != info.format()
                || meta.width() < info.width()
                || meta.height() < info.height()
        })
    {
        return Err(MediaError::new(
            MediaErrorKind::Negotiation,
            "video metadata does not match the negotiated format",
        ));
    }
    let format = VideoFormat {
        width: info.width(),
        height: info.height(),
        pixel_format: match info.format() {
            video::VideoFormat::Rgba => PixelFormat::Rgba8,
            video::VideoFormat::Bgra => PixelFormat::Bgra8,
            other => PixelFormat::Other(other.to_str().to_string()),
        },
        planes: (0..info.n_planes() as usize)
            .map(|plane| {
                usize::try_from(strides[plane])
                    .map(|stride| PlaneLayout {
                        stride,
                        offset: offsets[plane],
                    })
                    .map_err(|_| {
                        MediaError::new(
                            MediaErrorKind::Unsupported,
                            "negative video strides require an explicit conversion",
                        )
                    })
            })
            .collect::<MediaResult<_>>()?,
        pixel_aspect_ratio: (info.par().numer() as u32, info.par().denom() as u32),
        frame_rate: (info.fps().numer() as u32, info.fps().denom() as u32),
        colorimetry: info.colorimetry().to_string(),
        dynamic_range: match info.colorimetry().transfer() {
            video::VideoTransferFunction::Smpte2084 => VideoDynamicRange::Pq,
            video::VideoTransferFunction::AribStdB67 => VideoDynamicRange::Hlg,
            video::VideoTransferFunction::Unknown => VideoDynamicRange::Unknown,
            _ => VideoDynamicRange::Sdr,
        },
        negotiated_caps: caps.to_string(),
    };
    Ok(VideoFrame {
        generation,
        sequence,
        pts: buffer.pts().map(|t| MediaTime(t.nseconds())),
        dts: buffer.dts().map(|t| MediaTime(t.nseconds())),
        duration: buffer.duration().map(|t| MediaTime(t.nseconds())),
        format,
        storage: Arc::new(SampleStorage { sample, info }),
    })
}
