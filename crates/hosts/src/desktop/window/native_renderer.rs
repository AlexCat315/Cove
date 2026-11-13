//! Translation of Desktop/Sources/CoveUI/Window/Renderer.swift and
//! Window/NativeRenderer.swift.
//!
//! Presentation ownership is separate from layout and input. A renderer is
//! `Send`: hosts run it on a dedicated render thread
//! ([`super::super::rendering::render_thread::ThreadedWindowRenderer`]) and hand
//! it each frame's draw list.

use super::super::application::memory::{AppWindowMemorySnapshot, AppWindowRendererState};
use cove_core::DrawList;
use cove_graphics::rhi::{
    CommandBuffer, Device, RenderColorTarget, RhiError, RhiResult, SurfaceDescriptor, Swapchain,
    TextureDescriptor, TextureFormat, TextureResource, TextureSubresource, TextureUsage,
};
use cove_graphics::ui::{FontAtlasUpload, NativeDrawListRenderer, NativeUIViewport};
use cove_runtime::SurfaceFactory;
use cove_shell::NativeRenderSurface;
use std::sync::Arc;

/// Samples, clear colour and swapchain format for one window's presentation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppWindowRenderSettings {
    pub samples: u32,
    pub clear_color: [f32; 4],
    /// A window that shows a platform material through clear pixels presents
    /// linear float, so the compositor reads its translucent pixels as the
    /// linear premultiplied colour the frame holds.
    pub surface_format: TextureFormat,
    pub premultiplied_alpha: bool,
}

pub struct CapturedFrame {
    pub logical_size: (f32, f32),
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Reads a renderer-owned resolved image on its render thread.
pub(crate) fn capture_framebuffer(
    device: &Device,
    framebuffer: Option<&TextureResource>,
    logical_size: Option<(f32, f32)>,
) -> RhiResult<CapturedFrame> {
    let framebuffer =
        framebuffer.ok_or_else(|| RhiError::invalid_argument("No rendered frame is available."))?;
    let logical_size = logical_size
        .ok_or_else(|| RhiError::invalid_argument("No rendered frame is available."))?;
    let descriptor = framebuffer.descriptor();
    let (width, height) = (descriptor.width, descriptor.height);
    let byte_count = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .filter(|&bytes| bytes <= 128 * 1024 * 1024)
        .ok_or_else(|| {
            RhiError::invalid_argument("The framebuffer exceeds the capture size limit.")
        })?;
    let mut rgba = vec![0; byte_count];
    device.with_frame_session(|| {
        device.wait_until_idle()?;
        device.read_texture_data(
            framebuffer.texture(),
            width,
            height,
            width * 4,
            TextureSubresource::default(),
            &mut rgba,
        )
    })?;
    if matches!(
        descriptor.format,
        TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb
    ) {
        for pixel in rgba.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
    }
    // The render target stores premultiplied color. PNG expects straight alpha.
    let srgb = matches!(
        descriptor.format,
        TextureFormat::Bgra8UnormSrgb | TextureFormat::Rgba8UnormSrgb
    );
    for pixel in rgba.as_chunks_mut::<4>().0 {
        let alpha = f32::from(pixel[3]) / 255.0;
        if alpha > 0.0 && alpha < 1.0 {
            for channel in &mut pixel[..3] {
                let encoded = f32::from(*channel) / 255.0;
                let linear = if !srgb {
                    encoded
                } else if encoded <= 0.04045 {
                    encoded / 12.92
                } else {
                    ((encoded + 0.055) / 1.055).powf(2.4)
                };
                let straight = (linear / alpha).min(1.0);
                let encoded = if !srgb {
                    straight
                } else if straight <= 0.0031308 {
                    straight * 12.92
                } else {
                    1.055 * straight.powf(1.0 / 2.4) - 0.055
                };
                *channel = (encoded * 255.0).round() as u8;
            }
        }
    }
    Ok(CapturedFrame {
        logical_size,
        width: width as u32,
        height: height as u32,
        rgba,
    })
}

/// Presentation ownership for one window. Runs on the render thread, where its
/// calls may wait on the GPU and the display.
pub trait AppWindowRenderer {
    fn is_configured(&self) -> bool;
    fn memory_snapshot(&self) -> AppWindowMemorySnapshot;
    fn configure(
        &mut self,
        native: NativeRenderSurface,
        size: (u32, u32),
        vsync: bool,
    ) -> RhiResult<()>;
    fn resize(&mut self, size: (u32, u32), vsync: bool) -> RhiResult<()>;
    /// Present `list`, returning false when presentation was skipped.
    fn draw(
        &mut self,
        list: &mut DrawList,
        logical: (f32, f32),
        gpu: Option<&mut Option<cove_graphics::rhi::GpuFrameTiming>>,
    ) -> RhiResult<bool>;
    /// Write copied font-atlas regions before the next frame is encoded.
    fn upload_font_atlas(&mut self, upload: FontAtlasUpload) -> RhiResult<()>;
    fn capture(&mut self) -> RhiResult<CapturedFrame> {
        Err(RhiError::unsupported_feature(
            "This renderer has no app-local capture.",
        ))
    }
    fn set_presentation_active(&mut self, active: bool) -> RhiResult<()>;
    /// Releases what the next frame can rebuild.
    fn trim_memory(&mut self) -> RhiResult<()> {
        Ok(())
    }
    fn close(&mut self) -> RhiResult<()>;
}

/// Swapchain-backed renderer that presents the draw list through an owned
/// framebuffer so a frosted panel can sample the resolved frame.
/// Samples retained when a DevTools session reads GPU pass timings. The session
/// drains them per frame, so this bound only matters when a reader stalls.
const GPU_MARKER_LIMIT: usize = 4096;

pub struct NativeAppWindowRenderer {
    device: Arc<Device>,
    renderer: NativeDrawListRenderer,
    settings: AppWindowRenderSettings,
    native: Option<NativeRenderSurface>,
    swapchain: Option<Swapchain>,
    size: (u32, u32),
    /// Owned copy of the presented frame, so a panel can blur it.
    framebuffer: Option<TextureResource>,
    frame_logical_size: Option<(f32, f32)>,
    render_in_background: bool,
    presentation_active: bool,
    vsync: bool,
    /// GPU sampling attached while a DevTools session reads frames. Detached
    /// again when the session goes away, so ordinary frames pay nothing.
    gpu_profile: Option<std::sync::Arc<cove_graphics::rhi::GpuTimestampProfile>>,
    state: AppWindowRendererState,
}

impl NativeAppWindowRenderer {
    pub fn new(
        device: Arc<Device>,
        renderer: NativeDrawListRenderer,
        settings: AppWindowRenderSettings,
    ) -> Self {
        Self::with_background_rendering(device, renderer, settings, false)
    }

