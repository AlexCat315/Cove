//! Translation of NativeRHI/Device/Handles.swift.
//!
//! Handles are frontend IDs. The backend resolves them through its own
//! registries. ID 0 is always invalid.
use std::fmt;

/// Opaque frontend handle: a `u32` ID with a distinct Rust type per resource
/// kind, so a texture ID can never be passed where a buffer is expected.
pub trait RhiHandle: Copy + Eq + std::hash::Hash + fmt::Debug + 'static {
    const KIND: &'static str;
    fn id(self) -> u32;
    fn from_id(id: u32) -> Self;
}

macro_rules! handle {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);
        impl $name {
            pub const fn new(id: u32) -> Self { Self(id) }
            pub const fn id(self) -> u32 { self.0 }
            pub const fn is_valid(self) -> bool { self.0 != 0 }
        }
        impl RhiHandle for $name {
            const KIND: &'static str = $kind;
            fn id(self) -> u32 { self.0 }
            fn from_id(id: u32) -> Self { Self(id) }
        }
        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, concat!($kind, "({})"), self.0)
            }
        }
    };
}

handle!(Buffer, "Buffer");
handle!(Texture, "Texture");
handle!(Sampler, "Sampler");
handle!(ShaderModule, "ShaderModule");
handle!(BindingLayout, "BindingLayout");
handle!(BindingSet, "BindingSet");
handle!(PipelineLayout, "PipelineLayout");
handle!(GraphicsPipeline, "GraphicsPipeline");
handle!(ComputePipeline, "ComputePipeline");
// Handle to a mesh-shading pipeline (no vertex stage, no vertex layout).
handle!(MeshPipeline, "MeshPipeline");
// Handle to a BLAS or TLAS. Creation and build support are capability-gated.
handle!(AccelerationStructure, "AccelerationStructure");
// One native window swapchain on a device.
handle!(Swapchain, "Swapchain");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_carry_distinct_kinds_and_ids() {
        let buffer = Buffer::new(7);
        assert_eq!(buffer.id(), 7);
        assert!(buffer.is_valid());
        assert!(!Buffer::new(0).is_valid());
        assert_eq!(buffer.to_string(), "Buffer(7)");
        assert_eq!(Texture::from_id(3), Texture::new(3));
    }
}
