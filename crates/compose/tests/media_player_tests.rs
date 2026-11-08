//! Video surface, transport, level meter and waveform against a fake control.
//!
//! Behaviour checklist:
//! - the surface draws the newest frame under one stable texture id, letterboxed
//! - a frame published on another thread marks the surface dirty through the UI inbox
//! - the transport plays, pauses, restarts after the end and mutes through the control
//! - the scrubber seeks and stays disabled without a duration
//! - the waveform seeks to the pointer fraction
//! - the audio player recomposes when the control reports a change

use std::sync::{Arc, Mutex};

use crate::support::{self, InteractiveScene};
use cove_composition::AnyView;
use cove_core::scene::{AccessibilityRole, Node};
use cove_core::{DrawList, Point};
use cove_media::{
    AudioMeter, AudioPeak, ChangeCallback, ChangeListener, ChangeNotifier, MediaResult, MediaTime,
    PixelFormat, PlaneLayout, PlaybackControl, PlaybackState, PlayerSnapshot, SeekRequest,
    SharedPlanes, VideoDynamicRange, VideoFormat, VideoFrame, VideoOutput,
};
use cove_ui_compose::modifiers::layout::LayoutFrameExt;
use cove_ui_compose::primitives::media::{
    AudioPlayer, MediaTransport, VideoView, Waveform, format_media_time,
};

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Play,
    Pause,
    Seek(SeekRequest),
    Volume(f64),
}

struct FakeControl {
    snapshot: Mutex<PlayerSnapshot>,
    calls: Mutex<Vec<Call>>,
    output: VideoOutput,
    meter: AudioMeter,
    notifier: ChangeNotifier,
}

impl FakeControl {
    fn new(state: PlaybackState, duration: Option<MediaTime>) -> Arc<Self> {
        Arc::new(Self {
            snapshot: Mutex::new(PlayerSnapshot {
                state,
                duration,
                position: Some(MediaTime(0)),
                seekable_range: duration.map(|d| (MediaTime(0), Some(d))),
                ..PlayerSnapshot::default()
            }),
            calls: Mutex::new(Vec::new()),
            output: VideoOutput::new(),
            meter: AudioMeter::default(),
            notifier: ChangeNotifier::new(),
        })
    }
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    fn change(&self, change: impl FnOnce(&mut PlayerSnapshot)) {
        change(&mut self.snapshot.lock().unwrap());
        self.notifier.notify();
    }
    fn record(&self, call: Call) -> MediaResult<()> {
        self.calls.lock().unwrap().push(call);
        Ok(())
    }
}

impl PlaybackControl for FakeControl {
    fn snapshot(&self) -> PlayerSnapshot {
        self.snapshot.lock().unwrap().clone()
    }
    fn video_output(&self) -> Option<VideoOutput> {
        Some(self.output.clone())
    }
    fn audio_meter(&self) -> Option<AudioMeter> {
        Some(self.meter.clone())
    }
    fn play(&self) -> MediaResult<()> {
        self.record(Call::Play)
    }
    fn pause(&self) -> MediaResult<()> {
        self.record(Call::Pause)
    }
    fn seek(&self, request: SeekRequest) -> MediaResult<()> {
        self.record(Call::Seek(request))
    }
    fn set_volume(&self, volume: f64) -> MediaResult<()> {
        self.record(Call::Volume(volume))
    }
    fn set_rate(&self, _rate: f64) -> MediaResult<()> {
        Ok(())
    }
    fn on_change(&self, callback: ChangeCallback) -> ChangeListener {
        self.notifier.listen(callback)
    }
}

fn frame(output: &VideoOutput, generation: u64, sequence: u64, width: u32, height: u32) {
    let pixels: Arc<[u8]> = vec![200u8; (width * height * 4) as usize].into();
    assert!(output.publish(VideoFrame {
        generation,
        sequence,
        pts: Some(MediaTime(sequence * 40_000_000)),
        dts: None,
        duration: None,
        format: VideoFormat {
            width,
            height,
            pixel_format: PixelFormat::Rgba8,
            planes: vec![PlaneLayout {
                stride: width as usize * 4,
                offset: 0,
            }],
            pixel_aspect_ratio: (1, 1),
            frame_rate: (25, 1),
            colorimetry: "unknown".into(),
            dynamic_range: VideoDynamicRange::Sdr,
            format_description: "test".into(),
        },
        storage: Arc::new(SharedPlanes(vec![pixels])),
    }));
}

