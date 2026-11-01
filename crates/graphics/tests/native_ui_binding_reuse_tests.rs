//! Translation of `Tests/CoveUIRuntimeTests/NativeUIBindingReuseTests.swift`.
//!
//! A retained UI binding must not be confused with a transient uniform buffer.
//! The failure this guards is subtle: the image samples correctly in a fresh
//! frame and goes wrong once the frame ring wraps, because a fixed geometry size
//! lets a stale uniform offset be reused. Varying the upload size every frame is
//! what exposes it.
//!
//! Behaviour checklist:
//! - one renderer reconfigured between two passes in a single submission renders
//!   both formats, at two viewport scales, across more frames than the ring holds
//! - the retained binding bypasses the device cache: only the first frame misses,
//!   and nothing is ever evicted
//! - no frame after the first uploads a texture again
//! - every pass still matches a freshly configured reference renderer

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::scene::TextureId;
use cove_core::{Color, DrawList, Point, Rect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUploadRegion, TextureUsage,
};
use cove_graphics::ui::{NativeDrawListRenderer, NativeUIViewport};

/// `NativeUIDrawTestContext`, folded in: this suite and
/// `NativeUIDrawTestSupport.swift` are the only consumers of the harness fields
/// it needs.
struct Context {
    device: Arc<Device>,
    native: NativeDrawListRenderer,
    reference: NativeDrawListRenderer,
}

impl Context {
    fn new() -> Option<Self> {
        let device = match Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            enable_validation: true,
            frames_in_flight: 3,
            ..DeviceConfig::default()
        }) {
            Ok(device) => device,
            Err(error) => {
                eprintln!("no Metal device available ({error}); skipping");
                return None;
            }
        };
        let device = Arc::new(device);
        let mut native = NativeDrawListRenderer::new(device.clone()).expect("UI renderer");
        let mut reference =
            NativeDrawListRenderer::new(device.clone()).expect("reference renderer");
        native
            .configure(TextureFormat::Bgra8Unorm, 1)
            .expect("configure native");
        reference
            .configure(TextureFormat::Bgra8Unorm, 1)
            .expect("configure reference");
        Some(Self {
            device,
            native,
            reference,
        })
    }

    fn stage(
        &self,
        id: TextureId,
        pixels: Vec<u8>,
        size: [usize; 2],
        region: TextureUploadRegion,
        format: TextureFormat,
    ) {
        self.native
            .register_texture(id, pixels.clone(), size, region, format)
            .expect("stage native");
        self.reference
            .register_texture(id, pixels, size, region, format)
            .expect("stage reference");
    }

    fn output(&self, size: [usize; 2], format: TextureFormat) -> TextureResource {
        TextureResource::new(
            self.device.clone(),
            TextureDescriptor::new(
                size[0],
                size[1],
                format,
                TextureUsage::COLOR_TARGET
                    | TextureUsage::TRANSFER_SOURCE
                    | TextureUsage::TRANSFER_DESTINATION
                    | TextureUsage::SAMPLED,
            ),
        )
        .expect("colour target")
    }

    fn read(&self, resource: &TextureResource) -> Vec<u8> {
        let descriptor = resource.descriptor();
        let size = [descriptor.width, descriptor.height];
        let mut data = vec![0u8; size[0] * size[1] * 4];
        self.device
            .read_texture_data(
                resource.texture(),
                size[0],
                size[1],
                size[0] * 4,
                TextureSubresource::default(),
                &mut data,
            )
            .expect("read back");
        data
    }

    /// One full offscreen pass, matching the Swift `nativeImage`.
    fn image(
        &mut self,
        list: &DrawList,
        format: TextureFormat,
        samples: usize,
        viewport: NativeUIViewport,
        reference: bool,
    ) -> (Vec<u8>, cove_graphics::ui::NativeUIDrawStatistics) {
        let output = self.output(viewport.pixels, format);
        let renderer = if reference {
            &mut self.reference
        } else {
            &mut self.native
        };
        renderer.configure(format, samples).expect("configure");
        self.device.begin_frame().expect("begin frame");
        let statistics = {
            let mut commands = CommandBuffer::new();
            let mut frame = renderer
                .record(
                    list,
                    &mut commands,
                    RenderColorTarget {
                        resolve_texture: None,
                        texture: output.texture(),
                        load_action: ColorLoadAction::Clear([0.07, 0.09, 0.12, 0.5]),
                        store: true,
                    },
                    viewport,
                    None,
                )
                .expect("record");
            self.device
                .submit(&commands, QueueClass::Graphics, &[], &[], None)
                .expect("submit");
            frame.did_submit();
            frame.statistics
        };
        self.device.end_frame();
        self.device.wait_until_idle().expect("wait for idle");
        (self.read(&output), statistics)
    }
}

