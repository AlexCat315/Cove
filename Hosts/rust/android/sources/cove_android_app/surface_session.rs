//! Translation of Android/CoveAndroidApp/SurfaceSession.swift.

use super::content::scene::HostedScene;
use super::frame::gpu_timings::GpuTimingReporter;
use super::frame::timings::{FrameTimings, Phase};
use super::AndroidApplication;
use crate::android::contract::frame_clock::DisplayFrameClock;
use cove_core::DrawList;
use cove_graphics::rhi::{
    CommandBuffer, Device, NativeDrawListRenderer, QueueClass, RhiResult, SurfaceDescriptor,
    Swapchain, TextureDescriptor, TextureFormat, TextureResource, TextureUsage,
};
use cove_graphics::ui::NativeUIViewport;
use cove_logging::{Level, LogEvent, LogSystem};
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;
use std::sync::Arc;

/// JNI serializes this surface's GPU and scene operations on the UI thread.
pub struct SurfaceSession {
    application: Rc<AndroidApplication>,
    swapchain: Swapchain,
    pixels: (usize, usize),
    clock: DisplayFrameClock,
    timings: FrameTimings,
    gpu_timings: GpuTimingReporter,
    framebuffer: Option<TextureResource>,
}

impl SurfaceSession {
    pub fn new(
        application: Rc<AndroidApplication>,
        window: usize,
        width: usize,
        height: usize,
    ) -> Result<Self, String> {
        let device: Arc<Device> = application.device();
        let timings = FrameTimings::new(application.runtime().timings);
        let gpu_timings = GpuTimingReporter::new(&device, application.runtime().timings);
        let vsync = application.runtime().vsync;
        let swapchain = device
            .with_frame_session(|| {
                device.make_swapchain(
                    SurfaceDescriptor::new(window).with_size(width, height).with_vsync(vsync),
                )
            })
            .map_err(|error| error.to_string())?;
        application.scene().borrow().invalidate();
        let presentation = device.presentation_mode(swapchain).ok();
        LogSystem::logger("com.cove.android.surface").log(&LogEvent::new(
            Level::Info,
            format!(
                "OPEN {} {}x{} vsyncRequested={} presentMode={} displaySynchronized={} benchmark={} gpuTimings={}",
                device.device_name(),
                width,
                height,
                vsync,
                presentation.map_or_else(|| "unavailable".to_owned(), |mode| format!("{mode:?}")),
                presentation
                    .map(|mode| mode.synchronizes_to_display().to_string())
                    .unwrap_or_else(|| "unavailable".to_owned()),
                application.runtime().benchmark,
                gpu_timings.is_recording()
            ),
        ));
        Ok(Self {
            application,
            swapchain,
            pixels: (width, height),
            clock: DisplayFrameClock::new(),
            timings,
            gpu_timings,
            framebuffer: None,
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

    pub fn frame(&mut self, timestamp: u64) -> Result<bool, String> {
        let delta_time = self.clock.advance(timestamp);
        let needs_frame = self.application.runtime().benchmark
            || self.application.scene().borrow().needs_frame();
        if !needs_frame {
            return Ok(false);
        }
        let scale = self.scale();
        let pixels = self.pixels;
        self.timings.begin();
        {
            let mut scene = self.application.scene().borrow_mut();
            self.timings.measure(Phase::ScenePrepare, || {
                scene.prepare((pixels.0 as i32, pixels.1 as i32), scale, delta_time);
            });
        }
        let device = self.application.device();
        let result = device
            .with_frame_session(|| self.present_frame())
            .map_err(|error| error.to_string())?;
        let (vertices, indices) = {
            let scene = self.application.scene().borrow();
            let list: &DrawList = scene.list();
            (list.vertices().len(), list.indices().len())
        };
        self.timings.finish(timestamp, vertices, indices);
        self.gpu_timings.tick(timestamp);
        Ok(result)
    }

    fn present_frame(&mut self) -> RhiResult<bool> {
        let device: Arc<Device> = self.application.device();
        let renderer = self.application.renderer();
        self.timings.measure_result(Phase::FrameSlot, || device.begin_frame())?;
        struct EndFrame<'a> {
            device: &'a Device,
        }
        impl Drop for EndFrame<'_> {
            fn drop(&mut self) {
                self.device.end_frame();
            }
        }
        let _end = EndFrame { device: &device };
        let image = self
            .timings
            .measure_result(Phase::DrawableAcquire, || device.acquire_swapchain_image(self.swapchain))?;
        let atlas = self.application.scene().borrow().font_atlas();
        self.timings.measure_result(Phase::AtlasUpload, || {
            renderer.borrow().upload_font_atlas(&atlas, 1)
        })?;
        // A frosted panel samples what is already behind it, and a swapchain
        // image is a render target that cannot be sampled. Only pay for the
        // owned texture — and the blit that follows — on frames that really
        // have a panel doing the sampling; the rest render straight into the
        // drawable.
        let list: DrawList = self.application.scene().borrow().list().clone();
        let frosted = renderer.borrow().supports_backdrop()
            && list.batches().iter().any(|batch| batch.backdrop.is_some());
        if !frosted {
            self.framebuffer = None;
        } else if self.framebuffer.as_ref().is_none_or(|buffer| {
            let descriptor = buffer.descriptor();
            descriptor.width != image.width || descriptor.height != image.height
        }) {
            self.framebuffer = Some(TextureResource::new(
                device.clone(),
                TextureDescriptor::new(
                    image.width,
                    image.height,
                    TextureFormat::Rgba8Unorm,
                    TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
                )
                .with_label("android-ui-framebuffer"),
            )?);
        }
        let logical = self.application.scene().borrow().size();
        let viewport = NativeUIViewport::new([image.width, image.height], [logical.0, logical.1]);
        let mut commands = CommandBuffer::new();
        let framebuffer_texture = self.framebuffer.as_ref().map(|buffer| buffer.texture());
        let target_texture = framebuffer_texture.unwrap_or(image.texture);
        let target = framebuffer_texture.is_some();
        let record = self.timings.measure_result(Phase::CommandRecord, || -> RhiResult<_> {
            let mut renderer = renderer.borrow_mut();
            let frame = renderer.record(
                &list,
                &commands,
                cove_graphics::rhi::RenderColorTarget::clearing(
                    target_texture,
                    [0.025, 0.035, 0.06, 1.0],
                ),
                viewport,
                framebuffer_texture,
            )?;
            if target {
                renderer.record_blit(
                    target_texture,
                    &mut commands,
                    cove_graphics::rhi::RenderColorTarget::new(image.texture),
                    [image.width, image.height],
                )?;
            }
            Ok(frame)
        });
        let frame = record?;
        self.timings.measure_result(Phase::QueueSubmit, || {
            device.submit(&commands, QueueClass::Graphics, &[], &[], None)
        })?;
        let mut frame = frame;
        frame.did_submit();
        atlas.mark_clean();
        self.timings.measure_result(Phase::Present, || device.present(image))?;
        self.application.scene().borrow_mut().did_present();
        Ok(true)
    }

    pub fn close(&mut self) {
        let device = self.application.device();
        let _ = device.with_frame_session(|| {
            let _ = device.wait_until_idle();
            device.destroy_swapchain(self.swapchain);
            Ok(())
        });
        LogSystem::logger("com.cove.android.surface").log(&LogEvent::new(Level::Info, "CLOSE"));
    }
}
