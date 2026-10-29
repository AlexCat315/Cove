//! Translation of NativeRHI/Device/Commands.swift: the recorded command model.
//!
//! The Zig original serialized commands into a byte stream (opcode + packed
//! structs) decoded twice — once by the frontend planner and again by the C
//! bridge. Swift has no FFI boundary between frontend and backend, so a command
//! buffer is a typed list of `RecordedCommand` values. Passes are scoped
//! records with their own body arrays, which makes unterminated or mis-nested
//! passes unrepresentable; the Rust port keeps that shape.
use super::{
    AccelerationStructureBuild, BarrierPassScope, BarrierSyncAction, BindingSet, Buffer,
    ComputePipeline, GraphicsPipeline, IndexType, MeshPipeline, QueueClass, ResourceRef,
    ResourceState, RhiResult, ShaderStage, Texture, TextureBufferUpload, ThreadgroupSize,
};

// MARK: - Render pass targets

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorLoadAction {
    Clear([f32; 4]),
    Load,
    DontCare,
}
impl Default for ColorLoadAction {
    fn default() -> Self {
        Self::DontCare
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DepthLoadAction {
    Clear(f64),
    Load,
    DontCare,
}
impl Default for DepthLoadAction {
    fn default() -> Self {
        Self::Clear(1.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderColorTarget {
    /// Resolve the multisampled attachment into this single-sample color target.
    pub resolve_texture: Option<Texture>,
    pub texture: Texture,
    pub load_action: ColorLoadAction,
    pub store: bool,
}
impl RenderColorTarget {
    pub fn new(texture: Texture) -> Self {
        Self { resolve_texture: None, texture, load_action: ColorLoadAction::DontCare, store: true }
    }
    pub fn clearing(texture: Texture, color: [f32; 4]) -> Self {
        Self { resolve_texture: None, texture, load_action: ColorLoadAction::Clear(color), store: true }
    }
    pub const fn with_resolve(mut self, resolve_texture: Texture) -> Self {
        self.resolve_texture = Some(resolve_texture);
        self
    }
    pub const fn without_store(mut self) -> Self {
        self.store = false;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderDepthTarget {
    pub texture: Texture,
    pub load_action: DepthLoadAction,
    pub store: bool,
}
impl RenderDepthTarget {
    pub fn new(texture: Texture) -> Self {
        Self { texture, load_action: DepthLoadAction::Clear(1.0), store: true }
    }
    pub const fn with_load_action(mut self, load_action: DepthLoadAction) -> Self {
        self.load_action = load_action;
        self
    }
    pub const fn without_store(mut self) -> Self {
        self.store = false;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderPassDescriptor {
    pub color_targets: Vec<RenderColorTarget>,
    pub depth_target: Option<RenderDepthTarget>,
    /// Names this pass in GPU timing and validation output. Purely diagnostic,
    /// so it carries no rendering behaviour.
    pub label: Option<&'static str>,
}
impl RenderPassDescriptor {
    pub fn new(color_targets: Vec<RenderColorTarget>) -> Self {
        Self { color_targets, depth_target: None, label: None }
    }
    pub fn with_depth(mut self, depth_target: RenderDepthTarget) -> Self {
        self.depth_target = Some(depth_target);
        self
    }
    pub const fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub min_depth: f64,
    pub max_depth: f64,
}
impl Viewport {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { x: 0.0, y: 0.0, width, height, min_depth: 0.0, max_depth: 1.0 }
    }
    pub const fn at(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height, min_depth: 0.0, max_depth: 1.0 }
    }
    pub const fn with_depth_range(mut self, min_depth: f64, max_depth: f64) -> Self {
        self.min_depth = min_depth;
        self.max_depth = max_depth;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScissorRect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}
impl ScissorRect {
    pub const fn new(width: i64, height: i64) -> Self {
        Self { x: 0, y: 0, width, height }
    }
    pub const fn at(x: i64, y: i64, width: i64, height: i64) -> Self {
        Self { x, y, width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawIndexedArguments {
    pub index_count: i64,
    pub instance_count: i64,
    pub first_index: i64,
    pub vertex_offset: i64,
    pub first_instance: i64,
}
impl DrawIndexedArguments {
    pub const fn new(index_count: i64) -> Self {
        Self { index_count, instance_count: 1, first_index: 0, vertex_offset: 0, first_instance: 0 }
    }
}

// MARK: - Per-pass commands

#[derive(Clone, Debug, PartialEq)]
pub enum RenderCommand {
    SetPipeline(GraphicsPipeline),
    SetMeshPipeline(MeshPipeline),
    DrawMeshTasks(ThreadgroupSize),
    SetBindingSet { slot: u32, set: BindingSet, dynamic_offsets: Vec<u32> },
    SetVertexBuffer { slot: u32, buffer: Buffer, offset: usize },
    SetIndexBuffer { buffer: Buffer, offset: usize, index_type: IndexType },
    /// Small constants copied into per-frame upload storage and bound at the
    /// given argument index (Metal push-constant model).
    PushConstant { stage: ShaderStage, slot: u32, data: Vec<u8> },
    SetViewport(Viewport),
    SetScissor(ScissorRect),
    Draw { vertex_count: i64, instance_count: i64, first_vertex: i64, first_instance: i64 },
    DrawIndexed(DrawIndexedArguments),
    DrawIndirect { buffer: Buffer, offset: usize, draw_count: i64 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ComputeCommand {
    SetPipeline(ComputePipeline),
    SetBindingSet { slot: u32, set: BindingSet, dynamic_offsets: Vec<u32> },
    PushConstant { stage: ShaderStage, slot: u32, data: Vec<u8> },
    Dispatch { groups_x: i64, groups_y: i64, groups_z: i64 },
    DispatchIndirect { buffer: Buffer, offset: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyCommand {
    /// Top-left base-level copy between distinct single-sample 2D color
    /// textures.
    CopyTexture { src: Texture, dst: Texture, width: i64, height: i64 },
    CopyBuffer { src: Buffer, src_offset: usize, dst: Buffer, dst_offset: usize, size: usize },
    CopyBufferToTexture(TextureBufferUpload),
    CopyTextureToBuffer {
        texture: Texture,
        width: i64,
        height: i64,
        buffer: Buffer,
        offset: usize,
        bytes_per_row: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BarrierCommand {
    pub resource: ResourceRef,
    pub source_state: ResourceState,
    pub destination_state: ResourceState,
    pub sync_action: BarrierSyncAction,
    pub pass_scope: BarrierPassScope,
    pub source_queue: QueueClass,
    pub destination_queue: QueueClass,
}
impl BarrierCommand {
    pub const fn new(
        resource: ResourceRef,
        source_state: ResourceState,
        destination_state: ResourceState,
    ) -> Self {
        Self {
            resource,
            source_state,
            destination_state,
            sync_action: BarrierSyncAction::Full,
            pass_scope: BarrierPassScope::OutsidePass,
            source_queue: QueueClass::Graphics,
            destination_queue: QueueClass::Graphics,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderPassRecord {
    pub descriptor: RenderPassDescriptor,
    pub body: Vec<RenderCommand>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputePassRecord {
    pub body: Vec<ComputeCommand>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CopyPassRecord {
    pub body: Vec<CopyCommand>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecordedCommand {
    AccelerationStructureBuild(AccelerationStructureBuild),
    RenderPass(RenderPassRecord),
    ComputePass(ComputePassRecord),
    CopyPass(CopyPassRecord),
    Barrier(BarrierCommand),
}

// MARK: - Recorders

#[derive(Clone, Debug, Default)]
pub struct RenderPassEncoder {
    body: Vec<RenderCommand>,
}
impl RenderPassEncoder {
    pub fn body(&self) -> &[RenderCommand] {
        &self.body
    }

    pub fn set_mesh_pipeline(&mut self, pipeline: MeshPipeline) {
        self.body.push(RenderCommand::SetMeshPipeline(pipeline));
    }

    pub fn draw_mesh_tasks(&mut self, x: i64, y: i64, z: i64) {
        self.body.push(RenderCommand::DrawMeshTasks(ThreadgroupSize::new(x, y, z)));
    }

    pub fn set_pipeline(&mut self, pipeline: GraphicsPipeline) {
        self.body.push(RenderCommand::SetPipeline(pipeline));
    }

    pub fn set_binding_set(&mut self, set: BindingSet, slot: u32, dynamic_offsets: Vec<u32>) {
        self.body.push(RenderCommand::SetBindingSet { slot, set, dynamic_offsets });
    }

    pub fn set_vertex_buffer(&mut self, buffer: Buffer, offset: usize, slot: u32) {
        self.body.push(RenderCommand::SetVertexBuffer { slot, buffer, offset });
    }

    pub fn set_index_buffer(&mut self, buffer: Buffer, offset: usize, index_type: IndexType) {
        self.body.push(RenderCommand::SetIndexBuffer { buffer, offset, index_type });
    }

    pub fn push_constant_bytes(&mut self, stage: ShaderStage, slot: u32, data: &[u8]) {
        self.body.push(RenderCommand::PushConstant { stage, slot, data: data.to_vec() });
    }

    /// Copies the raw bytes of `value` into the recorded command. The shader
    /// side reads them back through a matching layout, exactly as the Swift
    /// `withUnsafeBytes(of:)` call did.
    pub fn push_constant<T: Copy>(&mut self, stage: ShaderStage, slot: u32, value: T) {
        // SAFETY: `value` is a live local of a `Copy` type, so the byte range
        // `[&value, &value + size_of::<T>())` is readable and never overlaps the
        // destination `Vec` (which allocates separately).
        let bytes = unsafe {
            std::slice::from_raw_parts((&value as *const T).cast::<u8>(), std::mem::size_of::<T>())
        };
        self.push_constant_bytes(stage, slot, bytes);
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.body.push(RenderCommand::SetViewport(viewport));
    }

    pub fn set_scissor(&mut self, scissor: ScissorRect) {
        self.body.push(RenderCommand::SetScissor(scissor));
    }

    pub fn draw(
        &mut self,
        vertex_count: i64,
        instance_count: i64,
        first_vertex: i64,
        first_instance: i64,
    ) {
        self.body.push(RenderCommand::Draw {
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        });
    }

    pub fn draw_indexed(&mut self, arguments: DrawIndexedArguments) {
        self.body.push(RenderCommand::DrawIndexed(arguments));
    }

    pub fn draw_indirect(&mut self, buffer: Buffer, offset: usize, draw_count: i64) {
        self.body.push(RenderCommand::DrawIndirect { buffer, offset, draw_count });
    }
}

#[derive(Clone, Debug, Default)]
pub struct ComputePassEncoder {
    body: Vec<ComputeCommand>,
}
impl ComputePassEncoder {
    pub fn body(&self) -> &[ComputeCommand] {
        &self.body
    }

    pub fn set_pipeline(&mut self, pipeline: ComputePipeline) {
        self.body.push(ComputeCommand::SetPipeline(pipeline));
    }

    pub fn set_binding_set(&mut self, set: BindingSet, slot: u32, dynamic_offsets: Vec<u32>) {
        self.body.push(ComputeCommand::SetBindingSet { slot, set, dynamic_offsets });
    }

    pub fn push_constant_bytes(&mut self, stage: ShaderStage, slot: u32, data: &[u8]) {
        self.body.push(ComputeCommand::PushConstant { stage, slot, data: data.to_vec() });
    }

    pub fn push_constant<T: Copy>(&mut self, stage: ShaderStage, slot: u32, value: T) {
        // SAFETY: `value` is a live local of a `Copy` type, so the byte range
        // `[&value, &value + size_of::<T>())` is readable and never overlaps the
        // destination `Vec`.
        let bytes = unsafe {
            std::slice::from_raw_parts((&value as *const T).cast::<u8>(), std::mem::size_of::<T>())
        };
        self.push_constant_bytes(stage, slot, bytes);
    }

    /// Dispatches workgroups; each group uses the shader module's local size.
    pub fn dispatch(&mut self, groups_x: i64, groups_y: i64, groups_z: i64) {
        self.body.push(ComputeCommand::Dispatch { groups_x, groups_y, groups_z });
    }

    pub fn dispatch_indirect(&mut self, buffer: Buffer, offset: usize) {
        self.body.push(ComputeCommand::DispatchIndirect { buffer, offset });
    }
}

#[derive(Clone, Debug, Default)]
pub struct CopyPassEncoder {
    body: Vec<CopyCommand>,
}
impl CopyPassEncoder {
    pub fn body(&self) -> &[CopyCommand] {
        &self.body
    }

    pub fn copy_texture(&mut self, src: Texture, dst: Texture, width: i64, height: i64) {
        self.body.push(CopyCommand::CopyTexture { src, dst, width, height });
    }

    pub fn copy_buffer(
        &mut self,
        src: Buffer,
        src_offset: usize,
        dst: Buffer,
        dst_offset: usize,
        size: usize,
    ) {
        self.body.push(CopyCommand::CopyBuffer { src, src_offset, dst, dst_offset, size });
    }

    pub fn upload_buffer_to_texture(&mut self, upload: TextureBufferUpload) {
        self.body.push(CopyCommand::CopyBufferToTexture(upload));
    }

    pub fn copy_texture_to_buffer(
        &mut self,
        texture: Texture,
        width: i64,
        height: i64,
        buffer: Buffer,
        offset: usize,
        bytes_per_row: usize,
    ) {
        self.body.push(CopyCommand::CopyTextureToBuffer {
            texture,
            width,
            height,
            buffer,
            offset,
            bytes_per_row,
        });
    }
}

/// A reusable, resettable list of recorded commands.
#[derive(Clone, Debug, Default)]
pub struct CommandBuffer {
    commands: Vec<RecordedCommand>,
}
impl CommandBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn commands(&self) -> &[RecordedCommand] {
        &self.commands
    }

    pub(crate) fn build_acceleration_structure(&mut self, build: AccelerationStructureBuild) {
        self.commands.push(RecordedCommand::AccelerationStructureBuild(build));
    }

    pub fn reset(&mut self) {
        self.commands.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn render_pass(
        &mut self,
        descriptor: RenderPassDescriptor,
        body: impl FnOnce(&mut RenderPassEncoder),
    ) {
        let mut encoder = RenderPassEncoder::default();
        body(&mut encoder);
        self.commands.push(RecordedCommand::RenderPass(RenderPassRecord {
            descriptor,
            body: encoder.body,
        }));
    }

    /// Fallible variant of [`Self::render_pass`]; nothing is recorded if the
    /// body fails, matching Swift's `rethrows` behaviour.
    pub fn try_render_pass(
        &mut self,
        descriptor: RenderPassDescriptor,
        body: impl FnOnce(&mut RenderPassEncoder) -> RhiResult<()>,
    ) -> RhiResult<()> {
        let mut encoder = RenderPassEncoder::default();
        body(&mut encoder)?;
        self.commands.push(RecordedCommand::RenderPass(RenderPassRecord {
            descriptor,
            body: encoder.body,
        }));
        Ok(())
    }

    pub fn compute_pass(&mut self, body: impl FnOnce(&mut ComputePassEncoder)) {
        let mut encoder = ComputePassEncoder::default();
        body(&mut encoder);
        self.commands.push(RecordedCommand::ComputePass(ComputePassRecord { body: encoder.body }));
    }

    pub fn try_compute_pass(
        &mut self,
        body: impl FnOnce(&mut ComputePassEncoder) -> RhiResult<()>,
    ) -> RhiResult<()> {
        let mut encoder = ComputePassEncoder::default();
        body(&mut encoder)?;
        self.commands.push(RecordedCommand::ComputePass(ComputePassRecord { body: encoder.body }));
        Ok(())
    }

    pub fn copy_pass(&mut self, body: impl FnOnce(&mut CopyPassEncoder)) {
        let mut encoder = CopyPassEncoder::default();
        body(&mut encoder);
        self.commands.push(RecordedCommand::CopyPass(CopyPassRecord { body: encoder.body }));
    }

    pub fn try_copy_pass(
        &mut self,
        body: impl FnOnce(&mut CopyPassEncoder) -> RhiResult<()>,
    ) -> RhiResult<()> {
        let mut encoder = CopyPassEncoder::default();
        body(&mut encoder)?;
        self.commands.push(RecordedCommand::CopyPass(CopyPassRecord { body: encoder.body }));
        Ok(())
    }

    pub fn barrier(&mut self, barrier: BarrierCommand) {
        self.commands.push(RecordedCommand::Barrier(barrier));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::ResourceKind;

    #[test]
    fn a_render_pass_keeps_its_own_body() {
        let mut buffer = CommandBuffer::new();
        assert!(buffer.is_empty());
        buffer.render_pass(
            RenderPassDescriptor::new(vec![RenderColorTarget::new(Texture::new(2))]),
            |encoder| {
                encoder.set_pipeline(GraphicsPipeline::new(5));
                encoder.draw(3, 1, 0, 0);
            },
        );
        assert_eq!(buffer.commands().len(), 1);
        match &buffer.commands()[0] {
            RecordedCommand::RenderPass(record) => {
                assert_eq!(record.body.len(), 2);
                assert_eq!(record.descriptor.color_targets[0].texture, Texture::new(2));
            }
            other => panic!("expected a render pass, got {other:?}"),
        }
        buffer.reset();
        assert!(buffer.is_empty());
    }

    #[test]
    fn push_constants_record_the_value_bytes() {
        let mut encoder = RenderPassEncoder::default();
        encoder.push_constant(ShaderStage::Fragment, 2, 0x1234_5678u32);
        match &encoder.body()[0] {
            RenderCommand::PushConstant { slot, data, .. } => {
                assert_eq!(*slot, 2);
                assert_eq!(data.as_slice(), &0x1234_5678u32.to_ne_bytes());
            }
            other => panic!("expected a push constant, got {other:?}"),
        }
    }

    #[test]
    fn a_failing_body_records_nothing() {
        let mut buffer = CommandBuffer::new();
        let result = buffer.try_copy_pass(|_| Err(super::super::RhiError::invalid_argument("no")));
        assert!(result.is_err());
        assert!(buffer.is_empty());
    }

    #[test]
    fn barriers_carry_the_default_queue_and_scope() {
        let barrier = BarrierCommand::new(
            ResourceRef::new(ResourceKind::Texture, 1),
            ResourceState::COPY_DESTINATION,
            ResourceState::SHADER_RESOURCE,
        );
        assert_eq!(barrier.sync_action, BarrierSyncAction::Full);
        assert_eq!(barrier.pass_scope, BarrierPassScope::OutsidePass);
        assert_eq!(barrier.source_queue, QueueClass::Graphics);
    }
}
