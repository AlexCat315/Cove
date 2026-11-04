#![cfg(all(feature = "gst", not(target_arch = "wasm32")))]

use cove_media::gst::{self, prelude::*};
use cove_media::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(10);
fn context() -> MediaContext {
    MediaContext::new(ContextOptions::default()).expect("media context")
}
fn result(player: &Player, request: RequestId) -> CommandOutcome {
    let outcome = player
        .wait_result(request, TIMEOUT)
        .unwrap_or_else(|error| {
            panic!(
                "{request:?}: {error}; snapshot={:?}; latest={:?}",
                player.snapshot(),
                player.video_output().latest()
            )
        });
    assert_eq!(
        player.take_result(request).expect("acknowledge command"),
        Some(outcome.clone())
    );
    outcome
}
fn completed(player: &Player, request: RequestId) {
    let outcome = result(player, request);
    assert!(
        matches!(outcome, CommandOutcome::Completed { .. }),
        "{request:?}: {outcome:?}; {:?}",
        player.snapshot()
    );
}
struct Fixture(PathBuf);
impl Fixture {
    fn video() -> Self {
        Self::generate(
            "videotestsrc num-buffers=90 pattern=ball ! video/x-raw,format=BGR,width=32,height=24,framerate=30/1 ! avimux",
            "avi",
        )
    }
    fn audio() -> Self {
        Self::generate(
            "audiotestsrc num-buffers=20 samplesperbuffer=480 ! audio/x-raw,format=S16LE,rate=48000,channels=2 ! wavenc",
            "wav",
        )
    }
    fn generate(source: &str, extension: &str) -> Self {
        Self::pipeline(&format!("{source} ! filesink name=out"), extension)
    }
    fn audiovisual() -> Self {
        Self::pipeline(
            "avimux name=mux ! filesink name=out \
            videotestsrc num-buffers=60 ! video/x-raw,format=BGR,width=32,height=24,framerate=30/1 ! queue ! mux. \
            audiotestsrc num-buffers=200 samplesperbuffer=480 freq=220 ! audio/x-raw,format=S16LE,rate=48000,channels=1 ! queue ! mux. \
            audiotestsrc num-buffers=200 samplesperbuffer=480 freq=440 ! audio/x-raw,format=S16LE,rate=48000,channels=1 ! queue ! mux.",
            "avi",
        )
    }
    fn pipeline(expression: &str, extension: &str) -> Self {
        gst::initialize().expect("GStreamer initialization");
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "cove-media-{}-{}.{extension}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let pipeline = gst::parse::launch(expression)
            .expect("fixture pipeline")
            .downcast::<gst::Pipeline>()
            .expect("pipeline");
        pipeline
            .by_name("out")
            .expect("file sink")
            .set_property("location", path.to_str().expect("temporary path"));
        pipeline
            .set_state(gst::State::Playing)
            .expect("fixture playing");
        let message = pipeline
            .bus()
            .expect("fixture bus")
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(10),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .expect("fixture completion");
        pipeline
            .set_state(gst::State::Null)
            .expect("fixture stopped");
        if let gst::MessageView::Error(error) = message.view() {
            panic!("fixture: {} {:?}", error.error(), error.debug());
        }
        Self(path)
    }
}

