#![cfg(all(feature = "ffmpeg", not(target_arch = "wasm32")))]

use cove_graphics::{rhi::*, ui::NativeTextureImageBacking};
use cove_imaging::TextureImage;
use cove_media::{
    PixelFormat, PlaneLayout, VideoDynamicRange, VideoFormat, VideoFrame, VideoOutput,
    ffmpeg::FrameStorage,
};
use cove_media_rhi::VideoPresenter;
use std::{any::Any, sync::Arc};

fn pattern(width: u32, height: u32, stride: usize) -> Vec<u8> {
    let mut bytes = vec![0; stride * height as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let index = y * stride + x * 4;
            bytes[index] = (x * 7) as u8;
            bytes[index + 1] = (y * 5) as u8;
            bytes[index + 2] = ((x + y) * 3) as u8;
            bytes[index + 3] = 255;
        }
    }
    bytes
}

fn publish(
    output: &VideoOutput,
    width: u32,
    height: u32,
    stride: usize,
    bytes: Vec<u8>,
    dynamic_range: VideoDynamicRange,
) {
    let generation = output.begin_generation().expect("generation");
    let frame = VideoFrame {
        generation,
        sequence: 1,
        pts: None,
        dts: None,
        duration: None,
        format: VideoFormat {
            width,
            height,
            pixel_format: PixelFormat::Rgba8,
            planes: vec![PlaneLayout { stride, offset: 0 }],
            pixel_aspect_ratio: (1, 1),
            frame_rate: (30, 1),
            colorimetry: "unknown".into(),
            dynamic_range,
            format_description: format!("rgba {width}x{height}"),
        },
        storage: Arc::new(FrameStorage::new(bytes)),
    };
    assert!(output.publish(frame));
}

fn produce(output: &VideoOutput, width: u32, height: u32) -> Vec<u8> {
    let bytes = pattern(width, height, width as usize * 4);
    publish(
        output,
        width,
        height,
        width as usize * 4,
        bytes.clone(),
        VideoDynamicRange::Unknown,
    );
    bytes
}

fn read(device: &Device, image: &Arc<TextureImage>) -> Vec<u8> {
    let backing: Arc<dyn Any + Send + Sync> = image.backing.clone();
    let backing = backing
        .downcast::<NativeTextureImageBacking>()
        .expect("native texture backing");
    let size = image.pixel_size;
    device.wait_until_idle().expect("GPU completion");
    let mut bytes = vec![0; size.width as usize * size.height as usize * 4];
    device
        .read_texture_data(
            backing.resource.texture(),
            size.width as usize,
            size.height as usize,
            size.width as usize * 4,
            TextureSubresource::default(),
            &mut bytes,
        )
        .expect("GPU readback");
    bytes
}

#[test]
fn native_upload_resize_and_close_preserve_old_texture_snapshots() {
    let device = Arc::new(Device::make(DeviceConfig::default()).expect("native RHI device"));
    let output = VideoOutput::new();
    let mut presenter = VideoPresenter::new(device.clone(), output.clone());
    assert!(presenter.update().expect("empty output").is_none());
    let first_pixels = produce(&output, 32, 24);
    let first = presenter.update().expect("upload").expect("first image");
    assert_eq!(read(&device, &first), first_pixels);
    assert!(Arc::ptr_eq(
        &first,
        &presenter.update().expect("unchanged").expect("image")
    ));
    assert_eq!(presenter.statistics().uploaded_frames, 1);

    let second_pixels = produce(&output, 16, 8);
    let second = presenter
        .update()
        .expect("resize upload")
        .expect("second image");
    assert_ne!(first.texture_id, second.texture_id);
    assert_eq!((second.pixel_size.width, second.pixel_size.height), (16, 8));
    assert_eq!(read(&device, &second), second_pixels);
    assert_eq!(read(&device, &first), first_pixels);
    assert_eq!(presenter.statistics().uploaded_frames, 2);
    assert_eq!(
        presenter.statistics().uploaded_bytes,
        32 * 24 * 4 + 16 * 8 * 4
    );

    output.close();
    assert!(presenter.update().expect("closed output").is_none());
    drop(presenter);
    drop(output);
    assert_eq!(read(&device, &first), first_pixels);
    assert_eq!(read(&device, &second), second_pixels);
}

#[test]
fn per_buffer_padding_is_preserved_in_metadata_and_uploaded_by_stride() {
    let device = Arc::new(Device::make(DeviceConfig::default()).expect("native RHI device"));
    let output = VideoOutput::new();
    let mut data = vec![0x77; 128];
    for pixel in data[..32].as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&[255, 0, 0, 255]);
    }
    for pixel in data[64..96].as_chunks_mut::<4>().0 {
        pixel.copy_from_slice(&[0, 255, 0, 255]);
    }
    let expected: Vec<_> = data[..32]
        .iter()
        .chain(data[64..96].iter())
        .copied()
        .collect();
    publish(&output, 8, 2, 64, data, VideoDynamicRange::Unknown);
    assert_eq!(output.latest().expect("frame").format.planes[0].stride, 64);
    let mut presenter = VideoPresenter::new(device.clone(), output);
    let image = presenter
        .update()
        .expect("upload padded rows")
        .expect("image");
    assert_eq!(read(&device, &image), expected);
}

#[test]
fn hdr_requires_explicit_conversion_instead_of_silent_sdr_presentation() {
    use cove_media::MediaErrorKind;
    let device = Arc::new(Device::make(DeviceConfig::default()).expect("native RHI device"));
    let output = VideoOutput::new();
    publish(
        &output,
        16,
        8,
        16 * 4,
        pattern(16, 8, 16 * 4),
        VideoDynamicRange::Pq,
    );
    let mut presenter = VideoPresenter::new(device, output);
    let error = match presenter.update() {
        Err(error) => error,
        Ok(_) => panic!("HDR must require a color conversion"),
    };
    assert_eq!(error.kind, MediaErrorKind::Unsupported);
    assert_eq!(presenter.statistics().uploaded_frames, 0);
}
