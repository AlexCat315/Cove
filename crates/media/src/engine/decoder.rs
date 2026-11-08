//! Synchronous demux and decode for callers that own their loop: thumbnails, frame
//! extraction, waveforms, transcoding front ends and probing.
//!
//! A decoder is `Send` but not shared; every `AV*` handle stays with its owner thread.
use super::io::CustomIo;
use super::video::{Converter, FrameStorage, codec_name};
use super::{Abort, c_path, c_str, error, from_micros, from_ns, initialize, source_error, to_ns};
use crate::{
    AudioBuffer, AudioFormat, CancellationToken, DecoderOptions, MediaError, MediaErrorKind,
    MediaInfo, MediaResult, MediaSource, MediaTime, PixelFormat, PlaneLayout, StreamDetails,
    StreamInfo, TrackKind, VideoFormat, VideoFrame, validate_uri,
};
use ffmpeg_sys_next as ffi;
use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::io::Cursor;
use std::sync::Arc;

/// One frame produced by [`MediaDecoder::next_frame`].
#[derive(Clone, Debug)]
pub enum DecodedFrame {
    Video(VideoFrame),
    Audio(AudioBuffer),
}

struct Active {
    index: usize,
    time_base: ffi::AVRational,
    context: *mut ffi::AVCodecContext,
    /// Frames before this time are dropped after a seek.
    discard_before: Option<MediaTime>,
    finished: bool,
}

/// What FFmpeg's interrupt callback consults while a read blocks.
struct Interrupt {
    abort: Arc<Abort>,
    token: CancellationToken,
}

/// Demuxes and decodes one source on the calling thread.
pub struct MediaDecoder {
    options: DecoderOptions,
    info: MediaInfo,
    abort: Arc<Abort>,
    token: CancellationToken,
    /// Boxed so the pointer handed to FFmpeg stays valid when the decoder moves.
    interrupt: Box<Interrupt>,
    format: *mut ffi::AVFormatContext,
    _io: Option<CustomIo>,
    packet: *mut ffi::AVPacket,
    frame: *mut ffi::AVFrame,
    pcm: *mut ffi::AVFrame,
    swr: *mut ffi::SwrContext,
    swr_key: Option<(c_int, c_int, c_int)>,
    video: Option<Active>,
    audio: Option<Active>,
    converter: Converter,
    sequence: u64,
    /// The demuxer reached the end and drain packets were sent.
    draining: bool,
    /// The audio resampler has been flushed after its decoder finished.
    swr_flushed: bool,
}

// SAFETY: every FFmpeg handle is owned exclusively by the decoder and only touched through
// `&mut self`; FFmpeg contexts may move between threads as long as they are not shared.
unsafe impl Send for MediaDecoder {}

impl MediaDecoder {
    /// Opens `source` and the decoders `options` asks for. Blocking: not for the UI thread.
    pub fn open(source: MediaSource, options: DecoderOptions) -> MediaResult<Self> {
        initialize()?;
        if let MediaSource::Uri(uri) = &source {
            validate_uri(uri)?;
        }
        let source = crate::http::route(source)?;
        let abort = Arc::new(Abort::new());
        let token = CancellationToken::default();
        let interrupt = Box::new(Interrupt {
            abort: abort.clone(),
            token: token.clone(),
        });
        let mut decoder = Self {
            options,
            info: MediaInfo::default(),
            abort,
            token,
            interrupt,
            format: std::ptr::null_mut(),
            _io: None,
            packet: std::ptr::null_mut(),
            frame: std::ptr::null_mut(),
            pcm: std::ptr::null_mut(),
            swr: std::ptr::null_mut(),
            swr_key: None,
            video: None,
            audio: None,
            converter: Converter::new(),
            sequence: 0,
            draining: false,
            swr_flushed: false,
        };
        decoder.open_input(source)?;
        decoder.describe();
        if decoder.options.video {
            decoder.video = decoder.open_best(ffi::AVMediaType::AVMEDIA_TYPE_VIDEO)?;
        }
        if decoder.options.audio {
            decoder.audio = decoder.open_best(ffi::AVMediaType::AVMEDIA_TYPE_AUDIO)?;
        }
        decoder.packet = alloc_packet()?;
        decoder.frame = alloc_frame()?;
        Ok(decoder)
    }

