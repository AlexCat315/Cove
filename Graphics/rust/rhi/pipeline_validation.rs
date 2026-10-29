//! Translation of NativeRHI/Validation/PipelineValidation.swift: per-command
//! capability and interface checks inside a recorded pass body.
use super::{
    BindingLayout, CommandBuffer, ComputeCommand, DeviceCaches, PipelineLayout, PipelineLayoutDescriptor,
    RecordedCommand, RenderCommand, RhiError, RhiResult, ShaderStage, ThreadgroupSize, rhi_count,
    rhi_require,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineKind {
    Graphics,
    Mesh,
    Compute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipelineUse {
    pub layout: PipelineLayout,
    pub kind: PipelineKind,
}
impl PipelineUse {
    pub const fn new(layout: PipelineLayout, kind: PipelineKind) -> Self {
        Self { layout, kind }
    }
}

impl super::PipelineInterfaces {
    /// One traversal per pass body. Pass-level capability checks belong to the
    /// caller, but per-command capability checks ride along here so the body is
    /// never walked twice.
    pub fn validate_commands(
        &self,
        buffer: &CommandBuffer,
        caches: &DeviceCaches,
        mesh_shading_enabled: bool,
    ) -> RhiResult<()> {
        for command in buffer.commands() {
            match command {
                RecordedCommand::RenderPass(pass) => {
                    self.validate_render(&pass.body, caches, mesh_shading_enabled)?;
                }
                RecordedCommand::ComputePass(pass) => {
                    self.validate_compute(&pass.body, caches)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn use_of(&self, id: u32, kind: PipelineKind) -> RhiResult<PipelineUse> {
        match self.uses.get(&id) {
            Some(use_of) if use_of.kind == kind => Ok(*use_of),
            _ => Err(RhiError::invalid_argument("unknown pipeline or incorrect pipeline kind")),
        }
    }

    fn layout_descriptor(&self, pipeline: Option<PipelineUse>) -> Option<&PipelineLayoutDescriptor> {
        let pipeline = pipeline?;
        self.pipelines.get(&pipeline.layout.id())
    }

    fn binding(
        &self,
        id: u32,
        slot: u32,
        dynamic_offsets: &[u32],
        pipeline: Option<PipelineUse>,
        caches: &DeviceCaches,
    ) -> RhiResult<()> {
        let descriptor = self.layout_descriptor(pipeline);
        let set_layout: BindingLayout = match descriptor {
            Some(descriptor) if (slot as usize) < descriptor.set_layouts.len() => {
                let candidate = descriptor.set_layouts[slot as usize];
                if Some(candidate.id()) != caches.binding_set_layout_id(id) {
                    return Err(RhiError::layout_mismatch(
                        "binding set must match the active pipeline's declared set layout",
                    ));
                }
                candidate
            }
            _ => {
                return Err(RhiError::layout_mismatch(
                    "binding set must match the active pipeline's declared set layout",
                ));
            }
        };
        let dynamic_count = self
            .bindings
            .get(&set_layout.id())
            .map(|layout| {
                layout.entries.iter().filter(|entry| entry.buffer.has_dynamic_offset).count()
            })
            .unwrap_or(0);
        rhi_require(
            dynamic_offsets.len() == dynamic_count,
            "dynamic offset count must match the binding layout",
        )
    }

    fn constants(
        &self,
        data: &[u8],
        slot: u32,
        stage: ShaderStage,
        pipeline: Option<PipelineUse>,
    ) -> RhiResult<()> {
        let descriptor = self.layout_descriptor(pipeline);
        let matches = descriptor.is_some_and(|descriptor| {
            descriptor.push_constants.iter().any(|range| {
                range.slot == slot
                    && range.stage == stage
                    && !data.is_empty()
                    && data.len() % 4 == 0
                    && (data.len() as i64) <= range.byte_count
            })
        });
        rhi_require(
            matches,
            "push constant data must match a declared range in the active pipeline",
        )
    }

    fn validate_render(
        &self,
        commands: &[RenderCommand],
        caches: &DeviceCaches,
        mesh_shading_enabled: bool,
    ) -> RhiResult<()> {
        let mut pipeline: Option<PipelineUse> = None;
        let mut indexed = false;
        for command in commands {
            match command {
                RenderCommand::SetPipeline(handle) => {
                    pipeline = Some(self.use_of(handle.id(), PipelineKind::Graphics)?);
                }
                RenderCommand::SetMeshPipeline(handle) => {
                    if !mesh_shading_enabled {
                        return Err(RhiError::unsupported_feature(
                            "mesh commands are not implemented by this backend",
                        ));
                    }
                    pipeline = Some(self.use_of(handle.id(), PipelineKind::Mesh)?);
                }
                RenderCommand::SetBindingSet { slot, set, dynamic_offsets } => {
                    self.binding(set.id(), *slot, dynamic_offsets, pipeline, caches)?;
                }
                RenderCommand::PushConstant { stage, slot, data } => {
                    self.constants(data, *slot, *stage, pipeline)?;
                }
                RenderCommand::SetIndexBuffer { offset, index_type, .. } => {
                    let alignment = index_type.size() as usize;
                    rhi_require(
                        offset % alignment == 0,
                        "invalid index buffer offset",
                    )?;
                    indexed = true;
                }
                RenderCommand::SetVertexBuffer { .. } => {}
                RenderCommand::Draw { vertex_count, instance_count, first_vertex, first_instance } => {
                    rhi_require(
                        pipeline.map(|use_of| use_of.kind) == Some(PipelineKind::Graphics),
                        "draw requires a graphics pipeline",
                    )?;
                    for count in [vertex_count, instance_count, first_vertex, first_instance] {
                        rhi_count(*count)?;
                    }
                }
                RenderCommand::DrawIndexed(arguments) => {
                    rhi_require(
                        indexed
                            && pipeline.map(|use_of| use_of.kind) == Some(PipelineKind::Graphics),
                        "indexed draw requires a graphics pipeline and index buffer",
                    )?;
                    for count in [
                        arguments.index_count,
                        arguments.instance_count,
                        arguments.first_index,
                        arguments.first_instance,
                    ] {
                        rhi_count(count)?;
                    }
                    rhi_require(
                        i32::try_from(arguments.vertex_offset).is_ok(),
                        "vertex offset exceeds Int32",
                    )?;
                }
                RenderCommand::DrawIndirect { offset, draw_count, .. } => {
                    rhi_require(
                        pipeline.map(|use_of| use_of.kind) == Some(PipelineKind::Graphics)
                            && offset % 4 == 0,
                        "invalid indirect draw pipeline or offset",
                    )?;
                    rhi_count(*draw_count)?;
                }
                RenderCommand::DrawMeshTasks(groups) => {
                    if !mesh_shading_enabled {
                        return Err(RhiError::unsupported_feature(
                            "mesh commands are not implemented by this backend",
                        ));
                    }
                    rhi_require(
                        pipeline.map(|use_of| use_of.kind) == Some(PipelineKind::Mesh),
                        "mesh dispatch requires a mesh pipeline",
                    )?;
                    groups.validate()?;
                }
                RenderCommand::SetScissor(rect) => {
                    rhi_require(
                        rect.x >= 0
                            && rect.y >= 0
                            && rect.width >= 0
                            && rect.height >= 0
                            && rect.x <= i32::MAX as i64 - rect.width
                            && rect.y <= i32::MAX as i64 - rect.height,
                        "scissor exceeds Int32 extent",
                    )?;
                }
                RenderCommand::SetViewport(value) => {
                    let finite = [
                        value.x,
                        value.y,
                        value.width,
                        value.height,
                        value.min_depth,
                        value.max_depth,
                    ]
                    .into_iter()
                    .all(f64::is_finite);
                    rhi_require(
                        finite
                            && value.width > 0.0
                            && value.height > 0.0
                            && value.min_depth >= 0.0
                            && value.max_depth <= 1.0
                            && value.min_depth <= value.max_depth,
                        "invalid viewport",
                    )?;
                }
            }
        }
        Ok(())
    }

    fn validate_compute(&self, commands: &[ComputeCommand], caches: &DeviceCaches) -> RhiResult<()> {
        let mut pipeline: Option<PipelineUse> = None;
        for command in commands {
            match command {
                ComputeCommand::SetPipeline(handle) => {
                    pipeline = Some(self.use_of(handle.id(), PipelineKind::Compute)?);
                }
                ComputeCommand::SetBindingSet { slot, set, dynamic_offsets } => {
                    self.binding(set.id(), *slot, dynamic_offsets, pipeline, caches)?;
                }
                ComputeCommand::PushConstant { stage, slot, data } => {
                    self.constants(data, *slot, *stage, pipeline)?;
                }
                ComputeCommand::Dispatch { groups_x, groups_y, groups_z } => {
                    rhi_require(
                        pipeline.is_some()
                            && *groups_x > 0
                            && *groups_y > 0
                            && *groups_z > 0,
                        "dispatch requires a pipeline and positive workgroups",
                    )?;
                    for count in [groups_x, groups_y, groups_z] {
                        rhi_count(*count)?;
                    }
                }
                ComputeCommand::DispatchIndirect { offset, .. } => {
                    rhi_require(
                        pipeline.is_some() && offset % 4 == 0,
                        "indirect dispatch requires a pipeline and aligned offset",
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// Convenience for validating a lone threadgroup size, used by mesh dispatch.
pub fn validate_mesh_groups(groups: ThreadgroupSize) -> RhiResult<()> {
    groups.validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::{
        BindingSet, ComputePassEncoder, GraphicsPipeline, MeshPipeline, PipelineLayout,
        RenderColorTarget, RenderPassDescriptor, Texture,
    };

    fn interfaces_with_graphics_pipeline() -> (super::super::PipelineInterfaces, PipelineLayout) {
        let layout = PipelineLayout::new(1);
        let mut interfaces = super::super::PipelineInterfaces::new();
        interfaces
            .uses
            .insert(10, PipelineUse::new(layout, PipelineKind::Graphics));
        interfaces
            .uses
            .insert(11, PipelineUse::new(layout, PipelineKind::Compute));
        interfaces
            .uses
            .insert(12, PipelineUse::new(layout, PipelineKind::Mesh));
        interfaces
            .pipelines
            .insert(1, PipelineLayoutDescriptor::new(Vec::new()));
        (interfaces, layout)
    }

    fn render_pass(body: impl FnOnce(&mut super::super::RenderPassEncoder)) -> CommandBuffer {
        let mut buffer = CommandBuffer::new();
        buffer.render_pass(
            RenderPassDescriptor::new(vec![RenderColorTarget::new(Texture::new(1))]),
            body,
        );
        buffer
    }

    #[test]
    fn draws_require_a_graphics_pipeline() {
        let (interfaces, _layout) = interfaces_with_graphics_pipeline();
        let caches = DeviceCaches::new();
        let ok = render_pass(|encoder| {
            encoder.set_pipeline(GraphicsPipeline::new(10));
            encoder.draw(3, 1, 0, 0);
        });
        assert!(interfaces.validate_commands(&ok, &caches, false).is_ok());

        let missing = render_pass(|encoder| encoder.draw(3, 1, 0, 0));
        assert!(interfaces.validate_commands(&missing, &caches, false).is_err());
    }

    #[test]
    fn mesh_commands_need_mesh_shading() {
        let (interfaces, _layout) = interfaces_with_graphics_pipeline();
        let caches = DeviceCaches::new();
        let buffer = render_pass(|encoder| {
            encoder.set_mesh_pipeline(MeshPipeline::new(12));
            encoder.draw_mesh_tasks(1, 1, 1);
        });
        assert!(interfaces.validate_commands(&buffer, &caches, false).is_err());
        assert!(interfaces.validate_commands(&buffer, &caches, true).is_ok());
    }

    #[test]
    fn indexed_draws_need_an_index_buffer() {
        let (interfaces, _layout) = interfaces_with_graphics_pipeline();
        let caches = DeviceCaches::new();
        let buffer = render_pass(|encoder| {
            encoder.set_pipeline(GraphicsPipeline::new(10));
            encoder.draw_indexed(super::super::DrawIndexedArguments::new(3));
        });
        assert!(interfaces.validate_commands(&buffer, &caches, false).is_err());
    }

    #[test]
    fn dispatches_need_a_compute_pipeline() {
        let (interfaces, _layout) = interfaces_with_graphics_pipeline();
        let caches = DeviceCaches::new();
        let mut buffer = CommandBuffer::new();
        buffer.compute_pass(|encoder: &mut ComputePassEncoder| {
            encoder.dispatch(1, 1, 1);
        });
        assert!(interfaces.validate_commands(&buffer, &caches, false).is_err());

        let mut good = CommandBuffer::new();
        good.compute_pass(|encoder| {
            encoder.set_pipeline(crate::rhi::ComputePipeline::new(11));
            encoder.dispatch(4, 1, 1);
        });
        assert!(interfaces.validate_commands(&good, &caches, false).is_ok());
    }

    #[test]
    fn binding_sets_must_match_the_declared_layout() {
        let (interfaces, layout) = interfaces_with_graphics_pipeline();
        let mut caches = DeviceCaches::new();
        caches.store_binding_set(50, 99);
        let buffer = render_pass(|encoder| {
            encoder.set_pipeline(GraphicsPipeline::new(10));
            encoder.set_binding_set(BindingSet::new(50), 0, Vec::new());
        });
        assert!(interfaces.validate_commands(&buffer, &caches, false).is_err());

        caches.store_binding_set(50, layout.id());
        assert!(interfaces.validate_commands(&buffer, &caches, false).is_ok());
    }
}
