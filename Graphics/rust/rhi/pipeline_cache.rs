//! Translation of NativeRHI/Device/PipelineCache.swift: pipeline interning and
//! compilation metrics.
//!
//! Identical pipeline descriptors share one backend object. The cache is
//! reference-counted: every successful `make_*_pipeline` adds a reference and
//! every destroy removes one, so the backend object is retired only after the
//! last owner releases it. Labels are diagnostic and never take part in a key.
//!
//! Creation is measured separately from cache hits. A miss is the cold path
//! that may compile native GPU code; a hit is only a map lookup.
use std::collections::HashMap;
use std::hash::Hash;

use super::{
    ColorAttachmentDescriptor, ComputePipeline, ComputePipelineDescriptor, DepthStencilState,
    GraphicsPipeline, GraphicsPipelineDescriptor, MeshPipeline, MeshPipelineDescriptor,
    PrimitiveType, RasterizationState, RhiHandle, ShaderFormat, ShaderModule,
    ShaderModuleDescriptor, ShaderStage, TextureFormat, VertexLayoutDescriptor,
};

// MARK: - Keys

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShaderModuleKey {
    pub stage: ShaderStage,
    pub format: ShaderFormat,
    pub code: Vec<u8>,
    pub entry_point: String,
    pub workgroup: [i64; 3],
    pub constants: Vec<String>,
}
impl ShaderModuleKey {
    pub fn new(descriptor: &ShaderModuleDescriptor) -> Self {
        let mut constants: Vec<&super::ShaderSpecializationConstant> =
            descriptor.specialization_constants.iter().collect();
        constants.sort_by_key(|constant| constant.id);
        Self {
            stage: descriptor.stage,
            format: descriptor.format,
            code: descriptor.code.clone(),
            entry_point: descriptor.entry_point.clone(),
            workgroup: [
                descriptor.threadgroup_size.x,
                descriptor.threadgroup_size.y,
                descriptor.threadgroup_size.z,
            ],
            constants: constants
                .into_iter()
                .map(|constant| {
                    format!(
                        "{}:{:?}:{}",
                        constant.id,
                        constant.value.constant_type(),
                        constant.value.bits()
                    )
                })
                .collect(),
        }
    }
}

