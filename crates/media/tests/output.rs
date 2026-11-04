use cove_media::*;
use std::{any::Any, sync::Arc};

struct Pixels(Vec<u8>);
impl VideoStorage for Pixels {
    fn copy_planes(&self) -> MediaResult<Vec<Vec<u8>>> {
        Ok(vec![self.0.clone()])
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
fn frame(generation: u64, sequence: u64) -> VideoFrame {
    VideoFrame {
        generation,
        sequence,
        pts: None,
        dts: None,
        duration: None,
        format: VideoFormat {
            width: 1,
            height: 1,
            pixel_format: PixelFormat::Rgba8,
            planes: vec![PlaneLayout {
                stride: 4,
                offset: 0,
            }],
            pixel_aspect_ratio: (1, 1),
            frame_rate: (0, 1),
            colorimetry: "unknown".into(),
            dynamic_range: VideoDynamicRange::Unknown,
            negotiated_caps: "application-owned RGBA".into(),
        },
        storage: Arc::new(Pixels(vec![255, 0, 0, 255])),
    }
}

#[test]
fn latest_frame_discards_stale_generations_and_out_of_order_samples() {
    let output = VideoOutput::new();
    let subscriber = output.subscribe();
    assert!(subscriber.try_changed());
    let first_generation = output.begin_generation().expect("generation");
    assert!(output.publish(frame(first_generation, 1)));
    let old = output.latest().expect("frame");
    assert!(!output.publish(frame(first_generation, 1)));
    let generation = output.begin_generation().expect("new source/seek");
    assert!(output.latest().is_none());
    assert!(!output.publish(frame(first_generation, 2)));
    for sequence in 1..1000 {
        assert!(output.publish(frame(generation, sequence)));
    }
    assert!(subscriber.try_changed()); // bounded wake-up, regardless of frame count
    assert!(!subscriber.try_changed());
    assert_eq!(output.latest().expect("latest").sequence, 999);
    output.close();
    assert!(subscriber.try_changed());
    let revision = output.revision();
    output.close();
    assert_eq!(output.revision(), revision);
    assert!(!output.publish(frame(generation, 1000)));
    assert_eq!(
        output.begin_generation().expect_err("closed").kind,
        MediaErrorKind::Closed
    );
    assert_eq!(
        old.storage.copy_planes().expect("retained CPU storage"),
        vec![vec![255, 0, 0, 255]]
    );
}

#[test]
fn byte_inputs_preserve_eof_seek_and_cancellation() {
    let cancellation = CancellationToken::default();
    let mut input = std::io::Cursor::new(Arc::<[u8]>::from([1, 2, 3, 4]));
    let mut bytes = [0; 3];
    assert_eq!(
        MediaInput::read(&mut input, &mut bytes, &cancellation).expect("read"),
        3
    );
    assert_eq!(bytes, [1, 2, 3]);
    assert_eq!(
        MediaInput::read(&mut input, &mut bytes, &cancellation).expect("tail"),
        1
    );
    assert_eq!(
        MediaInput::read(&mut input, &mut bytes, &cancellation).expect("EOF"),
        0
    );
    assert_eq!(input.seek(2, &cancellation), Ok(()));
    assert_eq!(
        input.seek(5, &cancellation).expect_err("out of range").kind,
        MediaErrorKind::InvalidInput
    );
    cancellation.cancel();
    assert_eq!(
        MediaInput::read(&mut input, &mut bytes, &cancellation)
            .expect_err("cancelled")
            .kind,
        MediaErrorKind::Cancelled
    );
}
