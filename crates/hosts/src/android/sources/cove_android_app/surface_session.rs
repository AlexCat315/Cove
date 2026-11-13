//! Translation of Android/CoveAndroidApp/SurfaceSession.swift.

use super::AndroidApplication;
use super::content::scene::HostedScene;
use super::frame::gpu_timings::GpuTimingReporter;
use super::frame::timings::FrameTimings;
use super::frame::window_renderer::{AndroidWindowRenderer, FrameSample, SurfaceLaunchInfo};
use crate::android::contract::frame_clock::DisplayFramePacer;
use crate::desktop::rendering::{FrameToken, ThreadedWindowRenderer};
use cove_graphics::rhi::{Device, RhiResult, TextureFormat};
use cove_graphics::ui::{FontAtlasUpload, NativeDrawListRenderer};
use cove_shell::NativeRenderSurface;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Instant;

/// JNI serializes this surface's scene operations on the UI thread; GPU work runs
/// on the surface's render thread.
pub struct SurfaceSession {
    application: Rc<AndroidApplication>,
    pixels: (usize, usize),
    clock: DisplayFramePacer,
    gpu_timings: GpuTimingReporter,
    window_renderer: Rc<RefCell<ThreadedWindowRenderer>>,
    frame_samples: Sender<FrameSample>,
}

impl SurfaceSession {
    pub fn new(
        application: Rc<AndroidApplication>,
        window: usize,
        width: usize,
        height: usize,
    ) -> Result<Self, String> {
        let device: Arc<Device> = application.device();
        let gpu_timings = GpuTimingReporter::new(&device, application.runtime().timings);
        let vsync = application.runtime().vsync;
        let presentation = {
            let root = application.renderer();
            device
                .with_frame_session(|| {
                    root.borrow().make_presentation_renderer(
                        TextureFormat::Rgba8Unorm,
                        application.samples(),
                    )
                })
                .map_err(|error| error.to_string())?
        };
        let (frame_samples, samples) = channel();
        let mut window_renderer = ThreadedWindowRenderer::spawn(
            Box::new(AndroidWindowRenderer::new(
                device,
                presentation,
                application.samples(),
                application.clear_color(),
                FrameTimings::new(application.runtime().timings),
                samples,
                SurfaceLaunchInfo {
                    capture_enabled: application.scene().borrow().content().has_diagnostics(),
                    benchmark: application.runtime().benchmark,
                    gpu_timings: gpu_timings.is_recording(),
                },
            )),
            "android-surface",
        );
        window_renderer
            .configure(
                NativeRenderSurface::AndroidWindow(window),
                (width as u32, height as u32),
                vsync,
            )
            .map_err(|error| error.to_string())?;
        let window_renderer = Rc::new(RefCell::new(window_renderer));
        application.attach_window_renderer(&window_renderer);
        application.scene().borrow().invalidate();
        Ok(Self {
            application,
            pixels: (width, height),
            clock: DisplayFramePacer::default(),
            gpu_timings,
            window_renderer,
            frame_samples,
        })
    }

    pub fn application(&self) -> &Rc<AndroidApplication> {
        &self.application
    }

    pub fn scene(&self) -> Ref<'_, HostedScene> {
        self.application.scene().borrow()
    }

    pub fn scene_mut(&self) -> RefMut<'_, HostedScene> {
        self.application.scene().borrow_mut()
    }

    pub fn device(&self) -> Arc<Device> {
        self.application.device()
    }

    pub fn renderer(&self) -> Rc<RefCell<NativeDrawListRenderer>> {
        self.application.renderer()
    }

    pub fn scale(&self) -> f32 {
        self.application.scale()
    }

    /// Prepares the scene and hands its draw list to the render thread. Returns
    /// whether a frame was handed over; with every frame in flight the callback
    /// returns at once and the next vsync tries again.
    pub fn frame(&mut self, timestamp: u64) -> Result<bool, String> {
        self.application
            .scene()
            .borrow()
            .content()
            .pump_diagnostics();
        self.collect_presented()
            .map_err(|error| error.to_string())?;
        if !self.window_renderer.borrow().has_frame_capacity() {
            return Ok(false);
        }
        let needs_frame =
            self.application.runtime().benchmark || self.application.scene().borrow().needs_frame();
        let Some(delta_time) =
            self.clock
                .advance(timestamp, needs_frame, self.application.target_frame_rate())
        else {
            return Ok(false);
        };
        let scale = self.scale();
        let pixels = self.pixels;
        let prepare_start = Instant::now();
        self.application
            .scene()
            .borrow_mut()
            .prepare(pixels, scale, delta_time);
        let prepare_nanos = prepare_start.elapsed().as_nanos() as u64;
        let submitted = self
            .submit(timestamp, prepare_nanos)
            .map_err(|error| error.to_string())?;
        if let Some(token) = submitted {
            self.application.scene().borrow_mut().did_submit(token.0);
        }
        self.gpu_timings.tick(timestamp);
        Ok(submitted.is_some())
    }

    fn collect_presented(&mut self) -> RhiResult<()> {
        let frames = self.window_renderer.borrow_mut().take_presented()?;
        for frame in &frames {
            self.application
                .scene()
                .borrow()
                .content()
                .presentation_finished(frame.token.0, frame.presented, frame.cpu_ms);
        }
        if frames.iter().any(|frame| !frame.presented) {
            self.application.scene().borrow().invalidate();
        }
        Ok(())
    }

    fn submit(&mut self, timestamp: u64, prepare_nanos: u64) -> RhiResult<Option<FrameToken>> {
        let mut renderer = self.window_renderer.borrow_mut();
        let scene = self.application.scene().borrow();
        // Ordered with the frame, so the glyphs land before they are sampled.
        if let Some(upload) = FontAtlasUpload::take(&scene.font_atlas(), 1)? {
            renderer.upload_font_atlas(upload)?;
        }
        let logical = scene.size();
        let mut list = scene.list().clone();
        drop(scene);
        let _ = self.frame_samples.send(FrameSample {
            timestamp,
            prepare_nanos,
        });
        renderer.submit(&mut list, logical)
    }

    /// Waits for the render thread to release the swapchain, within a bound; the
    /// caller releases its window reference right after.
    pub fn close(&mut self) {
        let _ = self.window_renderer.borrow_mut().close();
    }
}