    pub(crate) fn with_background_rendering(
        device: Arc<Device>,
        renderer: NativeDrawListRenderer,
        settings: AppWindowRenderSettings,
        render_in_background: bool,
    ) -> Self {
        Self {
            device,
            renderer,
            settings,
            native: None,
            swapchain: None,
            size: (0, 0),
            framebuffer: None,
            frame_logical_size: None,
            render_in_background,
            presentation_active: true,
            vsync: true,
            gpu_profile: None,
            state: AppWindowRendererState::Unconfigured,
        }
    }

    pub fn state(&self) -> AppWindowRendererState {
        self.state
    }

    fn surface_descriptor(
        &self,
        native: NativeRenderSurface,
        size: (u32, u32),
        vsync: bool,
    ) -> SurfaceDescriptor {
        let mut descriptor = SurfaceFactory::native_descriptor(native, [size.0, size.1], vsync);
        descriptor.color_format = self.settings.surface_format;
        descriptor.premultiplied_alpha = self.settings.premultiplied_alpha;
        descriptor
    }

    /// The swapchain the shell surface currently backs, if any.
    pub fn swapchain(&self) -> Option<Swapchain> {
        self.swapchain
    }
}

impl AppWindowRenderer for NativeAppWindowRenderer {
    fn is_configured(&self) -> bool {
        self.native.is_some()
    }

