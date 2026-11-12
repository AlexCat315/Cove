use crate::rhi::*;
use serde_json::{Value, json};

pub(super) fn encode(submit: &PlannedSubmit) -> RhiResult<Vec<Value>> {
    let mut passes = Vec::new();
    for command in &submit.commands {
        let pass = match command {
            PlannedCommand::RenderPass(record) => render_pass(record)?,
            PlannedCommand::ComputePass(record) => compute_pass(record),
            PlannedCommand::CopyPass(record) => copy_pass(record)?,
            // WebGPU tracks resource usages at pass boundaries. Graphics,
            // compute and transfer submissions are serialized by one GPUQueue.
            PlannedCommand::Barriers(_) => continue,
            PlannedCommand::AccelerationStructureBuild(_) => {
                return Err(RhiError::unsupported_feature(
                    "WebGPU acceleration structure builds",
                ));
            }
        };
        passes.push(pass);
    }
    Ok(passes)
}

fn render_pass(record: &RenderPassRecord) -> RhiResult<Value> {
    let colors: Vec<_> = record.descriptor.color_targets.iter().map(|target| {
        let (load, clear) = match target.load_action {
            ColorLoadAction::Clear(value) => ("clear", value),
            ColorLoadAction::Load => ("load", [0.0; 4]),
            ColorLoadAction::DontCare => ("clear", [0.0; 4]),
        };
        json!({ "texture": target.texture.id(), "resolve": target.resolve_texture.map(Texture::id),
            "loadOp": load, "clearValue": clear, "storeOp": store(target.store) })
    }).collect();
    let depth = record.descriptor.depth_target.map(|target| {
        let (load, clear) = match target.load_action {
            DepthLoadAction::Clear(value) => ("clear", value),
            DepthLoadAction::Load => ("load", 1.0),
            DepthLoadAction::DontCare => ("clear", 1.0),
        };
        json!({ "texture": target.texture.id(), "depthLoadOp": load, "depthClearValue": clear, "depthStoreOp": store(target.store) })
    });
    let body = record
        .body
        .iter()
        .map(|command| {
            Ok(match command {
                RenderCommand::SetPipeline(pipeline) => json!(["pipeline", pipeline.id()]),
                RenderCommand::SetMeshPipeline(_) | RenderCommand::DrawMeshTasks(_) => {
                    return Err(RhiError::unsupported_feature("WebGPU mesh commands"));
                }
                RenderCommand::SetBindingSet {
                    slot,
                    set,
                    dynamic_offsets,
                } => json!(["binding", slot, set.id(), dynamic_offsets]),
                RenderCommand::SetVertexBuffer {
                    slot,
                    buffer,
                    offset,
                } => json!(["vertex", slot, buffer.id(), offset]),
                RenderCommand::SetIndexBuffer {
                    buffer,
                    offset,
                    index_type,
                } => json!([
                    "index",
                    buffer.id(),
                    match index_type {
                        IndexType::Uint16 => "uint16",
                        IndexType::Uint32 => "uint32",
                    },
                    offset
                ]),
                RenderCommand::PushConstant { slot, data, .. } => json!(["constants", slot, data]),
                RenderCommand::SetViewport(v) => json!([
                    "viewport",
                    v.x,
                    v.y,
                    v.width,
                    v.height,
                    v.min_depth,
                    v.max_depth
                ]),
                RenderCommand::SetScissor(r) => json!(["scissor", r.x, r.y, r.width, r.height]),
                RenderCommand::Draw {
                    vertex_count,
                    instance_count,
                    first_vertex,
                    first_instance,
                } => json!([
                    "draw",
                    vertex_count,
                    instance_count,
                    first_vertex,
                    first_instance
                ]),
                RenderCommand::DrawIndexed(draw) => json!([
                    "indexed",
                    draw.index_count,
                    draw.instance_count,
                    draw.first_index,
                    draw.vertex_offset,
                    draw.first_instance
                ]),
                RenderCommand::DrawIndirect {
                    buffer,
                    offset,
                    draw_count,
                } => {
                    rhi_require(*draw_count >= 0, "negative WebGPU indirect draw count")?;
                    json!(["indirect", buffer.id(), offset, draw_count])
                }
            })
        })
        .collect::<RhiResult<Vec<_>>>()?;
    Ok(
        json!({ "type": "render", "colors": colors, "depth": depth, "body": body, "label": record.descriptor.label.unwrap_or_default() }),
    )
}

fn store(store: bool) -> &'static str {
    if store { "store" } else { "discard" }
}

fn compute_pass(record: &ComputePassRecord) -> Value {
    let body: Vec<_> = record
        .body
        .iter()
        .map(|command| match command {
            ComputeCommand::SetPipeline(pipeline) => json!(["pipeline", pipeline.id()]),
            ComputeCommand::SetBindingSet {
                slot,
                set,
                dynamic_offsets,
            } => json!(["binding", slot, set.id(), dynamic_offsets]),
            ComputeCommand::PushConstant { slot, data, .. } => json!(["constants", slot, data]),
            ComputeCommand::Dispatch {
                groups_x,
                groups_y,
                groups_z,
            } => json!(["dispatch", groups_x, groups_y, groups_z]),
            ComputeCommand::DispatchIndirect { buffer, offset } => {
                json!(["dispatchIndirect", buffer.id(), offset])
            }
        })
        .collect();
    json!({ "type": "compute", "body": body })
}

fn copy_pass(record: &CopyPassRecord) -> RhiResult<Value> {
    let body = record
        .body
        .iter()
        .map(|command| {
            Ok(match command {
                CopyCommand::CopyTexture {
                    src,
                    dst,
                    width,
                    height,
                } => json!(["texture", src.id(), dst.id(), [width, height, 1]]),
                CopyCommand::CopyBuffer {
                    src,
                    src_offset,
                    dst,
                    dst_offset,
                    size,
                } => {
                    rhi_require(
                        [src_offset, dst_offset, size]
                            .iter()
                            .all(|n| n.is_multiple_of(4)),
                        "WebGPU buffer copies require four-byte alignment",
                    )?;
                    json!(["buffer", src.id(), src_offset, dst.id(), dst_offset, size])
                }
                CopyCommand::CopyBufferToTexture(upload) => {
                    rhi_require(
                        upload.bytes_per_row.is_multiple_of(256),
                        "WebGPU recorded texture copies require 256-byte row alignment",
                    )?;
                    json!([
                        "upload",
                        upload.buffer.id(),
                        upload.offset,
                        upload.bytes_per_row,
                        upload.texture.id(),
                        upload.subresource.mip_level,
                        [
                            upload.region.origin[0],
                            upload.region.origin[1],
                            upload.subresource.layer as i64
                        ],
                        [upload.region.width, upload.region.height, 1]
                    ])
                }
                CopyCommand::CopyTextureToBuffer {
                    texture,
                    width,
                    height,
                    buffer,
                    offset,
                    bytes_per_row,
                } => {
                    rhi_require(
                        bytes_per_row.is_multiple_of(256),
                        "WebGPU recorded texture copies require 256-byte row alignment",
                    )?;
                    json!([
                        "download",
                        texture.id(),
                        buffer.id(),
                        offset,
                        bytes_per_row,
                        [width, height, 1]
                    ])
                }
            })
        })
        .collect::<RhiResult<Vec<_>>>()?;
    Ok(json!({ "type": "copy", "body": body }))
}
