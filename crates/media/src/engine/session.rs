//! One FFmpeg playback session: demux, decode, colour convert, deliver and clock.
//!
//! Every `AV*` handle is created and destroyed on the session's pump thread, so no
//! backend pointer is reachable from two threads at once. The clock follows the samples
//! actually delivered to the audio device and falls back to a monotonic wall clock when
//! no audio time is available, which keeps single-stream and silent playback moving.

use super::audio::Audio;
use super::io::CustomIo;
use super::pump::Task;
use super::video::{Converter, FrameStorage, Rgba, codec_name};
use super::{Abort, c_path, c_str, error, from_micros, from_ns, source_error, to_ns};
use crate::types::SharedPlayer;
use crate::{
    CancellationToken, CommandOutcome, MediaError, MediaErrorKind, MediaResult, MediaSource,
    MediaTime, PixelFormat, PlaneLayout, PlaybackIntent, PlaybackState, PlayerOptions,
    PlayerSnapshot, RequestId, SeekAccuracy, SeekRequest, TrackInfo, TrackKind, VideoFormat,
    VideoFrame, validate_uri,
};
use ffmpeg_sys_next as ffi;
use std::collections::{BTreeSet, VecDeque};
use std::ffi::{c_int, c_void};
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Bytes of demuxed packets one stream may hold before the pump waits for decoding.
const PACKET_BYTES: usize = 4 * 1024 * 1024;
/// Packets one stream may hold regardless of size.
const PACKET_COUNT: usize = 512;
/// How far past its limit a stream may grow while another selected stream starves.
const OVERFLOW_FACTOR: usize = 4;
/// Queued packets below which a stream counts as starving.
const STARVING_PACKETS: usize = 4;
/// Demux calls per pump tick.
const READ_LIMIT: usize = 64;
/// Decode iterations while prerolling a source or seek target.
const PREROLL_LIMIT: usize = 8192;
/// Snapshot position refresh interval.
const REPORT_INTERVAL: Duration = Duration::from_millis(50);
/// Sample rate of the software clock used when no device is available.
const VIRTUAL_RATE: u32 = 48_000;
const VIRTUAL_CHANNELS: usize = 2;
/// Queue fill that counts as fully buffered.
const COMPLETE: u8 = 100;

/// A converted frame waiting for its presentation time to arrive.
struct Ready {
    pts: Option<MediaTime>,
    duration: Option<MediaTime>,
    image: Rgba,
    frame_rate: (u32, u32),
}

/// A decoded audio frame and the output format the resampler must produce.
struct AudioFrame {
    frame: *mut ffi::AVFrame,
    format: c_int,
    rate: c_int,
    layout: c_int,
    sample_rate: u32,
    channels: usize,
    pts: Option<MediaTime>,
    span: MediaTime,
}

/// A seek or track change that is in progress, with the request it belongs to.
#[derive(Clone)]
struct Pending {
    id: RequestId,
    target: MediaTime,
    exact: bool,
    selection: Option<Vec<String>>,
}

/// One stream of the open source, with its decoder and packet queue.
struct Stream {
    index: c_int,
    id: String,
    kind: TrackKind,
    description: Option<String>,
    time_base: ffi::AVRational,
    frame_rate: (u32, u32),
    aspect: (u32, u32),
    selected: bool,
    decoder: *mut ffi::AVCodecContext,
    frame: *mut ffi::AVFrame,
    packets: VecDeque<*mut ffi::AVPacket>,
    bytes: usize,
    /// The demuxer reported end of stream for this stream.
    end: bool,
    /// A drain packet was sent, so only buffered frames remain.
    flushed: bool,
}

impl Stream {
    fn info(&self) -> TrackInfo {
        TrackInfo {
            id: self.id.clone(),
            kind: self.kind,
            description: self.description.clone(),
        }
    }

    fn full(&self) -> bool {
        self.bytes >= PACKET_BYTES || self.packets.len() >= PACKET_COUNT
    }

    /// The hard bound a full stream may grow to while another one starves.
    fn overflowing(&self) -> bool {
        self.bytes >= PACKET_BYTES * OVERFLOW_FACTOR
            || self.packets.len() >= PACKET_COUNT * OVERFLOW_FACTOR
    }

    /// Queue fill as a percentage, reported as `buffering_percent`.
    fn fill(&self) -> u8 {
        let percent = self.bytes / (PACKET_BYTES / 100).max(1);
        percent.min(COMPLETE as usize) as u8
    }

    fn size(packet: *mut ffi::AVPacket) -> usize {
        unsafe {
            // SAFETY: queued packets are live allocations owned by this stream.
            (*packet).size.max(0) as usize
        }
    }

    fn push(&mut self, packet: *mut ffi::AVPacket) {
        self.bytes += Self::size(packet);
        self.packets.push_back(packet);
    }

    fn pop(&mut self) -> Option<*mut ffi::AVPacket> {
        let packet = self.packets.pop_front()?;
        self.bytes = self.bytes.saturating_sub(Self::size(packet));
        Some(packet)
    }

    fn restore(&mut self, packet: *mut ffi::AVPacket) {
        self.bytes += Self::size(packet);
        self.packets.push_front(packet);
    }

    fn clear_packets(&mut self) {
        while let Some(packet) = self.packets.pop_front() {
            free_packet(packet);
        }
        self.bytes = 0;
    }

    fn close(&mut self) {
        self.clear_packets();
        if !self.decoder.is_null() {
            let mut decoder = self.decoder;
            unsafe {
                // SAFETY: the context is uniquely owned by this stream.
                ffi::avcodec_free_context(&mut decoder);
            }
            self.decoder = std::ptr::null_mut();
        }
        if !self.frame.is_null() {
            let mut frame = self.frame;
            unsafe {
                // SAFETY: the scratch frame is uniquely owned by this stream.
                ffi::av_frame_free(&mut frame);
            }
            self.frame = std::ptr::null_mut();
        }
    }