#[test]
fn multitrack_audiovisual_source_can_select_audio_only_and_still_seek() {
    let fixture = Fixture::audiovisual();
    for source in [
        MediaSource::file(&fixture.0),
        MediaSource::bytes(std::fs::read(&fixture.0).expect("encoded AV")),
    ] {
        let context = context();
        let player = context
            .create_player(PlayerOptions {
                audio: AudioOutput::Silent,
                ..PlayerOptions::default()
            })
            .expect("player");
        completed(&player, player.open(source).expect("open AV"));
        let snapshot = player.snapshot();
        assert!(
            snapshot
                .tracks
                .iter()
                .any(|track| track.kind == TrackKind::Video)
        );
        let audio: Vec<_> = snapshot
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Audio)
            .collect();
        assert_eq!(audio.len(), 2);
        assert!(player.video_output().latest().is_some());
        let selected = vec![audio[1].id.clone()];
        completed(
            &player,
            player
                .select_tracks(selected.clone())
                .expect("select second audio only"),
        );
        assert_eq!(player.snapshot().selected_tracks, selected);
        completed(
            &player,
            player
                .seek(SeekRequest::exact(MediaTime(500_000_000)))
                .expect("audio seek in AV container"),
        );
        assert!(!player.snapshot().seeking);
        assert!(player.video_output().latest().is_none());
        let video = snapshot
            .tracks
            .iter()
            .find(|track| track.kind == TrackKind::Video)
            .expect("video");
        let restored = vec![video.id.clone(), audio[0].id.clone()];
        completed(
            &player,
            player.select_tracks(restored.clone()).expect("restore AV"),
        );
        assert!(
            restored
                .iter()
                .all(|id| player.snapshot().selected_tracks.contains(id))
        );
        assert!(player.video_output().latest().is_some());
        assert_eq!(player.snapshot().state, PlaybackState::Paused);
        completed(&player, player.play().expect("play AV"));
        completed(
            &player,
            player
                .select_tracks(selected.clone())
                .expect("select audio while playing"),
        );
        assert_eq!(player.snapshot().selected_tracks, selected);
        completed(
            &player,
            player
                .select_tracks(restored)
                .expect("restore AV while playing"),
        );
        assert_eq!(player.snapshot().state, PlaybackState::Playing);
        let sequence = player
            .video_output()
            .latest()
            .expect("restored frame")
            .sequence;
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while player
            .video_output()
            .latest()
            .is_none_or(|frame| frame.sequence <= sequence)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "playing video did not advance: {:?}",
                player.snapshot()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        completed(&player, player.close().expect("close"));
        context.shutdown();
        context.wait_closed(TIMEOUT).expect("shutdown");
    }
}

