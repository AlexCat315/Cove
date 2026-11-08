#![cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]
//! Low-level decode, probe, control and metering against synthetic in-memory media.
use cove_media::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A mono 16-bit PCM WAV of a 440 Hz tone.
fn wav(sample_rate: u32, frames: u32) -> Arc<[u8]> {
    let data = frames * 2;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for i in 0..frames {
        let phase = i as f32 / sample_rate as f32 * 440.0 * std::f32::consts::TAU;
        let sample = (phase.sin() * 0.5 * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes.into()
}

/// A 4:2:0 Y4M clip whose luma steps by 40 per frame.
fn y4m(width: usize, height: usize, frames: usize, rate: u32) -> Arc<[u8]> {
    let mut bytes =
        format!("YUV4MPEG2 W{width} H{height} F{rate}:1 Ip A1:1 C420jpeg\n").into_bytes();
    for frame in 0..frames {
        bytes.extend_from_slice(b"FRAME\n");
        bytes.extend(std::iter::repeat_n((16 + frame * 40) as u8, width * height));
        bytes.extend(std::iter::repeat_n(128u8, width * height / 2));
    }
    bytes.into()
}

#[test]
fn probe_describes_streams_and_container() {
    let info = probe(MediaSource::bytes(wav(8000, 4000))).expect("probe");
    assert_eq!(info.container, "wav");
    assert!(info.seekable);
    let audio = info.first(TrackKind::Audio).expect("audio stream");
    assert_eq!(audio.id, "audio:0");
    assert!(audio.decodable);
    assert_eq!(
        audio.details,
        StreamDetails::Audio {
            sample_rate: 8000,
            channels: 1,
            sample_format: "s16".into()
        }
    );
    let duration = info.duration.expect("duration").seconds();
    assert!((duration - 0.5).abs() < 0.01, "{duration}");
}

#[test]
fn audio_decodes_and_resamples_to_interleaved_float() {
    let mut decoder = MediaDecoder::open(
        MediaSource::bytes(wav(8000, 4000)),
        DecoderOptions::audio_only(),
    )
    .expect("open");
    assert_eq!(decoder.audio_format(), AudioFormat::new(8000, 1).ok());
    let buffer = decoder.read_audio(None).expect("decode").expect("audio");
    assert_eq!(buffer.frames(), 4000);
    let peaks = buffer.peaks(10);
    assert!(peaks.iter().all(|peak| peak.max > 0.45 && peak.min < -0.45));
    assert!(peaks.iter().all(|peak| (peak.rms - 0.354).abs() < 0.02));

    let target = AudioFormat::new(16_000, 2).expect("format");
    let mut decoder = MediaDecoder::open(
        MediaSource::bytes(wav(8000, 4000)),
        DecoderOptions {
            audio_format: Some(target),
            ..DecoderOptions::audio_only()
        },
    )
    .expect("open");
    let buffer = decoder.read_audio(None).expect("decode").expect("audio");
    assert_eq!(buffer.format, target);
    let frames = buffer.frames() as i64;
    assert!(
        (frames - 8000).abs() <= 64,
        "{frames} frames after resampling"
    );
    assert_eq!(
        buffer.samples[200], buffer.samples[201],
        "mono is duplicated"
    );
}

#[test]
fn video_frames_are_rgba_timed_scaled_and_seekable() {
    let source = y4m(16, 8, 5, 5);
    let mut decoder = MediaDecoder::open(
        MediaSource::bytes(source.clone()),
        DecoderOptions::video_only(),
    )
    .expect("open");
    let info = decoder
        .info()
        .first(TrackKind::Video)
        .expect("video")
        .clone();
    assert!(matches!(
        info.details,
        StreamDetails::Video {
            width: 16,
            height: 8,
            frame_rate: (5, 1),
            ..
        }
    ));
    let mut frames = Vec::new();
    while let Some(frame) = decoder.next_video().expect("decode") {
        frames.push(frame);
    }
    assert_eq!(frames.len(), 5);
    assert_eq!(frames[3].pts, Some(MediaTime(600_000_000)));
    let pixels = frames[2].rgba_pixels().expect("rgba");
    assert_eq!(pixels.len(), 16 * 8 * 4);
    assert!(
        frames[2].storage.shared_plane(0).is_some(),
        "zero-copy plane"
    );
    assert!(pixels[0] > frames[0].rgba_pixels().expect("rgba")[0]);

    let mut scaled = MediaDecoder::open(
        MediaSource::bytes(source),
        DecoderOptions {
            video_size: Some((8, 8)),
            ..DecoderOptions::video_only()
        },
    )
    .expect("open");
    let frame = scaled
        .frame_at(MediaTime(600_000_000))
        .expect("seek")
        .expect("frame");
    assert_eq!((frame.format.width, frame.format.height), (8, 4));
    assert!(frame.pts.expect("pts") >= MediaTime(600_000_000));
}

#[test]
fn cancellation_stops_decoding() {
    let mut decoder = MediaDecoder::open(
        MediaSource::bytes(y4m(16, 8, 5, 5)),
        DecoderOptions::default(),
    )
    .expect("open");
    decoder.cancellation_token().cancel();
    let error = decoder.next_frame().expect_err("cancelled");
    assert_eq!(error.kind, MediaErrorKind::Cancelled);
}

#[test]
fn playback_control_detaches_results_and_notifies() {
    let context = MediaContext::new(ContextOptions {
        result_capacity: 4,
        ..ContextOptions::default()
    })
    .expect("context");
    let player = context
        .create_player(PlayerOptions {
            audio: AudioOutput::Silent,
            ..PlayerOptions::default()
        })
        .expect("player");
    let changes = Arc::new(AtomicUsize::new(0));
    let counter = changes.clone();
    let listener = player.on_change(Arc::new(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    }));
    let frames = Arc::new(AtomicUsize::new(0));
    let frame_counter = frames.clone();
    let _frames = player.video_output().on_change(Arc::new(move || {
        frame_counter.fetch_add(1, Ordering::Relaxed);
    }));
    let open = player
        .open(MediaSource::bytes(y4m(16, 8, 5, 5)))
        .expect("open");
    let outcome = player
        .wait_result(open, Duration::from_secs(10))
        .expect("opened");
    assert!(
        matches!(outcome, CommandOutcome::Completed { .. }),
        "{outcome:?}"
    );
    player.take_result(open).expect("ack");

    let control: Arc<dyn PlaybackControl> = Arc::new(player.clone());
    for _ in 0..16 {
        control
            .pause()
            .expect("detached commands never exhaust capacity");
    }
    control.play().expect("play");
    let deadline = Instant::now() + Duration::from_secs(10);
    while control.snapshot().state != PlaybackState::Ended {
        assert!(Instant::now() < deadline, "{:?}", control.snapshot());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(changes.load(Ordering::Relaxed) > 0);
    assert!(frames.load(Ordering::Relaxed) >= 2);
    assert!(control.video_output().expect("video").latest().is_some());
    assert!(control.audio_meter().is_some());
    drop(listener);
    context.shutdown();
    context.wait_closed(Duration::from_secs(5)).expect("closed");
}

#[test]
fn audio_stream_renders_application_samples_when_a_device_exists() {
    let devices = audio_output_devices().unwrap_or_default();
    for device in &devices {
        assert!(!device.id.is_empty());
    }
    let mut phase = 0.0f32;
    let stream = match AudioStream::open(
        AudioStreamOptions::default(),
        Box::new(move |data, format| {
            for frame in data.chunks_mut(usize::from(format.channels)) {
                let sample = (phase * std::f32::consts::TAU).sin() * 0.1;
                frame.fill(sample);
                phase = (phase + 440.0 / format.sample_rate as f32).fract();
            }
        }),
    ) {
        Ok(stream) => stream,
        Err(error) if error.kind == MediaErrorKind::Device => return,
        Err(error) => panic!("{error}"),
    };
    assert!(!stream.is_playing());
    stream.play();
    let meter = stream.meter();
    let deadline = Instant::now() + Duration::from_secs(2);
    while meter.levels().max_peak() < 0.05 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    stream.pause();
    // Headless hosts may accept the stream without ever running callbacks.
    if meter.revision() > 0 {
        let mut history = Vec::new();
        assert_eq!(meter.history(&mut history), Some(stream.format()));
    }
}

#[test]
fn player_resamples_unspecified_layouts_and_reports_true_duration() {
    let context = MediaContext::new(ContextOptions::default()).expect("context");
    let player = context
        .create_player(PlayerOptions {
            autoplay: true,
            audio: AudioOutput::Silent,
        })
        .expect("player");
    let open = player
        .open(MediaSource::bytes(wav(8000, 2000)))
        .expect("open");
    let outcome = player
        .wait_result(open, Duration::from_secs(10))
        .expect("opened");
    assert!(
        matches!(outcome, CommandOutcome::Completed { .. }),
        "{outcome:?}"
    );
    assert_eq!(player.snapshot().duration, Some(MediaTime(250_000_000)));
    let deadline = Instant::now() + Duration::from_secs(10);
    while player.snapshot().state != PlaybackState::Ended {
        let snapshot = player.snapshot();
        assert!(snapshot.last_error.is_none(), "{snapshot:?}");
        assert!(Instant::now() < deadline, "{snapshot:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
    context.shutdown();
    context.wait_closed(Duration::from_secs(5)).expect("closed");
}

/// Real codecs: H.264 video and AAC audio from the gallery sample clip.
#[test]
fn compressed_sample_decodes_video_and_audio() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sample.mp4");
    let info = probe(MediaSource::file(&path)).expect("probe");
    assert_eq!(info.title(), Some("Test Pattern"));
    assert_eq!(info.first(TrackKind::Video).expect("video").codec, "h264");
    assert_eq!(info.first(TrackKind::Audio).expect("audio").codec, "aac");
    let mut decoder = MediaDecoder::open(
        MediaSource::file(&path),
        DecoderOptions {
            audio_format: AudioFormat::new(8000, 1).ok(),
            ..DecoderOptions::audio_only()
        },
    )
    .expect("open");
    let buffer = decoder.read_audio(None).expect("decode").expect("audio");
    let seconds = buffer.duration().seconds();
    assert!((seconds - 8.0).abs() < 0.2, "{seconds} s of audio");
    let peaks = buffer.peaks(32);
    let loudest = peaks.iter().map(|peak| peak.max).fold(0.0, f32::max);
    let quietest = peaks.iter().map(|peak| peak.max).fold(1.0, f32::min);
    assert!((0.4..=1.0).contains(&loudest), "{peaks:?}");
    assert!(quietest < 0.2, "the tone is amplitude-modulated: {peaks:?}");
    let mut video = MediaDecoder::open(
        MediaSource::file(&path),
        DecoderOptions {
            video_size: Some((96, 96)),
            ..DecoderOptions::video_only()
        },
    )
    .expect("open");
    let frame = video
        .frame_at(MediaTime::from_seconds(4.0).expect("time"))
        .expect("seek")
        .expect("frame");
    assert_eq!((frame.format.width, frame.format.height), (96, 54));
    assert!(frame.pts.expect("pts").seconds() >= 3.99);
}

#[test]
fn seeking_after_the_end_repositions_and_pauses() {
    let context = MediaContext::new(ContextOptions::default()).expect("context");
    let player = context
        .create_player(PlayerOptions {
            autoplay: true,
            audio: AudioOutput::Silent,
        })
        .expect("player");
    let open = player
        .open(MediaSource::bytes(y4m(16, 8, 5, 5)))
        .expect("open");
    player
        .wait_result(open, Duration::from_secs(10))
        .expect("opened");
    let deadline = Instant::now() + Duration::from_secs(10);
    while player.snapshot().state != PlaybackState::Ended {
        assert!(Instant::now() < deadline, "{:?}", player.snapshot());
        std::thread::sleep(Duration::from_millis(10));
    }
    let seek = player
        .seek(SeekRequest::exact(MediaTime(400_000_000)))
        .expect("seek");
    let outcome = player
        .wait_result(seek, Duration::from_secs(10))
        .expect("seeked");
    assert!(
        matches!(outcome, CommandOutcome::Completed { .. }),
        "{outcome:?}"
    );
    std::thread::sleep(Duration::from_millis(120));
    let snapshot = player.snapshot();
    assert_eq!(snapshot.state, PlaybackState::Paused, "{snapshot:?}");
    assert_eq!(
        snapshot.position,
        Some(MediaTime(400_000_000)),
        "{snapshot:?}"
    );
    context.shutdown();
    context.wait_closed(Duration::from_secs(5)).expect("closed");
}

#[test]
#[ignore = "diagnostic: set COVE_MEDIA_PROBE to a media file"]
fn diagnostic_device_playback_timeline() {
    let Ok(path) = std::env::var("COVE_MEDIA_PROBE") else {
        return;
    };
    let context = MediaContext::new(ContextOptions::default()).expect("context");
    let player = context
        .create_player(PlayerOptions {
            autoplay: true,
            audio: AudioOutput::DefaultDevice,
        })
        .expect("player");
    let open = player.open(MediaSource::file(path)).expect("open");
    player.wait_result(open, Duration::from_secs(10)).expect("opened");
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(24) {
        let snapshot = player.snapshot();
        let frame = player.video_output().latest().and_then(|frame| frame.pts);
        println!(
            "{:5.2}s state={:?} position={:?} frame={:?}",
            start.elapsed().as_secs_f64(),
            snapshot.state,
            snapshot.position.map(MediaTime::seconds),
            frame.map(MediaTime::seconds)
        );
        if snapshot.state == PlaybackState::Ended {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// More packets than one stream's queue holds must be deferred, never dropped.
#[test]
fn read_ahead_beyond_the_queue_limit_keeps_every_frame() {
    let context = MediaContext::new(ContextOptions::default()).expect("context");
    let player = context
        .create_player(PlayerOptions {
            autoplay: true,
            audio: AudioOutput::Silent,
        })
        .expect("player");
    let output = player.video_output();
    let seen = Arc::new(std::sync::Mutex::new(Vec::<f64>::new()));
    let record = seen.clone();
    let watched = output.clone();
    let _listener = output.on_change(Arc::new(move || {
        if let Some(pts) = watched.latest().and_then(|frame| frame.pts) {
            record.lock().unwrap().push(pts.seconds());
        }
    }));
    let open = player
        .open(MediaSource::bytes(y4m(16, 8, 700, 200)))
        .expect("open");
    player.wait_result(open, Duration::from_secs(10)).expect("opened");
    let deadline = Instant::now() + Duration::from_secs(15);
    while player.snapshot().state != PlaybackState::Ended {
        assert!(Instant::now() < deadline, "{:?}", player.snapshot());
        std::thread::sleep(Duration::from_millis(20));
    }
    let seen = seen.lock().unwrap().clone();
    let last = seen.iter().copied().fold(0.0, f64::max);
    assert!(last > 3.4, "the clip plays to its last frame, got {last}");
    let largest_gap = seen
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .fold(0.0, f64::max);
    assert!(largest_gap < 0.25, "frames were lost: gap {largest_gap} s");
    context.shutdown();
    context.wait_closed(Duration::from_secs(5)).expect("closed");
}