/// Immutable graphics pipeline state. Every field that changes the native
/// pipeline object is present; the label is not.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GraphicsPipelineKey {
    pub sample_count: usize,
    pub layout: u32,
    pub vertex: u32,
    pub fragment: Option<u32>,
    pub color_attachments: Vec<ColorAttachmentDescriptor>,
    pub depth_format: Option<TextureFormat>,
    pub stencil_format: Option<TextureFormat>,
    pub primitive: PrimitiveType,
    pub rasterization: RasterizationState,
    pub depth_stencil: Option<DepthStencilState>,
    pub vertex_layout: Option<VertexLayoutDescriptor>,
}
impl GraphicsPipelineKey {
    pub fn new(descriptor: &GraphicsPipelineDescriptor) -> Self {
        Self {
            sample_count: descriptor.sample_count,
            layout: descriptor.layout.id(),
            vertex: descriptor.vertex.id(),
            fragment: descriptor.fragment.map(|fragment| fragment.id()),
            color_attachments: descriptor.color_attachments.clone(),
            depth_format: descriptor.depth_format,
            stencil_format: descriptor.stencil_format,
            primitive: descriptor.primitive,
            rasterization: descriptor.rasterization,
            depth_stencil: descriptor.depth_stencil,
            vertex_layout: descriptor.vertex_layout.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ComputePipelineKey {
    pub layout: u32,
    pub shader: u32,
}
impl ComputePipelineKey {
    pub fn new(descriptor: &ComputePipelineDescriptor) -> Self {
        Self { layout: descriptor.layout.id(), shader: descriptor.shader.id() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MeshPipelineKey {
    pub layout: u32,
    pub mesh: u32,
    pub task: Option<u32>,
    pub fragment: Option<u32>,
    pub color_attachments: Vec<ColorAttachmentDescriptor>,
    pub depth_format: Option<TextureFormat>,
    pub rasterization: RasterizationState,
    pub depth_stencil: Option<DepthStencilState>,
}
impl MeshPipelineKey {
    pub fn new(descriptor: &MeshPipelineDescriptor) -> Self {
        Self {
            layout: descriptor.layout.id(),
            mesh: descriptor.mesh.id(),
            task: descriptor.task.map(|task| task.id()),
            fragment: descriptor.fragment.map(|fragment| fragment.id()),
            color_attachments: descriptor.color_attachments.clone(),
            depth_format: descriptor.depth_format,
            rasterization: descriptor.rasterization,
            depth_stencil: descriptor.depth_stencil,
        }
    }
}

// MARK: - Interning cache

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineLease<Handle> {
    pub handle: Handle,
    /// True when this call created the backend object.
    pub created: bool,
    /// A duplicate the backend built concurrently and which must be retired.
    pub discarded: Option<Handle>,
}
impl<Handle> PipelineLease<Handle> {
    pub const fn new(handle: Handle, created: bool) -> Self {
        Self { handle, created, discarded: None }
    }
    pub const fn discarded(handle: Handle, duplicate: Handle) -> Self {
        Self { handle, created: false, discarded: Some(duplicate) }
    }
}

/// Reference-counted value-keyed pipeline cache.
///
/// `release` returning true means the last owner is gone and the backend object
/// must be retired. Unknown handles are ignored so a repeated destroy cannot
/// affect a live pipeline.
pub struct PipelineCache<Key, Handle> {
    entries: HashMap<Key, Handle>,
    keys_by_handle: HashMap<u32, Key>,
    references: HashMap<u32, usize>,
}
impl<Key: Hash + Eq + Clone, Handle: RhiHandle> PipelineCache<Key, Handle> {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), keys_by_handle: HashMap::new(), references: HashMap::new() }
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Looks up `key`, calling `create` on a miss. `create` may re-enter the
    /// device (a backend compile can release its lock), so a second lookup
    /// after creation guards against a concurrent publisher.
    pub fn acquire(
        &mut self,
        key: Key,
        create: impl FnOnce() -> super::RhiResult<Handle>,
    ) -> super::RhiResult<PipelineLease<Handle>> {
        if let Some(existing) = self.entries.get(&key) {
            *self.references.entry(existing.id()).or_insert(0) += 1;
            return Ok(PipelineLease::new(*existing, false));
        }
        let handle = create()?;
        // A backend compile may release the device lock. Another caller can
        // publish this key first; keep its lease and retire our duplicate.
        if let Some(existing) = self.entries.get(&key) {
            *self.references.entry(existing.id()).or_insert(0) += 1;
            return Ok(PipelineLease::discarded(*existing, handle));
        }
        self.entries.insert(key.clone(), handle);
        self.keys_by_handle.insert(handle.id(), key);
        self.references.insert(handle.id(), 1);
        Ok(PipelineLease::new(handle, true))
    }

    /// Returns true when the last owner released `handle`.
    pub fn release(&mut self, handle: Handle) -> bool {
        let Some(&reference_count) = self.references.get(&handle.id()) else { return false };
        if reference_count > 1 {
            self.references.insert(handle.id(), reference_count - 1);
            return false;
        }
        self.references.remove(&handle.id());
        if let Some(key) = self.keys_by_handle.remove(&handle.id()) {
            self.entries.remove(&key);
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
impl<Key: Hash + Eq + Clone, Handle: RhiHandle> Default for PipelineCache<Key, Handle> {
    fn default() -> Self {
        Self::new()
    }
}

// MARK: - Statistics

/// Successful creations of one resource kind, measured on the calling thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PipelineCompileTiming {
    /// Number of successful creations.
    pub count: u64,
    /// Sum of creation durations.
    pub total_nanoseconds: u64,
    /// Longest single creation.
    pub max_nanoseconds: u64,
    /// Creations that happened between `begin_frame` and `end_frame`.
    pub during_frame: u64,
}
impl PipelineCompileTiming {
    pub fn average_microseconds(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.total_nanoseconds as f64 / self.count as f64 / 1_000.0
    }

    pub fn record(&mut self, nanoseconds: u64, during_frame: bool) {
        self.count += 1;
        self.total_nanoseconds = self.total_nanoseconds.saturating_add(nanoseconds);
        self.max_nanoseconds = self.max_nanoseconds.max(nanoseconds);
        if during_frame {
            self.during_frame += 1;
        }
    }
}

/// Cold compiles plus warm cache hits for one pipeline kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PipelineKindStatistics {
    /// Cache misses, including the backend pipeline creation.
    pub cold: PipelineCompileTiming,
    /// Cache hits and the time spent looking them up.
    pub warm_count: u64,
    pub warm_total_nanoseconds: u64,
    pub warm_max_nanoseconds: u64,
    /// Handles currently interned in the cache.
    pub live: usize,
}
impl PipelineKindStatistics {
    pub fn warm_average_microseconds(&self) -> f64 {
        if self.warm_count == 0 {
            return 0.0;
        }
        self.warm_total_nanoseconds as f64 / self.warm_count as f64 / 1_000.0
    }

    pub fn record(&mut self, nanoseconds: u64, created: bool, during_frame: bool) {
        if created {
            self.cold.record(nanoseconds, during_frame);
        } else {
            self.warm_count += 1;
            self.warm_total_nanoseconds = self.warm_total_nanoseconds.saturating_add(nanoseconds);
            self.warm_max_nanoseconds = self.warm_max_nanoseconds.max(nanoseconds);
        }
    }
}

/// Snapshot of pipeline creation and cache behavior for one `Device`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PipelineStatistics {
    /// Cold shader module creations; identical modules share a leased handle.
    pub shader_modules: PipelineCompileTiming,
    pub shader_module_cache_hits: u64,
    pub live_shader_modules: usize,
    pub graphics: PipelineKindStatistics,
    pub compute: PipelineKindStatistics,
    pub mesh: PipelineKindStatistics,
}
impl PipelineStatistics {
    pub fn cache_hits(&self) -> u64 {
        self.graphics.warm_count + self.compute.warm_count + self.mesh.warm_count
    }

    pub fn cache_misses(&self) -> u64 {
        self.graphics.cold.count + self.compute.cold.count + self.mesh.cold.count
    }

    pub fn live_pipelines(&self) -> usize {
        self.graphics.live + self.compute.live + self.mesh.live
    }

    pub fn creations_during_frame(&self) -> u64 {
        self.shader_modules.during_frame
            + self.graphics.cold.during_frame
            + self.compute.cold.during_frame
            + self.mesh.cold.during_frame
    }
}

// MARK: - Device pipeline store

/// Groups the pipeline caches and their counters, keeping `Device` small.
#[derive(Default)]
pub struct PipelineStore {
    pub shaders: PipelineCache<ShaderModuleKey, ShaderModule>,
    pub graphics: PipelineCache<GraphicsPipelineKey, GraphicsPipeline>,
    pub compute: PipelineCache<ComputePipelineKey, ComputePipeline>,
    pub mesh: PipelineCache<MeshPipelineKey, MeshPipeline>,
    statistics: PipelineStatistics,
}
impl PipelineStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> PipelineStatistics {
        let mut result = self.statistics;
        result.live_shader_modules = self.shaders.count();
        result.graphics.live = self.graphics.count();
        result.compute.live = self.compute.count();
        result.mesh.live = self.mesh.count();
        result
    }

    pub fn record_shader_module(&mut self, nanoseconds: u64, during_frame: bool) {
        self.statistics.shader_modules.record(nanoseconds, during_frame);
    }

    pub fn record_shader_cache_hit(&mut self) {
        self.statistics.shader_module_cache_hits += 1;
    }

    pub fn record_graphics(&mut self, nanoseconds: u64, created: bool, during_frame: bool) {
        self.statistics.graphics.record(nanoseconds, created, during_frame);
    }

    pub fn record_compute(&mut self, nanoseconds: u64, created: bool, during_frame: bool) {
        self.statistics.compute.record(nanoseconds, created, during_frame);
    }

    pub fn record_mesh(&mut self, nanoseconds: u64, created: bool, during_frame: bool) {
        self.statistics.mesh.record(nanoseconds, created, during_frame);
    }

    pub fn reset_statistics(&mut self) {
        self.statistics = PipelineStatistics::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::PipelineLayout;

    fn shader_descriptor(entry: &str) -> ShaderModuleDescriptor {
        ShaderModuleDescriptor::new(ShaderStage::Fragment, ShaderFormat::Spirv, vec![1, 2, 3])
            .with_entry_point(entry)
    }

    #[test]
    fn keys_ignore_labels_but_capture_state() {
        let mut first = GraphicsPipelineDescriptor::new(
            PipelineLayout::new(1),
            ShaderModule::new(2),
        );
        first.label = Some("first");
        let mut second = first.clone();
        second.label = Some("second");
        assert_eq!(GraphicsPipelineKey::new(&first), GraphicsPipelineKey::new(&second));

        second.sample_count = 4;
        assert_ne!(GraphicsPipelineKey::new(&first), GraphicsPipelineKey::new(&second));
    }

    #[test]
    fn shader_keys_differ_by_entry_point() {
        assert_ne!(
            ShaderModuleKey::new(&shader_descriptor("main")),
            ShaderModuleKey::new(&shader_descriptor("other"))
        );
    }

    #[test]
    fn the_cache_refcounts_and_retires_on_the_last_release() {
        let mut cache: PipelineCache<ComputePipelineKey, ComputePipeline> = PipelineCache::new();
        let key = ComputePipelineKey { layout: 1, shader: 2 };
        let mut created = 0;
        let first = cache
            .acquire(key, || {
                created += 1;
                Ok(ComputePipeline::new(10))
            })
            .expect("lease");
        assert!(first.created);
        let second = cache
            .acquire(key, || {
                created += 1;
                Ok(ComputePipeline::new(11))
            })
            .expect("lease");
        assert!(!second.created);
        assert_eq!(created, 1);
        assert_eq!(cache.count(), 1);
        assert!(!cache.release(ComputePipeline::new(10)));
        assert!(cache.release(ComputePipeline::new(10)));
        assert_eq!(cache.count(), 0);
        assert!(!cache.release(ComputePipeline::new(10)));
    }

    #[test]
    fn statistics_separate_cold_from_warm() {
        let mut store = PipelineStore::new();
        store.record_graphics(1_000_000, true, false);
        store.record_graphics(1_000, false, false);
        store.record_shader_module(2_000, true);
        store.record_shader_cache_hit();
        let snapshot = store.snapshot();
        assert_eq!(snapshot.graphics.cold.count, 1);
        assert_eq!(snapshot.graphics.warm_count, 1);
        assert_eq!(snapshot.cache_hits(), 1);
        assert_eq!(snapshot.cache_misses(), 1);
        assert_eq!(snapshot.creations_during_frame(), 1);
        assert_eq!(snapshot.graphics.cold.average_microseconds(), 1_000.0);
        store.reset_statistics();
        assert_eq!(store.snapshot().graphics.cold.count, 0);
    }
}