#[test]
fn audio_only_seek_track_selection_and_eos_need_no_gpu() {
    let fixture = Fixture::audio();
    let context = context();
    let player = context
        .create_player(PlayerOptions {
            audio: AudioOutput::Silent,
            ..PlayerOptions::default()
        })
        .expect("player");
    completed(
        &player,
        player.open(MediaSource::file(&fixture.0)).expect("open"),
    );
    assert!(player.video_output().latest().is_none());
    let snapshot = player.snapshot();
    assert!(!snapshot.tracks.is_empty());
    assert!(
        snapshot
            .tracks
            .iter()
            .all(|track| track.kind == TrackKind::Audio)
    );
    completed(
        &player,
        player
            .select_tracks(snapshot.selected_tracks)
            .expect("select audio"),
    );
    completed(&player, player.set_volume(0.4).expect("volume"));
    assert_eq!(player.snapshot().volume, 0.4);
    completed(&player, player.set_rate(1.25).expect("rate"));
    completed(
        &player,
        player
            .seek(SeekRequest::exact(MediaTime(50_000_000)))
            .expect("seek"),
    );
    completed(&player, player.play().expect("play"));
    let deadline = std::time::Instant::now() + TIMEOUT;
    while player.snapshot().state != PlaybackState::Ended {
        assert!(
            std::time::Instant::now() < deadline,
            "EOS timeout: {:?}",
            player.snapshot()
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(player.video_output().latest().is_none());
    let events = player.events_since(0);
    assert!(events.events.iter().any(|event| matches!(&event.kind,
        PlayerEventKind::SnapshotChanged(state) if state.state == PlaybackState::Ended)));
    completed(&player, player.close().expect("close"));
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("shutdown");
}

#[test]
fn slow_event_consumers_detect_loss_without_losing_request_results() {
    let context = MediaContext::new(ContextOptions {
        event_capacity: 2,
        ..ContextOptions::default()
    })
    .expect("context");
    let player = context
        .create_player(PlayerOptions::default())
        .expect("player");
    let first = player.pause().expect("accepted");
    player.wait_result(first, TIMEOUT).expect("first terminal");
    let cursor = player.events_since(0).next_cursor;
    let mut requests = vec![first];
    for _ in 0..4 {
        let request = player.pause().expect("accepted");
        player.wait_result(request, TIMEOUT).expect("terminal");
        requests.push(request);
    }
    let batch = player.events_since(cursor);
    assert_eq!(batch.events.len(), 2);
    assert_eq!(batch.missed, 2);
    for request in requests {
        assert!(
            player
                .take_result(request)
                .expect("durable result")
                .is_some()
        );
    }
    assert!(player.events_since(batch.next_cursor).events.is_empty());
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("shutdown");
}

#[test]
fn last_player_drop_closes_the_output_without_waiting_for_the_worker() {
    let context = context();
    let player = context
        .create_player(PlayerOptions::default())
        .expect("player");
    let output = player.video_output();
    let subscription = output.subscribe();
    assert!(subscription.try_changed());
    drop(player);
    subscription
        .wait_changed(TIMEOUT)
        .expect("closed notification");
    assert!(output.is_closed());
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("shutdown");
}

#[test]
fn an_open_deadline_cancels_a_cooperative_blocked_input() {
    struct BlockedInput(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl MediaInput for BlockedInput {
        fn length(&self) -> Option<u64> {
            None
        }
        fn is_seekable(&self) -> bool {
            false
        }
        fn read(&mut self, _: &mut [u8], cancellation: &CancellationToken) -> MediaResult<usize> {
            self.0.store(true, Ordering::Release);
            while !cancellation.is_cancelled() {
                std::thread::sleep(Duration::from_millis(1));
            }
            cancellation.check()?;
            Ok(0)
        }
        fn seek(&mut self, _: u64, _: &CancellationToken) -> MediaResult<()> {
            Err(MediaError::new(
                MediaErrorKind::Unsupported,
                "streaming input",
            ))
        }
    }
    let context = MediaContext::new(ContextOptions {
        operation_timeout: Duration::from_millis(100),
        ..ContextOptions::default()
    })
    .expect("context");
    let player = context
        .create_player(PlayerOptions {
            audio: AudioOutput::Silent,
            ..PlayerOptions::default()
        })
        .expect("player");
    let reading = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let request = player
        .open(MediaSource::Input(Box::new(BlockedInput(reading.clone()))))
        .expect("accepted");
    assert!(matches!(
        result(&player, request),
        CommandOutcome::Failed(MediaError {
            kind: MediaErrorKind::Timeout,
            ..
        })
    ));
    assert!(reading.load(Ordering::Acquire));
    assert_eq!(player.snapshot().state, PlaybackState::Failed);
    completed(&player, player.close().expect("close"));
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("shutdown");
}

#[test]
fn context_shutdown_finishes_every_accepted_queued_request() {
    let context = context();
    let player = context
        .create_player(PlayerOptions::default())
        .expect("player");
    let requests: Vec<_> = (0..32).map(|_| player.pause().expect("accepted")).collect();
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("shutdown");
    for request in requests {
        assert!(matches!(
            result(&player, request),
            CommandOutcome::Failed(_)
        ));
    }
    assert!(player.video_output().is_closed());
    assert_eq!(
        player.pause().expect_err("closed context").kind,
        MediaErrorKind::Closed
    );
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn file_memory_playback_seek_and_retained_frame() {
    let fixture = Fixture::video();
    let context = context();
    let player = context
        .create_player(PlayerOptions {
            autoplay: false,
            audio: AudioOutput::Silent,
        })
        .expect("player");
    completed(&player, player.set_volume(0.6).expect("volume before open"));
    assert_eq!(
        player
            .seek(SeekRequest::exact(MediaTime(u64::MAX)))
            .expect_err("unrepresentable seek")
            .kind,
        MediaErrorKind::InvalidInput
    );
    completed(
        &player,
        player
            .open(MediaSource::file(&fixture.0))
            .expect("open accepted"),
    );
    assert_eq!(player.snapshot().state, PlaybackState::Paused);
    assert_eq!(player.snapshot().volume, 0.6);
    let old = player.video_output().latest().expect("preroll frame");
    assert_eq!((old.format.width, old.format.height), (32, 24));
    assert_eq!(old.format.pixel_format, PixelFormat::Rgba8);
    let pixels = old.storage.copy_planes().expect("first pixels");
    let first = player
        .seek(SeekRequest::exact(MediaTime(1_200_000_000)))
        .expect("first seek");
    let second = player
        .seek(SeekRequest::exact(MediaTime(400_000_000)))
        .expect("second seek");
    assert!(matches!(
        result(&player, first),
        CommandOutcome::Superseded | CommandOutcome::Completed { .. }
    ));
    let second_result = result(&player, second);
    let CommandOutcome::Completed {
        position: Some(position),
    } = second_result
    else {
        panic!("{second_result:?}")
    };
    assert!(
        (400_000_000..450_000_000).contains(&position.0),
        "{position:?}"
    );
    let current = player.video_output().latest().expect("seek output");
    assert!(current.generation > old.generation);
    assert!(player.snapshot().revision > 1);
    completed(&player, player.play().expect("play"));
    completed(&player, player.pause().expect("pause"));
    let bytes = std::fs::read(&fixture.0).expect("encoded bytes");
    completed(
        &player,
        player.open(MediaSource::bytes(bytes)).expect("memory open"),
    );
    assert_eq!(player.snapshot().volume, 0.6);
    completed(
        &player,
        player
            .seek(SeekRequest::exact(MediaTime(500_000_000)))
            .expect("memory seek"),
    );
    let closing = player.close().expect("close");
    assert_eq!(
        player
            .pause()
            .expect_err("closing session rejects commands")
            .kind,
        MediaErrorKind::Closed
    );
    completed(&player, closing);
    assert!(player.video_output().is_closed());
    assert_eq!(player.snapshot().state, PlaybackState::Closed);
    assert_eq!(
        old.storage.copy_planes().expect("lease after close"),
        pixels
    );
    completed(&player, player.close().expect("idempotent close"));
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("worker shutdown");
}

#[test]
fn failed_open_is_reported_and_the_session_can_recover() {
    let fixture = Fixture::video();
    let context = context();
    let player = context
        .create_player(PlayerOptions {
            audio: AudioOutput::Silent,
            ..PlayerOptions::default()
        })
        .expect("player");
    let failure = result(
        &player,
        player
            .open(MediaSource::file(fixture.0.with_extension("missing")))
            .expect("accepted"),
    );
    assert!(matches!(
        failure,
        CommandOutcome::Failed(MediaError {
            kind: MediaErrorKind::Source,
            ..
        })
    ));
    assert_eq!(player.snapshot().state, PlaybackState::Failed);
    let invalid = player
        .open(MediaSource::Uri("file:\0invalid".into()))
        .expect("accepted validation");
    assert!(matches!(
        result(&player, invalid),
        CommandOutcome::Failed(MediaError {
            kind: MediaErrorKind::InvalidInput,
            ..
        })
    ));
    assert!(!context.has_element("bad\0factory"));
    completed(
        &player,
        player.open(MediaSource::file(&fixture.0)).expect("reopen"),
    );
    completed(&player, player.close().expect("close"));
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("shutdown");
}

#[test]
fn request_results_are_bounded_and_only_acknowledgement_releases_capacity() {
    let context = MediaContext::new(ContextOptions {
        command_capacity: 8,
        result_capacity: 1,
        ..ContextOptions::default()
    })
    .expect("context");
    let player = context
        .create_player(PlayerOptions::default())
        .expect("player");
    let first = player.pause().expect("accepted");
    assert_eq!(
        player.pause().expect_err("result table full").kind,
        MediaErrorKind::Busy
    );
    assert!(matches!(
        player.wait_result(first, TIMEOUT).expect("terminal"),
        CommandOutcome::Failed(_)
    ));
    assert_eq!(
        player
            .pause()
            .expect_err("unacknowledged terminal result")
            .kind,
        MediaErrorKind::Busy
    );
    assert!(player.take_result(first).expect("acknowledge").is_some());
    let second = player.pause().expect("capacity released");
    assert!(matches!(result(&player, second), CommandOutcome::Failed(_)));
    context.shutdown();
    context.wait_closed(TIMEOUT).expect("shutdown");
}

#[test]
fn application_owned_pipeline_uses_the_same_retained_output_endpoint() {
    let output = VideoOutput::new();
    let endpoint = gst::GstVideoEndpoint::new(output.clone()).expect("endpoint");
    let pipeline = gst::Pipeline::new();
    let source = gst::ElementFactory::make("videotestsrc")
        .property("num-buffers", 3_i32)
        .build()
        .expect("source");
    pipeline
        .add_many([&source, endpoint.element()])
        .expect("elements");
    source.link(endpoint.element()).expect("link");
    pipeline.set_state(gst::State::Playing).expect("playing");
    let message = pipeline
        .bus()
        .expect("bus")
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("completion");
    pipeline.set_state(gst::State::Null).expect("stopped");
    if let gst::MessageView::Error(error) = message.view() {
        panic!("{} {:?}", error.error(), error.debug());
    }
    let frame = output.latest().expect("last frame");
    assert_eq!(frame.sequence, 4); // preroll plus the three presentation samples
    assert!(
        !frame
            .storage
            .copy_planes()
            .expect("retained sample")
            .is_empty()
    );
    assert!(
        frame
            .storage
            .as_any()
            .downcast_ref::<gst::SampleStorage>()
            .is_some()
    );
}
