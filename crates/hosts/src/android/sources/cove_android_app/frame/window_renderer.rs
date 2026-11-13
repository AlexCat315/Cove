//! A surface's presentation, run on its render thread.
//!
//! The Choreographer callback only prepares the scene and hands over a draw list;
//! acquiring a drawable, recording and presenting wait here instead. The renderer
//! holds its own reference to the `ANativeWindow`, so a surface Java already
//! reclaimed stays valid until this thread has let go of it.

use super::timings::{FrameTimings, Phase};
use crate::android::sources::cove_android_app::bridge::AcquiredNativeWindow;
use crate::desktop::application::memory::{AppWindowMemorySnapshot, AppWindowRendererState};
use crate::desktop::window::native_renderer::{
    AppWindowRenderer, CapturedFrame, capture_framebuffer,
};
use cove_core::DrawList;
use cove_graphics::rhi::{
    CommandBuffer, Device, QueueClass, RenderColorTarget, RhiError, RhiResult, SurfaceDescriptor,
    Swapchain, TextureDescriptor, TextureFormat, TextureResource, TextureUsage,
};
use cove_graphics::ui::{FontAtlasUpload, NativeDrawListRenderer, NativeUIViewport};
use cove_logging::{Level, LogEvent, LogSystem};
use cove_shell::NativeRenderSurface;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

/// What the UI side measured for one frame, in submission order.
pub struct FrameSample {
    pub timestamp: u64,
    pub prepare_nanos: u64,
}

/// Facts the open log line reports alongside the swapchain's.
pub struct SurfaceLaunchInfo {
    pub capture_enabled: bool,
    pub benchmark: bool,
    pub gpu_timings: bool,
}

pub struct AndroidWindowRenderer {
    device: Arc<Device>,
    renderer: NativeDrawListRenderer,
    samples: usize,
    clear_color: [f32; 4],
    info: SurfaceLaunchInfo,
    window: Option<AcquiredNativeWindow>,
    swapchain: Option<Swapchain>,
    framebuffer: Option<TextureResource>,
    frame_logical_size: Option<(f32, f32)>,
    timings: FrameTimings,
    frame_samples: Receiver<FrameSample>,
    state: AppWindowRendererState,
}

impl AndroidWindowRenderer {
    pub fn new(
        device: Arc<Device>,
        renderer: NativeDrawListRenderer,
        samples: usize,
        clear_color: [f32; 4],
        timings: FrameTimings,
        frame_samples: Receiver<FrameSample>,
        info: SurfaceLaunchInfo,
    ) -> Self {
        Self {
            device,
            renderer,
            samples,
            clear_color,
            info,
            window: None,
            swapchain: None,
            framebuffer: None,
            frame_logical_size: None,
            timings,
            frame_samples,
            state: AppWindowRendererState::Unconfigured,
        }
    }

    fn present(&mut self, list: &DrawList, logical: (f32, f32)) -> RhiResult<bool> {
        let device = self.device.clone();
        let Some(swapchain) = self.swapchain else {
            return Ok(false);
        };
        self.timings
            .measure_result(Phase::FrameSlot, || device.begin_frame())?;
        struct EndFrame<'a>(&'a Device);
        impl Drop for EndFrame<'_> {
            fn drop(&mut self) {
                self.0.end_frame();
            }
        }
        let _end = EndFrame(&device);
        let image = self.timings.measure_result(Phase::DrawableAcquire, || {
            device.acquire_swapchain_image(swapchain)
        })?;
        // A frosted panel samples what is already behind it, and a swapchain
        // image is a render target that cannot be sampled. Only pay for the
        // owned texture — and the blit that follows — on frames that really
        // have a panel doing the sampling; the rest render straight into the
        // drawable.
        let frosted = self.renderer.supports_backdrop()
            && list.batches().iter().any(|batch| batch.backdrop.is_some());
        if !frosted && !self.info.capture_enabled {
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
                    TextureUsage::COLOR_TARGET
                        | TextureUsage::SAMPLED
                        | TextureUsage::TRANSFER_SOURCE,
                )
                .with_label("android-ui-framebuffer"),
            )?);
        }
        let viewport = NativeUIViewport::new([image.width, image.height], [logical.0, logical.1]);
        let mut commands = CommandBuffer::new();
        let framebuffer_texture = self.framebuffer.as_ref().map(|buffer| buffer.texture());
        let target_texture = framebuffer_texture.unwrap_or(image.texture);
        let clear_color = self.clear_color;
        let renderer = &mut self.renderer;
        let frame = self
            .timings
            .measure_result(Phase::CommandRecord, || -> RhiResult<_> {
                let frame = renderer.record(
                    list,
                    &mut commands,
                    RenderColorTarget::clearing(target_texture, clear_color),
                    viewport,
                    framebuffer_texture.filter(|_| frosted),
                )?;
                if framebuffer_texture.is_some() {
                    renderer.record_blit(
                        target_texture,
                        &mut commands,
                        RenderColorTarget::new(image.texture),
                        cove_graphics::rhi::platform_default_surface_format(),
                        [image.width, image.height],
                    )?;
                }
                Ok(frame)
            })?;
        self.timings.measure_result(Phase::QueueSubmit, || {
            device.submit(&commands, QueueClass::Graphics, &[], &[], None)
        })?;
        let mut frame = frame;
        frame.did_submit();
        self.timings
            .measure_result(Phase::Present, || device.present(image))?;
        self.frame_logical_size = Some(logical);
        Ok(true)
    }

    fn release_swapchain(&mut self) -> RhiResult<()> {
        let swapchain = self.swapchain.take();
        self.framebuffer = None;
        let device = self.device.clone();
        device.with_frame_session(|| {
            device.wait_until_idle()?;
            if let Some(swapchain) = swapchain {
                device.destroy_swapchain(swapchain);
            }
            Ok(())
        })
    }
}