fn painted(root: &Node) -> Vec<Node> {
    let mut nodes = vec![root.clone()];
    let mut index = 0;
    while index < nodes.len() {
        nodes.extend(nodes[index].children());
        index += 1;
    }
    nodes
        .into_iter()
        .filter(|node| node.draw().is_some())
        .collect()
}

fn draw(node: &Node) -> DrawList {
    let mut list = DrawList::default();
    (node.draw().expect("draw hook"))(&mut list, Point::default());
    list
}

#[test]
fn surface_draws_the_newest_frame_under_one_texture_id() {
    let output = VideoOutput::new();
    let generation = output.begin_generation().expect("generation");
    let graph = support::graph();
    graph.install_view(
        AnyView::new(VideoView::new(Some(output.clone())).frame_size(200.0, 200.0)),
        Default::default(),
    );
    graph.compute_layout(400.0, 400.0);
    let root = graph.tree.borrow().root().expect("root");
    let surface = painted(&root).pop().expect("video node draws");

    let empty = draw(&surface);
    assert!(empty.batches().iter().all(|batch| batch.texture_id == 0));

    frame(&output, generation, 1, 160, 90);
    let first = draw(&surface);
    let textured: Vec<_> = first
        .batches()
        .iter()
        .filter(|batch| batch.texture_id != 0)
        .map(|batch| batch.texture_id)
        .collect();
    assert_eq!(textured.len(), 1, "one image quad over the background");
    let ys: Vec<f32> = first.vertices().iter().map(|vertex| vertex.y).collect();
    let top = ys.iter().copied().fold(f32::MAX, f32::min);
    assert_eq!(top, 0.0, "the background spans the frame");

    frame(&output, generation, 2, 160, 90);
    let second = draw(&surface);
    let id = second
        .batches()
        .iter()
        .find(|batch| batch.texture_id != 0)
        .map(|batch| batch.texture_id);
    assert_eq!(
        id,
        Some(textured[0]),
        "streamed frames reuse the texture id"
    );
}

#[test]
fn transport_drives_the_control() {
    InteractiveScene::run(600.0, 200.0, |scene| {
        let control = FakeControl::new(PlaybackState::Paused, Some(MediaTime(90_000_000_000)));
        scene.install(MediaTransport::new(control.clone()));

        let play = scene
            .labelled("Play", Some(AccessibilityRole::Button))
            .expect("play button");
        scene.click(&play, 1);
        assert_eq!(control.calls(), vec![Call::Play]);

        control.change(|snapshot| snapshot.state = PlaybackState::Playing);
        scene.settle(3, 1.0);
        let pause = scene
            .labelled("Pause", Some(AccessibilityRole::Button))
            .expect("pause after the control reports playing");
        scene.click(&pause, 1);
        assert_eq!(control.calls().last(), Some(&Call::Pause));

        control.change(|snapshot| snapshot.state = PlaybackState::Ended);
        scene.settle(3, 1.0);
        let replay = scene
            .labelled("Play", Some(AccessibilityRole::Button))
            .expect("play at the end");
        scene.click(&replay, 1);
        let calls = control.calls();
        assert_eq!(
            &calls[calls.len() - 2..],
            &[Call::Seek(SeekRequest::exact(MediaTime(0))), Call::Play]
        );

        let mute = scene
            .labelled("Mute", Some(AccessibilityRole::Button))
            .expect("mute");
        scene.click(&mute, 1);
        assert_eq!(control.calls().last(), Some(&Call::Volume(0.0)));
        scene
            .labelled("Unmute", Some(AccessibilityRole::Button))
            .expect("the mute button flips");

        let scrubber = scene
            .first(|node| {
                node.accessibility().is_some_and(|semantics| {
                    semantics.role == AccessibilityRole::Slider && semantics.label.is_empty()
                })
            })
            .expect("scrubber");
        let frame = scrubber.absolute_frame();
        let y = frame.y + frame.height / 2.0;
        scene.drag(
            Point::new(frame.x + 1.0, y),
            Point::new(frame.x + frame.width / 2.0, y),
            4,
        );
        let seeks: Vec<SeekRequest> = control
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                Call::Seek(request) => Some(request),
                _ => None,
            })
            .collect();
        let last = seeks.last().expect("dragging seeks");
        assert_eq!(
            last.accuracy,
            cove_media::SeekAccuracy::Exact,
            "release is exact"
        );
        assert!((last.position.seconds() - 45.0).abs() < 3.0, "{last:?}");
    });
}