    /// Drops queued packets and rewinds the decoder, ready to decode from a new position.
    fn flush(&mut self) {
        self.clear_packets();
        self.end = false;
        self.flushed = false;
        if !self.decoder.is_null() {
            unsafe {
                // SAFETY: the context is open and only this thread uses it.
                ffi::avcodec_flush_buffers(self.decoder);
            }
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.close();
    }
}

/// The FFmpeg session behind one [`crate::Player`].
pub(crate) struct Engine {
    shared: Arc<SharedPlayer>,
    options: PlayerOptions,
    abort: Arc<Abort>,
    converter: Converter,
    audio: Audio,
    format: *mut ffi::AVFormatContext,
    io: Option<CustomIo>,
    streams: Vec<Stream>,
    /// Reused demux target.
    packet: *mut ffi::AVPacket,
    /// Packet whose stream was full, kept so no data is dropped.
    deferred: Option<*mut ffi::AVPacket>,
    swr: *mut ffi::SwrContext,
    swr_key: Option<(c_int, c_int, c_int)>,
    pcm: *mut ffi::AVFrame,
    token: CancellationToken,
    generation: u64,
    sequence: u64,
    rate: f64,
    volume: f64,
    playing: bool,
    opened: bool,
    ended: bool,
    /// The demuxer reported end of file.
    drained: bool,
    live: bool,
    duration: Option<MediaTime>,
    end_time: Option<MediaTime>,
    position: MediaTime,
    anchor: (Instant, MediaTime),
    pending: Option<Ready>,
    seeking: Option<Pending>,
    last_report: Instant,
    buffering: Option<u8>,
    decoders: Vec<String>,
}

impl Engine {
    pub(crate) fn new(
        shared: Arc<SharedPlayer>,
        options: PlayerOptions,
        abort: Arc<Abort>,
    ) -> Self {
        Self {
            audio: Audio::open(
                &options.audio,
                shared.meter.clone(),
                VIRTUAL_RATE,
                VIRTUAL_CHANNELS,
            ),
            converter: Converter::new(),
            shared,
            options,
            abort,
            format: std::ptr::null_mut(),
            io: None,
            streams: Vec::new(),
            packet: std::ptr::null_mut(),
            deferred: None,
            swr: std::ptr::null_mut(),
            swr_key: None,
            pcm: std::ptr::null_mut(),
            token: CancellationToken::default(),
            generation: 0,
            sequence: 0,
            rate: 1.0,
            volume: 1.0,
            playing: false,
            opened: false,
            ended: false,
            drained: false,
            live: false,
            duration: None,
            end_time: None,
            position: MediaTime(0),
            anchor: (Instant::now(), MediaTime(0)),
            pending: None,
            seeking: None,
            last_report: Instant::now(),
            buffering: None,
            decoders: Vec::new(),
        }
    }

    /// FFmpeg's cooperative abort hook. It may be called repeatedly while a read or
    /// seek is stuck, so it only reads atomics.
    unsafe extern "C" fn aborted(opaque: *mut c_void) -> c_int {
        if opaque.is_null() {
            return 0;
        }
        let abort = unsafe {
            // SAFETY: the opaque pointer addresses the allocation of an `Arc<Abort>` that
            // outlives the format context, and the pointee is only read through atomics.
            &*(opaque.cast::<Abort>())
        };
        abort.aborted() as c_int
    }

    pub(crate) fn handle(&mut self, task: Task) {
        match task {
            Task::Open {
                id,
                source,
                cancellation,
            } => match self.open(id, source, cancellation) {
                Ok(()) => {}
                Err(error) => self.fail(error),
            },
            Task::Play { id } => self.call(id, |engine| engine.transition(id, true)),
            Task::Pause { id } => self.call(id, |engine| engine.transition(id, false)),
            Task::Seek { id, request } => self.call(id, |engine| engine.seek(id, request)),
            Task::Tracks { id, tracks } => self.call(id, |engine| engine.select_tracks(id, tracks)),
            Task::Rate { id, rate } => self.call(id, |engine| engine.set_rate(id, rate)),
            Task::Volume { id, volume } => self.call(id, |engine| engine.set_volume(id, volume)),
            Task::Stop => {}
        }
    }

    /// Runs a control operation; a rejected one fails only that request, while a media
    /// failure ends the session like a backend error would.
    fn call(&mut self, id: RequestId, operation: impl FnOnce(&mut Self) -> MediaResult<()>) {
        let state = self.state();
        if state == PlaybackState::Closed {
            self.reject(id, closed());
            return;
        }
        if let Err(error) = operation(self) {
            self.reject(id, error.clone());
            if state == PlaybackState::Failed && matches!(error.kind, MediaErrorKind::Source) {
                self.teardown();
            }
        }
    }

    /// Advances playback: consume realtime, demux, decode, publish and refresh.
    pub(crate) fn tick(&mut self) {
        if !self.opened || self.ended {
            return;
        }
        if let Err(error) = self.pump() {
            self.fail(error);
            return;
        }
        self.publish_due();
        if self.check_end() {
            return;
        }
        self.report(false);
    }

    /// Releases the session when its pump thread exits.
    pub(crate) fn shutdown(&mut self) {
        self.close_session(None);
    }

    fn pump(&mut self) -> MediaResult<()> {
        self.read_packets()?;
        self.decode_audio()?;
        self.decode_video(None)
    }

