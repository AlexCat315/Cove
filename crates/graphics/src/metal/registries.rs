//! Translation of NativeRHI/Metal/Bindings/MetalRegistries.swift and
//! Metal/Bindings/MetalBindingRegistry.swift.
//!
//! The backend keeps every concrete Metal object here, keyed by the opaque RHI
//! handle. This is the backend's own namespace: frontend handles come from the
//! frontend `IdentifierPool`, while resources the backend creates internally
//! (upload-ring chunks, swapchain drawables) use a high-offset ID space so they
//! can never collide with frontend handles.
use std::collections::HashMap;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_metal::{
    MTLBuffer, MTLComputePipelineState, MTLDepthStencilState, MTLFunction, MTLLibrary,
    MTLPrimitiveType, MTLRenderPipelineState, MTLSamplerState, MTLSharedEvent, MTLTexture,
};

use crate::rhi::{
    AccelerationStructure, Buffer, GraphicsPipelineDescriptor, MeshPipelineDescriptor,
    RasterizationState, ShaderStage, ShaderVisibility, Texture, ThreadgroupSize,
};

use super::acceleration::MetalAccelerationStructure;

/// Native resources are resolved and range-checked at set registration, so the
/// encoder never re-resolves a handle while binding.
#[derive(Clone)]
pub enum MetalBoundValue {
    Buffer {
        buffer: Retained<ProtocolObject<dyn MTLBuffer>>,
        offset: usize,
    },
    Texture(Retained<ProtocolObject<dyn MTLTexture>>),
    Sampler(Retained<ProtocolObject<dyn MTLSamplerState>>),
    AccelerationStructure(std::sync::Arc<MetalAccelerationStructure>),
}

/// One bound resource, with the frontend handle it was resolved from so
/// retirement can find the sets that depend on it.
#[derive(Clone)]
pub struct MetalBoundResource {
    pub id: u32,
    pub value: MetalBoundValue,
}

#[derive(Clone)]
pub struct MetalBoundEntry {
    pub slot: u32,
    pub visibility: ShaderVisibility,
    pub resource: MetalBoundResource,
}

/// One already-resolved (stage, index, resource) binding. The encoder replays
/// this list directly, so it never intersects visibility per draw or builds
/// per-entry closures while binding.
#[derive(Clone)]
pub struct MetalBindingApply {
    pub stage: ShaderStage,
    pub index: usize,
    pub resource: MetalBoundValue,
}

/// Key identifying a dynamic buffer's position in the dynamic-offset array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MetalDynamicKey {
    pub stage: ShaderStage,
    pub index: usize,
}

/// The stage lists each apply table is built from, in Swift's declaration
/// order so replay order stays identical.
pub const GRAPHICS_STAGES: [ShaderStage; 2] = [ShaderStage::Vertex, ShaderStage::Fragment];
pub const MESH_STAGES: [ShaderStage; 3] =
    [ShaderStage::Task, ShaderStage::Mesh, ShaderStage::Fragment];
pub const COMPUTE_STAGES: [ShaderStage; 1] = [ShaderStage::Compute];
/// Every stage a dynamic-offset slot can be visible through.
const DYNAMIC_STAGES: [ShaderStage; 5] = [
    ShaderStage::Vertex,
    ShaderStage::Fragment,
    ShaderStage::Task,
    ShaderStage::Mesh,
    ShaderStage::Compute,
];

pub struct MetalBindingSet {
    pub entries: Vec<MetalBoundEntry>,
    /// Raster visibility: vertex and fragment stages.
    pub graphics_applies: Vec<MetalBindingApply>,
    /// Mesh pipeline visibility: task, mesh and fragment stages.
    pub mesh_applies: Vec<MetalBindingApply>,
    /// Compute visibility.
    pub compute_applies: Vec<MetalBindingApply>,
    /// Maps a dynamic buffer's (stage, index) to its offset position in the
    /// dynamic offsets array supplied at bind time.
    pub dynamic_offset_index: HashMap<MetalDynamicKey, usize>,
}