#[test]
fn scrubber_is_disabled_without_a_duration() {
    InteractiveScene::run(600.0, 200.0, |scene| {
        let control = FakeControl::new(PlaybackState::Playing, None);
        scene.install(MediaTransport::new(control.clone()).shows_volume(false));
        let slider = scene
            .first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.role == AccessibilityRole::Slider)
            })
            .expect("scrubber");
        assert!(!slider.accessibility().expect("semantics").state.is_enabled);
        assert!(scene.labelled("Mute", None).is_none());
    });
}

#[test]
fn waveform_seeks_to_the_pointer_fraction() {
    InteractiveScene::run(400.0, 200.0, |scene| {
        let seeks = Arc::new(Mutex::new(Vec::new()));
        let sink = seeks.clone();
        let peaks: Vec<AudioPeak> = (0..64)
            .map(|i| AudioPeak {
                min: -0.5,
                max: (i as f32 / 64.0),
                rms: 0.3,
            })
            .collect();
        scene.install(
            Waveform::new(peaks)
                .progress(0.25)
                .on_seek(move |fraction| sink.lock().unwrap().push(fraction))
                .frame_size(200.0, 40.0),
        );
        let node = scene.labelled("Waveform", None).expect("waveform");
        let frame = node.absolute_frame();
        scene.click_at_point(
            Point::new(frame.x + frame.width * 0.75, frame.y + frame.height / 2.0),
            1,
        );
        let recorded = seeks.lock().unwrap().clone();
        assert_eq!(recorded.len(), 1);
        assert!((recorded[0] - 0.75).abs() < 0.02, "{recorded:?}");
        assert!(draw(&node).vertices().len() > 64 * 4, "one bar per column");
    });
}

#[test]
fn audio_player_follows_control_changes() {
    InteractiveScene::run(600.0, 300.0, |scene| {
        let control = FakeControl::new(PlaybackState::Paused, Some(MediaTime(200_000_000_000)));
        scene.install(
            AudioPlayer::new(control.clone())
                .title("Track")
                .subtitle("Artist"),
        );
        let text = |scene: &InteractiveScene, needle: &str| {
            scene.first(|node| {
                node.accessibility()
                    .is_some_and(|semantics| semantics.label == needle)
            })
        };
        assert!(text(scene, "0:00").is_some());
        control.change(|snapshot| snapshot.position = Some(MediaTime(65_000_000_000)));
        scene.settle(3, 1.0);
        assert!(
            text(scene, "1:05").is_some(),
            "elapsed time follows the snapshot"
        );
        assert!(
            text(
                scene,
                &format_media_time(Some(MediaTime(200_000_000_000)), None)
            )
            .is_some()
        );
        scene.labelled("Audio level", None).expect("meter");
    });
}

#[test]
fn listener_is_released_with_the_view() {
    InteractiveScene::run(600.0, 200.0, |scene| {
        let control = FakeControl::new(PlaybackState::Paused, None);
        scene.install(MediaTransport::new(control.clone()));
        assert_eq!(control.notifier.listener_count(), 1);
        scene.install(cove_composition::EmptyView);
        assert_eq!(control.notifier.listener_count(), 0);
    });
}