impl AppWindowRenderer for AndroidWindowRenderer {
    fn is_configured(&self) -> bool {
        self.swapchain.is_some()
    }

    fn memory_snapshot(&self) -> AppWindowMemorySnapshot {
        let bytes = self.framebuffer.as_ref().map_or(0, |buffer| {
            let descriptor = buffer.descriptor();
            descriptor.width * descriptor.height * 4
        });
        AppWindowMemorySnapshot {
            state: self.state,
            samples: self.samples as u32,
            has_swapchain: self.swapchain.is_some(),
            estimated_target_bytes: bytes + self.renderer.estimated_presentation_texture_bytes(),
            msaa_logical_bytes: self.renderer.presentation_msaa_logical_bytes(),
            msaa_allocated_bytes: self.renderer.presentation_msaa_allocated_bytes(),
            msaa_memoryless: self.renderer.presentation_msaa_memoryless(),
        }
    }

    fn configure(
        &mut self,
        native: NativeRenderSurface,
        size: (u32, u32),
        vsync: bool,
    ) -> RhiResult<()> {
        let NativeRenderSurface::AndroidWindow(raw) = native else {
            return Err(RhiError::invalid_argument(
                "an Android surface needs an ANativeWindow",
            ));
        };
        let window = AcquiredNativeWindow::acquire(raw)
            .ok_or_else(|| RhiError::invalid_argument("ANativeWindow_acquire is unavailable"))?;
        let device = self.device.clone();
        let swapchain = device.with_frame_session(|| {
            device.make_swapchain(
                SurfaceDescriptor::new(raw)
                    .with_size(size.0 as usize, size.1 as usize)
                    .with_vsync(vsync),
            )
        })?;
        let presentation = device.presentation_mode(swapchain).ok();
        LogSystem::logger("com.cove.android.surface").log(&LogEvent::new(
            Level::Info,
            format!(
                "OPEN {} {}x{} vsyncRequested={} presentMode={} displaySynchronized={} benchmark={} gpuTimings={}",
                device.device_name(),
                size.0,
                size.1,
                vsync,
                presentation.map_or_else(|| "unavailable".to_owned(), |mode| format!("{mode:?}")),
                presentation
                    .map(|mode| mode.synchronizes_to_display().to_string())
                    .unwrap_or_else(|| "unavailable".to_owned()),
                self.info.benchmark,
                self.info.gpu_timings
            ),
        ));
        self.window = Some(window);
        self.swapchain = Some(swapchain);
        self.state = AppWindowRendererState::Active;
        Ok(())
    }

    fn resize(&mut self, size: (u32, u32), vsync: bool) -> RhiResult<()> {
        let (Some(window), Some(swapchain)) = (&self.window, self.swapchain) else {
            return Ok(());
        };
        if size.0 == 0 || size.1 == 0 {
            return Ok(());
        }
        let descriptor = SurfaceDescriptor::new(window.raw())
            .with_size(size.0 as usize, size.1 as usize)
            .with_vsync(vsync);
        let device = self.device.clone();
        device.with_frame_session(|| device.configure_swapchain(swapchain, descriptor))
    }

    fn draw(&mut self, list: &mut DrawList, logical: (f32, f32)) -> RhiResult<bool> {
        let sample = self.frame_samples.try_recv().ok();
        self.timings.begin_with(
            Phase::ScenePrepare,
            sample.as_ref().map_or(0, |s| s.prepare_nanos),
        );
        let device = self.device.clone();
        let presented = device.with_frame_session(|| self.present(list, logical))?;
        self.timings.finish(
            sample.map_or(0, |s| s.timestamp),
            list.vertices().len(),
            list.indices().len(),
        );
        Ok(presented)
    }

    fn capture(&mut self) -> RhiResult<CapturedFrame> {
        capture_framebuffer(
            &self.device,
            self.framebuffer.as_ref(),
            self.frame_logical_size,
        )
    }

    fn upload_font_atlas(&mut self, upload: FontAtlasUpload) -> RhiResult<()> {
        let device = self.device.clone();
        let renderer = &self.renderer;
        self.timings.measure_result(Phase::AtlasUpload, || {
            device.with_frame_session(|| renderer.apply_font_atlas_upload(upload))
        })
    }

    /// Java tears the surface down instead; there is no suspended state here.
    fn set_presentation_active(&mut self, _active: bool) -> RhiResult<()> {
        Ok(())
    }

    fn trim_memory(&mut self) -> RhiResult<()> {
        let device = self.device.clone();
        let renderer = &mut self.renderer;
        let result = device.with_frame_session(|| {
            device.wait_until_idle()?;
            renderer.trim_presentation_resources();
            Ok(())
        });
        self.framebuffer = None;
        result
    }

    fn close(&mut self) -> RhiResult<()> {
        let result = self.release_swapchain();
        self.renderer.trim_presentation_resources();
        self.window = None;
        self.state = AppWindowRendererState::Closed;
        LogSystem::logger("com.cove.android.surface").log(&LogEvent::new(Level::Info, "CLOSE"));
        result
    }
}