    fn read_packets(&mut self) -> MediaResult<()> {
        if self.drained {
            self.drain_decoders();
            return Ok(());
        }
        if self.format.is_null() {
            return Ok(());
        }
        for _ in 0..READ_LIMIT {
            if let Some(packet) = self.deferred.take() {
                self.queue(packet)?;
                if self.deferred.is_some() {
                    // Still full: wait for the decoders instead of spinning.
                    return Ok(());
                }
                continue;
            }
            let packet = self.scratch();
            if packet.is_null() {
                return Err(error(
                    MediaErrorKind::Initialization,
                    ffi::AVERROR(ffi::ENOMEM),
                ));
            }
            let status = unsafe {
                // SAFETY: the context is open, the packet is a valid empty allocation and
                // both belong to this thread.
                ffi::av_read_frame(self.format, packet)
            };
            if status == ffi::AVERROR(ffi::EAGAIN) {
                return Ok(());
            }
            if status < 0 {
                if status != ffi::AVERROR_EOF {
                    return Err(source_error(status));
                }
                self.drained = true;
                self.drain_decoders();
                return Ok(());
            }
            self.queue(packet)?;
            if self.audio.is_full() {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Routes a freshly read packet to its stream, holding it if that stream is full.
    fn queue(&mut self, packet: *mut ffi::AVPacket) -> MediaResult<()> {
        let index = unsafe {
            // SAFETY: the packet is a live allocation read from the demuxer.
            (*packet).stream_index
        };
        let Some(target) = self
            .streams
            .iter()
            .find(|stream| stream.index == index && stream.selected && stream.decodable())
        else {
            // An unselected stream or one with no decoder: drop it so the demuxer
            // keeps making progress.
            unsafe {
                // SAFETY: the packet is owned here and never queued.
                ffi::av_packet_unref(packet);
            }
            return Ok(());
        };
        // A full stream waits, unless another selected stream is starving behind
        // it in the file; then it may overflow up to a hard bound, or both stall.
        let starving = self.streams.iter().any(|stream| {
            stream.index != index
                && stream.selected
                && stream.decodable()
                && !stream.end
                && stream.packets.len() < STARVING_PACKETS
        });
        if target.full() && !(starving && !target.overflowing()) {
            self.deferred = Some(packet);
            if self.packet == packet {
                self.packet = std::ptr::null_mut();
            }
            return Ok(());
        }
        let Some(stream) = self.streams.iter_mut().find(|stream| stream.index == index) else {
            return Ok(());
        };
        stream.push(packet);
        if self.packet == packet {
            self.packet = std::ptr::null_mut();
        }
        Ok(())
    }

    /// Returns the reusable demux packet, allocating it after a hand-off.
    fn scratch(&mut self) -> *mut ffi::AVPacket {
        if self.packet.is_null() {
            self.packet = unsafe {
                // SAFETY: allocation returns null when out of memory, checked by callers.
                ffi::av_packet_alloc()
            };
        }
        self.packet
    }

    /// Tells every selected decoder whose queue is empty that no further packets will
    /// arrive. Streams still holding packets drain on a later tick, once decoded.
    fn drain_decoders(&mut self) {
        for stream in self.streams.iter_mut() {
            if !stream.selected
                || stream.decoder.is_null()
                || stream.flushed
                || !stream.packets.is_empty()
            {
                continue;
            }
            stream.end = true;
            stream.flushed = true;
            unsafe {
                // SAFETY: a null packet is the documented decoder drain signal.
                ffi::avcodec_send_packet(stream.decoder, std::ptr::null());
            }
        }
    }

    /// Feeds queued packets until a frame is available. Null means more input is needed
    /// or the decoder is drained.
    fn receive(stream: &mut Stream) -> MediaResult<*mut ffi::AVFrame> {
        if stream.decoder.is_null() {
            return Ok(std::ptr::null_mut());
        }
        if stream.frame.is_null() {
            stream.frame = unsafe {
                // SAFETY: allocation returns null when out of memory, checked here.
                ffi::av_frame_alloc()
            };
            if stream.frame.is_null() {
                return Err(error(
                    MediaErrorKind::Initialization,
                    ffi::AVERROR(ffi::ENOMEM),
                ));
            }
        }
        loop {
            let status = unsafe {
                // SAFETY: the context is open and the frame is a valid scratch.
                ffi::avcodec_receive_frame(stream.decoder, stream.frame)
            };
            if status == 0 {
                return Ok(stream.frame);
            }
            if status != ffi::AVERROR(ffi::EAGAIN) {
                if status == ffi::AVERROR_EOF {
                    stream.end = true;
                    return Ok(std::ptr::null_mut());
                }
                return Err(error(MediaErrorKind::Decode, status));
            }
            let Some(packet) = stream.pop() else {
                return Ok(std::ptr::null_mut());
            };
            let send = unsafe {
                // SAFETY: the packet is owned here and the decoder consumes or rejects it.
                ffi::avcodec_send_packet(stream.decoder, packet)
            };
            if send == ffi::AVERROR(ffi::EAGAIN) {
                stream.restore(packet);
                continue;
            }
            free_packet(packet);
            if send == 0 || send == ffi::AVERROR_INVALIDDATA {
                continue;
            }
            if send == ffi::AVERROR_EOF {
                stream.end = true;
                return Ok(std::ptr::null_mut());
            }
            return Err(error(MediaErrorKind::Decode, send));
        }
    }

    fn decode_audio(&mut self) -> MediaResult<()> {
        let Some(index) = self.index_of(TrackKind::Audio) else {
            return Ok(());
        };
        let discard = self.exact_target();
        let (sample_rate, channels) = self.audio.format();
        loop {
            if self.audio.is_full() {
                return Ok(());
            }
            let frame = {
                let Self { streams, .. } = self;
                Self::receive(&mut streams[index])?
            };
            if frame.is_null() {
                return Ok(());
            }
            let (pts, frames, format, rate, layout) = unsafe {
                // SAFETY: the frame is a live decoded audio frame until it is unreferenced.
                (
                    (*frame).best_effort_timestamp,
                    (*frame).nb_samples.max(0),
                    (*frame).format,
                    (*frame).sample_rate.max(1),
                    (*frame).ch_layout.nb_channels.max(1),
                )
            };
            let time_base = self.streams[index].time_base;
            let start = to_ns(pts, time_base);
            let seconds = frames as f64 / f64::from(rate);
            let span = MediaTime((seconds * 1_000_000_000.0) as u64);
            let late = discard.is_none_or(|target| {
                start.is_none_or(|start| MediaTime(start.0.saturating_add(span.0)) >= target)
                    || start == Some(MediaTime(0))
            });
            let delivered = if late {
                self.resample(AudioFrame {
                    frame,
                    format,
                    rate,
                    layout,
                    sample_rate,
                    channels,
                    pts: start,
                    span,
                })?
            } else {
                0
            };
            unsafe {
                // SAFETY: releases the decoded frame, which is not retained elsewhere.
                ffi::av_frame_unref(frame);
            }
            if delivered > 0 {
                let clock = start.unwrap_or(self.position);
                self.end_time = max_time(self.end_time, MediaTime(clock.0.saturating_add(span.0)));
            }
        }
    }

    /// Resamples one decoded frame into the output format and queues it.
    fn resample(&mut self, input: AudioFrame) -> MediaResult<usize> {
        let AudioFrame {
            frame,
            format,
            rate,
            layout,
            sample_rate,
            channels,
            pts,
            span,
        } = input;
        if self.pcm.is_null() {
            self.pcm = unsafe {
                // SAFETY: allocation returns null when out of memory, checked here.
                ffi::av_frame_alloc()
            };
            if self.pcm.is_null() {
                return Err(error(
                    MediaErrorKind::Initialization,
                    ffi::AVERROR(ffi::ENOMEM),
                ));
            }
        }
        let key = (format, rate, layout);
        if self.swr_key != Some(key) {
            self.reconfigure(key, sample_rate, channels)?;
        }
        unsafe {
            // SAFETY: the decoded frame is live and owned by this session until unreferenced.
            super::normalize_layout(frame);
        }
        unsafe {
            // SAFETY: the frame is empty apart from the descriptor fields written here,
            // and `swr_convert_frame` allocates its own output buffer from them.
            ffi::av_frame_unref(self.pcm);
            (*self.pcm).format = ffi::AVSampleFormat::AV_SAMPLE_FMT_FLT as c_int;
            (*self.pcm).sample_rate = c_int::try_from(sample_rate).unwrap_or(VIRTUAL_RATE as c_int);
            ffi::av_channel_layout_default(&mut (*self.pcm).ch_layout, channels as c_int);
        }
        let status = unsafe {
            // SAFETY: the resampler is uninitialised or matches this input format, the
            // output frame has no buffer attached, and both frames outlive the call.
            ffi::swr_convert_frame(self.swr, self.pcm, frame)
        };
        if status < 0 {
            return Err(error(MediaErrorKind::Decode, status));
        }
        let produced = unsafe {
            // SAFETY: the output frame is live and was just filled by the resampler.
            (*self.pcm).nb_samples.max(0) as usize
        };
        if produced == 0 {
            return Ok(0);
        }
        let samples = unsafe {
            // SAFETY: interleaved float output is a single plane of `nb_samples * channels`
            // samples owned by the output frame, valid until the next unref.
            std::slice::from_raw_parts(
                (*self.pcm).data[0].cast::<f32>(),
                produced.saturating_mul(channels),
            )
        };
        self.audio.push(pts, samples, span);
        Ok(produced)
    }

    /// Points the resampler at a new input format; the next conversion reconfigures it.
    fn reconfigure(
        &mut self,
        key: (c_int, c_int, c_int),
        sample_rate: u32,
        channels: usize,
    ) -> MediaResult<()> {
        if self.swr.is_null() {
            self.swr = unsafe {
                // SAFETY: allocation returns null when out of memory, checked here.
                ffi::swr_alloc()
            };
            if self.swr.is_null() {
                return Err(error(
                    MediaErrorKind::Initialization,
                    ffi::AVERROR(ffi::ENOMEM),
                ));
            }
        } else {
            unsafe {
                // SAFETY: closing keeps the context so `swr_convert_frame` reconfigures
                // from the next pair of frames.
                ffi::swr_close(self.swr);
            }
        }
        self.swr_key = Some(key);
        let _ = (sample_rate, channels);
        Ok(())
    }

    fn decode_video(&mut self, discard_before: Option<MediaTime>) -> MediaResult<()> {
        let Some(index) = self.index_of(TrackKind::Video) else {
            return Ok(());
        };
        let discard = discard_before.or_else(|| {
            self.exact_target().filter(|_| {
                self.seeking
                    .as_ref()
                    .is_some_and(|pending| pending.exact && !pending.selection.is_none())
            })
        });
        loop {
            if self.pending.is_some() {
                return Ok(());
            }
            let frame = {
                let Self { streams, .. } = self;
                Self::receive(&mut streams[index])?
            };
            if frame.is_null() {
                return Ok(());
            }
            let (stamp, duration) = unsafe {
                // SAFETY: the frame is a live decoded video frame.
                ((*frame).best_effort_timestamp, (*frame).duration.max(0))
            };
            let (time_base, frame_rate, aspect) = {
                let stream = &self.streams[index];
                (stream.time_base, stream.frame_rate, stream.aspect)
            };
            let pts = to_ns(stamp, time_base);
            let span = (duration > 0).then(|| to_ns(duration, time_base)).flatten();
            let stale = discard.is_some_and(|target| pts.is_some_and(|pts| pts < target));
            let image = if stale {
                None
            } else {
                Some(self.converter.convert(frame, None)?)
            };
            unsafe {
                // SAFETY: the decoded frame is not retained past this point.
                ffi::av_frame_unref(frame);
            }
            let Some(mut image) = image else {
                continue;
            };
            if image.aspect_ratio.0 == 0 && aspect.0 > 0 {
                image.aspect_ratio = aspect;
            }
            self.pending = Some(Ready {
                pts,
                duration: span,
                image,
                frame_rate,
            });
        }
    }

    fn publish_due(&mut self) {
        let clock = self.clock();
        if self
            .pending
            .as_ref()
            .is_some_and(|ready| ready.pts.is_some_and(|pts| pts > clock))
        {
            return;
        }
        let Some(ready) = self.pending.take() else {
            return;
        };
        self.publish(ready);
    }

    /// Moves a converted frame into the shared output, rejecting stale generations.
    fn publish(&mut self, ready: Ready) {
        let pts = ready.pts.unwrap_or(self.position);
        let width = ready.image.width;
        let height = ready.image.height;
        let stride = usize::try_from(width).unwrap_or(0).saturating_mul(4);
        let description = self
            .stream_description(TrackKind::Video)
            .unwrap_or_else(|| "rgba".to_owned());
        let format = VideoFormat {
            width,
            height,
            pixel_format: PixelFormat::Rgba8,
            planes: vec![PlaneLayout { stride, offset: 0 }],
            pixel_aspect_ratio: ready.image.aspect_ratio,
            frame_rate: ready.frame_rate,
            colorimetry: ready.image.colorimetry.clone(),
            dynamic_range: ready.image.dynamic_range,
            format_description: format!(
                "{description}, {width}x{height}, {}",
                ready.image.colorimetry
            ),
        };
        self.sequence += 1;
        if self.shared.output.publish(VideoFrame {
            generation: self.generation,
            sequence: self.sequence,
            pts: Some(pts),
            dts: None,
            duration: ready.duration,
            format,
            storage: Arc::new(FrameStorage::new(ready.image.bytes)),
        }) {
            self.end_time = max_time(self.end_time, pts);
            self.position = self.position.max(pts);
        }
    }

    fn check_end(&mut self) -> bool {
        if !self.drained
            || self.pending.is_some()
            || !self.queues_empty()
            || self.audio.has_queued()
        {
            return false;
        }
        let clock = self.clock();
        if self.end_time.is_some_and(|end| clock < end) {
            return false;
        }
        self.ended = true;
        self.playing = false;
        self.audio.set_playing(false);
        self.anchor = (Instant::now(), clock);
        self.shared.update(|snapshot| {
            if snapshot.state != PlaybackState::Failed {
                snapshot.state = PlaybackState::Ended;
            }
            snapshot.position = Some(clock);
            snapshot.buffering_percent = Some(COMPLETE);
        });
        true
    }

    fn queues_empty(&self) -> bool {
        self.deferred.is_none()
            && self
                .streams
                .iter()
                .all(|stream| !stream.selected || stream.packets.is_empty())
    }

    /// The media time the session is currently presenting.
    fn clock(&mut self) -> MediaTime {
        let delivered = if self.audio_finished() {
            None
        } else {
            self.audio.clock()
        };
        if let Some(clock) = delivered {
            self.anchor = (Instant::now(), clock);
            self.position = clock;
            return clock;
        }
        if !self.playing {
            return self.anchor.1;
        }
        let elapsed = self.anchor.0.elapsed().as_secs_f64() * self.rate;
        let clock = MediaTime(
            self.anchor
                .1
                .0
                .saturating_add((elapsed * 1_000_000_000.0).max(0.0) as u64),
        );
        self.position = clock;
        clock
    }

    /// Whether audio can no longer advance the clock, so the wall clock must.
    fn audio_finished(&self) -> bool {
        self.index_of(TrackKind::Audio).is_none_or(|index| {
            self.streams
                .get(index)
                .is_some_and(|stream| stream.end && stream.packets.is_empty())
        })
    }

    fn report(&mut self, force: bool) {
        if !force && self.last_report.elapsed() < REPORT_INTERVAL {
            return;
        }
        self.last_report = Instant::now();
        let position = Some(self.position);
        let duration = self.duration;
        let range = (!self.live).then_some((MediaTime(0), duration));
        let buffering = self.buffering_percent();
        self.shared.update(|snapshot| {
            snapshot.position = position;
            snapshot.duration = duration;
            snapshot.seekable_range = range;
            snapshot.buffering_percent = buffering;
        });
    }

    fn buffering_percent(&mut self) -> Option<u8> {
        if !self.opened {
            return None;
        }
        let percent = if self.drained && self.queues_empty() {
            COMPLETE
        } else {
            self.streams
                .iter()
                .filter(|stream| stream.selected)
                .map(Stream::fill)
                .max()
                .unwrap_or(COMPLETE)
        };
        if self
            .buffering
            .is_none_or(|last| last.abs_diff(percent) >= 5)
        {
            self.buffering = Some(percent);
        }
        self.buffering
    }

    fn index_of(&self, kind: TrackKind) -> Option<usize> {
        self.streams
            .iter()
            .position(|stream| stream.selected && stream.kind == kind)
    }

    fn exact_target(&self) -> Option<MediaTime> {
        self.seeking
            .as_ref()
            .filter(|pending| pending.exact)
            .map(|pending| pending.target)
    }

    fn stream_description(&self, kind: TrackKind) -> Option<String> {
        self.streams
            .iter()
            .find(|stream| stream.selected && stream.kind == kind)
            .and_then(|stream| stream.description.clone())
    }

    fn state(&self) -> PlaybackState {
        self.shared
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state
    }

    fn open(
        &mut self,
        id: RequestId,
        source: MediaSource,
        cancellation: CancellationToken,
    ) -> MediaResult<()> {
        self.teardown();
        self.supersede();
        if self.state() == PlaybackState::Closed {
            return Err(closed());
        }
        if let MediaSource::Uri(uri) = &source {
            validate_uri(uri)?;
        }
        let source = crate::http::route(source)?;
        self.token = cancellation;
        self.abort.reset();
        let volume = self
            .shared
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .volume;
        self.volume = volume;
        self.rate = 1.0;
        self.audio.set_volume(volume);
        self.audio.set_rate(1.0);
        self.generation = self.shared.output.begin_generation()?;

        let mut format = unsafe {
            // SAFETY: allocation returns null when out of memory, checked here.
            ffi::avformat_alloc_context()
        };
        if format.is_null() {
            return Err(error(
                MediaErrorKind::Initialization,
                ffi::AVERROR(ffi::ENOMEM),
            ));
        }
        unsafe {
            // SAFETY: the context owns the callback slot for as long as it lives, and the
            // opaque pointer addresses an `Arc` allocation held by this session.
            (*format).interrupt_callback.callback = Some(Self::aborted);
            (*format).interrupt_callback.opaque =
                Arc::as_ptr(&self.abort).cast_mut().cast::<c_void>();
        }
        let (url, io) = match source {
            MediaSource::File(path) => (Some(c_path(&path)?), None),
            MediaSource::Uri(uri) => (Some(c_str(&uri)?), None),
            MediaSource::Bytes(bytes) => (
                None,
                Some(CustomIo::new(
                    Box::new(Cursor::new(bytes)),
                    self.token.clone(),
                    self.abort.clone(),
                )?),
            ),
            MediaSource::Input(input) => (
                None,
                Some(CustomIo::new(
                    input,
                    self.token.clone(),
                    self.abort.clone(),
                )?),
            ),
        };
        self.io = io;
        if let Some(io) = self.io.as_mut() {
            unsafe {
                // SAFETY: the custom context owns its buffer and callbacks, is installed
                // before the format context opens, and outlives it in `self.io`.
                (*format).pb = io.context();
                (*format).flags |= ffi::AVFMT_FLAG_CUSTOM_IO;
            }
        }
        let status = unsafe {
            // SAFETY: `format` is a context owned by this call. FFmpeg frees it and nulls
            // the pointer on failure; a caller-supplied `pb` is marked custom and left alone.
            let url = url.as_ref().map_or(std::ptr::null(), |url| url.as_ptr());
            ffi::avformat_open_input(&mut format, url, std::ptr::null(), std::ptr::null_mut())
        };
        if status < 0 {
            self.format = std::ptr::null_mut();
            return Err(source_error(status));
        }
        self.format = format;
        let status = unsafe {
            // SAFETY: the context is open and its interrupt callback is installed.
            ffi::avformat_find_stream_info(format, std::ptr::null_mut())
        };
        if status < 0 {
            let error = source_error(status);
            self.teardown();
            return Err(error);
        }
        let collected = self.collect_streams();
        if let Err(error) = collected {
            self.teardown();
            return Err(error);
        }
        let (seekable, duration) = unsafe {
            // SAFETY: the context is open; a null protocol means the demuxer reads
            // internally, otherwise the protocol reports whether it can seek.
            let pb = (*format).pb;
            (
                pb.is_null() || ((*pb).seekable & ffi::AVIO_SEEKABLE_NORMAL) != 0,
                (*format).duration,
            )
        };
        self.live = !seekable;
        self.duration = (duration > 0).then(|| from_micros(duration));
        self.end_time = self.duration;
        self.opened = true;
        self.drained = false;
        self.ended = false;
        self.position = MediaTime(0);
        self.anchor = (Instant::now(), MediaTime(0));
        self.playing = false;
        self.pending = None;
        self.buffering = None;
        self.decoders = self.decoder_names();
        let requested = if self.options.autoplay {
            PlaybackIntent::Playing
        } else {
            PlaybackIntent::Paused
        };
        let tracks = self.track_infos();
        let selected = self.selected();
        let duration = self.duration;
        let range = (!self.live).then_some((MediaTime(0), duration));
        let generation = self.generation;
        self.shared.update(|snapshot| {
            let source_generation = snapshot
                .source_generation
                .checked_add(1)
                .expect("source generation exhausted");
            *snapshot = PlayerSnapshot {
                state: PlaybackState::Opening,
                requested,
                source_generation,
                frame_generation: generation,
                volume: self.volume,
                rate: self.rate,
                tracks,
                selected_tracks: selected,
                duration,
                seekable_range: range,
                decoders: self.decoders.clone(),
                ..PlayerSnapshot::default()
            };
        });
        let preroll = self.preroll(None);
        if let Err(error) = preroll {
            self.teardown();
            return Err(error);
        }
        self.playing = self.options.autoplay;
        self.audio.set_playing(self.playing);
        self.anchor = (Instant::now(), self.position);
        self.last_report = Instant::now();
        self.shared.update(|snapshot| {
            snapshot.state = if self.playing {
                PlaybackState::Playing
            } else {
                PlaybackState::Paused
            };
            snapshot.seeking = false;
        });
        self.report(true);
        if self.abort.aborted() {
            return Err(MediaError::new(
                MediaErrorKind::Cancelled,
                "media open was abandoned",
            ));
        }
        self.complete(id);
        Ok(())
    }

    /// Decodes until the first selected video frame is available and shows it.
    fn preroll(&mut self, discard: Option<MediaTime>) -> MediaResult<()> {
        for _ in 0..PREROLL_LIMIT {
            if self.pending.is_some() || (self.drained && self.queues_empty()) {
                break;
            }
            self.read_packets()?;
            self.decode_audio()?;
            self.decode_video(discard)?;
        }
        if let Some(ready) = self.pending.take() {
            let pts = ready.pts.unwrap_or(self.position);
            self.position = pts.max(self.position);
            self.publish(ready);
        }
        Ok(())
    }

    fn collect_streams(&mut self) -> MediaResult<()> {
        let count = unsafe {
            // SAFETY: the context is open.
            (*self.format).nb_streams as usize
        };
        let mut streams = Vec::with_capacity(count);
        for position in 0..count {
            let (index, kind, name, par) = unsafe {
                // SAFETY: `position` is within the context's stream table and every stream
                // in an open context has codec parameters.
                let stream = *(*self.format).streams.add(position);
                let par = *(*stream).codecpar;
                let kind = match par.codec_type {
                    ffi::AVMediaType::AVMEDIA_TYPE_VIDEO => TrackKind::Video,
                    ffi::AVMediaType::AVMEDIA_TYPE_AUDIO => TrackKind::Audio,
                    ffi::AVMediaType::AVMEDIA_TYPE_SUBTITLE => TrackKind::Subtitle,
                    ffi::AVMediaType::AVMEDIA_TYPE_DATA
                    | ffi::AVMediaType::AVMEDIA_TYPE_ATTACHMENT
                    | ffi::AVMediaType::AVMEDIA_TYPE_UNKNOWN
                    | ffi::AVMediaType::AVMEDIA_TYPE_NB => TrackKind::Other,
                };
                (
                    (*stream).index,
                    kind,
                    codec_name(par.codec_id),
                    (
                        par.width,
                        par.height,
                        par.sample_rate,
                        par.ch_layout.nb_channels,
                    ),
                )
            };
            let description = match kind {
                TrackKind::Video => Some(format!("{name} {}x{}", par.0.max(0), par.1.max(0))),
                TrackKind::Audio => Some(format!("{name} {} Hz {} ch", par.2.max(0), par.3.max(0))),
                TrackKind::Subtitle | TrackKind::Other => Some(name),
            };
            let (time_base, rate, aspect) = unsafe {
                // SAFETY: `position` is within the context's stream table.
                let stream = *(*self.format).streams.add(position);
                (
                    (*stream).time_base,
                    stream_rates((*stream).avg_frame_rate, (*stream).r_frame_rate),
                    aspect((*stream).codecpar),
                )
            };
            streams.push(Stream {
                index,
                id: format!("{}:{position}", slug(&kind)),
                kind,
                description,
                time_base,
                frame_rate: rate,
                aspect,
                selected: false,
                decoder: std::ptr::null_mut(),
                frame: std::ptr::null_mut(),
                packets: VecDeque::new(),
                bytes: 0,
                end: false,
                flushed: false,
            });
        }
        for kind in [TrackKind::Video, TrackKind::Audio] {
            if let Some(index) = streams
                .iter()
                .position(|stream| stream.kind == kind && stream.decodable())
            {
                streams[index].selected = true;
            }
        }
        if streams.iter().all(|stream| !stream.selected) {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "no decodable audio or video stream",
            ));
        }
        self.streams = streams;
        for index in 0..self.streams.len() {
            if let Err(error) = self.open_decoder(index) {
                self.streams[index].selected = false;
                if self.streams.iter().all(|stream| !stream.selected) {
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn open_decoder(&mut self, index: usize) -> MediaResult<()> {
        let stream = &self.streams[index];
        if !stream.selected || matches!(stream.kind, TrackKind::Subtitle | TrackKind::Other) {
            return Ok(());
        }
        let par = unsafe {
            // SAFETY: the index is within the open context's stream table.
            let stream = *(*self.format).streams.add(index);
            *(*stream).codecpar
        };
        let codec = unsafe {
            // SAFETY: the codec id came from the demuxer.
            ffi::avcodec_find_decoder(par.codec_id)
        };
        if codec.is_null() {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                format!("FFmpeg cannot decode {}", codec_name(par.codec_id)),
            ));
        }
        let context = unsafe {
            // SAFETY: allocation with a valid codec returns a context or null.
            ffi::avcodec_alloc_context3(codec)
        };
        if context.is_null() {
            return Err(error(
                MediaErrorKind::Initialization,
                ffi::AVERROR(ffi::ENOMEM),
            ));
        }
        let time_base = self.streams[index].time_base;
        let status = unsafe {
            // SAFETY: the context was created for this codec and the parameters are live.
            ffi::avcodec_parameters_to_context(context, std::ptr::from_ref(&par))
        };
        let status = if status < 0 {
            status
        } else {
            unsafe {
                // SAFETY: the context is uniquely owned and the time base is the stream's.
                (*context).pkt_timebase = time_base;
            }
            unsafe {
                // SAFETY: the codec matches the context created for it.
                ffi::avcodec_open2(context, codec, std::ptr::null_mut())
            }
        };
        if status < 0 {
            let mut context = context;
            unsafe {
                // SAFETY: the context is still uniquely owned here.
                ffi::avcodec_free_context(&mut context);
            }
            return Err(error(MediaErrorKind::Negotiation, status));
        }
        self.streams[index].decoder = context;
        Ok(())
    }

    fn track_infos(&self) -> Vec<TrackInfo> {
        self.streams.iter().map(Stream::info).collect()
    }

    fn selected(&self) -> Vec<String> {
        self.streams
            .iter()
            .filter(|stream| stream.selected)
            .map(|stream| stream.id.clone())
            .collect()
    }

    fn decoder_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for stream in self.streams.iter().filter(|stream| stream.selected) {
            if stream.decoder.is_null() {
                continue;
            }
            let id = unsafe {
                // SAFETY: the context is open and its codec id was copied from parameters.
                (*stream.decoder).codec_id
            };
            let name = codec_name(id);
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }

    fn require_open(&self) -> MediaResult<()> {
        if !self.opened {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "open media before controlling playback",
            ));
        }
        Ok(())
    }

    fn transition(&mut self, id: RequestId, playing: bool) -> MediaResult<()> {
        self.require_open()?;
        self.playing = playing;
        self.ended = false;
        self.audio.set_playing(playing);
        self.anchor = (Instant::now(), self.position);
        self.shared.update(|snapshot| {
            snapshot.requested = if playing {
                PlaybackIntent::Playing
            } else {
                PlaybackIntent::Paused
            };
            snapshot.state = if playing {
                PlaybackState::Playing
            } else {
                PlaybackState::Paused
            };
        });
        self.complete(id);
        Ok(())
    }

    fn set_rate(&mut self, id: RequestId, rate: f64) -> MediaResult<()> {
        self.require_open()?;
        self.rate = rate;
        self.audio.set_rate(rate);
        self.anchor = (Instant::now(), self.position);
        self.shared.update(|snapshot| snapshot.rate = rate);
        self.complete(id);
        Ok(())
    }

    fn set_volume(&mut self, id: RequestId, volume: f64) -> MediaResult<()> {
        self.require_open()?;
        self.volume = volume;
        self.audio.set_volume(volume);
        self.shared.update(|snapshot| snapshot.volume = volume);
        self.complete(id);
        Ok(())
    }

    fn seek(&mut self, id: RequestId, request: SeekRequest) -> MediaResult<()> {
        self.require_open()?;
        if self.live {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "current media is not time-seekable",
            ));
        }
        self.begin_pending(Pending {
            id,
            target: request.position,
            exact: request.accuracy == SeekAccuracy::Exact,
            selection: None,
        })
    }