    pub fn info(&self) -> &MediaInfo {
        &self.info
    }

    /// Cancelling it makes a blocked read on another thread return `Cancelled`.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }

    /// The format of produced audio, when an audio stream is decoded.
    pub fn audio_format(&self) -> Option<AudioFormat> {
        let active = self.audio.as_ref()?;
        if let Some(format) = self.options.audio_format {
            return Some(format);
        }
        let context = active.context;
        // SAFETY: the context is open for the decoder's lifetime.
        let (rate, channels) =
            unsafe { ((*context).sample_rate, (*context).ch_layout.nb_channels) };
        AudioFormat::new(rate.max(1) as u32, channels.clamp(1, 64) as u16).ok()
    }

    /// Repositions to the keyframe at or before `position`; later frames before it are dropped.
    pub fn seek(&mut self, position: MediaTime) -> MediaResult<()> {
        if !self.info.seekable {
            return Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "source cannot seek",
            ));
        }
        let target = from_ns(
            position,
            ffi::AVRational {
                num: 1,
                den: ffi::AV_TIME_BASE as c_int,
            },
        );
        let status = unsafe {
            // SAFETY: the context is open; a stream index of -1 uses AV_TIME_BASE units.
            ffi::avformat_seek_file(self.format, -1, i64::MIN, target, target, 0)
        };
        if status < 0 {
            return Err(error(MediaErrorKind::Source, status));
        }
        for active in [self.video.as_mut(), self.audio.as_mut()]
            .into_iter()
            .flatten()
        {
            unsafe {
                // SAFETY: the context is open and owned by this decoder.
                ffi::avcodec_flush_buffers(active.context);
            }
            active.discard_before = Some(position);
            active.finished = false;
        }
        if !self.swr.is_null() {
            unsafe {
                // SAFETY: the resampler is owned here; closing drops its buffered samples.
                ffi::swr_close(self.swr);
            }
            self.swr_key = None;
        }
        self.draining = false;
        self.swr_flushed = false;
        Ok(())
    }

    /// The next decoded frame of any requested stream, or `None` at the end.
    pub fn next_frame(&mut self) -> MediaResult<Option<DecodedFrame>> {
        loop {
            self.token.check()?;
            if let Some(frame) = self.receive()? {
                return Ok(Some(frame));
            }
            if self.video.as_ref().is_none_or(|active| active.finished)
                && self.audio.as_ref().is_none_or(|active| active.finished)
            {
                return Ok(None);
            }
            self.read()?;
        }
    }

    /// The next video frame, skipping audio.
    pub fn next_video(&mut self) -> MediaResult<Option<VideoFrame>> {
        if self.video.is_none() {
            return Ok(None);
        }
        while let Some(frame) = self.next_frame()? {
            if let DecodedFrame::Video(frame) = frame {
                return Ok(Some(frame));
            }
        }
        Ok(None)
    }

    /// The next audio buffer, skipping video.
    pub fn next_audio(&mut self) -> MediaResult<Option<AudioBuffer>> {
        if self.audio.is_none() {
            return Ok(None);
        }
        while let Some(frame) = self.next_frame()? {
            if let DecodedFrame::Audio(buffer) = frame {
                return Ok(Some(buffer));
            }
        }
        Ok(None)
    }

    /// The first frame presented at or after `position`.
    pub fn frame_at(&mut self, position: MediaTime) -> MediaResult<Option<VideoFrame>> {
        self.seek(position)?;
        self.next_video()
    }

    /// Decodes the remaining audio into one buffer, stopping at `limit` of media time.
    pub fn read_audio(&mut self, limit: Option<MediaTime>) -> MediaResult<Option<AudioBuffer>> {
        let Some(format) = self.audio_format() else {
            return Ok(None);
        };
        let mut all = AudioBuffer::new(format);
        while let Some(buffer) = self.next_audio()? {
            all.append(&buffer)?;
            if limit.is_some_and(|limit| all.duration() >= limit) {
                break;
            }
        }
        Ok(Some(all))
    }

    fn open_input(&mut self, source: MediaSource) -> MediaResult<()> {
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
            // SAFETY: the opaque pointer addresses the decoder's boxed `Interrupt`, which
            // outlives the format context; the callback only reads atomics.
            (*format).interrupt_callback.callback = Some(interrupted);
            (*format).interrupt_callback.opaque = std::ptr::from_ref::<Interrupt>(&self.interrupt)
                .cast_mut()
                .cast();
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
        if let Some(io) = &io {
            unsafe {
                // SAFETY: the custom context outlives the format context in `self._io`.
                (*format).pb = io.context();
                (*format).flags |= ffi::AVFMT_FLAG_CUSTOM_IO;
            }
        }
        self._io = io;
        let status = unsafe {
            // SAFETY: FFmpeg frees the context and nulls the pointer on failure.
            let url = url.as_ref().map_or(std::ptr::null(), |url| url.as_ptr());
            ffi::avformat_open_input(&mut format, url, std::ptr::null(), std::ptr::null_mut())
        };
        if status < 0 {
            return Err(source_error(status));
        }
        self.format = format;
        let status = unsafe {
            // SAFETY: the context is open.
            ffi::avformat_find_stream_info(format, std::ptr::null_mut())
        };
        if status < 0 {
            return Err(source_error(status));
        }
        Ok(())
    }

    fn describe(&mut self) {
        let format = self.format;
        let (container, duration, bit_rate, seekable, count, metadata) = unsafe {
            // SAFETY: the context is open; `iformat` and its name are static.
            let input = (*format).iformat;
            let pb = (*format).pb;
            (
                if input.is_null() {
                    String::new()
                } else {
                    text((*input).name)
                },
                (*format).duration,
                (*format).bit_rate,
                pb.is_null() || ((*pb).seekable & ffi::AVIO_SEEKABLE_NORMAL) != 0,
                (*format).nb_streams as usize,
                dictionary((*format).metadata),
            )
        };
        let mut streams = Vec::with_capacity(count);
        for index in 0..count {
            streams.push(unsafe {
                // SAFETY: the index is within the open context's stream table.
                describe_stream(*(*format).streams.add(index), index)
            });
        }
        self.info = MediaInfo {
            container,
            duration: (duration > 0).then(|| from_micros(duration)),
            bit_rate: u64::try_from(bit_rate).ok().filter(|rate| *rate > 0),
            seekable,
            metadata,
            streams,
        };
    }

    fn open_best(&mut self, kind: ffi::AVMediaType) -> MediaResult<Option<Active>> {
        let index = unsafe {
            // SAFETY: the context is open; no related stream or decoder output is requested.
            ffi::av_find_best_stream(self.format, kind, -1, -1, std::ptr::null_mut(), 0)
        };
        if index < 0 {
            return Ok(None);
        }
        let stream = unsafe {
            // SAFETY: `av_find_best_stream` returned an index within the stream table.
            *(*self.format).streams.add(index as usize)
        };
        let (par, time_base) = unsafe {
            // SAFETY: every stream of an open context has codec parameters.
            ((*stream).codecpar, (*stream).time_base)
        };
        let codec = unsafe {
            // SAFETY: the codec id comes from the demuxer.
            ffi::avcodec_find_decoder((*par).codec_id)
        };
        if codec.is_null() {
            return Ok(None);
        }
        let mut context = unsafe {
            // SAFETY: allocation with a valid codec returns a context or null.
            ffi::avcodec_alloc_context3(codec)
        };
        if context.is_null() {
            return Err(error(
                MediaErrorKind::Initialization,
                ffi::AVERROR(ffi::ENOMEM),
            ));
        }
        let mut status = unsafe {
            // SAFETY: the context was created for this codec and the parameters are live.
            ffi::avcodec_parameters_to_context(context, par)
        };
        if status >= 0 {
            status = unsafe {
                // SAFETY: the context is uniquely owned; the codec matches it.
                (*context).pkt_timebase = time_base;
                ffi::avcodec_open2(context, codec, std::ptr::null_mut())
            };
        }
        if status < 0 {
            unsafe {
                // SAFETY: the context is still uniquely owned here.
                ffi::avcodec_free_context(&mut context);
            }
            return Err(error(MediaErrorKind::Negotiation, status));
        }
        Ok(Some(Active {
            index: index as usize,
            time_base,
            context,
            discard_before: None,
            finished: false,
        }))
    }

    /// Demuxes one packet into its decoder, or starts draining at the end of input.
    fn read(&mut self) -> MediaResult<()> {
        if self.draining {
            for active in [self.video.as_mut(), self.audio.as_mut()]
                .into_iter()
                .flatten()
            {
                active.finished = true;
            }
            return Ok(());
        }
        let status = unsafe {
            // SAFETY: the context is open and the packet is an owned scratch.
            ffi::av_read_frame(self.format, self.packet)
        };
        if status == ffi::AVERROR_EOF {
            self.draining = true;
            for active in [self.video.as_ref(), self.audio.as_ref()]
                .into_iter()
                .flatten()
            {
                unsafe {
                    // SAFETY: a null packet enters draining mode on an open decoder.
                    ffi::avcodec_send_packet(active.context, std::ptr::null());
                }
            }
            return Ok(());
        }
        if status < 0 {
            if self.abort.aborted() || self.token.is_cancelled() {
                return Err(MediaError::new(
                    MediaErrorKind::Cancelled,
                    "media decode cancelled",
                ));
            }
            return Err(source_error(status));
        }
        let index = unsafe {
            // SAFETY: the packet was just filled by the demuxer.
            (*self.packet).stream_index
        };
        let target = [self.video.as_ref(), self.audio.as_ref()]
            .into_iter()
            .flatten()
            .find(|active| active.index as c_int == index)
            .map(|active| active.context);
        let sent = target.map(|context| unsafe {
            // SAFETY: the decoder is open and the packet is valid; frames are drained
            // before every read, so the decoder accepts input.
            ffi::avcodec_send_packet(context, self.packet)
        });
        unsafe {
            // SAFETY: the packet's payload is released; the scratch is reused.
            ffi::av_packet_unref(self.packet);
        }
        match sent {
            Some(status) if status < 0 && status != ffi::AVERROR_INVALIDDATA => {
                Err(error(MediaErrorKind::Decode, status))
            }
            _ => Ok(()),
        }
    }

    /// Takes a ready frame from the video then the audio decoder.
    fn receive(&mut self) -> MediaResult<Option<DecodedFrame>> {
        for video in [true, false] {
            loop {
                let active = if video {
                    self.video.as_mut()
                } else {
                    self.audio.as_mut()
                };
                let Some(active) = active else { break };
                if active.finished && (video || self.swr_flushed) {
                    break;
                }
                let status = if active.finished {
                    ffi::AVERROR_EOF
                } else {
                    unsafe {
                        // SAFETY: the context is open and the scratch frame is owned here.
                        ffi::avcodec_receive_frame(active.context, self.frame)
                    }
                };
                if status == ffi::AVERROR(ffi::EAGAIN) {
                    break;
                }
                if status == ffi::AVERROR_EOF {
                    active.finished = true;
                    if !video && !self.swr_flushed {
                        self.swr_flushed = true;
                        if let Some(buffer) = self.flush_resampler()? {
                            return Ok(Some(DecodedFrame::Audio(buffer)));
                        }
                    }
                    break;
                }
                if status < 0 {
                    return Err(error(MediaErrorKind::Decode, status));
                }
                let produced = if video {
                    self.take_video()
                } else {
                    self.take_audio()
                };
                unsafe {
                    // SAFETY: the decoded frame is not retained past this point.
                    ffi::av_frame_unref(self.frame);
                }
                if let Some(frame) = produced? {
                    return Ok(Some(frame));
                }
            }
        }
        Ok(None)
    }

    fn take_video(&mut self) -> MediaResult<Option<DecodedFrame>> {
        let Some(active) = self.video.as_mut() else {
            return Ok(None);
        };
        let (stamp, duration) = unsafe {
            // SAFETY: the scratch frame holds a decoded video frame.
            ((*self.frame).best_effort_timestamp, (*self.frame).duration)
        };
        let pts = to_ns(stamp, active.time_base);
        let duration = (duration > 0)
            .then(|| to_ns(duration, active.time_base))
            .flatten();
        if let Some(target) = active.discard_before {
            let stale = pts.is_some_and(|pts| match duration {
                Some(span) => pts.0.saturating_add(span.0) <= target.0,
                None => pts < target,
            });
            if stale {
                return Ok(None);
            }
            active.discard_before = None;
        }
        let stream = self
            .info
            .streams
            .get(active.index)
            .map(|stream| (stream.codec.clone(), stream.details.clone()));
        let image = self
            .converter
            .convert(self.frame, self.options.video_size)?;
        let frame_rate = match stream.as_ref().map(|(_, details)| details) {
            Some(StreamDetails::Video { frame_rate, .. }) => *frame_rate,
            _ => (0, 1),
        };
        let codec = stream.map_or_else(|| "video".to_owned(), |(codec, _)| codec);
        self.sequence += 1;
        let (width, height) = (image.width, image.height);
        Ok(Some(DecodedFrame::Video(VideoFrame {
            generation: 1,
            sequence: self.sequence,
            pts,
            dts: None,
            duration,
            format: VideoFormat {
                width,
                height,
                pixel_format: PixelFormat::Rgba8,
                planes: vec![PlaneLayout {
                    stride: width as usize * 4,
                    offset: 0,
                }],
                pixel_aspect_ratio: image.aspect_ratio,
                frame_rate,
                colorimetry: image.colorimetry.clone(),
                dynamic_range: image.dynamic_range,
                format_description: format!("{codec}, {width}x{height}, {}", image.colorimetry),
            },
            storage: Arc::new(FrameStorage::new(image.bytes)),
        })))
    }

    fn take_audio(&mut self) -> MediaResult<Option<DecodedFrame>> {
        let Some(output) = self.audio_format() else {
            return Ok(None);
        };
        let Some(active) = self.audio.as_mut() else {
            return Ok(None);
        };
        let (stamp, frames, format, rate, layout) = unsafe {
            // SAFETY: the scratch frame holds a decoded audio frame.
            (
                (*self.frame).best_effort_timestamp,
                (*self.frame).nb_samples.max(0),
                (*self.frame).format,
                (*self.frame).sample_rate.max(1),
                (*self.frame).ch_layout.nb_channels.max(1),
            )
        };
        let pts = to_ns(stamp, active.time_base);
        let span = MediaTime((f64::from(frames) / f64::from(rate) * 1e9) as u64);
        if let Some(target) = active.discard_before {
            if pts.is_some_and(|pts| pts.0.saturating_add(span.0) <= target.0) {
                return Ok(None);
            }
            active.discard_before = None;
        }
        let key = (format, rate, layout);
        if self.swr_key != Some(key) {
            self.prepare_resampler(key)?;
        }
        let input = self.frame;
        unsafe {
            // SAFETY: the scratch frame holds a live decoded audio frame.
            super::normalize_layout(input);
        }
        let buffer = self.convert_audio(input, output)?;
        Ok(buffer.map(|mut buffer| {
            buffer.pts = pts;
            DecodedFrame::Audio(buffer)
        }))
    }

    fn flush_resampler(&mut self) -> MediaResult<Option<AudioBuffer>> {
        if self.swr.is_null() || self.swr_key.is_none() {
            return Ok(None);
        }
        let Some(output) = self.audio_format() else {
            return Ok(None);
        };
        self.convert_audio(std::ptr::null_mut(), output)
    }

    fn prepare_resampler(&mut self, key: (c_int, c_int, c_int)) -> MediaResult<()> {
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
                // SAFETY: closing lets `swr_convert_frame` reconfigure from the next frames.
                ffi::swr_close(self.swr);
            }
        }
        if self.pcm.is_null() {
            self.pcm = alloc_frame()?;
        }
        self.swr_key = Some(key);
        Ok(())
    }

    /// Resamples `input` (or flushes with null) into interleaved `f32`.
    fn convert_audio(
        &mut self,
        input: *mut ffi::AVFrame,
        output: AudioFormat,
    ) -> MediaResult<Option<AudioBuffer>> {
        unsafe {
            // SAFETY: the output frame is owned here; `swr_convert_frame` allocates its
            // buffer from the descriptor fields written below.
            ffi::av_frame_unref(self.pcm);
            (*self.pcm).format = ffi::AVSampleFormat::AV_SAMPLE_FMT_FLT as c_int;
            (*self.pcm).sample_rate = output.sample_rate as c_int;
            ffi::av_channel_layout_default(
                &mut (*self.pcm).ch_layout,
                c_int::from(output.channels),
            );
            if input.is_null() {
                // Room for the samples the resampler still buffers.
                (*self.pcm).nb_samples = ffi::swr_get_out_samples(self.swr, 0).max(0);
                if (*self.pcm).nb_samples == 0 {
                    return Ok(None);
                }
                let status = ffi::av_frame_get_buffer(self.pcm, 0);
                if status < 0 {
                    return Err(error(MediaErrorKind::Initialization, status));
                }
            }
        }
        let status = unsafe {
            // SAFETY: the resampler matches the input format or is unconfigured, and both
            // frames outlive the call; a null input drains buffered samples.
            ffi::swr_convert_frame(self.swr, self.pcm, input)
        };
        if status < 0 {
            return Err(error(MediaErrorKind::Decode, status));
        }
        let produced = unsafe {
            // SAFETY: the output frame was just filled.
            (*self.pcm).nb_samples.max(0) as usize
        };
        if produced == 0 {
            return Ok(None);
        }
        let samples = unsafe {
            // SAFETY: packed float output is one plane of `nb_samples * channels` samples.
            std::slice::from_raw_parts(
                (*self.pcm).data[0].cast::<f32>(),
                produced * usize::from(output.channels),
            )
        };
        Ok(Some(AudioBuffer {
            pts: None,
            format: output,
            samples: samples.to_vec(),
        }))
    }
}

