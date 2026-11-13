use std::sync::Arc;

use cove_core::DrawList;
use cove_graphics::rhi::*;
use cove_graphics::ui::{NativeDrawListRenderer, NativeUIViewport};
use cove_text::FontAtlas;

pub(super) struct BrowserGpu {
    device: Arc<Device>,
    renderer: NativeDrawListRenderer,
    format: TextureFormat,
    surface: Option<(Swapchain, [usize; 2])>,
    framebuffer: Option<TextureResource>,
}

impl BrowserGpu {
    pub fn new(format: TextureFormat) -> RhiResult<Self> {
        let device = Arc::new(Device::make(DeviceConfig {
            frames_in_flight: 1,
            ..DeviceConfig::default()
        })?);
        let mut renderer = NativeDrawListRenderer::new(device.clone())?;
        renderer.configure(format, 4)?;
        Ok(Self {
            device,
            renderer,
            format,
            surface: None,
            framebuffer: None,
        })
    }

    pub fn ready(&self) -> RhiResult<()> {
        self.device.wait_until_idle()
    }

    pub fn draw(
        &mut self,
        list: &DrawList,
        atlas: &FontAtlas,
        logical: [f32; 2],
        scale: f32,
        measure_gpu: bool,
    ) -> RhiResult<Option<GpuFrameTiming>> {
        self.renderer.upload_font_atlas(atlas, 1)?;
        let scale = scale.min(8192.0 / logical[0].max(logical[1]));
        let pixels = logical.map(|axis| (axis * scale).round().max(1.0) as usize);
        let surface = SurfaceDescriptor {
            premultiplied_alpha: true,
            ..SurfaceDescriptor::new(1)
                .with_size(pixels[0], pixels[1])
                .with_color_format(self.format)
        };
        let swapchain = match self.surface {
            Some((handle, size)) => {
                if size != pixels {
                    self.device.configure_swapchain(handle, surface)?;
                }
                handle
            }
            None => self.device.make_swapchain(surface)?,
        };
        self.surface = Some((swapchain, pixels));
        if self.framebuffer.as_ref().is_none_or(|target| {
            let descriptor = target.descriptor();
            [descriptor.width, descriptor.height] != pixels
        }) {
            self.framebuffer = Some(TextureResource::new(
                self.device.clone(),
                TextureDescriptor::new(
                    pixels[0],
                    pixels[1],
                    self.format,
                    TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
                )
                .with_label("browser-framebuffer"),
            )?);
        }
        let framebuffer = self
            .framebuffer
            .as_ref()
            .expect("created browser framebuffer")
            .texture();
        self.device.begin_frame()?;
        let timing = measure_gpu.then(|| self.device.measure_gpu_frame()).flatten();
        let result = (|| {
            let image = self.device.acquire_swapchain_image(swapchain)?;
            let mut commands = CommandBuffer::new();
            let mut frame = self.renderer.record_presentation(
                list,
                &mut commands,
                framebuffer,
                NativeUIViewport::new(pixels, logical),
                [0.0, 0.0, 0.0, 0.0],
                usize::MAX,
            )?;
            self.renderer.record_blit(
                framebuffer,
                &mut commands,
                RenderColorTarget::new(image.texture),
                self.format,
                pixels,
            )?;
            self.device
                .submit(&commands, QueueClass::Graphics, &[], &[], None)?;
            frame.did_submit();
            self.device.present(image)
        })();
        self.device.end_frame();
        result.map(|()| timing)
    }
}
