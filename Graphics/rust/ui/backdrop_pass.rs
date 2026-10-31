use super::{
    shader_resources::{NativeUIShaderResources, make_pipeline},
    viewport::NativeUIViewport,
};
use crate::rhi::*;
use cove_core::{Backdrop, Rect, Reveal};
use std::sync::Arc;
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Params {
    uv: [f32; 4],
    blur: [f32; 4],
    shape: [f32; 4],
    reveal: [f32; 4],
}
const _: () = assert!(std::mem::size_of::<Params>() == 64);
#[derive(Clone)]
struct Scratch {
    resource: Arc<TextureResource>,
    binding: Option<BindingSet>,
}
impl Scratch {
    fn width(&self) -> usize {
        self.resource.descriptor().width
    }
    fn height(&self) -> usize {
        self.resource.descriptor().height
    }
}
#[derive(Clone)]
pub(super) struct BackdropComposite {
    binding: BindingSet,
    uv: [f32; 4],
    blur: [f32; 4],
    shape: [f32; 4],
    quad: [f32; 4],
}
pub(super) struct NativeUIBackdropPass {
    device: Arc<Device>,
    shaders: Arc<NativeUIShaderResources>,
    format: TextureFormat,
    offscreen: GraphicsPipeline,
    composite: GraphicsPipeline,
    blit: GraphicsPipeline,
    scratch_b: Option<Scratch>,
    captures: Vec<Scratch>,
    capture_index: usize,
    reveal_capture: Option<Scratch>,
    frame_binding: Option<(Texture, BindingSet)>,
}
#[derive(Clone, Copy)]
struct PixelRect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}
impl NativeUIBackdropPass {
    pub fn new(
        device: Arc<Device>,
        shaders: Arc<NativeUIShaderResources>,
        format: TextureFormat,
        samples: usize,
    ) -> RhiResult<Self> {
        let offscreen = make_pipeline(&shaders, format, 1, true, false, "native-ui-backdrop")?;
        let composite = match make_pipeline(
            &shaders,
            format,
            samples,
            true,
            true,
            "native-ui-backdrop-composite",
        ) {
            Ok(p) => p,
            Err(e) => {
                device.destroy_graphics_pipeline(offscreen);
                return Err(e);
            }
        };
        let blit = match make_pipeline(&shaders, format, 1, true, false, "native-ui-backdrop-blit")
        {
            Ok(p) => p,
            Err(e) => {
                device.destroy_graphics_pipeline(offscreen);
                device.destroy_graphics_pipeline(composite);
                return Err(e);
            }
        };
        Ok(Self {
            device,
            shaders,
            format,
            offscreen,
            composite,
            blit,
            scratch_b: None,
            captures: Vec::new(),
            capture_index: 0,
            reveal_capture: None,
            frame_binding: None,
        })
    }
    pub fn estimated_scratch_bytes(&self) -> usize {
        self.captures
            .iter()
            .chain(self.scratch_b.iter())
            .chain(self.reveal_capture.iter())
            .map(|s| {
                s.width()
                    * s.height()
                    * if s.resource.descriptor().format == TextureFormat::Rgba16Float {
                        8
                    } else {
                        4
                    }
            })
            .sum()
    }
    pub fn trim_scratch(&mut self) {
        self.captures.clear();
        self.scratch_b = None;
        self.reveal_capture = None;
        self.frame_binding = None;
    }
    pub fn begin_captures(&mut self) {
        self.capture_index = 0;
    }
    pub fn finish_captures(&mut self) {
        self.captures.truncate(self.capture_index);
    }
    pub fn clear_reveal_capture(&mut self) {
        self.reveal_capture = None;
    }
    fn scratch(
        &self,
        existing: Option<&Scratch>,
        width: usize,
        height: usize,
    ) -> RhiResult<Scratch> {
        if let Some(s) = existing {
            if s.width() == width && s.height() == height {
                return Ok(s.clone());
            }
        }
        let mut d = TextureDescriptor::new(
            width,
            height,
            self.format,
            TextureUsage::COLOR_TARGET | TextureUsage::SAMPLED,
        );
        d.label = Some("native-ui-backdrop-scratch");
        Ok(Scratch {
            resource: Arc::new(TextureResource::new(self.device.clone(), d)?),
            binding: None,
        })
    }
    fn binding(&mut self, texture: Texture) -> RhiResult<BindingSet> {
        for scratch in self
            .captures
            .iter_mut()
            .chain(self.scratch_b.iter_mut())
            .chain(self.reveal_capture.iter_mut())
        {
            if scratch.resource.texture() == texture {
                if let Some(binding) = scratch.binding {
                    return Ok(binding);
                }
                let binding = self.shaders.binding(texture)?;
                scratch.binding = Some(binding);
                return Ok(binding);
            }
        }
        if let Some((old, binding)) = self.frame_binding {
            if old == texture {
                return Ok(binding);
            }
        }
        let binding = self.shaders.binding(texture)?;
        self.frame_binding = Some((texture, binding));
        Ok(binding)
    }
    pub fn record(
        &mut self,
        backdrop: Backdrop,
        source: Option<Texture>,
        viewport: NativeUIViewport,
        commands: &mut CommandBuffer,
    ) -> RhiResult<Option<BackdropComposite>> {
        let Some(source) = source else {
            return Ok(None);
        };
        if backdrop.opacity <= 0.0 {
            return Ok(None);
        }
        let frame = viewport.pixels;
        let sx = frame[0] as f32 / viewport.logical[0].max(0.0001);
        let sy = frame[1] as f32 / viewport.logical[1].max(0.0001);
        if ![
            backdrop.rect.x,
            backdrop.rect.y,
            backdrop.rect.max_x(),
            backdrop.rect.max_y(),
            backdrop.blur_radius,
            backdrop.corner_radius,
            backdrop.opacity,
        ]
        .iter()
        .all(|n| n.is_finite())
        {
            return Err(RhiError::invalid_argument("non-finite backdrop parameters"));
        }
        let panel = pixel_rect(backdrop.rect, sx, sy, frame);
        if panel.width <= 0 || panel.height <= 0 {
            return Ok(None);
        }
        let mx = capture_margin(backdrop.blur_radius, sx);
        let my = capture_margin(backdrop.blur_radius, sy);
        let capture = clamp(
            PixelRect {
                x: panel.x - mx,
                y: panel.y - my,
                width: panel.width + mx * 2,
                height: panel.height + my * 2,
            },
            frame,
        );
        let factor = if capture.width.min(capture.height) >= 96 {
            4
        } else if capture.width.min(capture.height) >= 48 {
            2
        } else {
            1
        };
        let width = ((capture.width + factor - 1) / factor).max(1) as usize;
        let height = ((capture.height + factor - 1) / factor).max(1) as usize;
        let a = self.scratch(self.captures.get(self.capture_index), width, height)?;
        if self.capture_index < self.captures.len() {
            self.captures[self.capture_index] = a.clone();
        } else {
            self.captures.push(a.clone());
        }
        self.capture_index += 1;
        let b = self.scratch(self.scratch_b.as_ref(), width, height)?;
        self.scratch_b = Some(b.clone());
        let sigma = (backdrop.blur_radius * sx.min(sy) / factor as f32).max(0.5);
        let step = (sigma / 6.0).max(1.0);
        let tap_sigma = sigma / step;
        let capture_uv = [
            capture.width as f32 / frame[0] as f32,
            capture.height as f32 / frame[1] as f32,
            capture.x as f32 / frame[0] as f32,
            capture.y as f32 / frame[1] as f32,
        ];
        let source_binding = self.binding(source)?;
        self.draw_scratch(
            &a,
            source_binding,
            Params {
                uv: capture_uv,
                blur: [0.0, 1.0 / frame[0] as f32, 1.0 / frame[1] as f32, 0.0],
                ..Default::default()
            },
            "backdrop-downsample",
            commands,
        )?;
        let a_binding = self.binding(a.resource.texture())?;
        self.draw_scratch(
            &b,
            a_binding,
            Params {
                uv: [1.0, 1.0, 0.0, 0.0],
                blur: [1.0, step / width as f32, 0.0, tap_sigma],
                ..Default::default()
            },
            "backdrop-blur-h",
            commands,
        )?;
        let b_binding = self.binding(b.resource.texture())?;
        self.draw_scratch(
            &a,
            b_binding,
            Params {
                uv: [1.0, 1.0, 0.0, 0.0],
                blur: [1.0, 0.0, step / height as f32, tap_sigma],
                ..Default::default()
            },
            "backdrop-blur-v",
            commands,
        )?;
        Ok(Some(BackdropComposite {
            binding: a_binding,
            uv: [
                panel.width as f32 / capture.width as f32,
                panel.height as f32 / capture.height as f32,
                (panel.x - capture.x) as f32 / capture.width as f32,
                (panel.y - capture.y) as f32 / capture.height as f32,
            ],
            blur: [2.0, 1.8, 0.0, 0.08],
            shape: [
                panel.width as f32 / 2.0,
                panel.height as f32 / 2.0,
                backdrop.corner_radius * sx.min(sy),
                backdrop.opacity.min(1.0),
            ],
            quad: [
                clip_x(panel.x, frame[0]),
                clip_y(panel.y, frame[1]),
                clip_x(panel.x + panel.width, frame[0]),
                clip_y(panel.y + panel.height, frame[1]),
            ],
        }))
    }
    pub fn draw(&self, value: &BackdropComposite, pass: &mut RenderPassEncoder) -> RhiResult<()> {
        let quad = self.upload_quad(value.quad)?;
        pass.set_pipeline(self.composite);
        pass.set_vertex_buffer(quad.buffer, quad.offset, 0);
        pass.set_binding_set(value.binding, 0, Vec::new());
        pass.push_constant(
            ShaderStage::Fragment,
            self.shaders.constants.slot,
            Params {
                uv: value.uv,
                blur: value.blur,
                shape: value.shape,
                ..Default::default()
            },
        );
        pass.draw(6, 1, 0, 0);
        Ok(())
    }
    pub fn record_blit(
        &mut self,
        source: Texture,
        target: RenderColorTarget,
        size: [usize; 2],
        commands: &mut CommandBuffer,
    ) -> RhiResult<()> {
        self.record_copy(source, target, size, "backdrop-blit", commands)
    }
    pub fn record_reveal_capture(
        &mut self,
        source: Texture,
        viewport: NativeUIViewport,
        commands: &mut CommandBuffer,
    ) -> RhiResult<Texture> {
        let scratch = self.scratch(
            self.reveal_capture.as_ref(),
            viewport.pixels[0],
            viewport.pixels[1],
        )?;
        let retained = scratch.resource.texture();
        self.reveal_capture = Some(scratch);
        self.record_copy(
            source,
            RenderColorTarget::new(retained),
            viewport.pixels,
            "ui-reveal-capture",
            commands,
        )?;
        Ok(retained)
    }
    pub fn draw_reveal(
        &mut self,
        reveal: Reveal,
        source: Texture,
        viewport: NativeUIViewport,
        render_viewport: Viewport,
        pass: &mut RenderPassEncoder,
    ) -> RhiResult<()> {
        let size = viewport.pixels;
        let sx = size[0] as f32 / viewport.logical[0].max(0.0001);
        let sy = size[1] as f32 / viewport.logical[1].max(0.0001);
        let scale = sx.min(sy);
        let quad = self.upload_quad([-1.0, 1.0, 1.0, -1.0])?;
        let binding = self.binding(source)?;
        pass.set_pipeline(self.composite);
        pass.set_viewport(render_viewport);
        pass.set_vertex_buffer(quad.buffer, quad.offset, 0);
        pass.set_binding_set(binding, 0, Vec::new());
        pass.push_constant(
            ShaderStage::Fragment,
            self.shaders.constants.slot,
            Params {
                uv: [1.0, 1.0, 0.0, 0.0],
                blur: [4.0, 0.0, 0.0, 0.0],
                shape: [size[0] as f32 / 2.0, size[1] as f32 / 2.0, 0.0, 0.0],
                reveal: [
                    reveal.center_x * sx,
                    reveal.center_y * sy,
                    reveal.radius * scale,
                    (reveal.feather * scale).max(0.5),
                ],
            },
        );
        pass.draw(6, 1, 0, 0);
        Ok(())
    }
    fn record_copy(
        &mut self,
        source: Texture,
        target: RenderColorTarget,
        size: [usize; 2],
        label: &'static str,
        commands: &mut CommandBuffer,
    ) -> RhiResult<()> {
        let quad = self.upload_quad([-1.0, 1.0, 1.0, -1.0])?;
        let binding = self.binding(source)?;
        commands.render_pass(
            RenderPassDescriptor::new(vec![target]).with_label(label),
            |pass| {
                pass.set_pipeline(self.blit);
                pass.set_viewport(Viewport::new(size[0] as f64, size[1] as f64));
                pass.set_vertex_buffer(quad.buffer, quad.offset, 0);
                pass.set_binding_set(binding, 0, Vec::new());
                pass.push_constant(
                    ShaderStage::Fragment,
                    self.shaders.constants.slot,
                    Params {
                        uv: [1.0, 1.0, 0.0, 0.0],
                        blur: [3.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    },
                );
                pass.draw(6, 1, 0, 0);
            },
        );
        Ok(())
    }
    pub fn record_strip(
        &mut self,
        source: Texture,
        target: RenderColorTarget,
        frame: [usize; 2],
        y: usize,
        height: usize,
        source_height: usize,
        commands: &mut CommandBuffer,
    ) -> RhiResult<()> {
        let quad = self.upload_quad([
            -1.0,
            clip_y(y as i64, frame[1]),
            1.0,
            clip_y((y + height) as i64, frame[1]),
        ])?;
        let binding = self.binding(source)?;
        commands.render_pass(
            RenderPassDescriptor::new(vec![target]).with_label("ui-strip-copy"),
            |pass| {
                pass.set_pipeline(self.blit);
                pass.set_viewport(Viewport::new(frame[0] as f64, frame[1] as f64));
                pass.set_vertex_buffer(quad.buffer, quad.offset, 0);
                pass.set_binding_set(binding, 0, Vec::new());
                pass.push_constant(
                    ShaderStage::Fragment,
                    self.shaders.constants.slot,
                    Params {
                        uv: [1.0, height as f32 / source_height as f32, 0.0, 0.0],
                        blur: [3.0, 0.0, 0.0, 0.0],
                        ..Default::default()
                    },
                );
                pass.draw(6, 1, 0, 0);
            },
        );
        Ok(())
    }
    fn draw_scratch(
        &self,
        target: &Scratch,
        binding: BindingSet,
        params: Params,
        label: &'static str,
        commands: &mut CommandBuffer,
    ) -> RhiResult<()> {
        let quad = self.upload_quad([-1.0, 1.0, 1.0, -1.0])?;
        commands.render_pass(
            RenderPassDescriptor::new(vec![RenderColorTarget::new(target.resource.texture())])
                .with_label(label),
            |pass| {
                pass.set_pipeline(self.offscreen);
                pass.set_viewport(Viewport::new(target.width() as f64, target.height() as f64));
                pass.set_vertex_buffer(quad.buffer, quad.offset, 0);
                pass.set_binding_set(binding, 0, Vec::new());
                pass.push_constant(ShaderStage::Fragment, self.shaders.constants.slot, params);
                pass.draw(6, 1, 0, 0);
            },
        );
        Ok(())
    }
    fn upload_quad(&self, [x0, y0, x1, y1]: [f32; 4]) -> RhiResult<UploadLocation> {
        let vertices = [
            x0, y0, 0.0, 0.0, x1, y0, 1.0, 0.0, x1, y1, 1.0, 1.0, x0, y0, 0.0, 0.0, x1, y1, 1.0,
            1.0, x0, y1, 0.0, 1.0,
        ];
        self.device.upload_transient(float_bytes(&vertices), 4)
    }
}
impl Drop for NativeUIBackdropPass {
    fn drop(&mut self) {
        self.device.destroy_graphics_pipeline(self.offscreen);
        self.device.destroy_graphics_pipeline(self.composite);
        self.device.destroy_graphics_pipeline(self.blit);
    }
}
fn capture_margin(radius: f32, scale: f32) -> i64 {
    (radius.max(0.0) * 2.0 * scale).ceil().min(128.0) as i64
}
fn clip_x(x: i64, width: usize) -> f32 {
    x as f32 / width.max(1) as f32 * 2.0 - 1.0
}
fn clip_y(y: i64, height: usize) -> f32 {
    1.0 - y as f32 / height.max(1) as f32 * 2.0
}
fn pixel_rect(rect: Rect, sx: f32, sy: f32, frame: [usize; 2]) -> PixelRect {
    let x = (rect.x * sx).floor() as i64;
    let y = (rect.y * sy).floor() as i64;
    let r = (rect.max_x() * sx).ceil() as i64;
    let b = (rect.max_y() * sy).ceil() as i64;
    clamp(
        PixelRect {
            x,
            y,
            width: r.saturating_sub(x),
            height: b.saturating_sub(y),
        },
        frame,
    )
}
fn clamp(rect: PixelRect, frame: [usize; 2]) -> PixelRect {
    let x = rect.x.clamp(0, frame[0] as i64);
    let y = rect.y.clamp(0, frame[1] as i64);
    let r = rect.x.saturating_add(rect.width).clamp(0, frame[0] as i64);
    let b = rect.y.saturating_add(rect.height).clamp(0, frame[1] as i64);
    PixelRect {
        x,
        y,
        width: (r - x).max(0),
        height: (b - y).max(0),
    }
}
pub(super) fn float_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding and every byte in the borrowed slice is initialized.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}