/// `CacheStats::delta(since:)` from Swift: the change between two snapshots.
struct CacheDelta {
    misses: u64,
    evictions: u64,
}

/// Samples a BGRA8 image as RGBA so the comparison is channel-order agnostic.
fn as_rgba_bgra(pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len());
    for texel in pixels.as_chunks::<4>().0 {
        out.extend_from_slice(&[texel[2], texel[1], texel[0], texel[3]]);
    }
    out
}

#[test]
fn recording_constants_are_independent() {
    let Some(mut context) = Context::new() else {
        return;
    };

    context.stage(
        77,
        vec![179, 113, 211, 193],
        [1, 1],
        TextureUploadRegion {
            width: 1,
            height: 1,
            ..TextureUploadRegion::default()
        },
        TextureFormat::Rgba8Unorm,
    );

    let mut list = DrawList::default();
    list.add_image_quad(
        Rect::new(2.0, 3.0, 8.0, 6.0),
        77,
        Color::from_rgba8(113, 191, 77, 211),
        Point::new(0.0, 0.0),
        Point::new(1.0, 1.0),
    )
    .expect("image quad");

    let viewports = [
        NativeUIViewport::new([32, 24], [32.0, 24.0]),
        NativeUIViewport::new([32, 24], [16.0, 12.0]),
    ];
    // `.cases[1]` and `.cases[3]` are bgra8Unorm and bgra8UnormSRGB.
    let formats = [TextureFormat::Bgra8Unorm, TextureFormat::Bgra8UnormSrgb];
    let outputs = [
        context.output(viewports[0].pixels, formats[0]),
        context.output(viewports[0].pixels, formats[1]),
    ];
    let clear = [0.07, 0.09, 0.12, 0.5];

    // Reconfigure the same renderer between two passes in one submission. More
    // than three frames also exercises upload-ring reuse without idle waits.
    for frame_index in 0..8 {
        let previous = context.device.binding_set_cache_stats();
        context.device.begin_frame().expect("begin frame");
        let outcome = {
            let mut commands = CommandBuffer::new();
            let mut frames = Vec::new();
            for index in 0..formats.len() {
                context
                    .native
                    .configure(formats[index], 1)
                    .expect("reconfigure between passes");
                let frame = context
                    .native
                    .record(
                        &list,
                        &mut commands,
                        RenderColorTarget {
                            resolve_texture: None,
                            texture: outputs[index].texture(),
                            load_action: ColorLoadAction::Clear(clear),
                            store: true,
                        },
                        viewports[index],
                        None,
                    )
                    .expect("record");
                if frame_index > 0 {
                    assert_eq!(
                        frame.statistics.texture_uploads, 0,
                        "frame {frame_index} pass {index} must not re-upload a staged texture"
                    );
                }
                frames.push(frame);
            }
            context
                .device
                .submit(&commands, QueueClass::Graphics, &[], &[], None)
                .expect("submit");
            for frame in &mut frames {
                frame.did_submit();
            }
            Ok::<_, String>(())
        };
        context.device.end_frame();
        outcome.expect("frame renders");

        let current = context.device.binding_set_cache_stats();
        let delta = CacheDelta {
            misses: current.misses - previous.misses,
            evictions: current.evictions - previous.evictions,
        };
        // Retained UI bindings bypass the Device cache on reuse; count allocations.
        assert_eq!(
            delta.misses,
            if frame_index == 0 { 1 } else { 0 },
            "only the first frame may allocate a binding set (frame {frame_index})"
        );
        assert_eq!(
            delta.evictions, 0,
            "retained bindings are never evicted (frame {frame_index})"
        );

        // Change upload sizes every frame. Fixed geometry can accidentally reuse
        // a transient uniform's offset after the frame ring wraps.
        list.add_image_quad(
            Rect::new(-10.0, -10.0, 1.0, 1.0),
            77,
            Color::WHITE,
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
        )
        .expect("changing-size quad");
    }

    context.device.wait_until_idle().expect("drain");

    for index in 0..formats.len() {
        let actual = as_rgba_bgra(&context.read(&outputs[index]));
        let (expected, _) = context.image(&list, formats[index], 1, viewports[index], true);
        let expected = as_rgba_bgra(&expected);
        assert_eq!(
            actual.len(),
            expected.len(),
            "pass {index} produced a differently sized image"
        );
        for (offset, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                actual.abs_diff(*expected) <= 3,
                "pass {index} byte {offset}: native {actual} vs reference {expected} \
                 (binding-reuse/{index})"
            );
        }
    }
}