    fn select_tracks(&mut self, id: RequestId, tracks: Vec<String>) -> MediaResult<()> {
        self.require_open()?;
        if tracks.is_empty() {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "select at least one track",
            ));
        }
        if tracks.len() != tracks.iter().collect::<BTreeSet<_>>().len() {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "track selection contains duplicates",
            ));
        }
        let known = self.track_infos();
        if tracks
            .iter()
            .any(|id| !known.iter().any(|track| &track.id == id))
        {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "selected track is not in the current collection",
            ));
        }
        if !tracks.iter().any(|id| {
            known.iter().any(|track| {
                &track.id == id && matches!(track.kind, TrackKind::Audio | TrackKind::Video)
            })
        }) {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "playback requires an audio or video track",
            ));
        }
        if tracks == self.selected() {
            self.complete(id);
            return Ok(());
        }
        if self.live {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "changing tracks requires a seekable source",
            ));
        }
        let target = self.position;
        self.apply_selection(&tracks)?;
        self.begin_pending(Pending {
            id,
            target,
            exact: true,
            selection: Some(tracks),
        })
    }

    /// Applies a new selection, then reopens decoders and repositions.
    fn apply_selection(&mut self, tracks: &[String]) -> MediaResult<()> {
        for index in 0..self.streams.len() {
            let wanted = tracks.contains(&self.streams[index].id);
            self.streams[index].selected = wanted;
            if wanted && self.streams[index].decoder.is_null() {
                self.open_decoder(index)?;
            }
        }
        self.decoders = self.decoder_names();
        Ok(())
    }

    /// Starts a reposition: flush, seek, decode forward and show the frame at the target.
    fn begin_pending(&mut self, pending: Pending) -> MediaResult<()> {
        self.supersede();
        let target = pending.target;
        let selection = pending.selection.clone();
        let format = self.format;
        if format.is_null() {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "open media before controlling playback",
            ));
        }
        let reference = self
            .streams
            .iter()
            .find(|stream| stream.selected && stream.kind == TrackKind::Video)
            .or_else(|| self.streams.iter().find(|stream| stream.selected))
            .ok_or_else(|| MediaError::new(MediaErrorKind::Unsupported, "no track is selected"))?;
        let (index, time_base) = (reference.index, reference.time_base);
        let timestamp = from_ns(target, time_base);
        let status = unsafe {
            // SAFETY: the context is open and the timestamp is in the reference stream's
            // time base.
            ffi::av_seek_frame(format, index, timestamp, ffi::AVSEEK_FLAG_BACKWARD)
        };
        if status < 0 {
            return Err(error(MediaErrorKind::Source, status));
        }
        if let Some(packet) = self.deferred.take() {
            free_packet(packet);
        }
        for stream in self.streams.iter_mut() {
            if stream.selected {
                stream.flush();
            }
        }
        self.audio.flush();
        self.drained = false;
        self.ended = false;
        self.pending = None;
        self.generation = self.shared.output.begin_generation()?;
        self.seeking = Some(pending);
        let generation = self.generation;
        let selected = selection.clone();
        self.shared.update(|snapshot| {
            snapshot.seeking = true;
            snapshot.frame_generation = generation;
            if let Some(selection) = &selected {
                snapshot.selected_tracks = selection.clone();
            }
        });
        self.position = target;
        self.anchor = (Instant::now(), target);
        let Some(pending) = self.seeking.take() else {
            return Ok(());
        };
        let preroll = self.preroll(pending.exact.then_some(pending.target));
        if let Err(error) = preroll {
            self.fail(error.clone());
            return Err(error);
        }
        if self.abort.aborted() {
            self.teardown();
            return Err(MediaError::new(
                MediaErrorKind::Cancelled,
                "media seek was abandoned",
            ));
        }
        let selected = self.selected();
        let decoders = self.decoder_names();
        self.shared.update(|snapshot| {
            snapshot.seeking = false;
            snapshot.selected_tracks = selected;
            snapshot.decoders = decoders;
            snapshot.state = if self.playing {
                PlaybackState::Playing
            } else {
                PlaybackState::Paused
            };
        });
        self.audio.set_playing(self.playing);
        self.complete(pending.id);
        Ok(())
    }

    fn complete(&self, id: RequestId) {
        self.shared.finish(
            id,
            CommandOutcome::Completed {
                position: Some(self.position),
            },
        );
    }

    fn reject(&self, id: RequestId, error: MediaError) {
        self.shared.finish(id, CommandOutcome::Failed(error));
    }

    /// A reposition can only serve one request; the previous one is superseded.
    fn supersede(&mut self) {
        if let Some(pending) = self.seeking.take() {
            self.shared.finish(pending.id, CommandOutcome::Superseded);
        }
    }

    /// Ends the session after a media failure, leaving it ready for a new source.
    fn fail(&mut self, error: MediaError) {
        let state = self.state();
        self.teardown();
        if let Some(pending) = self.seeking.take() {
            self.shared.finish(pending.id, CommandOutcome::Superseded);
        }
        if state == PlaybackState::Failed || state == PlaybackState::Closed {
            return;
        }
        self.shared.update(|snapshot| {
            snapshot.state = PlaybackState::Failed;
            snapshot.seeking = false;
            snapshot.last_error = Some(error.clone());
        });
        let pending: Vec<RequestId> = self
            .shared
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|(id, value)| value.is_none().then_some(*id))
            .collect();
        for id in pending {
            self.shared
                .finish(id, CommandOutcome::Failed(error.clone()));
        }
    }

    fn close_session(&mut self, keep: Option<RequestId>) {
        self.teardown();
        if let Some(pending) = self.seeking.take() {
            let outcome = if Some(pending.id) == keep {
                CommandOutcome::Completed {
                    position: Some(self.position),
                }
            } else {
                CommandOutcome::Superseded
            };
            self.shared.finish(pending.id, outcome);
        }
        self.shared.output.close();
        if keep.is_some() || self.state() != PlaybackState::Failed {
            self.shared.update(|snapshot| {
                snapshot.state = PlaybackState::Closed;
                snapshot.seeking = false;
            });
        } else {
            // A failed session keeps its diagnosis when the context shuts it down.
            self.shared.update(|snapshot| snapshot.seeking = false);
        }
        let pending: Vec<RequestId> = self
            .shared
            .results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(|(id, value)| value.is_none().then_some(*id))
            .collect();
        for id in pending {
            if Some(id) != keep {
                self.shared.finish(id, CommandOutcome::Failed(closed()));
            }
        }
        if let Some(id) = keep {
            self.shared.finish(
                id,
                CommandOutcome::Completed {
                    position: Some(self.position),
                },
            );
        }
    }

    /// Releases every backend handle while keeping the session reusable.
    fn teardown(&mut self) {
        self.audio.stop();
        self.opened = false;
        self.playing = false;
        self.ended = false;
        self.drained = false;
        self.pending = None;
        self.streams.clear();
        if let Some(packet) = self.deferred.take() {
            free_packet(packet);
        }
        if !self.packet.is_null() {
            let mut packet = self.packet;
            unsafe {
                // SAFETY: the scratch packet is uniquely owned by the session.
                ffi::av_packet_free(&mut packet);
            }
            self.packet = std::ptr::null_mut();
        }
        if !self.pcm.is_null() {
            let mut frame = self.pcm;
            unsafe {
                // SAFETY: the resample scratch frame is uniquely owned here.
                ffi::av_frame_free(&mut frame);
            }
            self.pcm = std::ptr::null_mut();
        }
        if !self.swr.is_null() {
            let mut swr = self.swr;
            unsafe {
                // SAFETY: the resampler is uniquely owned here.
                ffi::swr_free(&mut swr);
            }
            self.swr = std::ptr::null_mut();
        }
        self.swr_key = None;
        if !self.format.is_null() {
            let mut format = self.format;
            unsafe {
                // SAFETY: a caller-supplied custom I/O context is never closed or freed by
                // FFmpeg, so only the format context is released here.
                ffi::avformat_close_input(&mut format);
            }
            self.format = std::ptr::null_mut();
        }
        self.io = None;
        self.live = false;
        self.duration = None;
        self.end_time = None;
    }
}