    fn memory_snapshot(&self) -> AppWindowMemorySnapshot {
        let bytes = self.framebuffer.as_ref().map_or(0, |buffer| {
            let descriptor = buffer.descriptor();
            descriptor.width * descriptor.height * 4
        });
        AppWindowMemorySnapshot {
            state: self.state,
            samples: self.settings.samples,
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
        if self.native.is_some() {
            return Err(cove_graphics::rhi::RhiError::invalid_argument(
                "window is already configured",
            ));
        }
        let device = self.device.clone();
        device.with_frame_session(|| {
            let descriptor = self.surface_descriptor(native, size, vsync);
            let swapchain = device.make_swapchain(descriptor)?;
            self.native = Some(native);
            self.swapchain = Some(swapchain);
            self.size = size;
            self.vsync = vsync;
            self.presentation_active = true;
            self.state = AppWindowRendererState::Active;
            Ok(())
        })
    }

    fn resize(&mut self, size: (u32, u32), vsync: bool) -> RhiResult<()> {
        self.size = size;
        self.vsync = vsync;
        let (Some(native), Some(swapchain)) = (self.native, self.swapchain) else {
            return Ok(());
        };
        // Native swapchains require a positive extent. The host suspends
        // zero-sized windows; keep their desired size for the next activation.
        if size.0 == 0 || size.1 == 0 {
            return Ok(());
        }
        let device = self.device.clone();
        device.with_frame_session(|| {
            device.configure_swapchain(swapchain, self.surface_descriptor(native, size, vsync))
        })
    }

    fn upload_font_atlas(&mut self, upload: FontAtlasUpload) -> RhiResult<()> {
        let device = self.device.clone();
        device.with_frame_session(|| self.renderer.apply_font_atlas_upload(upload))
    }

    fn draw(
        &mut self,
        list: &mut DrawList,
        logical: (f32, f32),
        mut gpu: Option<&mut Option<cove_graphics::rhi::GpuFrameTiming>>,
    ) -> RhiResult<bool> {
        let list: &DrawList = list;
        if self.native.is_none() && !self.render_in_background {
            return Ok(false);
        }
        if (!self.presentation_active && !self.render_in_background)
            || self.size.0 == 0
            || self.size.1 == 0
        {
            return Ok(false);
        }
        let device = self.device.clone();
        let size = self.size;
        let vsync = self.vsync;
        device.with_frame_session(|| {
            if self.presentation_active
                && self.swapchain.is_none()
                && let Some(native) = self.native
            {
                self.swapchain =
                    Some(device.make_swapchain(self.surface_descriptor(native, size, vsync))?);
            }
            self.renderer.configure(
                TextureFormat::Bgra8UnormSrgb,
                self.settings.samples as usize,
            )?;
            device.begin_frame()?;
            struct EndFrame<'a>(&'a Device);
            impl Drop for EndFrame<'_> {
                fn drop(&mut self) {
                    self.0.end_frame();
                }
            }
            let _end = EndFrame(&device);
            if let Some(slot) = gpu.as_deref_mut() {
                *slot = device.measure_gpu_frame();
                // Sample passes only while something reads them: a profile
                // allocates a query pool per submission on Vulkan.
                if self.gpu_profile.is_none() && device.supports_gpu_timestamps() {
                    let profile = std::sync::Arc::new(
                        cove_graphics::rhi::GpuTimestampProfile::new(GPU_MARKER_LIMIT),
                    );
                    device.set_gpu_timestamp_profile(Some(profile.clone()));
                    self.gpu_profile = Some(profile);
                }
            } else if self.gpu_profile.take().is_some() {
                device.set_gpu_timestamp_profile(None);
            }
            let image = self
                .swapchain
                .map(|swapchain| device.acquire_swapchain_image(swapchain))
                .transpose()?;
            let pixels = image.map_or((size.0 as usize, size.1 as usize), |image| {
                (image.width, image.height)
            });
            let clear = self.settings.clear_color;
            // Resolved history serves every frame. Effects never promote the
            // raster workspace into a persistent full-frame MSAA allocation.
            let resize_framebuffer = self.framebuffer.as_ref().is_none_or(|buffer| {
                let descriptor = buffer.descriptor();
                descriptor.width != pixels.0 || descriptor.height != pixels.1
            });
            if resize_framebuffer {
                self.framebuffer = Some(TextureResource::new(
                    device.clone(),
                    TextureDescriptor::new(
                        pixels.0,
                        pixels.1,
                        TextureFormat::Bgra8UnormSrgb,
                        TextureUsage::COLOR_TARGET
                            | TextureUsage::SAMPLED
                            | TextureUsage::TRANSFER_SOURCE,
                    )
                    .with_label("ui-framebuffer"),
                )?);
            }
            let Some(framebuffer) = &self.framebuffer else {
                return Ok(false);
            };
            let mut commands = CommandBuffer::new();
            let viewport = NativeUIViewport::new([pixels.0, pixels.1], [logical.0, logical.1]);
            let frame = self.renderer.record_presentation(
                list,
                &mut commands,
                framebuffer.texture(),
                viewport,
                clear,
                usize::MAX,
            )?;
            if let Some(image) = image {
                let present = RenderColorTarget {
                    resolve_texture: None,
                    texture: image.texture,
                    load_action: cove_graphics::rhi::ColorLoadAction::DontCare,
                    store: true,
                };
                self.renderer.record_blit(
                    framebuffer.texture(),
                    &mut commands,
                    present,
                    self.settings.surface_format,
                    [pixels.0, pixels.1],
                )?;
            }
            // Publish what this frame recorded so the host can attribute passes
            // to composited layers once the GPU reports completion.
            if let Some(frame) = gpu.as_deref().and_then(|slot| slot.as_ref()) {
                frame.set_pass_reports(commands.pass_reports());
                if let Some(profile) = &self.gpu_profile {
                    frame.set_timestamp_profile(profile.clone());
                }
            }
            device.submit(
                &commands,
                cove_graphics::rhi::QueueClass::Graphics,
                &[],
                &[],
                None,
            )?;
            let mut frame = frame;
            frame.did_submit();
            if let Some(image) = image {
                device.present(image)?;
            }
            self.frame_logical_size = Some(logical);
            Ok(image.is_some())
        })
    }

