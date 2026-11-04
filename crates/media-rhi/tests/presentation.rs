#![cfg(all(feature = "gst", not(target_arch = "wasm32")))]

use cove_graphics::{rhi::*, ui::NativeTextureImageBacking};
use cove_imaging::TextureImage;
use cove_media::{
    VideoOutput,
    gst::{self, prelude::*},
};
use cove_media_rhi::VideoPresenter;
use std::{any::Any, sync::Arc};

fn produce(output: VideoOutput, width: i32, height: i32) -> Vec<u8> {
    let endpoint = gst::GstVideoEndpoint::new(output.clone()).expect("video endpoint");
    let pipeline = gst::Pipeline::new();
    let source = gst::ElementFactory::make("videotestsrc")
        .property("num-buffers", 1_i32)
        .property_from_str("pattern", "smpte")
        .build()
        .expect("test video");
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("width", width)
        .field("height", height)
        .build();
    pipeline
        .add_many([&source, endpoint.element()])
        .expect("elements");
    source
        .link_filtered(endpoint.element(), &caps)
        .expect("RGBA link");
    pipeline.set_state(gst::State::Playing).expect("start");
    let message = pipeline
        .bus()
        .expect("bus")
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("completion");
    pipeline.set_state(gst::State::Null).expect("stop");
    if let gst::MessageView::Error(error) = message.view() {
        panic!("{} {:?}", error.error(), error.debug());
    }
    output
        .latest()
        .expect("sample")
        .storage
        .copy_planes()
        .expect("mapped sample")
        .remove(0)
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
    let first_pixels = produce(output.clone(), 32, 24);
    let first = presenter.update().expect("upload").expect("first image");
    assert_eq!(read(&device, &first), first_pixels);
    assert!(Arc::ptr_eq(
        &first,
        &presenter.update().expect("unchanged").expect("image")
    ));
    assert_eq!(presenter.statistics().uploaded_frames, 1);

    let second_pixels = produce(output.clone(), 16, 8);
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
    gst::initialize().expect("GStreamer");
    let device = Arc::new(Device::make(DeviceConfig::default()).expect("native RHI device"));
    let output = VideoOutput::new();
    let endpoint = gst::GstVideoEndpoint::new(output.clone()).expect("endpoint");
    let caps = gst::Caps::builder("video/x-raw")
        .field("format", "RGBA")
        .field("width", 8_i32)
        .field("height", 2_i32)
        .field("framerate", gst::Fraction::new(30, 1))
        .build();
    let source = gst::app::AppSrc::builder()
        .caps(&caps)
        .format(gst::Format::Time)
        .build();
    let pipeline = gst::Pipeline::new();
    pipeline
        .add_many([source.upcast_ref::<gst::Element>(), endpoint.element()])
        .expect("elements");
    source.link(endpoint.element()).expect("link");
    pipeline.set_state(gst::State::Playing).expect("play");
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
    let mut buffer = gst::Buffer::from_mut_slice(data);
    let writable = buffer.get_mut().expect("writable sample");
    writable.set_pts(gst::ClockTime::ZERO);
    gst::video::VideoMeta::add_full(
        writable,
        gst::video::VideoFrameFlags::empty(),
        gst::video::VideoFormat::Rgba,
        8,
        2,
        &[0],
        &[64],
    )
    .expect("padded metadata");
    source.push_buffer(buffer).expect("input buffer");
    source.end_of_stream().expect("EOF");
    let message = pipeline
        .bus()
        .expect("bus")
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("completion");
    pipeline.set_state(gst::State::Null).expect("stop");
    if let gst::MessageView::Error(error) = message.view() {
        panic!("{} {:?}", error.error(), error.debug());
    }
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
    use cove_media::{MediaErrorKind, VideoDynamicRange};
    let device = Arc::new(Device::make(DeviceConfig::default()).expect("native RHI device"));
    let output = VideoOutput::new();
    produce(output.clone(), 16, 8);
    let mut frame = (*output.latest().expect("frame")).clone();
    frame.sequence += 1;
    frame.format.dynamic_range = VideoDynamicRange::Pq;
    assert!(output.publish(frame));
    let mut presenter = VideoPresenter::new(device, output);
    let error = match presenter.update() {
        Err(error) => error,
        Ok(_) => panic!("HDR must require a color conversion"),
    };
    assert_eq!(error.kind, MediaErrorKind::Unsupported);
    assert_eq!(presenter.statistics().uploaded_frames, 0);
}
