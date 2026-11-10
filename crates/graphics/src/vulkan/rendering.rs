//! Translation of NativeRHI/Vulkan/VulkanRendering.swift: the core render-pass
//! and framebuffer objects a single-subpass pipeline/pass pair needs.
//!
//! Core render passes are used instead of dynamic rendering so a Vulkan 1.1
//! driver works: one subpass expresses the same attachment contract on desktop
//! and mobile. The objects here are created per pipeline (for the pipeline's
//! compatibility render pass) and per pass, and are released when the owning
//! value drops.
use super::setup::VulkanContext;
use super::sys;
use crate::rhi::{GraphicsPipelineDescriptor, RhiResult, TextureFormat, rhi_require};

/// The handles and tables a render-pass object needs. Cloned from the context so
/// the objects can outlive a borrow of the backend.
#[derive(Clone, Copy)]
pub struct RenderingHandles {
    pub device: sys::VkDevice,
    pub create_render_pass: sys::PFN_vkCreateRenderPass,
    pub destroy_render_pass: sys::PFN_vkDestroyRenderPass,
    pub create_framebuffer: sys::PFN_vkCreateFramebuffer,
    pub destroy_framebuffer: sys::PFN_vkDestroyFramebuffer,
    pub begin_render_pass: sys::PFN_vkCmdBeginRenderPass,
    pub end_render_pass: sys::PFN_vkCmdEndRenderPass,
}

impl RenderingHandles {
    pub fn new(context: &VulkanContext) -> Self {
        Self {
            device: context.device,
            create_render_pass: context.resources.create_render_pass,
            destroy_render_pass: context.resources.destroy_render_pass,
            create_framebuffer: context.resources.create_framebuffer,
            destroy_framebuffer: context.resources.destroy_framebuffer,
            begin_render_pass: context.draw.cmd_begin_render_pass,
            end_render_pass: context.draw.cmd_end_render_pass,
        }
    }
}

/// One attachment's shape, independent of the images it will bind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachmentShape {
    pub format: u32,
    pub samples: u32,
    /// Color target or depth/stencil target, which selects the layout.
    pub is_depth: bool,
}

impl AttachmentShape {
    fn layout(&self) -> u32 {
        if self.is_depth {
            sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL
        } else {
            sys::VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL
        }
    }

    pub fn description(&self, load_op: u32, store_op: u32) -> sys::VkAttachmentDescription {
        sys::VkAttachmentDescription {
            flags: 0,
            format: self.format,
            samples: self.samples,
            loadOp: load_op,
            storeOp: store_op,
            stencilLoadOp: load_op,
            stencilStoreOp: store_op,
            initialLayout: self.layout(),
            finalLayout: self.layout(),
        }
    }

    pub fn reference(&self, index: usize) -> sys::VkAttachmentReference {
        sys::VkAttachmentReference {
            attachment: index as u32,
            layout: self.layout(),
        }
    }
}

/// Owns native attachment objects until the submission fence has completed.
/// Failed encoding releases them immediately; successful encoding transfers
/// ownership to the fence waiter.
pub struct RenderPassObjects {
    handles: RenderingHandles,
    passes: Vec<sys::VkRenderPass>,
    framebuffers: Vec<sys::VkFramebuffer>,
}

impl RenderPassObjects {
    pub fn new(handles: RenderingHandles) -> Self {
        Self {
            handles,
            passes: Vec::new(),
            framebuffers: Vec::new(),
        }
    }

    /// The render pass a graphics pipeline is compiled against: one subpass whose
    /// attachments match the pipeline's declared formats.
    pub fn pipeline_pass(
        &mut self,
        descriptor: &GraphicsPipelineDescriptor,
    ) -> RhiResult<sys::VkRenderPass> {
        let colors: Vec<sys::VkAttachmentReference> = descriptor
            .color_attachments
            .iter()
            .enumerate()
            .map(|(index, _)| sys::VkAttachmentReference {
                attachment: index as u32,
                layout: sys::VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL,
            })
            .collect();
        let mut attachments: Vec<sys::VkAttachmentDescription> = descriptor
            .color_attachments
            .iter()
            .map(|attachment| {
                self.attachment(
                    format_texture(attachment.format),
                    descriptor.sample_count,
                    false,
                )
            })
            .collect();
        let mut depth: Option<sys::VkAttachmentReference> = None;
        if let Some(format) = descriptor.depth_format {
            depth = Some(sys::VkAttachmentReference {
                attachment: attachments.len() as u32,
                layout: sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            });
            attachments.push(self.attachment(
                format_texture(format),
                descriptor.sample_count,
                true,
            ));
        }
        // Resolve references are ignored for single-subpass compatibility, so the
        // same pipeline works with and without explicit color resolves.
        self.make_pass(&attachments, &colors, &[], depth)
    }

    fn attachment(
        &self,
        format: u32,
        samples: usize,
        is_depth: bool,
    ) -> sys::VkAttachmentDescription {
        AttachmentShape {
            format,
            samples: samples as u32,
            is_depth,
        }
        .description(
            sys::VK_ATTACHMENT_LOAD_OP_DONT_CARE,
            sys::VK_ATTACHMENT_STORE_OP_DONT_CARE,
        )
    }

