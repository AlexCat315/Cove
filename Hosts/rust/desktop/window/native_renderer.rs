//! Translation of Desktop/Sources/CoveUI/Window/Renderer.swift and
//! Window/NativeRenderer.swift.
//!
//! Presentation ownership is separate from layout and input. All calls occur on
//! the UI thread; the renderer also coordinates scene frames.

use super::super::application::memory::{
    AppWindowMemorySnapshot, AppWindowRendererState,
};
use cove_core::DrawList;
use cove_graphics::rhi::{
    CommandBuffer, Device, RenderColorTarget, RhiResult, Swapchain, TextureDescriptor,
    TextureFormat, TextureResource, TextureUsage,
};
use cove_graphics::ui::{NativeDrawListRenderer, NativeUIViewport};
use cove_runtime::SurfaceFactory;
use cove_shell::NativeRenderSurface;
use std::sync::Arc;

/// Samples and clear colour for one window's presentation target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AppWindowRenderSettings {
    pub samples: u32,
    pub clear_color: [f32; 4],
}

/// Presentation ownership for one window.
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
    fn draw(&mut self, list: &DrawList, logical: (f32, f32)) -> RhiResult<bool>;
    fn set_presentation_active(&mut self, active: bool) -> RhiResult<()>;
    fn close(&mut self) -> RhiResult<()>;
}

/// Swapchain-backed renderer that presents the draw list through an owned
/// framebuffer so a frosted panel can sample the resolved frame.
pub struct NativeAppWindowRenderer {
    device: Arc<Device>,
    renderer: NativeDrawListRenderer,
    settings: AppWindowRenderSettings,
    native: Option<NativeRenderSurface>,
    swapchain: Option<Swapchain>,
    size: (u32, u32),
    /// Owned copy of the presented frame, so a panel can blur it.
    framebuffer: Option<TextureResource>,
    presentation_active: bool,
    vsync: bool,
    state: AppWindowRendererState,
}

impl NativeAppWindowRenderer {
    pub fn new(
        device: Arc<Device>,
        renderer: NativeDrawListRenderer,
        settings: AppWindowRenderSettings,
    ) -> Self {
        Self {
            device,
            renderer,
            settings,
            native: None,
            swapchain: None,
            size: (0, 0),
            framebuffer: None,
            presentation_active: true,
            vsync: true,
            state: AppWindowRendererState::Unconfigured,
        }
    }

    pub fn state(&self) -> AppWindowRendererState {
        self.state
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
        let bytes = self
            .framebuffer
            .as_ref()
            .map_or(0, |buffer| {
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
            let descriptor = SurfaceFactory::native_descriptor(
                native,
                [size.0, size.1],
                vsync,
            );
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
            device.configure_swapchain(
                swapchain,
                SurfaceFactory::native_descriptor(native, [size.0, size.1], vsync),
            )
        })
    }

    fn draw(&mut self, list: &DrawList, logical: (f32, f32)) -> RhiResult<bool> {
        let Some(native) = self.native else { return Ok(false) };
        if !self.presentation_active || self.size.0 == 0 || self.size.1 == 0 {
            return Ok(false);
        }
        let device = self.device.clone();
        let size = self.size;
        let vsync = self.vsync;
        device.with_frame_session(|| {
            if self.swapchain.is_none() {
                self.swapchain = Some(device.make_swapchain(SurfaceFactory::native_descriptor(
                    native,
                    [size.0, size.1],
                    vsync,
                ))?);
            }
            let Some(swapchain) = self.swapchain else { return Ok(false) };
            self.renderer.configure(TextureFormat::Bgra8UnormSrgb, self.settings.samples as usize)?;
            device.begin_frame()?;
            struct EndFrame<'a>(&'a Device);
            impl Drop for EndFrame<'_> {
                fn drop(&mut self) {
                    self.0.end_frame();
                }
            }
            let _end = EndFrame(&device);
            let image = device.acquire_swapchain_image(swapchain)?;
            let pixels = (image.width, image.height);
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
                        TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
                    )
                    .with_label("ui-framebuffer"),
                )?);
            }
            let Some(framebuffer) = &self.framebuffer else { return Ok(false) };
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
                [pixels.0, pixels.1],
            )?;
            device.submit(
                &commands,
                cove_graphics::rhi::QueueClass::Graphics,
                &[],
                &[],
                None,
            )?;
            let mut frame = frame;
            frame.did_submit();
            device.present(image)?;
            Ok(true)
        })
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
            self.framebuffer = None;
            self.renderer.trim_presentation_resources();
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