impl Drop for MediaDecoder {
    fn drop(&mut self) {
        self.abort.stop();
        for active in [self.video.take(), self.audio.take()].into_iter().flatten() {
            let mut context = active.context;
            unsafe {
                // SAFETY: the codec context is uniquely owned by this decoder.
                ffi::avcodec_free_context(&mut context);
            }
        }
        unsafe {
            // SAFETY: each pointer is null or a uniquely owned allocation; the free functions
            // accept null and null the local copy.
            ffi::av_packet_free(&mut self.packet);
            ffi::av_frame_free(&mut self.frame);
            ffi::av_frame_free(&mut self.pcm);
            ffi::swr_free(&mut self.swr);
            if !self.format.is_null() {
                // A custom I/O context is left alone and freed with `self._io` afterwards.
                ffi::avformat_close_input(&mut self.format);
            }
        }
    }
}

/// Describes `stream` at stream-table position `index`.
///
/// # Safety
/// `stream` must belong to an open format context.
unsafe fn describe_stream(stream: *mut ffi::AVStream, index: usize) -> StreamInfo {
    // SAFETY: guaranteed by the caller; every stream has codec parameters.
    let (par, time_base, duration, metadata, avg_rate, real_rate) = unsafe {
        (
            &*(*stream).codecpar,
            (*stream).time_base,
            (*stream).duration,
            dictionary((*stream).metadata),
            (*stream).avg_frame_rate,
            (*stream).r_frame_rate,
        )
    };
    let kind = match par.codec_type {
        ffi::AVMediaType::AVMEDIA_TYPE_VIDEO => TrackKind::Video,
        ffi::AVMediaType::AVMEDIA_TYPE_AUDIO => TrackKind::Audio,
        ffi::AVMediaType::AVMEDIA_TYPE_SUBTITLE => TrackKind::Subtitle,
        _ => TrackKind::Other,
    };
    let details = match kind {
        TrackKind::Video => StreamDetails::Video {
            width: par.width.max(0) as u32,
            height: par.height.max(0) as u32,
            frame_rate: [avg_rate, real_rate]
                .into_iter()
                .find(|rate| rate.num > 0 && rate.den > 0)
                .map_or((0, 1), |rate| (rate.num as u32, rate.den as u32)),
            pixel_aspect_ratio: if par.sample_aspect_ratio.num > 0 {
                (
                    par.sample_aspect_ratio.num as u32,
                    par.sample_aspect_ratio.den.max(1) as u32,
                )
            } else {
                (1, 1)
            },
            pixel_format: text(unsafe {
                // SAFETY: video parameters hold an `AVPixelFormat` value; the name is static.
                ffi::av_get_pix_fmt_name(std::mem::transmute::<c_int, ffi::AVPixelFormat>(
                    par.format,
                ))
            }),
        },
        TrackKind::Audio => StreamDetails::Audio {
            sample_rate: par.sample_rate.max(0) as u32,
            channels: par.ch_layout.nb_channels.clamp(0, i32::from(u16::MAX)) as u16,
            sample_format: text(unsafe {
                // SAFETY: audio parameters hold an `AVSampleFormat` value; the name is static.
                ffi::av_get_sample_fmt_name(std::mem::transmute::<c_int, ffi::AVSampleFormat>(
                    par.format,
                ))
            }),
        },
        _ => StreamDetails::Other,
    };
    let slug = match kind {
        TrackKind::Video => "video",
        TrackKind::Audio => "audio",
        TrackKind::Subtitle => "subtitle",
        TrackKind::Other => "other",
    };
    let decodable = matches!(kind, TrackKind::Video | TrackKind::Audio)
        && !unsafe {
            // SAFETY: the codec id comes from the demuxer.
            ffi::avcodec_find_decoder(par.codec_id)
        }
        .is_null();
    StreamInfo {
        id: format!("{slug}:{index}"),
        index,
        kind,
        codec: codec_name(par.codec_id),
        decodable,
        duration: (duration > 0).then(|| to_ns(duration, time_base)).flatten(),
        bit_rate: u64::try_from(par.bit_rate).ok().filter(|rate| *rate > 0),
        language: metadata.get("language").cloned(),
        metadata,
        details,
    }
}