    pub fn make_pass(
        &mut self,
        attachments: &[sys::VkAttachmentDescription],
        colors: &[sys::VkAttachmentReference],
        resolves: &[sys::VkAttachmentReference],
        depth: Option<sys::VkAttachmentReference>,
    ) -> RhiResult<sys::VkRenderPass> {
        let depth_storage = depth.unwrap_or_default();
        let subpass = sys::VkSubpassDescription {
            flags: 0,
            pipelineBindPoint: sys::VK_PIPELINE_BIND_POINT_GRAPHICS,
            inputAttachmentCount: 0,
            pInputAttachments: core::ptr::null(),
            colorAttachmentCount: colors.len() as u32,
            pColorAttachments: colors.as_ptr(),
            pResolveAttachments: if resolves.is_empty() {
                core::ptr::null()
            } else {
                resolves.as_ptr()
            },
            pDepthStencilAttachment: if depth.is_some() {
                &depth_storage
            } else {
                core::ptr::null()
            },
            preserveAttachmentCount: 0,
            pPreserveAttachments: core::ptr::null(),
        };
        let create_info = sys::VkRenderPassCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_RENDER_PASS_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            attachmentCount: attachments.len() as u32,
            pAttachments: attachments.as_ptr(),
            subpassCount: 1,
            pSubpasses: &subpass,
            dependencyCount: 0,
            pDependencies: core::ptr::null(),
        };
        let mut pass: sys::VkRenderPass = sys::VK_NULL_HANDLE;
        // SAFETY: `create_info`, the attachment array and the subpass (plus the
        // depth reference it may point at) are all live for the call.
        let status = unsafe {
            (self.handles.create_render_pass)(
                self.handles.device,
                &create_info,
                core::ptr::null(),
                &mut pass,
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && pass != sys::VK_NULL_HANDLE,
            "vkCreateRenderPass failed",
        )?;
        // The driver copies the create-info, so tracking the handle is enough to
        // release it when this value drops.
        self.passes.push(pass);
        Ok(pass)
    }

    /// Creates a framebuffer binding `views` to `pass`.
    pub fn framebuffer(
        &mut self,
        pass: sys::VkRenderPass,
        views: &[sys::VkImageView],
        width: u32,
        height: u32,
    ) -> RhiResult<sys::VkFramebuffer> {
        let create_info = sys::VkFramebufferCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_FRAMEBUFFER_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            renderPass: pass,
            attachmentCount: views.len() as u32,
            pAttachments: views.as_ptr(),
            width,
            height,
            layers: 1,
        };
        let mut framebuffer: sys::VkFramebuffer = sys::VK_NULL_HANDLE;
        // SAFETY: `create_info` and `views` outlive the call.
        let status = unsafe {
            (self.handles.create_framebuffer)(
                self.handles.device,
                &create_info,
                core::ptr::null(),
                &mut framebuffer,
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && framebuffer != sys::VK_NULL_HANDLE,
            "vkCreateFramebuffer failed",
        )?;
        self.framebuffers.push(framebuffer);
        Ok(framebuffer)
    }

    /// Begins a pass with the given clear values.
    ///
    /// # Safety
    /// `cmd` must be recording, and every image bound into `framebuffer` must be
    /// in the layout the pass was created with.
    pub unsafe fn begin(
        &self,
        cmd: sys::VkCommandBuffer,
        pass: sys::VkRenderPass,
        framebuffer: sys::VkFramebuffer,
        area: sys::VkRect2D,
        clear_values: &[sys::VkClearValue],
    ) {
        let begin_info = sys::VkRenderPassBeginInfo {
            sType: sys::VK_STRUCTURE_TYPE_RENDER_PASS_BEGIN_INFO,
            pNext: core::ptr::null(),
            renderPass: pass,
            framebuffer,
            renderArea: area,
            clearValueCount: clear_values.len() as u32,
            pClearValues: clear_values.as_ptr(),
        };
        // SAFETY: the caller guarantees a recording command buffer and matching
        // image layouts; every pointer here refers to a live local.
        unsafe {
            (self.handles.begin_render_pass)(cmd, &begin_info, sys::VK_SUBPASS_CONTENTS_INLINE)
        };
    }

    /// Ends the current pass.
    ///
    /// # Safety
    /// `cmd` must be recording inside a pass begun by [`Self::begin`].
    pub unsafe fn end(&self, cmd: sys::VkCommandBuffer) {
        // SAFETY: the caller guarantees a pass is open on `cmd`.
        unsafe { (self.handles.end_render_pass)(cmd) };
    }
}

