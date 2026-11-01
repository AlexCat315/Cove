//! Translation of `Tests/CoveUIRuntimeTests/Rendering/MSAAEvaluationTests.swift`.
//!
//! Opt-in measurement, independent from normal CI regression runs. Swift gates
//! the whole suite behind `COVE_MSAA_EVALUATION=1` because it renders 30 timed
//! passes per configuration and writes PNGs and a JSON report; that is a
//! benchmarking run, not a regression test.
//!
//! Behaviour checklist:
//! - without the opt-in environment variable the suite renders nothing
//! - with it, each scale renders a 1x, a 4x and a 4x-transient configuration, and
//!   the transient attachment reproduces the persistent 4x image byte for byte
//! - every configuration reports real geometry and an image of the reference's
//!   extent
//! - MSAA beats analytic coverage on the raw triangle, which is why the global
//!   default cannot yet become 1x
//! - the report is written where the caller asked for it
//!
//! Named differences (`GAP — evaluation harness reachability`):
//!
//! `transientAttachment`. Swift sets `TextureDescriptor.transientAttachment` and
//! reads `TextureResource.allocation?.allocatedBytes` to prove the MSAA backing
//! can live in tile memory. Rust has `TextureDescriptor::transient_attachment`
//! and `TextureResource::allocation()`, so that half ports directly.
//!
//! `GPUTimestampProfile`. Swift calls `device.setGPUTimestampProfile(profile)`
//! and reads `profile.recordedMarkers`, `profile.status`. Rust's
//! `Device::set_gpu_timestamp_profile` takes `Option<Arc<GpuTimestampProfile>>`
//! and `recorded_markers()`/`status()` are reachable, so the GPU timing columns
//! port too.
//!
//! `writePNG`. Swift writes PNGs through `ImageIO`. There is no PNG encoder in
//! `cove-graphics`, and the crate graph gives no dev-dependency that provides
//! one, so the images are written as the raw BGRA bytes the suite already holds
//! plus a PPM of the same pixels, which any viewer opens. The JSON report keeps
//! the same fields. This is a harness difference, not a behavioural gap.

#![cfg(target_os = "macos")]

use std::sync::Arc;

use cove_core::{Color, DrawList, GeometryAntialiasing, Point, Rect, RoundedRect};
use cove_graphics::rhi::{
    ColorLoadAction, CommandBuffer, Device, DeviceConfig, GpuTimestampProfile, GraphicsApi,
    QueueClass, RenderColorTarget, TextureDescriptor, TextureFormat, TextureResource,
    TextureSubresource, TextureUsage,
};
use cove_graphics::ui::{NativeDrawListRenderer, NativeUIDrawStatistics, NativeUIViewport};

/// The Swift suite only runs when the caller opts in.
fn enabled() -> bool {
    std::env::var("COVE_MSAA_EVALUATION").as_deref() == Ok("1")
}

fn directory() -> std::path::PathBuf {
    std::env::var("COVE_MSAA_OUTPUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/cove-msaa-evaluation"))
}

struct Context {
    device: Arc<Device>,
    native: NativeDrawListRenderer,
}