impl MetalBindingSet {
    pub fn new(entries: Vec<MetalBoundEntry>, dynamic_slots: &HashMap<u32, usize>) -> Self {
        let mut dynamic_offset_index = HashMap::new();
        for entry in &entries {
            let Some(offset_index) = dynamic_slots.get(&entry.slot) else {
                continue;
            };
            for stage in DYNAMIC_STAGES {
                if entry
                    .visibility
                    .contains(ShaderVisibility::from_stage(stage))
                {
                    dynamic_offset_index.insert(
                        MetalDynamicKey {
                            stage,
                            index: entry.slot as usize,
                        },
                        *offset_index,
                    );
                }
            }
        }
        Self {
            graphics_applies: Self::applies(&entries, &GRAPHICS_STAGES),
            mesh_applies: Self::applies(&entries, &MESH_STAGES),
            compute_applies: Self::applies(&entries, &COMPUTE_STAGES),
            dynamic_offset_index,
            entries,
        }
    }

    fn applies(entries: &[MetalBoundEntry], stages: &[ShaderStage]) -> Vec<MetalBindingApply> {
        let mut result = Vec::new();
        for entry in entries {
            for stage in stages {
                if entry
                    .visibility
                    .contains(ShaderVisibility::from_stage(*stage))
                {
                    result.push(MetalBindingApply {
                        stage: *stage,
                        index: entry.slot as usize,
                        resource: entry.resource.value.clone(),
                    });
                }
            }
        }
        result
    }
}

pub struct MetalGraphicsPipeline {
    pub state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub descriptor: GraphicsPipelineDescriptor,
}

pub struct MetalMeshPipeline {
    pub descriptor: MeshPipelineDescriptor,
    pub state: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub mesh_size: ThreadgroupSize,
    pub task_size: ThreadgroupSize,
    pub rasterization: RasterizationState,
}

/// Compiled function per shader-module handle, plus the library that owns it.
pub struct MetalShaderRecord {
    pub function: Retained<ProtocolObject<dyn MTLFunction>>,
    pub library: Retained<ProtocolObject<dyn MTLLibrary>>,
    pub stage: ShaderStage,
    pub threadgroup_size: ThreadgroupSize,
}

/// Owns immutable native bindings and invalidates only sets that reference a
/// retired resource. Called under the frontend's device/frame synchronization.
#[derive(Default)]
pub struct MetalBindingRegistry {
    sets: HashMap<u32, MetalBindingSet>,
    users: HashMap<u32, Vec<u32>>,
}

impl MetalBindingRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: u32) -> Option<&MetalBindingSet> {
        self.sets.get(&id)
    }

    /// Inserts (or replaces) one binding set, keeping the reverse index exact.
    pub fn insert(&mut self, id: u32, set: MetalBindingSet) {
        self.remove(id);
        for entry in &set.entries {
            let users = self.users.entry(entry.resource.id).or_default();
            if !users.contains(&id) {
                users.push(id);
            }
        }
        self.sets.insert(id, set);
    }

    pub fn remove(&mut self, id: u32) {
        let Some(old) = self.sets.remove(&id) else {
            return;
        };
        for entry in &old.entries {
            let resource = entry.resource.id;
            if let Some(users) = self.users.get_mut(&resource) {
                users.retain(|user| *user != id);
                if users.is_empty() {
                    self.users.remove(&resource);
                }
            }
        }
    }

    /// Retirement has already waited for every frame that could use these sets.
    pub fn remove_all_referencing(&mut self, resource: u32) {
        let Some(dependents) = self.users.get(&resource).cloned() else {
            return;
        };
        for id in dependents {
            self.remove(id);
        }
    }
}

/// All concrete Metal objects the backend owns. Grouped on its own so the
/// device type stays small (one responsibility per type).
pub struct MetalRegistries {
    pub acceleration_structures: HashMap<u32, std::sync::Arc<MetalAccelerationStructure>>,
    pub buffers: HashMap<u32, Retained<ProtocolObject<dyn MTLBuffer>>>,
    pub textures: HashMap<u32, Retained<ProtocolObject<dyn MTLTexture>>>,
    pub samplers: HashMap<u32, Retained<ProtocolObject<dyn MTLSamplerState>>>,
    pub shaders: HashMap<u32, MetalShaderRecord>,
    pub mesh_pipelines: HashMap<u32, MetalMeshPipeline>,
    pub render_pipelines: HashMap<u32, MetalGraphicsPipeline>,
    pub compute_pipelines: HashMap<u32, Retained<ProtocolObject<dyn MTLComputePipelineState>>>,
    pub compute_threadgroup_sizes: HashMap<u32, ThreadgroupSize>,
    pub depth_states: HashMap<u32, Retained<ProtocolObject<dyn MTLDepthStencilState>>>,
    pub pipeline_primitives: HashMap<u32, MTLPrimitiveType>,
    /// Cull/winding/fill are encoder state in Metal; stash per pipeline so the
    /// encoder can re-apply them when the pipeline is bound.
    pub pipeline_raster_states: HashMap<u32, RasterizationState>,
    pub binding_sets: MetalBindingRegistry,
    pub shared_events: HashMap<u32, Retained<ProtocolObject<dyn MTLSharedEvent>>>,
    /// Internal-handle counter for backend-owned resources (upload chunks,
    /// swapchain drawables). Starts above the frontend's low IDs.
    internal_id: u32,
}