impl Stream {
    /// Whether FFmpeg has a decoder for this stream's codec.
    fn decodable(&self) -> bool {
        !matches!(self.kind, TrackKind::Subtitle | TrackKind::Other)
    }
}

fn closed() -> MediaError {
    MediaError::new(MediaErrorKind::Closed, "media session is closed")
}

fn free_packet(packet: *mut ffi::AVPacket) {
    let mut packet = packet;
    unsafe {
        // SAFETY: the caller hands over sole ownership of the packet allocation.
        ffi::av_packet_free(&mut packet);
    }
}

fn max_time(current: Option<MediaTime>, value: MediaTime) -> Option<MediaTime> {
    Some(current.map_or(value, |last| last.max(value)))
}

fn slug(kind: &TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "video",
        TrackKind::Audio => "audio",
        TrackKind::Subtitle => "subtitle",
        TrackKind::Other => "other",
    }
}

fn stream_rates(primary: ffi::AVRational, fallback: ffi::AVRational) -> (u32, u32) {
    for rate in [primary, fallback] {
        if rate.num > 0 && rate.den > 0 {
            return (rate.num as u32, rate.den as u32);
        }
    }
    (0, 0)
}

fn aspect(par: *mut ffi::AVCodecParameters) -> (u32, u32) {
    let ratio = unsafe {
        // SAFETY: the parameters pointer came from an open context's stream.
        (*par).sample_aspect_ratio
    };
    if ratio.num > 0 && ratio.den > 0 {
        (ratio.num as u32, ratio.den as u32)
    } else {
        (0, 1)
    }
}