impl Context {
    fn new() -> Option<Self> {
        let device = match Device::make(DeviceConfig {
            preferred_backends: vec![GraphicsApi::Metal],
            enable_validation: false,
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
        let native = NativeDrawListRenderer::new(device.clone()).expect("UI renderer");
        Some(Self { device, native })
    }

    fn output(&self, pixels: [usize; 2]) -> TextureResource {
        TextureResource::new(
            self.device.clone(),
            TextureDescriptor::new(
                pixels[0],
                pixels[1],
                TextureFormat::Bgra8Unorm,
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

    fn render(
        &mut self,
        list: &DrawList,
        output: &TextureResource,
        multisample: Option<&TextureResource>,
        viewport: NativeUIViewport,
        store_samples: bool,
    ) -> NativeUIDrawStatistics {
        self.native
            .configure(
                TextureFormat::Bgra8Unorm,
                if multisample.is_some() { 4 } else { 1 },
            )
            .expect("configure");
        self.device.begin_frame().expect("begin frame");
        let statistics = {
            let mut commands = CommandBuffer::new();
            let mut frame = self
                .native
                .record(
                    list,
                    &mut commands,
                    RenderColorTarget {
                        resolve_texture: multisample.map(|_| output.texture()),
                        texture: multisample.map_or(output.texture(), TextureResource::texture),
                        load_action: ColorLoadAction::Clear([0.07, 0.09, 0.12, 1.0]),
                        store: multisample.is_none() || store_samples,
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
        statistics
    }
}

fn scene(scale: f32, analytic: bool) -> DrawList {
    let mut list = DrawList::default();
    list.pixel_scale = scale;
    list.geometry_antialiasing = if analytic {
        GeometryAntialiasing::Analytic
    } else {
        GeometryAntialiasing::Multisample
    };
    for i in 0..10 {
        let x = (i % 5) as f32 * 98.0 + 10.3;
        let y = (i / 5) as f32 * 75.0 + 8.7;
        list.add_rounded_rect(
            Rect::new(x, y, 87.4, 58.2),
            11.7,
            Color {
                r: 0.15 + i as f32 * 0.06,
                g: 0.55,
                b: 0.8,
                a: 0.7,
            },
        )
        .expect("rounded fill");
        list.add_rounded_rect_stroke(
            Rect::new(x + 4.2, y + 4.5, 78.2, 48.3),
            9.3,
            1.1,
            Color::WHITE,
        )
        .expect("rounded stroke");
    }
    let panel = RoundedRect::new(Rect::new(154.3, 264.6, 151.4, 43.8), 14.2);
    list.push_mask(panel);
    list.add_rect(
        Rect::new(142.0, 255.0, 180.0, 65.0),
        Color {
            r: 0.2,
            g: 0.8,
            b: 0.5,
            a: 0.65,
        },
    )
    .expect("masked fill");
    list.pop_mask();
    list.add_triangle(
        Point::new(345.2, 188.4),
        Point::new(493.6, 290.7),
        Point::new(353.8, 308.3),
        Color {
            r: 0.9,
            g: 0.3,
            b: 0.55,
            a: 1.0,
        },
    )
    .expect("triangle");
    list
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|left, right| left.total_cmp(right));
    values.get(values.len() / 2).copied().unwrap_or_default()
}

/// Box-downsamples by `factor`, matching the Swift `downsample`.
fn downsample(source: &[u8], width: usize, height: usize, factor: usize) -> Vec<u8> {
    let mut output = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            for channel in 0..4 {
                let mut sum = 0_u32;
                for sy in 0..factor {
                    for sx in 0..factor {
                        let offset =
                            ((y * factor + sy) * width * factor + x * factor + sx) * 4 + channel;
                        sum += source[offset] as u32;
                    }
                }
                output[(y * width + x) * 4 + channel] =
                    ((sum + (factor * factor / 2) as u32) / (factor * factor) as u32) as u8;
            }
        }
    }
    output
}

/// Mean absolute error overall, on edges, and inside the triangle region.
fn error(
    image: &[u8],
    reference: &[u8],
    width: usize,
    height: usize,
    scale: usize,
) -> (f64, f64, f64) {
    let mut total = 0.0;
    let mut edge = 0.0;
    let mut triangle = 0.0;
    let mut edges = 0_u32;
    let mut triangles = 0_u32;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let offset = (y * width + x) * 4;
            let mut delta = 0.0;
            let mut contrast = 0_i32;
            for channel in 0..3 {
                delta += (image[offset + channel] as i32 - reference[offset + channel] as i32).abs()
                    as f64
                    / 3.0;
                contrast += (reference[offset + channel] as i32
                    - reference[offset - 4 + channel] as i32)
                    .abs();
                contrast += (reference[offset + channel] as i32
                    - reference[offset - width * 4 + channel] as i32)
                    .abs();
            }
            total += delta;
            if contrast > 12 {
                edge += delta;
                edges += 1;
            }
            if x > 335 * scale && y > 175 * scale {
                triangle += delta;
                triangles += 1;
            }
        }
    }
    let pixels = ((width - 2) * (height - 2)) as f64;
    (
        total / pixels,
        edge / edges.max(1) as f64,
        triangle / triangles.max(1) as f64,
    )
}

/// Writes the raw BGRA bytes plus a P6 PPM of the same pixels. See the module
/// doc for why a PNG is not produced.
fn write_image(
    pixels: &[u8],
    width: usize,
    height: usize,
    path: &std::path::Path,
) -> std::io::Result<()> {
    std::fs::write(path.with_extension("bgra"), pixels)?;
    let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
    for texel in pixels.as_chunks::<4>().0 {
        ppm.extend_from_slice(&[texel[2], texel[1], texel[0]]);
    }
    std::fs::write(path.with_extension("ppm"), ppm)
}

#[test]
fn compare_coverage_and_cost() {
    if !enabled() {
        // The Swift suite is `.enabled(if:)` on the same variable, so an
        // un-opted run must render nothing and pass.
        return;
    }
    let Some(mut context) = Context::new() else {
        return;
    };
    let directory = directory();
    std::fs::create_dir_all(&directory).expect("the output directory is created");

    let mut report: Vec<serde_json::Value> = Vec::new();

    for scale in [1_usize, 2] {
        let logical = [512.0_f32, 320.0];
        let pixels = [512 * scale, 320 * scale];
        let reference_viewport = NativeUIViewport::new([pixels[0] * 4, pixels[1] * 4], logical);
        let reference_output = context.output(reference_viewport.pixels);
        let reference_list = scene((scale * 4) as f32, true);
        context
            .native
            .configure(TextureFormat::Bgra8Unorm, 1)
            .expect("configure");
        context.render(
            &reference_list,
            &reference_output,
            None,
            reference_viewport,
            false,
        );
        let reference_pixels = context.read(&reference_output);
        let reference = downsample(&reference_pixels, pixels[0], pixels[1], 4);
        write_image(
            &reference,
            pixels[0],
            pixels[1],
            &directory.join(format!("reference-{scale}x")),
        )
        .expect("the reference image is written");

        let mut persistent: Option<Vec<u8>> = None;
        for (samples, transient) in [(1_usize, false), (4, false), (4, true)] {
            let list = scene(scale as f32, samples == 1);
            let viewport = NativeUIViewport::new(pixels, logical);
            let output = context.output(pixels);
            let multisample = (samples != 1).then(|| {
                let mut descriptor = TextureDescriptor::new(
                    pixels[0],
                    pixels[1],
                    TextureFormat::Bgra8Unorm,
                    TextureUsage::COLOR_TARGET,
                );
                descriptor.sample_count = samples;
                descriptor.transient_attachment = transient;
                TextureResource::new(context.device.clone(), descriptor)
                    .expect("multisample target")
            });

            context
                .native
                .configure(TextureFormat::Bgra8Unorm, samples)
                .expect("configure");

            // Warm the shader, the pipeline and the upload buffers before timing.
            for _ in 0..4 {
                context.render(&list, &output, multisample.as_ref(), viewport, !transient);
            }

            let profile = Arc::new(GpuTimestampProfile::new(100));
            context
                .device
                .set_gpu_timestamp_profile(Some(profile.clone()));

            let mut cpu = Vec::new();
            let mut statistics = NativeUIDrawStatistics::default();
            for _ in 0..30 {
                let started = std::time::Instant::now();
                statistics =
                    context.render(&list, &output, multisample.as_ref(), viewport, !transient);
                cpu.push(started.elapsed().as_secs_f64() * 1000.0);
                context.device.wait_until_idle().expect("wait for idle");
            }
            context.device.set_gpu_timestamp_profile(None);

            let image = context.read(&output);
            if transient && let Some(baseline) = &persistent {
                assert_eq!(
                    &image, baseline,
                    "a transient attachment reproduces the persistent 4x image \
                     (scale {scale})"
                );
            } else if samples == 4 {
                persistent = Some(image.clone());
            }
            write_image(
                &image,
                pixels[0],
                pixels[1],
                &directory.join(if transient {
                    format!("{samples}sample-transient-{scale}x")
                } else {
                    format!("{samples}sample-{scale}x")
                }),
            )
            .expect("the frame image is written");

            let errors = error(&image, &reference, pixels[0], pixels[1], scale);
            let mut gpu: Vec<f64> = profile
                .recorded_markers()
                .iter()
                .map(|marker| marker.nanoseconds as f64 / 1e6)
                .collect();
            let gpu_median = if gpu.is_empty() {
                None
            } else {
                Some(median(&mut gpu))
            };

            report.push(serde_json::json!({
                "scale": scale,
                "samples": samples,
                "transientAttachment": transient,
                "allocatedMSAABytes": multisample
                    .as_ref()
                    .and_then(|resource| resource.allocation())
                    .map(|allocation| allocation.allocated_bytes),
                "vertices": statistics.vertices,
                "indices": statistics.indices,
                "drawCalls": statistics.draw_calls,
                "msaaTargetBytes": if samples == 1 { 0 } else { pixels[0] * pixels[1] * 4 * samples },
                "cpuMedianMilliseconds": median(&mut cpu),
                "gpuMedianMilliseconds": gpu_median,
                "meanRGBError": errors.0,
                "edgeRGBError": errors.1,
                "triangleRGBError": errors.2,
            }));

            assert!(
                statistics.vertices > 0,
                "scale {scale} samples {samples}: real geometry was drawn"
            );
            assert_eq!(
                image.len(),
                reference.len(),
                "scale {scale} samples {samples}: the image matches the reference extent"
            );
        }

        let one = report
            .iter()
            .find(|row| row["scale"] == scale && row["samples"] == 1)
            .expect("the 1x row exists")
            .clone();
        let four = report
            .iter()
            .find(|row| row["scale"] == scale && row["samples"] == 4)
            .expect("the 4x row exists")
            .clone();
        // Raw custom triangles currently have no analytic boundary. This guards
        // the reason the global default cannot yet become 1x.
        assert!(
            four["triangleRGBError"].as_f64().unwrap() < one["triangleRGBError"].as_f64().unwrap(),
            "scale {scale}: MSAA beats analytic coverage on the raw triangle \
             ({} vs {})",
            four["triangleRGBError"],
            one["triangleRGBError"]
        );
    }

    let json = serde_json::to_string_pretty(&report).expect("the report serialises");
    std::fs::write(directory.join("report.json"), json).expect("the report is written");
}