fn dictionary(dictionary: *const ffi::AVDictionary) -> BTreeMap<String, String> {
    let mut entries = BTreeMap::new();
    let mut entry: *const ffi::AVDictionaryEntry = std::ptr::null();
    loop {
        entry = unsafe {
            // SAFETY: iterating a dictionary owned by an open context; an empty key with
            // `AV_DICT_IGNORE_SUFFIX` visits every entry.
            ffi::av_dict_get(dictionary, c"".as_ptr(), entry, ffi::AV_DICT_IGNORE_SUFFIX)
        };
        if entry.is_null() {
            return entries;
        }
        let (key, value) = unsafe {
            // SAFETY: entries hold NUL-terminated strings owned by the dictionary.
            (text((*entry).key), text((*entry).value))
        };
        entries.insert(key.to_lowercase(), value);
    }
}

fn text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        return String::new();
    }
    // SAFETY: FFmpeg hands out NUL-terminated strings that outlive this call.
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

fn alloc_packet() -> MediaResult<*mut ffi::AVPacket> {
    let packet = unsafe {
        // SAFETY: allocation returns null when out of memory, checked here.
        ffi::av_packet_alloc()
    };
    if packet.is_null() {
        return Err(error(
            MediaErrorKind::Initialization,
            ffi::AVERROR(ffi::ENOMEM),
        ));
    }
    Ok(packet)
}

fn alloc_frame() -> MediaResult<*mut ffi::AVFrame> {
    let frame = unsafe {
        // SAFETY: allocation returns null when out of memory, checked here.
        ffi::av_frame_alloc()
    };
    if frame.is_null() {
        return Err(error(
            MediaErrorKind::Initialization,
            ffi::AVERROR(ffi::ENOMEM),
        ));
    }
    Ok(frame)
}

unsafe extern "C" fn interrupted(opaque: *mut c_void) -> c_int {
    if opaque.is_null() {
        return 0;
    }
    // SAFETY: the opaque pointer addresses the decoder's live `Interrupt`, read through atomics.
    let interrupt = unsafe { &*(opaque.cast::<Interrupt>()) };
    (interrupt.abort.aborted() || interrupt.token.is_cancelled()) as c_int
}

/// Describes `source` without decoding it.
pub fn probe(source: MediaSource) -> MediaResult<MediaInfo> {
    Ok(MediaDecoder::open(source, DecoderOptions::probe())?
        .info
        .clone())
}