    fn capture(&mut self) -> RhiResult<CapturedFrame> {
        capture_framebuffer(
            &self.device,
            self.framebuffer.as_ref(),
            self.frame_logical_size,
        )
    }

    fn set_presentation_active(&mut self, active: bool) -> RhiResult<()> {
        if self.presentation_active == active {
            return Ok(());
        }
        self.presentation_active = active;
        if self.native.is_some() {
            self.state = if active {
                AppWindowRendererState::Active
            } else {
                AppWindowRendererState::Suspended
            };
        }
        if !active {
            // Dropping attachment ownership waits for in-flight work first.
            if let Some(swapchain) = self.swapchain.take() {
                let device = self.device.clone();
                device.with_frame_session(|| {
                    device.wait_until_idle()?;
                    device.destroy_swapchain(swapchain);
                    Ok(())
                })?;
            }
            if !self.render_in_background {
                self.framebuffer = None;
                self.renderer.trim_presentation_resources();
            }
        }
        Ok(())
    }

    fn close(&mut self) -> RhiResult<()> {
        let swapchain = self.swapchain.take();
        self.native = None;
        self.framebuffer = None;
        self.size = (0, 0);
        self.state = AppWindowRendererState::Closed;
        let device = self.device.clone();
        device.with_frame_session(|| {
            if let Some(swapchain) = swapchain {
                device.wait_until_idle()?;
                device.destroy_swapchain(swapchain);
            }
            Ok(())
        })?;
        self.renderer.trim_presentation_resources();
        Ok(())
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use cove_core::{Color, Rect};
    use cove_graphics::rhi::{DeviceConfig, GraphicsApi};

    #[test]
    fn capture_reads_the_current_offscreen_render_target() {
        let device = Arc::new(
            Device::make(DeviceConfig {
                preferred_backends: vec![GraphicsApi::Metal],
                ..DeviceConfig::default()
            })
            .expect("Metal device"),
        );
        let renderer = NativeDrawListRenderer::new(device.clone()).unwrap();
        let mut renderer = NativeAppWindowRenderer::with_background_rendering(
            device,
            renderer,
            AppWindowRenderSettings {
                samples: 1,
                clear_color: [0.0, 0.0, 0.0, 1.0],
                surface_format: TextureFormat::Bgra8UnormSrgb,
                premultiplied_alpha: false,
            },
            true,
        );
        renderer.size = (128, 96);
        let mut list = DrawList::default();
        let _ = list.add_rect(
            Rect::new(16.0, 12.0, 32.0, 24.0),
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        );
        assert!(!renderer.draw(&mut list, (64.0, 48.0)).unwrap());
        let first = renderer.capture().unwrap();
        assert_eq!(first.logical_size, (64.0, 48.0));
        assert_eq!((first.width, first.height), (128, 96));
        let offset = (30 * 128 + 40) * 4;
        assert_eq!(&first.rgba[offset..offset + 4], &[255, 0, 0, 255]);
        list.reset();
        assert!(!renderer.draw(&mut list, (64.0, 48.0)).unwrap());
        let next = renderer.capture().unwrap();
        assert_eq!(&next.rgba[offset..offset + 4], &[0, 0, 0, 255]);
    }
}