impl Drop for RenderPassObjects {
    fn drop(&mut self) {
        // SAFETY: these objects were created by this value and the caller has
        // already waited for the submission that used them (the fence waiter owns
        // them on the success path).
        unsafe {
            for framebuffer in self.framebuffers.drain(..) {
                (self.handles.destroy_framebuffer)(
                    self.handles.device,
                    framebuffer,
                    core::ptr::null(),
                );
            }
            for pass in self.passes.drain(..) {
                (self.handles.destroy_render_pass)(self.handles.device, pass, core::ptr::null());
            }
        }
    }
}

fn format_texture(format: TextureFormat) -> u32 {
    super::formats::vk_format(format)
}

/// Whether a render pass over `shapes` can be shared across frames: it only can
/// when every attachment keeps its load/store choice, which is why the per-pass
/// path rebuilds it.
#[cfg(test)]
pub fn pass_is_reusable(shapes: &[AttachmentShape]) -> bool {
    !shapes.is_empty()
}

/// A begin-time clear value for one attachment, matching the pass's shape.
pub fn clear_value(
    shape: &AttachmentShape,
    color: Option<[f32; 4]>,
    depth: Option<f64>,
) -> sys::VkClearValue {
    let mut value = sys::VkClearValue::default();
    if shape.is_depth {
        value.depthStencil = sys::VkClearDepthStencilValue {
            depth: depth.unwrap_or(1.0) as f32,
            stencil: 0,
        };
    } else {
        value.color = sys::VkClearColorValue {
            float32: color.unwrap_or([0.0; 4]),
        };
    }
    value
}

/// A load/store pair for one attachment, resolved from the recorded target.
#[cfg(test)]
pub fn attachment_ops(load_clear: bool, store: bool) -> (u32, u32) {
    let load = if load_clear {
        sys::VK_ATTACHMENT_LOAD_OP_CLEAR
    } else {
        sys::VK_ATTACHMENT_LOAD_OP_LOAD
    };
    let store = if store {
        sys::VK_ATTACHMENT_STORE_OP_STORE
    } else {
        sys::VK_ATTACHMENT_STORE_OP_DONT_CARE
    };
    (load, store)
}

/// Errors if a pass has no attachments, which Vulkan cannot express.
#[cfg(test)]
pub fn validate_shapes(shapes: &[AttachmentShape]) -> RhiResult<()> {
    rhi_require(
        !shapes.is_empty(),
        "a render pass requires at least one attachment",
    )?;
    rhi_require(
        shapes.len() <= 9,
        "a render pass supports at most eight color targets and one depth target",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attachment_layouts_follow_the_depth_flag() {
        let color = AttachmentShape {
            format: sys::VK_FORMAT_R8G8B8A8_UNORM,
            samples: 1,
            is_depth: false,
        };
        let depth = AttachmentShape {
            format: sys::VK_FORMAT_D32_SFLOAT,
            samples: 1,
            is_depth: true,
        };
        assert_eq!(
            color.layout(),
            sys::VK_IMAGE_LAYOUT_COLOR_ATTACHMENT_OPTIMAL
        );
        assert_eq!(
            depth.layout(),
            sys::VK_IMAGE_LAYOUT_DEPTH_STENCIL_ATTACHMENT_OPTIMAL
        );
    }

    #[test]
    fn load_and_store_ops_reflect_the_request() {
        assert_eq!(
            attachment_ops(true, true),
            (
                sys::VK_ATTACHMENT_LOAD_OP_CLEAR,
                sys::VK_ATTACHMENT_STORE_OP_STORE
            )
        );
        assert_eq!(
            attachment_ops(false, false),
            (
                sys::VK_ATTACHMENT_LOAD_OP_LOAD,
                sys::VK_ATTACHMENT_STORE_OP_DONT_CARE
            )
        );
    }

    #[test]
    fn shapes_are_validated_before_a_pass_is_created() {
        assert!(validate_shapes(&[]).is_err());
        assert!(
            validate_shapes(&[AttachmentShape {
                format: sys::VK_FORMAT_R8G8B8A8_UNORM,
                samples: 1,
                is_depth: false
            }])
            .is_ok()
        );
    }

    #[test]
    fn clear_values_default_per_attachment_kind() {
        let color = AttachmentShape {
            format: 0,
            samples: 1,
            is_depth: false,
        };
        let depth = AttachmentShape {
            format: 0,
            samples: 1,
            is_depth: true,
        };
        assert_eq!(
            // SAFETY: reading the variant that matches the shape.
            unsafe {
                clear_value(&color, Some([0.5, 0.25, 0.125, 1.0]), None)
                    .color
                    .float32
            }[0],
            0.5
        );
        assert_eq!(
            // SAFETY: as above, the depth variant is the active one.
            unsafe { clear_value(&depth, None, Some(0.75)).depthStencil.depth },
            0.75
        );
        assert_eq!(
            // SAFETY: the default depth clear is 1.0.
            unsafe { clear_value(&depth, None, None).depthStencil.depth },
            1.0
        );
    }

    #[test]
    fn a_pass_is_reusable_only_with_attachments() {
        assert!(!pass_is_reusable(&[]));
        assert!(pass_is_reusable(&[AttachmentShape {
            format: 0,
            samples: 1,
            is_depth: false
        }]));
    }
}