/// First backend-internal handle. Frontend handles grow from 1, so the two
/// spaces never overlap for any realistic session.
pub const INTERNAL_ID_BASE: u32 = 0x8000_0000;

// SAFETY: Metal documents its objects as thread-safe, and every field here is a
// retained Metal object (or plain data keyed by handle). The registry is only
// reachable through its own `Mutex`, so mutation stays serialized; the unsafe
// impl exists because `objc2-metal`'s protocol traits do not themselves require
// `Send`/`Sync`, which leaves the auto-derived bounds too weak for `Arc`.
unsafe impl Send for MetalRegistries {}
// SAFETY: same reasoning as the `Send` impl above: shared access is only ever
// a read under the registry mutex, and the contained Metal objects are
// documented as safe to use from multiple threads.
unsafe impl Sync for MetalRegistries {}

impl Default for MetalRegistries {
    fn default() -> Self {
        Self::new()
    }
}

impl MetalRegistries {
    pub fn new() -> Self {
        Self {
            acceleration_structures: HashMap::new(),
            buffers: HashMap::new(),
            textures: HashMap::new(),
            samplers: HashMap::new(),
            shaders: HashMap::new(),
            mesh_pipelines: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            compute_threadgroup_sizes: HashMap::new(),
            depth_states: HashMap::new(),
            pipeline_primitives: HashMap::new(),
            pipeline_raster_states: HashMap::new(),
            binding_sets: MetalBindingRegistry::new(),
            shared_events: HashMap::new(),
            internal_id: INTERNAL_ID_BASE,
        }
    }

    pub fn next_internal_id(&mut self) -> u32 {
        let id = self.internal_id;
        self.internal_id = self.internal_id.wrapping_add(1);
        id
    }

    /// Removes a buffer and every binding set that referenced it.
    pub fn retire_buffer(&mut self, handle: Buffer) {
        self.binding_sets.remove_all_referencing(handle.id());
        self.buffers.remove(&handle.id());
    }

    pub fn retire_texture(&mut self, handle: Texture) {
        self.binding_sets.remove_all_referencing(handle.id());
        self.textures.remove(&handle.id());
    }

    pub fn retire_acceleration_structure(&mut self, handle: AccelerationStructure) {
        self.binding_sets.remove_all_referencing(handle.id());
        self.acceleration_structures.remove(&handle.id());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_tables_project_the_swift_stage_groups() {
        assert_eq!(
            GRAPHICS_STAGES,
            [ShaderStage::Vertex, ShaderStage::Fragment]
        );
        assert_eq!(
            MESH_STAGES,
            [ShaderStage::Task, ShaderStage::Mesh, ShaderStage::Fragment]
        );
        assert_eq!(COMPUTE_STAGES, [ShaderStage::Compute]);
        assert_eq!(DYNAMIC_STAGES.len(), 5);
    }

    #[test]
    fn an_empty_set_has_no_applies_and_no_dynamic_offsets() {
        let set = MetalBindingSet::new(Vec::new(), &HashMap::new());
        assert!(set.graphics_applies.is_empty());
        assert!(set.mesh_applies.is_empty());
        assert!(set.compute_applies.is_empty());
        assert!(set.dynamic_offset_index.is_empty());
        assert!(set.entries.is_empty());
    }

    #[test]
    fn retirement_is_a_no_op_for_unknown_sets_and_resources() {
        let mut registry = MetalBindingRegistry::new();
        registry.remove(404);
        registry.remove_all_referencing(404);
        assert!(registry.get(404).is_none());
    }

    #[test]
    fn internal_ids_start_above_the_frontend_space() {
        let mut registries = MetalRegistries::new();
        assert_eq!(registries.next_internal_id(), INTERNAL_ID_BASE);
        assert_eq!(registries.next_internal_id(), INTERNAL_ID_BASE + 1);
    }
}
