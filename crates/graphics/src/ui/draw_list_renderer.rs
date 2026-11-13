use super::{
    backdrop_pass::{BackdropComposite, NativeUIBackdropPass},
    draw_geometry::NativeUIGeometry,
    raster_workspace::NativeUIRasterWorkspace,
    shader_resources::{NativeUIPipeline, NativeUIShaderResources},
    texture_store::{
        NativeUITextureStore, TEXTURE_ROW_ALIGNMENT, TextureSlot, padded_texture_rows,
    },
    viewport::NativeUIViewport,
};
use crate::rhi::*;
use cove_core::{DrawBatch, DrawList, Reveal, TextureId, Vertex};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeUIDrawStatistics {
    pub vertices: usize,
    pub indices: usize,
    pub draw_calls: usize,
    pub texture_uploads: usize,
    /// Logical texel bytes, excluding staging row padding and atlas gutters.
    pub texture_upload_bytes: usize,
}
pub struct NativeUIDrawFrame {
    pub statistics: NativeUIDrawStatistics,
    _pipeline: Arc<NativeUIPipeline>,
    _textures: Vec<Arc<TextureSlot>>,
    _store: Arc<Mutex<NativeUITextureStore>>,
    _backdrop: Option<Arc<Mutex<NativeUIBackdropPass>>>,
    commits: Vec<(Arc<TextureSlot>, u64)>,
    atlas_sequence: u64,
    presentation_commit: Option<Box<dyn FnOnce() + Send>>,
}
impl NativeUIDrawFrame {
    /// Call only after successful submission. Abandoned tokens keep patches queued.
    pub fn did_submit(&mut self) {
        for (slot, sequence) in &self.commits {
            slot.commit(*sequence);
        }
        if self.atlas_sequence != 0 {
            self._store
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .commit_atlas_uploads(self.atlas_sequence);
            self.atlas_sequence = 0;
        }
        if let Some(commit) = self.presentation_commit.take() {
            commit();
        }
    }
}
#[derive(Clone)]
struct Batch {
    indices: DrawBatch,
    scissor: ScissorRect,
    bindings: BindingSet,
    relies_on_clip: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PipelineKey {
    format: TextureFormat,
    samples: usize,
}
#[derive(Clone, Copy)]
struct RetainedReveal {
    sequence: u64,
    texture: Option<Texture>,
    extent: NativeUIViewport,
}
#[derive(Default)]
struct PresentationState {
    epoch: u64,
    submitted_sequence: u64,
    reveal: Option<RetainedReveal>,
    last_extent: Option<NativeUIViewport>,
}
/// Record serially on the owning device. Sibling presentation state is independent.
pub struct NativeDrawListRenderer {
    pub(super) device: Arc<Device>,
    shaders: Arc<NativeUIShaderResources>,
    backdrop_shaders: Arc<NativeUIShaderResources>,
    pub(super) texture_store: Arc<Mutex<NativeUITextureStore>>,
    geometry: Option<NativeUIGeometry>,
    pipeline: Option<Arc<NativeUIPipeline>>,
    backdrop: Option<Arc<Mutex<NativeUIBackdropPass>>>,
    raster: Option<NativeUIRasterWorkspace>,
    pipelines: HashMap<PipelineKey, Arc<NativeUIPipeline>>,
    backdrops: HashMap<PipelineKey, Arc<Mutex<NativeUIBackdropPass>>>,
    configured: Option<PipelineKey>,
    presentation: Arc<Mutex<PresentationState>>,
    record_sequence: u64,
}
impl NativeDrawListRenderer {
    pub fn new(device: Arc<Device>) -> RhiResult<Self> {
        let shaders = NativeUIShaderResources::new(device.clone(), false)?;
        let backdrop_shaders = NativeUIShaderResources::new(device.clone(), true)?;
        let store = Arc::new(Mutex::new(NativeUITextureStore::new(
            device.clone(),
            shaders.clone(),
        )?));
        Ok(Self::with_resources(
            device,
            shaders,
            backdrop_shaders,
            store,
        ))
    }
    fn with_resources(
        device: Arc<Device>,
        shaders: Arc<NativeUIShaderResources>,
        backdrop_shaders: Arc<NativeUIShaderResources>,
        texture_store: Arc<Mutex<NativeUITextureStore>>,
    ) -> Self {
        Self {
            device,
            shaders,
            backdrop_shaders,
            texture_store,
            geometry: None,
            pipeline: None,
            backdrop: None,
            raster: None,
            pipelines: HashMap::new(),
            backdrops: HashMap::new(),
            configured: None,
            presentation: Arc::new(Mutex::new(PresentationState::default())),
            record_sequence: 0,
        }
    }
    pub fn configure(&mut self, format: TextureFormat, samples: usize) -> RhiResult<()> {
        let key = PipelineKey { format, samples };
        if self.configured == Some(key) {
            return Ok(());
        }
        let pipeline = if let Some(p) = self.pipelines.get(&key) {
            p.clone()
        } else {
            let p = NativeUIPipeline::new(self.shaders.clone(), format, samples)?;
            self.pipelines.insert(key, p.clone());
            p
        };
        let backdrop = if let Some(p) = self.backdrops.get(&key) {
            p.clone()
        } else {
            let p = Arc::new(Mutex::new(NativeUIBackdropPass::new(
                self.device.clone(),
                self.backdrop_shaders.clone(),
                format,
                samples,
            )?));
            self.backdrops.insert(key, p.clone());
            p
        };
        self.pipeline = Some(pipeline);
        self.backdrop = Some(backdrop);
        self.configured = Some(key);
        self.raster = None;
        for (other, pass) in &self.backdrops {
            if *other != key {
                pass.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .trim_scratch();
            }
        }
        let mut state = self
            .presentation
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state.epoch = state.epoch.wrapping_add(1);
        state.reveal = None;
        state.last_extent = None;
        Ok(())
    }
    pub fn make_sibling(&self, format: TextureFormat, samples: usize) -> RhiResult<Self> {
        let mut store = NativeUITextureStore::new(self.device.clone(), self.shaders.clone())?;
        store.synchronize(&self.texture_store.lock().unwrap_or_else(|e| e.into_inner()))?;
        let mut sibling = Self::with_resources(
            self.device.clone(),
            self.shaders.clone(),
            self.backdrop_shaders.clone(),
            Arc::new(Mutex::new(store)),
        );
        sibling.configure(format, samples)?;
        Ok(sibling)
    }
    pub fn make_presentation_renderer(
        &self,
        format: TextureFormat,
        samples: usize,
    ) -> RhiResult<Self> {
        let mut sibling = Self::with_resources(
            self.device.clone(),
            self.shaders.clone(),
            self.backdrop_shaders.clone(),
            self.texture_store.clone(),
        );
        sibling.configure(format, samples)?;
        Ok(sibling)
    }
    pub fn estimated_presentation_texture_bytes(&self) -> usize {
        self.backdrops
            .values()
            .map(|p| {
                p.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .estimated_scratch_bytes()
            })
            .sum::<usize>()
            + self.raster.as_ref().map_or(0, |r| r.estimated_bytes())
    }
    pub fn presentation_msaa_logical_bytes(&self) -> usize {
        self.raster.as_ref().map_or(0, |r| r.logical_bytes())
    }
    pub fn presentation_msaa_allocated_bytes(&self) -> Option<usize> {
        self.raster
            .as_ref()
            .map_or(Some(0), |r| r.allocated_bytes())
    }
    pub fn presentation_msaa_memoryless(&self) -> bool {
        self.raster.as_ref().is_some_and(|r| r.memoryless())
    }
    pub fn supports_backdrop(&self) -> bool {
        self.backdrop.is_some()
    }
    pub fn trim_presentation_resources(&mut self) {
        self.geometry = None;
        self.raster = None;
        for pass in self.backdrops.values() {
            pass.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .trim_scratch();
        }
        let mut state = self
            .presentation
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        state.epoch = state.epoch.wrapping_add(1);
        state.reveal = None;
        state.last_extent = None;
    }
    /// Copies an owned frame into a presentation target of `format`.
    pub fn record_blit(
        &self,
        source: Texture,
        commands: &mut CommandBuffer,
        target: RenderColorTarget,
        format: TextureFormat,
        size: [usize; 2],
    ) -> RhiResult<()> {
        self.backdrop
            .as_ref()
            .ok_or_else(|| {
                RhiError::invalid_argument("configure the renderer before recording a blit")
            })?
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .record_blit(source, target, format, size, commands)
    }
    pub fn synchronize_textures(&self, source: &Self) -> RhiResult<()> {
        if Arc::ptr_eq(&self.texture_store, &source.texture_store) {
            return Ok(());
        }
        self.texture_store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .synchronize(
                &source
                    .texture_store
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
            )
    }
    pub fn register_texture(
        &self,
        id: TextureId,
        pixels: impl Into<Arc<[u8]>>,
        size: [usize; 2],
        region: TextureUploadRegion,
        format: TextureFormat,
    ) -> RhiResult<()> {
        self.texture_store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stage(id, pixels.into(), size, region, format)
    }
    pub fn register_external_color_texture(
        &self,
        id: TextureId,
        resource: Arc<TextureResource>,
    ) -> RhiResult<()> {
        self.texture_store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .register(id, resource)
    }
    pub fn unregister_texture(&self, id: TextureId) {
        self.texture_store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unregister(id);
    }
    pub fn record(
        &mut self,
        list: &DrawList,
        commands: &mut CommandBuffer,
        target: RenderColorTarget,
        viewport: NativeUIViewport,
        backdrop_source: Option<Texture>,
    ) -> RhiResult<NativeUIDrawFrame> {
        self.record_inner(list, commands, target, viewport, backdrop_source, None)
    }
    pub fn record_presentation(
        &mut self,
        list: &DrawList,
        commands: &mut CommandBuffer,
        output: Texture,
        viewport: NativeUIViewport,
        clear: [f32; 4],
        msaa_budget_bytes: usize,
    ) -> RhiResult<NativeUIDrawFrame> {
        let pipeline = self.pipeline.as_ref().ok_or_else(|| {
            RhiError::invalid_argument("configure the native UI renderer before recording")
        })?;
        viewport.validate()?;
        let row = viewport.pixels[0]
            .checked_mul(if pipeline.format == TextureFormat::Rgba16Float {
                8
            } else {
                4
            })
            .and_then(|n| n.checked_mul(pipeline.samples))
            .ok_or_else(|| RhiError::invalid_argument("MSAA budget row overflow"))?;
        if msaa_budget_bytes < row {
            return Err(RhiError::invalid_argument(
                "MSAA budget must fit at least one pixel row",
            ));
        }
        if self.raster.as_ref().is_none_or(|r| {
            r.pixels != viewport.pixels
                || r.format != pipeline.format
                || r.samples != pipeline.samples
                || r.budget != msaa_budget_bytes
        }) {
            self.raster = Some(NativeUIRasterWorkspace::new(
                self.device.clone(),
                viewport.pixels,
                pipeline.format,
                pipeline.samples,
                msaa_budget_bytes,
                true,
            )?);
        }
        let raster = self.raster.take();
        let result = self.record_inner(
            list,
            commands,
            RenderColorTarget::clearing(output, clear),
            viewport,
            Some(output),
            raster.as_ref(),
        );
        self.raster = raster;
        result
    }
    fn record_inner(
        &mut self,
        list: &DrawList,
        commands: &mut CommandBuffer,
        target: RenderColorTarget,
        viewport: NativeUIViewport,
        backdrop_source: Option<Texture>,
        raster: Option<&NativeUIRasterWorkspace>,
    ) -> RhiResult<NativeUIDrawFrame> {
        let pipeline = self.pipeline.clone().ok_or_else(|| {
            RhiError::invalid_argument("configure the native UI renderer before recording")
        })?;
        viewport.validate()?;
        if self.geometry.as_ref().is_none_or(|g| !g.matches(list)) {
            self.geometry = Some(NativeUIGeometry::new(list)?);
        }
        let reliance = &self.geometry.as_ref().unwrap().relies_on_clip;
        let mut retained = None;
        let mut next_reveal = None;
        if let (Some(backdrop), Some(source), Some(reveal)) =
            (&self.backdrop, backdrop_source, list.reveal)
        {
            let state = self
                .presentation
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if state
                .reveal
                .is_none_or(|old| old.sequence != reveal.sequence)
            {
                if state.last_extent == Some(viewport) {
                    retained = Some(
                        backdrop
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .record_reveal_capture(source, viewport, commands)?,
                    );
                }
            } else if let Some(old) = state.reveal
                && old.extent == viewport
            {
                retained = old.texture;
            }
            next_reveal = Some(RetainedReveal {
                sequence: reveal.sequence,
                texture: retained,
                extent: viewport,
            });
        }
        let mut store = self.texture_store.lock().unwrap_or_else(|e| e.into_inner());
        store.prepare_assets(&list.resources)?;
        let mut statistics = NativeUIDrawStatistics {
            vertices: list.vertices().len(),
            indices: list.indices().len(),
            ..Default::default()
        };
        let slots = std::iter::once(store.fallback.clone())
            .chain(store.textures.values().cloned())
            .collect::<Vec<_>>();
        let (mut uploads, atlas_bytes, atlas_sequence) = store.build_atlas_uploads()?;
        statistics.texture_uploads = uploads.len();
        statistics.texture_upload_bytes = atlas_bytes;
        let mut commits = Vec::new();
        for slot in &slots {
            let patches = slot.snapshot();
            for patch in &patches {
                let channels = if slot.resource.descriptor().format == TextureFormat::R8Unorm {
                    1
                } else {
                    4
                };
                // Recorded WebGPU copies require a 256-byte aligned row stride
                // and matching padded source bytes; the other backends ignore
                // the padding and read only the tight row.
                let (pixels, stride) = padded_texture_rows(
                    &patch.pixels,
                    patch.region.width as usize,
                    patch.region.height as usize,
                    channels,
                )?;
                let location = self
                    .device
                    .upload_transient(&pixels, TEXTURE_ROW_ALIGNMENT)?;
                let mut upload = TextureBufferUpload::new(
                    location.buffer,
                    stride,
                    slot.resource.texture(),
                    patch.region,
                );
                upload.offset = location.offset;
                uploads.push(upload);
                statistics.texture_uploads += 1;
                statistics.texture_upload_bytes +=
                    patch.region.width as usize * patch.region.height as usize * channels;
            }
            if let Some(last) = patches.last() {
                commits.push((slot.clone(), last.sequence));
            }
        }
        let (mut vertices, mut indices) = (None, None);
        let mut draws = Vec::new();
        if !list.indices().is_empty() && !list.batches().is_empty() {
            if store.has_atlased_placements() {
                vertices = Some(
                    self.device
                        .upload_transient(vertex_bytes(&store.remap_atlas_uvs(list)), 4)?,
                );
            } else {
                vertices = Some(
                    self.device
                        .upload_transient(vertex_bytes(list.vertices()), 4)?,
                );
            }
            indices = Some(
                self.device
                    .upload_transient(index_bytes(list.indices()), 4)?,
            );
            for (index, batch) in list.batches().iter().enumerate() {
                if batch.index_count == 0 {
                    continue;
                }
                if let Some(scissor) = viewport.scissor(batch.scissor)? {
                    draws.push(Batch {
                        indices: batch.clone(),
                        scissor,
                        bindings: store.binding_set(batch.texture_id)?,
                        relies_on_clip: reliance[index],
                    });
                }
            }
        }
        drop(store);
        let draws = coalesce(draws);
        if !uploads.is_empty() {
            commands.copy_pass(|pass| {
                for upload in uploads {
                    pass.upload_buffer_to_texture(upload);
                }
            });
        }
        let backdrop = self.backdrop.clone();
        let mut backdrop = backdrop
            .as_ref()
            .map(|p| p.lock().unwrap_or_else(PoisonError::into_inner));
        if let Some(pass) = &mut backdrop {
            pass.begin_captures();
        }
        let mut composites = HashMap::new();
        let mut draw_calls = 0;
        for (index, draw) in draws.iter().enumerate() {
            if let (Some(effect), Some(pass), Some(source)) = (
                draw.indices.backdrop,
                backdrop.as_deref_mut(),
                backdrop_source,
            ) {
                if effect.opacity <= 0.0 {
                    continue;
                }
                if target.load_action == ColorLoadAction::Load {
                    return Err(RhiError::invalid_argument(
                        "backdrop replay requires a complete frame with clear or dontCare load",
                    ));
                }
                let mut capture_target = target;
                capture_target.store = pipeline.samples == 1 || raster.is_some();
                if pipeline.samples > 1 && raster.is_none() {
                    capture_target.resolve_texture = Some(source);
                } else {
                    capture_target.texture = source;
                }
                draw_calls += record_content(
                    &draws[..index],
                    &composites,
                    commands,
                    capture_target,
                    viewport,
                    &pipeline,
                    vertices,
                    indices,
                    Some(pass),
                    raster,
                    None,
                )?;
                if let Some(composite) = pass.record(effect, Some(source), viewport, commands)? {
                    composites.insert(index, composite);
                    draw_calls += 3;
                }
            }
        }
        draw_calls += record_content(
            &draws,
            &composites,
            commands,
            target,
            viewport,
            &pipeline,
            vertices,
            indices,
            backdrop.as_deref_mut(),
            raster,
            list.reveal.zip(retained),
        )?;
        statistics.draw_calls = draw_calls;
        drop(backdrop);
        self.record_sequence = self
            .record_sequence
            .checked_add(1)
            .ok_or_else(|| RhiError::invalid_argument("UI frame sequence exhausted"))?;
        let sequence = self.record_sequence;
        let state = Arc::downgrade(&self.presentation);
        let epoch = self
            .presentation
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .epoch;
        let weak_pass = self.backdrop.as_ref().map(Arc::downgrade);
        Ok(NativeUIDrawFrame {
            statistics,
            _pipeline: pipeline,
            _textures: slots,
            _store: self.texture_store.clone(),
            _backdrop: self.backdrop.clone(),
            commits,
            atlas_sequence,
            presentation_commit: Some(Box::new(move || {
                if let Some(state) = state.upgrade() {
                    let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
                    if state.epoch == epoch && sequence >= state.submitted_sequence {
                        state.submitted_sequence = sequence;
                        state.reveal = next_reveal;
                        state.last_extent = Some(viewport);
                        if let Some(pass) = weak_pass.and_then(|p| p.upgrade()) {
                            let mut pass = pass.lock().unwrap_or_else(PoisonError::into_inner);
                            pass.finish_captures();
                            if next_reveal.is_none() {
                                pass.clear_reveal_capture();
                            }
                        }
                    }
                }
            })),
        })
    }
}
#[allow(clippy::too_many_arguments)]
fn record_content(
    draws: &[Batch],
    composites: &HashMap<usize, BackdropComposite>,
    commands: &mut CommandBuffer,
    target: RenderColorTarget,
    viewport: NativeUIViewport,
    pipeline: &NativeUIPipeline,
    vertices: Option<UploadLocation>,
    indices: Option<UploadLocation>,
    mut backdrop: Option<&mut NativeUIBackdropPass>,
    raster: Option<&NativeUIRasterWorkspace>,
    reveal: Option<(Reveal, Texture)>,
) -> RhiResult<usize> {
    let Some(raster) = raster else {
        return record_raster_pass(
            draws,
            composites,
            commands,
            target,
            viewport,
            pipeline,
            vertices,
            indices,
            backdrop,
            reveal,
            0,
            viewport.pixels[1],
        );
    };
    let mut calls = 0;
    for y in (0..viewport.pixels[1]).step_by(raster.strip_height()) {
        let height = raster.strip_height().min(viewport.pixels[1] - y);
        calls += record_raster_pass(
            draws,
            composites,
            commands,
            raster.target(target.texture, target.load_action),
            viewport,
            pipeline,
            vertices,
            indices,
            backdrop.as_deref_mut(),
            reveal,
            y,
            height,
        )?;
        if let (Some(strip), Some(pass)) = (&raster.strip, backdrop.as_deref_mut()) {
            let destination = RenderColorTarget {
                texture: target.texture,
                load_action: if y == 0 {
                    target.load_action
                } else {
                    ColorLoadAction::Load
                },
                store: true,
                resolve_texture: None,
            };
            pass.record_strip(
                strip.texture(),
                destination,
                viewport.pixels,
                y,
                height,
                raster.strip_height(),
                commands,
            )?;
            calls += 1;
        }
    }
    Ok(calls)
}
#[allow(clippy::too_many_arguments)]
fn record_raster_pass(
    draws: &[Batch],
    composites: &HashMap<usize, BackdropComposite>,
    commands: &mut CommandBuffer,
    target: RenderColorTarget,
    viewport: NativeUIViewport,
    pipeline: &NativeUIPipeline,
    vertices: Option<UploadLocation>,
    indices: Option<UploadLocation>,
    mut backdrop: Option<&mut NativeUIBackdropPass>,
    reveal: Option<(Reveal, Texture)>,
    strip_y: usize,
    height: usize,
) -> RhiResult<usize> {
    let mut calls = 0;
    commands.try_render_pass(
        RenderPassDescriptor::new(vec![target]).with_label("ui-frame"),
        |pass| {
            let render_viewport = Viewport::at(
                0.0,
                -(strip_y as f64),
                viewport.pixels[0] as f64,
                viewport.pixels[1] as f64,
            );
            pass.set_viewport(render_viewport);
            let full = ScissorRect::new(viewport.pixels[0] as i64, height as i64);
            let bind = |pass: &mut RenderPassEncoder| {
                pass.set_pipeline(pipeline.handle);
                if let Some(v) = vertices {
                    pass.set_vertex_buffer(v.buffer, v.offset, 0);
                }
                if let Some(i) = indices {
                    pass.set_index_buffer(i.buffer, i.offset, IndexType::Uint32);
                }
                pass.push_constant(
                    ShaderStage::Vertex,
                    pipeline.shaders.constants.slot,
                    [
                        viewport.logical[0],
                        viewport.logical[1],
                        pipeline.srgb(),
                        0.0,
                    ],
                );
            };
            bind(pass);
            let (mut bound, mut scissor) = (None, None);
            for (index, draw) in draws.iter().enumerate() {
                if let (Some(composite), Some(backdrop)) =
                    (composites.get(&index), backdrop.as_deref_mut())
                {
                    pass.set_scissor(full);
                    backdrop.draw(composite, pass)?;
                    calls += 1;
                    bind(pass);
                    bound = None;
                    scissor = None;
                }
                if vertices.is_none() || indices.is_none() {
                    continue;
                }
                let min_y = draw.scissor.y.max(strip_y as i64);
                let max_y = (draw.scissor.y + draw.scissor.height).min((strip_y + height) as i64);
                if max_y <= min_y {
                    continue;
                }
                let next = ScissorRect::at(
                    draw.scissor.x,
                    min_y - strip_y as i64,
                    draw.scissor.width,
                    max_y - min_y,
                );
                if bound != Some(draw.bindings) {
                    pass.set_binding_set(draw.bindings, 0, Vec::new());
                    bound = Some(draw.bindings);
                }
                if scissor != Some(next) {
                    pass.set_scissor(next);
                    scissor = Some(next);
                }
                let mut arguments = DrawIndexedArguments::new(draw.indices.index_count as i64);
                arguments.first_index = draw.indices.index_offset as i64;
                pass.draw_indexed(arguments);
                calls += 1;
            }
            if let (Some((request, texture)), Some(backdrop)) = (reveal, backdrop) {
                pass.set_scissor(full);
                backdrop.draw_reveal(request, texture, viewport, render_viewport, pass)?;
                calls += 1;
            }
            Ok(())
        },
    )?;
    Ok(calls)
}
fn coalesce(draws: Vec<Batch>) -> Vec<Batch> {
    let mut result: Vec<Batch> = Vec::new();
    for draw in draws {
        if let Some(last) = result.last_mut()
            && last.bindings == draw.bindings
            && !last.relies_on_clip
            && !draw.relies_on_clip
            && draw.indices.backdrop.is_none()
            && last
                .indices
                .index_offset
                .checked_add(last.indices.index_count)
                == Some(draw.indices.index_offset)
        {
            let x = last.scissor.x.min(draw.scissor.x);
            let y = last.scissor.y.min(draw.scissor.y);
            let r = (last.scissor.x + last.scissor.width).max(draw.scissor.x + draw.scissor.width);
            let b =
                (last.scissor.y + last.scissor.height).max(draw.scissor.y + draw.scissor.height);
            last.indices.index_count += draw.indices.index_count;
            last.scissor = ScissorRect::at(x, y, r - x, b - y);
            continue;
        }
        result.push(draw);
    }
    result
}
fn vertex_bytes(values: &[Vertex]) -> &[u8] {
    // SAFETY: Vertex is repr(C), has exactly 20 initialized bytes and no padding, enforced in cove-core.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}
fn index_bytes(values: &[u32]) -> &[u8] {
    // SAFETY: u32 has no padding and the view borrows the same initialized storage.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}
