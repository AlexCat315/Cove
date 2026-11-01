//! Translation of `Tests/CoveUIRuntimeTests/NativeAnalyticClipTests.swift`.
//!
//! Analytic coverage is what keeps a translucent edge from doubling up its
//! alpha. A rounded mask and a rounded stroke both put a soft boundary between
//! the shape and the background; if the two coverage terms compose by addition
//! instead of by `over`, the seam reads as a bright rim at half alpha and as a
//! dark one at high alpha. Both cases below sample the real GPU output.
//!
//! Behaviour checklist:
//! - a translucent rounded mask over a full-frame rect never accumulates alpha
//!   past 76% anywhere, and still produces a genuine antialiased edge
//! - a translucent rounded stroke behaves the same way
//!
//! `Tests/CoveUIRuntimeTests/NativeUIDrawTestSupport.swift` is a support file, not
//! a suite: `NativeUIDrawTestContext` is folded in below as [`Context`], and the
//! parity helper is not needed by this suite. No shared `tests/support` module is
//! created because `crates/compose/tests/support` belongs to another crate and a
//! parallel worker owns the shared Helpers space in `Core`.

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, Rect, RoundedRect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GraphicsApi, QueueClass,
    RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource, TextureSubresource,
    TextureUsage,
};
use cove_graphics::ui::{NativeDrawListRenderer, NativeUIDrawStatistics, NativeUIViewport};

/// `NativeUIDrawTestContext`: one device, one renderer, real colour targets.
struct Context {
    device: Arc<Device>,
    native: NativeDrawListRenderer,
    format: TextureFormat,
    samples: usize,
}

impl Context {
    fn new(format: TextureFormat, samples: usize) -> Option<Self> {
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
        native
            .configure(TextureFormat::Bgra8Unorm, 1)
            .expect("configure the renderer");
        Some(Self {
            device,
            native,
            format,
            samples,
        })
    }

    fn output(&self, size: [usize; 2]) -> TextureResource {
        TextureResource::new(
            self.device.clone(),
            TextureDescriptor::new(
                size[0],
                size[1],
                self.format,
                TextureUsage::COLOR_TARGET
                    | TextureUsage::TRANSFER_SOURCE
                    | TextureUsage::TRANSFER_DESTINATION
                    | TextureUsage::SAMPLED,
            ),
        )
        .expect("colour target")
    }

    /// The Swift `nativeImage`: configure, record into an offscreen pass, submit,
    /// wait, read back.
    fn render(&mut self, list: &DrawList, size: [usize; 2]) -> (Vec<u8>, NativeUIDrawStatistics) {
        let viewport = NativeUIViewport::new(size, [size[0] as f32, size[1] as f32]);
        self.native
            .configure(self.format, self.samples)
            .expect("configure for this pass");
        let output = self.output(size);
        let multisample = (self.samples > 1).then(|| {
            let mut descriptor =
                TextureDescriptor::new(size[0], size[1], self.format, TextureUsage::COLOR_TARGET);
            descriptor.sample_count = self.samples;
            TextureResource::new(self.device.clone(), descriptor).expect("multisample target")
        });

        let color = multisample
            .as_ref()
            .map_or(output.texture(), TextureResource::texture);
        let target = RenderColorTarget {
            resolve_texture: multisample.as_ref().map(|_| output.texture()),
            texture: color,
            load_action: ColorLoadAction::Clear([0.07, 0.09, 0.12, 0.5]),
            store: multisample.is_none(),
        };

        self.device.begin_frame().expect("begin frame");
        let result = (|| {
            let mut commands = CommandBuffer::new();
            let mut frame = self
                .native
                .record(list, &mut commands, target, viewport, None)
                .map_err(|error| error.to_string())?;
            self.device
                .submit(&commands, QueueClass::Graphics, &[], &[], None)
                .map_err(|error| error.to_string())?;
            frame.did_submit();
            Ok::<_, String>(frame.statistics)
        })();
        self.device.end_frame();
        let statistics = match result {
            Ok(statistics) => statistics,
            Err(error) => panic!("render failed: {error}"),
        };
        self.device.wait_until_idle().expect("wait for idle");
        (self.read(&output), statistics)
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
            .expect("read back the frame");
        data
    }
}

/// `Color(r:g:b:a:)` with the alpha that matters here.
fn white(alpha: f32) -> Color {
    Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: alpha,
    }
}

#[test]
fn translucent_mask_and_stroke_do_not_accumulate_alpha() {
    let Some(mut context) = Context::new(TextureFormat::Rgba8Unorm, 1) else {
        return;
    };
    let rect = Rect::new(12.0, 12.0, 40.0, 40.0);

    for stroke in [false, true] {
        let mut list = DrawList::default();
        list.pixel_scale = 1.0;
        list.geometry_antialiasing = cove_core::GeometryAntialiasing::Analytic;
        if stroke {
            list.add_rounded_rect_stroke(rect, 10.0, 3.0, white(0.5))
                .expect("rounded stroke");
        } else {
            list.push_mask(RoundedRect::new(rect, 10.0));
            list.add_rect(Rect::new(0.0, 0.0, 64.0, 64.0), white(0.5))
                .expect("full-frame rect");
            list.pop_mask();
        }

        let (image, _) = context.render(&list, [64, 64]);
        // Alpha is the fourth channel of an RGBA8 target.
        let alpha: Vec<u8> = image.iter().skip(3).step_by(4).copied().collect();
        assert!(
            alpha.iter().all(|&value| value <= 193),
            "no texel may accumulate past 76% alpha (stroke={stroke}); worst was {}",
            alpha.iter().copied().max().unwrap_or(0)
        );
        assert!(
            alpha.iter().any(|&value| value > 130 && value < 188),
            "the edge must still be genuinely antialiased (stroke={stroke}); \
             alphas seen: {:?}",
            {
                let mut sample: Vec<u8> = alpha
                    .iter()
                    .copied()
                    .filter(|&value| value > 100 && value < 200)
                    .collect();
                sample.sort_unstable();
                sample.dedup();
                sample
            }
        );
    }
}
