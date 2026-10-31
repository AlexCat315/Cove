//! Translation of NativeRHI/Device/Capabilities.swift.
//!
//! Capabilities describe the operations implemented by a backend. Adapter
//! support is reported separately; an extension name alone is not a promise.
use super::QueueLimits;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RayTracingCapabilities {
    pub acceleration_structures: bool,
    pub compute_ray_query: bool,
    pub pipelines: bool,
    pub update: bool,
    pub compaction: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeshShadingCapabilities {
    pub mesh: bool,
    pub task: bool,
    pub indirect: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureCapabilities {
    pub texture_3d: bool,
    pub cube: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub graphics: bool,
    pub compute: bool,
    pub indirect_draw: bool,
    pub ray_tracing: RayTracingCapabilities,
    pub mesh_shading: MeshShadingCapabilities,
    pub textures: TextureCapabilities,
    pub max_queues: QueueLimits,
}
impl Capabilities {
    /// Builds a capability set through a configuring closure, matching the
    /// `Capabilities { $0.graphics = true }` call shape.
    pub fn configure(configure: impl FnOnce(&mut Self)) -> Self {
        let mut capabilities = Self::default();
        configure(&mut capabilities);
        capabilities
    }
}
impl Default for Capabilities {
    fn default() -> Self {
        Self {
            graphics: false,
            compute: false,
            indirect_draw: false,
            ray_tracing: RayTracingCapabilities::default(),
            mesh_shading: MeshShadingCapabilities::default(),
            textures: TextureCapabilities::default(),
            max_queues: QueueLimits {
                graphics: 0,
                compute: 0,
                transfer: 0,
            },
        }
    }
}

/// Diagnostic information about the adapter, not the implemented RHI API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AdapterCapabilities {
    pub ray_tracing: bool,
    pub mesh_shading: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configure_mutates_only_the_requested_flags() {
        let capabilities = Capabilities::configure(|capabilities| {
            capabilities.graphics = true;
            capabilities.compute = true;
            capabilities.max_queues.graphics = 1;
        });
        assert!(capabilities.graphics && capabilities.compute);
        assert!(!capabilities.indirect_draw);
        assert_eq!(capabilities.max_queues.graphics, 1);
        assert_eq!(
            AdapterCapabilities::default(),
            AdapterCapabilities {
                ray_tracing: false,
                mesh_shading: false
            }
        );
    }
}
