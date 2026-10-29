//! Translation of NativeRHI/Resources/{TextureAllocation,TextureUploadRegion,AccelerationStructure,BackgroundCompilation}.swift.
use super::{rhi_require, AccelerationStructure, Buffer, RhiResult, ShaderFormat, ShaderStage};

/// A 2D upload rectangle within one mip/array subresource.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureUploadRegion {
    pub width: i64,
    pub height: i64,
    pub origin: [i64; 2],
}
impl TextureUploadRegion {
    pub const fn new(width: i64, height: i64) -> Self {
        Self { width, height, origin: [0, 0] }
    }
    pub const fn with_origin(width: i64, height: i64, x: i64, y: i64) -> Self {
        Self { width, height, origin: [x, y] }
    }
}

/// Backend-reported allocation for one texture, excluding replacement resources
/// still retained by in-flight work. This is resource allocation, not process
/// RSS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureAllocation {
    pub allocated_bytes: i64,
    pub is_memoryless: bool,
}
impl TextureAllocation {
    pub const fn new(allocated_bytes: i64, is_memoryless: bool) -> Self {
        Self { allocated_bytes, is_memoryless }
    }
}

/// Non-indexed opaque triangle geometry. Vertices contain three float32
/// coordinates at each stride. Future geometry kinds get their own types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TriangleGeometry {
    pub vertices: Buffer,
    pub vertex_offset: i64,
    pub vertex_stride: i64,
    pub triangle_count: i64,
}
impl TriangleGeometry {
    pub const fn new(vertices: Buffer, triangle_count: i64) -> Self {
        Self { vertices, vertex_offset: 0, vertex_stride: 12, triangle_count }
    }
}

/// Three rows of an affine transform. Backends pack their native GPU layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InstanceTransform {
    pub x: [f32; 4],
    pub y: [f32; 4],
    pub z: [f32; 4],
}
impl Default for InstanceTransform {
    fn default() -> Self {
        Self { x: [1.0, 0.0, 0.0, 0.0], y: [0.0, 1.0, 0.0, 0.0], z: [0.0, 0.0, 1.0, 0.0] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AccelerationInstance {
    pub structure: AccelerationStructure,
    pub transform: InstanceTransform,
    pub mask: u32,
}
impl AccelerationInstance {
    pub const fn new(structure: AccelerationStructure) -> Self {
        Self { structure, transform: InstanceTransform { x: [1.0, 0.0, 0.0, 0.0], y: [0.0, 1.0, 0.0, 0.0], z: [0.0, 0.0, 1.0, 0.0] }, mask: 0xff }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AccelerationStructureDescriptor {
    BottomLevel(Vec<TriangleGeometry>),
    TopLevel(Vec<AccelerationInstance>),
}

/// The build command retains its input dependencies in the recorded stream so
/// `SubmissionPlanner` can order build-input reads, BLAS/TLAS writes and
/// queries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccelerationStructureBuild {
    pub structure: AccelerationStructure,
    pub inputs: Vec<super::ResourceRef>,
}

/// A backend snapshots its inputs while the device is locked, performs driver
/// work outside that lock, and publishes the finished object under the lock
/// again.
pub struct RhiCompilationWork {
    compile: Box<dyn FnOnce() -> RhiResult<RhiCompiledObject>>,
}
impl RhiCompilationWork {
    pub fn new(compile: impl FnOnce() -> RhiResult<RhiCompiledObject> + 'static) -> Self {
        Self { compile: Box::new(compile) }
    }
    pub fn run_and_install(self) -> RhiResult<()> {
        self.compile_boxed().install()
    }
    fn compile_boxed(self) -> RhiCompiledObject {
        match (self.compile)() {
            Ok(object) => object,
            Err(error) => RhiCompiledObject::failed(error),
        }
    }
}

/// Owns an unpublished native object. Cancellation/failure releases it without
/// inserting a half-built resource into a live backend registry.
pub struct RhiCompiledObject {
    publish: Option<Box<dyn FnOnce()>>,
    discard: Option<Box<dyn FnOnce()>>,
    installed: bool,
    failure: Option<super::RhiError>,
}
impl RhiCompiledObject {
    pub fn new(publish: impl FnOnce() + 'static, discard: impl FnOnce() + 'static) -> Self {
        Self { publish: Some(Box::new(publish)), discard: Some(Box::new(discard)), installed: false, failure: None }
    }
    pub fn failed(error: super::RhiError) -> Self {
        Self { publish: None, discard: None, installed: false, failure: Some(error) }
    }
    pub fn install(mut self) -> RhiResult<()> {
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        let Some(publish) = self.publish.take() else {
            return Err(super::RhiError::invalid_argument("compiled object was already installed"));
        };
        publish();
        self.installed = true;
        Ok(())
    }
    pub fn is_installed(&self) -> bool {
        self.installed
    }
}
impl Drop for RhiCompiledObject {
    fn drop(&mut self) {
        if !self.installed
            && let Some(discard) = self.discard.take()
        {
            discard();
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RhiCompilationRequest {
    Shader(super::ShaderModule, Box<super::ShaderModuleDescriptor>),
    Graphics(super::GraphicsPipeline, Box<super::GraphicsPipelineDescriptor>),
    Compute(super::ComputePipeline, Box<super::ComputePipelineDescriptor>),
    Mesh(super::MeshPipeline, Box<super::MeshPipelineDescriptor>),
}

/// Validates that a shader artifact's declared entry point and code are usable.
pub fn validate_shader_artifact(stage: ShaderStage, format: ShaderFormat, entry_point: &str, code: &[u8]) -> RhiResult<()> {
    let _ = (stage, format);
    rhi_require(!entry_point.is_empty() && !code.is_empty(), "shader artifact is empty")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn compiled_object_publishes_once_and_discards_when_dropped() {
        let published = Rc::new(Cell::new(0));
        let discarded = Rc::new(Cell::new(0));
        let publish = published.clone();
        let discard = discarded.clone();
        let object = RhiCompiledObject::new(move || publish.set(publish.get() + 1), move || discard.set(discard.get() + 1));
        object.install().unwrap();
        assert_eq!((published.get(), discarded.get()), (1, 0));

        let discard_again = discarded.clone();
        let uninstalled =
            RhiCompiledObject::new(|| {}, move || discard_again.set(discard_again.get() + 1));
        drop(uninstalled);
        assert_eq!(discarded.get(), 1);
    }

    #[test]
    fn upload_region_defaults_to_the_origin() {
        let region = TextureUploadRegion::new(4, 8);
        assert_eq!(region.origin, [0, 0]);
        assert_eq!(TextureUploadRegion::with_origin(1, 1, 3, 5).origin, [3, 5]);
    }

    #[test]
    fn triangle_geometry_uses_a_12_byte_stride() {
        let geometry = TriangleGeometry::new(Buffer::new(1), 2);
        assert_eq!(geometry.vertex_stride, 12);
        assert_eq!(geometry.vertex_offset, 0);
        assert_eq!(AccelerationInstance::new(AccelerationStructure::new(1)).mask, 0xff);
    }
}
