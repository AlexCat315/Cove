//! Translation of NativeRHI/Vulkan/Presentation/{VulkanSwapchain,
//! VulkanPresentation, VulkanSynchronization, SurfaceState, PresentMode}.swift.
//!
//! Images are acquired into a fixed pool and recycled: acquire never appends to a
//! growing array. `OUT_OF_DATE`/`SUBOPTIMAL` triggers a recreate, and present has
//! no queue idle on the hot path.
use std::collections::HashMap;

use crate::rhi::{
    PresentationMode, RhiError, RhiResult, SurfaceDescriptor, Swapchain, TextureFormat, rhi_require,
};

use super::formats;
use super::registries::{VulkanRegistries, VulkanTextureRecord};
use super::resources::lock;
use super::setup::VulkanContext;
use super::sys;

/// Native surface creation, per platform.
///
/// Swift delegated this to the C shim `grhi_vulkan_create_native_surface` because
/// the platform `Vk*SurfaceCreateInfoKHR` structs pull in platform headers. The
/// Rust port declares the three structs it needs inline, which keeps the same
/// isolation without a C file.
pub fn create_native_surface(
    context: &VulkanContext,
    descriptor: &SurfaceDescriptor,
) -> RhiResult<sys::VkSurfaceKHR> {
    rhi_require(
        descriptor.native_handle != 0,
        "Vulkan requires a native surface handle",
    )?;
    // SAFETY: each branch below resolves the one entry point its platform needs and
    // passes a struct whose layout matches that extension.
    unsafe { create_native_surface_impl(context, descriptor) }
}

#[cfg(target_os = "windows")]
unsafe fn create_native_surface_impl(
    context: &VulkanContext,
    descriptor: &SurfaceDescriptor,
) -> RhiResult<sys::VkSurfaceKHR> {
    use core::ffi::c_void;
    rhi_require(
        descriptor.kind == crate::rhi::NativeSurfaceKind::Win32Window,
        "Vulkan requires a Win32 window surface",
    )?;
    /// `VkWin32SurfaceCreateInfoKHR`.
    #[repr(C)]
    struct CreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        hinstance: *mut c_void,
        hwnd: *mut c_void,
    }
    const STRUCTURE_TYPE: u32 = 1_000_009_000;
    type Create = unsafe extern "system" fn(
        sys::VkInstance,
        *const CreateInfo,
        *const sys::VkAllocationCallbacks,
        *mut sys::VkSurfaceKHR,
    ) -> sys::VkResult;
    // SAFETY: the caller guarantees the handle belongs to a live instance.
    let create: Create = unsafe {
        context
            .loader
            .load(context.instance, c"vkCreateWin32SurfaceKHR")
    }
    .ok_or_else(|| {
        RhiError::unsupported_feature("Vulkan loader does not support Win32 presentation")
    })?;
    let info = CreateInfo {
        s_type: STRUCTURE_TYPE,
        p_next: core::ptr::null(),
        flags: 0,
        // The window's own module instance is what the loader expects; passing the
        // process instance is the documented fallback when the caller does not own
        // the window class.
        hinstance: core::ptr::null_mut(),
        hwnd: descriptor.native_handle as *mut c_void,
    };
    let mut surface: sys::VkSurfaceKHR = sys::VK_NULL_HANDLE;
    // SAFETY: `info` outlives the call and the out-parameter is a live local.
    let status = unsafe { create(context.instance, &info, core::ptr::null(), &mut surface) };
    rhi_require(
        status == sys::VK_SUCCESS && surface != sys::VK_NULL_HANDLE,
        "native Vulkan surface creation failed",
    )?;
    Ok(surface)
}

#[cfg(target_os = "android")]
unsafe fn create_native_surface_impl(
    context: &VulkanContext,
    descriptor: &SurfaceDescriptor,
) -> RhiResult<sys::VkSurfaceKHR> {
    use core::ffi::c_void;
    rhi_require(
        descriptor.kind == crate::rhi::NativeSurfaceKind::AndroidWindow,
        "Vulkan requires an Android window surface",
    )?;
    /// `VkAndroidSurfaceCreateInfoKHR`.
    #[repr(C)]
    struct CreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        window: *mut c_void,
    }
    const STRUCTURE_TYPE: u32 = 1_000_023_000;
    type Create = unsafe extern "system" fn(
        sys::VkInstance,
        *const CreateInfo,
        *const sys::VkAllocationCallbacks,
        *mut sys::VkSurfaceKHR,
    ) -> sys::VkResult;
    // SAFETY: the caller guarantees the instance is live.
    let create: Create = unsafe {
        context
            .loader
            .load(context.instance, c"vkCreateAndroidSurfaceKHR")
    }
    .ok_or_else(|| {
        RhiError::unsupported_feature("Vulkan loader does not support Android presentation")
    })?;
    let info = CreateInfo {
        s_type: STRUCTURE_TYPE,
        p_next: core::ptr::null(),
        flags: 0,
        window: descriptor.native_handle as *mut c_void,
    };
    let mut surface: sys::VkSurfaceKHR = sys::VK_NULL_HANDLE;
    // SAFETY: `info` outlives the call and the out-parameter is a live local.
    let status = unsafe { create(context.instance, &info, core::ptr::null(), &mut surface) };
    rhi_require(
        status == sys::VK_SUCCESS && surface != sys::VK_NULL_HANDLE,
        "native Vulkan surface creation failed",
    )?;
    Ok(surface)
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
unsafe fn create_native_surface_impl(
    context: &VulkanContext,
    descriptor: &SurfaceDescriptor,
) -> RhiResult<sys::VkSurfaceKHR> {
    use core::ffi::c_void;

    if descriptor.kind == crate::rhi::NativeSurfaceKind::WaylandSurface {
        rhi_require(
            descriptor.display != 0,
            "Vulkan Wayland surface needs a display",
        )?;
        /// `VkWaylandSurfaceCreateInfoKHR`.
        #[repr(C)]
        struct CreateInfo {
            s_type: u32,
            p_next: *const c_void,
            flags: u32,
            display: *mut c_void,
            surface: *mut c_void,
        }
        const STRUCTURE_TYPE: u32 = 1_000_006_000;
        type Create = unsafe extern "system" fn(
            sys::VkInstance,
            *const CreateInfo,
            *const sys::VkAllocationCallbacks,
            *mut sys::VkSurfaceKHR,
        ) -> sys::VkResult;
        // SAFETY: the caller guarantees the instance is live.
        let create: Create = unsafe {
            context
                .loader
                .load(context.instance, c"vkCreateWaylandSurfaceKHR")
        }
        .ok_or_else(|| {
            RhiError::unsupported_feature("Vulkan loader does not support Wayland presentation")
        })?;
        let info = CreateInfo {
            s_type: STRUCTURE_TYPE,
            p_next: core::ptr::null(),
            flags: 0,
            display: descriptor.display as *mut c_void,
            surface: descriptor.native_handle as *mut c_void,
        };
        let mut surface: sys::VkSurfaceKHR = sys::VK_NULL_HANDLE;
        // SAFETY: `info` outlives the call and the out-parameter is a live local.
        let status = unsafe { create(context.instance, &info, core::ptr::null(), &mut surface) };
        rhi_require(
            status == sys::VK_SUCCESS && surface != sys::VK_NULL_HANDLE,
            "native Vulkan surface creation failed",
        )?;
        return Ok(surface);
    }

    rhi_require(
        descriptor.kind == crate::rhi::NativeSurfaceKind::XlibWindow,
        "Vulkan requires a Win32, Xlib, Wayland or Android surface",
    )?;
    rhi_require(
        descriptor.display != 0,
        "Vulkan Xlib surface needs a display",
    )?;
    /// `VkXlibSurfaceCreateInfoKHR`. `window` is an `unsigned long` in C.
    #[repr(C)]
    struct CreateInfo {
        s_type: u32,
        p_next: *const c_void,
        flags: u32,
        dpy: *mut c_void,
        window: core::ffi::c_ulong,
    }
    const STRUCTURE_TYPE: u32 = 1_000_004_000;
    type Create = unsafe extern "system" fn(
        sys::VkInstance,
        *const CreateInfo,
        *const sys::VkAllocationCallbacks,
        *mut sys::VkSurfaceKHR,
    ) -> sys::VkResult;
    // SAFETY: the caller guarantees the instance is live.
    let create: Create = unsafe {
        context
            .loader
            .load(context.instance, c"vkCreateXlibSurfaceKHR")
    }
    .ok_or_else(|| {
        RhiError::unsupported_feature("Vulkan loader does not support Xlib presentation")
    })?;
    let info = CreateInfo {
        s_type: STRUCTURE_TYPE,
        p_next: core::ptr::null(),
        flags: 0,
        dpy: descriptor.display as *mut c_void,
        window: descriptor.native_handle as core::ffi::c_ulong,
    };
    let mut surface: sys::VkSurfaceKHR = sys::VK_NULL_HANDLE;
    // SAFETY: `info` outlives the call and the out-parameter is a live local.
    let status = unsafe { create(context.instance, &info, core::ptr::null(), &mut surface) };
    rhi_require(
        status == sys::VK_SUCCESS && surface != sys::VK_NULL_HANDLE,
        "native Vulkan surface creation failed",
    )?;
    Ok(surface)
}

#[cfg(not(any(
    target_os = "windows",
    target_os = "android",
    target_os = "linux",
    target_os = "freebsd"
)))]
unsafe fn create_native_surface_impl(
    _context: &VulkanContext,
    _descriptor: &SurfaceDescriptor,
) -> RhiResult<sys::VkSurfaceKHR> {
    Err(RhiError::unsupported_backend(
        "Vulkan presentation is not implemented for this platform",
    ))
}

// MARK: - Presentation state

/// Per-image binary semaphores are reused only after that image is acquired
/// again, which proves the previous presentation consumed the semaphore.
#[derive(Default)]
pub struct VulkanPresentationState {
    pub acquired_id: Option<u32>,
    pub pending_acquire: bool,
    pub ready: HashMap<u32, sys::VkSemaphore>,
}

/// Observed native surface properties, independent of the image orientation the
/// renderer deliberately selects. Repeated advisory statuses alone must not retire
/// a still-usable swapchain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceState {
    pub extent: [u32; 2],
    pub transform: u32,
}

impl SurfaceState {
    pub fn from_capabilities(capabilities: &sys::VkSurfaceCapabilitiesKHR) -> Self {
        Self {
            extent: [
                capabilities.currentExtent.width,
                capabilities.currentExtent.height,
            ],
            transform: capabilities.currentTransform,
        }
    }
}

/// Chooses the presentation mode for a VSync policy.
///
/// Kept independent of Vulkan handles so platform policy can be tested without a
/// native window or a loader.
pub fn choose_presentation_mode(
    vsync_enabled: bool,
    supported_modes: &[PresentationMode],
) -> RhiResult<PresentationMode> {
    let preference: &[PresentationMode] = if vsync_enabled {
        &[PresentationMode::Fifo]
    } else {
        &[
            PresentationMode::Immediate,
            PresentationMode::Mailbox,
            PresentationMode::Fifo,
        ]
    };
    preference
        .iter()
        .copied()
        .find(|mode| supported_modes.contains(mode))
        .ok_or_else(|| {
            RhiError::unsupported_feature(
                "Vulkan surface has no supported presentation mode for the requested VSync policy",
            )
        })
}

/// The native present mode for a chosen RHI mode.
pub fn vk_present_mode(mode: PresentationMode) -> RhiResult<u32> {
    match mode {
        PresentationMode::Immediate => Ok(sys::VK_PRESENT_MODE_IMMEDIATE_KHR),
        PresentationMode::Mailbox => Ok(sys::VK_PRESENT_MODE_MAILBOX_KHR),
        PresentationMode::Fifo => Ok(sys::VK_PRESENT_MODE_FIFO_KHR),
        PresentationMode::Compositor => Err(RhiError::unsupported_feature(
            "Vulkan requires an explicit presentation mode",
        )),
    }
}

/// The RHI mode a native present mode represents, or `None` for a mode the RHI
/// does not model.
pub fn presentation_mode(mode: u32) -> Option<PresentationMode> {
    match mode {
        sys::VK_PRESENT_MODE_IMMEDIATE_KHR => Some(PresentationMode::Immediate),
        sys::VK_PRESENT_MODE_MAILBOX_KHR => Some(PresentationMode::Mailbox),
        sys::VK_PRESENT_MODE_FIFO_KHR => Some(PresentationMode::Fifo),
        _ => None,
    }
}

// MARK: - Swapchain

/// One window's swapchain plus the images wrapped as texture records.
pub struct VulkanSwapchain {
    pub surface: sys::VkSurfaceKHR,
    pub handle: sys::VkSwapchainKHR,
    pub format: u32,
    pub extent: sys::VkExtent2D,
    pub presentation_mode: PresentationMode,
    pub images: Vec<sys::VkImage>,
    /// Frontend texture handles, one per swapchain image.
    pub texture_ids: Vec<u32>,
    pub vsync: bool,
    pub requested_color_format: TextureFormat,
    pub surface_state: SurfaceState,
}

/// Each native window owns its acquire semaphores and presentation pool.
pub struct VulkanWindowSwapchain {
    pub chain: VulkanSwapchain,
    pub generation: u64,
    pub presentation: VulkanPresentationState,
    pub acquire_semaphores: HashMap<usize, sys::VkSemaphore>,
    pub needs_recreate: bool,
}

impl VulkanWindowSwapchain {
    pub fn new(chain: VulkanSwapchain, generation: u64) -> Self {
        Self {
            chain,
            generation,
            presentation: VulkanPresentationState::default(),
            acquire_semaphores: HashMap::new(),
            needs_recreate: false,
        }
    }
}

impl VulkanSwapchain {
    /// Creates a swapchain for `surface`, wrapping each image as a texture record.
    pub fn create(
        context: &VulkanContext,
        surface: sys::VkSurfaceKHR,
        descriptor: &SurfaceDescriptor,
        registries: &std::sync::Arc<std::sync::Mutex<VulkanRegistries>>,
    ) -> RhiResult<Self> {
        Self::build(
            context,
            surface,
            descriptor.width,
            descriptor.height,
            descriptor.color_format,
            descriptor.vsync_enabled,
            sys::VK_NULL_HANDLE,
            registries,
        )
    }

    /// Recreates the swapchain after a resize or `OUT_OF_DATE`. The previous image
    /// records are dropped; the fixed pool is rebuilt.
    pub fn recreate(
        &mut self,
        context: &VulkanContext,
        registries: &std::sync::Arc<std::sync::Mutex<VulkanRegistries>>,
    ) -> RhiResult<()> {
        // SAFETY: the caller waits for idle before recreating, so no queued work
        // references the old swapchain.
        unsafe { (context.instance_commands.device_wait_idle)(context.device) };
        let old = self.handle;
        let old_ids = self.texture_ids.clone();
        let rebuilt = Self::build(
            context,
            self.surface,
            self.extent.width as usize,
            self.extent.height as usize,
            self.requested_color_format,
            self.vsync,
            old,
            registries,
        )?;
        self.handle = rebuilt.handle;
        self.format = rebuilt.format;
        self.extent = rebuilt.extent;
        self.presentation_mode = rebuilt.presentation_mode;
        self.images = rebuilt.images;
        self.texture_ids = rebuilt.texture_ids;
        self.surface_state = rebuilt.surface_state;
        {
            let mut registries = lock(registries);
            for id in old_ids {
                if let Some(record) = registries.textures.remove(&id) {
                    // SAFETY: the device is idle, so this view is unused.
                    unsafe {
                        (context.core.destroy_image_view)(
                            context.device,
                            record.view,
                            core::ptr::null(),
                        )
                    };
                }
            }
        }
        // SAFETY: the device is idle and the new swapchain replaced the old one.
        unsafe {
            (context.sync.destroy_swapchain)(context.device, old, core::ptr::null());
        }
        Ok(())
    }

    /// Acquires the next image into `semaphore`, returning its index and the raw
    /// result so the caller can handle `OUT_OF_DATE`/`SUBOPTIMAL`.
    pub fn acquire(
        &self,
        context: &VulkanContext,
        semaphore: sys::VkSemaphore,
        timeout: u64,
    ) -> (u32, sys::VkResult) {
        let mut index: u32 = 0;
        // SAFETY: the swapchain is live and the out-parameter is a live local; a
        // null fence means "signal the semaphore only".
        let result = unsafe {
            (context.sync.acquire_next_image)(
                context.device,
                self.handle,
                timeout,
                semaphore,
                sys::VK_NULL_HANDLE,
                &mut index,
            )
        };
        (index, result)
    }

    /// Queues one presentation that waits on every semaphore in `wait`.
    pub fn present_with(
        &self,
        context: &VulkanContext,
        queue: sys::VkQueue,
        wait: &[sys::VkSemaphore],
        index: u32,
    ) -> sys::VkResult {
        let info = sys::VkPresentInfoKHR {
            sType: sys::VK_STRUCTURE_TYPE_PRESENT_INFO_KHR,
            pNext: core::ptr::null(),
            waitSemaphoreCount: wait.len() as u32,
            pWaitSemaphores: wait.as_ptr(),
            swapchainCount: 1,
            pSwapchains: &self.handle,
            pImageIndices: &index,
            pResults: core::ptr::null_mut(),
        };
        // SAFETY: every pointer in `info` refers to a live local and the swapchain
        // is live.
        unsafe { (context.sync.queue_present)(queue, &info) }
    }

    /// `SUBOPTIMAL` is advisory. Android can return it on every landscape frame
    /// when identity `preTransform` leaves rotation to the compositor; rebuilding
    /// the identical chain cannot resolve that condition.
    pub fn surface_changed(&self, context: &VulkanContext) -> bool {
        let mut capabilities = sys::VkSurfaceCapabilitiesKHR::default();
        // SAFETY: the surface is live and the out-parameter is a live local.
        let status = unsafe {
            (context.instance_commands.get_surface_capabilities)(
                context.physical_device,
                self.surface,
                &mut capabilities,
            )
        };
        if status != sys::VK_SUCCESS {
            // Present may already be queued; reporting a change lets the next
            // recreation surface an unavailable/lost surface.
            return true;
        }
        self.surface_state != SurfaceState::from_capabilities(&capabilities)
    }

    /// Destroys the swapchain, its views and the surface.
    pub fn destroy(
        self,
        context: &VulkanContext,
        registries: &std::sync::Arc<std::sync::Mutex<VulkanRegistries>>,
    ) {
        {
            let mut registries = lock(registries);
            for id in self.texture_ids {
                if let Some(record) = registries.textures.remove(&id) {
                    // SAFETY: the frame ring waited for every presentation.
                    unsafe {
                        (context.core.destroy_image_view)(
                            context.device,
                            record.view,
                            core::ptr::null(),
                        )
                    };
                }
            }
        }
        // SAFETY: the device is idle and this swapchain is no longer presented.
        unsafe {
            (context.sync.destroy_swapchain)(context.device, self.handle, core::ptr::null());
            (context.instance_commands.destroy_surface)(
                context.instance,
                self.surface,
                core::ptr::null(),
            );
        }
    }

    fn build(
        context: &VulkanContext,
        surface: sys::VkSurfaceKHR,
        width: usize,
        height: usize,
        color_format: TextureFormat,
        vsync: bool,
        old_swapchain: sys::VkSwapchainKHR,
        registries: &std::sync::Arc<std::sync::Mutex<VulkanRegistries>>,
    ) -> RhiResult<Self> {
        let mut capabilities = sys::VkSurfaceCapabilitiesKHR::default();
        // SAFETY: the surface is live and the out-parameter is a live local.
        let status = unsafe {
            (context.instance_commands.get_surface_capabilities)(
                context.physical_device,
                surface,
                &mut capabilities,
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS,
            "Vulkan surface capabilities unavailable",
        )?;

        let extent = if capabilities.currentExtent.width != u32::MAX {
            capabilities.currentExtent
        } else {
            rhi_require(
                width > 0
                    && height > 0
                    && width <= u32::MAX as usize
                    && height <= u32::MAX as usize,
                "Vulkan surface needs a positive drawable extent",
            )?;
            sys::VkExtent2D {
                width: (width as u32).clamp(
                    capabilities.minImageExtent.width,
                    capabilities.maxImageExtent.width,
                ),
                height: (height as u32).clamp(
                    capabilities.minImageExtent.height,
                    capabilities.maxImageExtent.height,
                ),
            }
        };
        rhi_require(
            extent.width > 0 && extent.height > 0,
            "Vulkan drawable has zero extent",
        )?;

        let formats = surface_formats(context, surface)?;
        let desired = formats::vk_format(color_format);
        let mut chosen = formats
            .iter()
            .copied()
            .find(|format| format.format == desired);
        if chosen.is_none() && formats.len() == 1 && formats[0].format == sys::VK_FORMAT_UNDEFINED {
            chosen = Some(sys::VkSurfaceFormatKHR {
                format: desired,
                colorSpace: formats[0].colorSpace,
            });
        }
        let chosen = chosen.ok_or_else(|| {
            RhiError::unsupported_feature(
                "Vulkan surface cannot present the requested color format",
            )
        })?;

        let native_modes = present_modes(context, surface)?;
        let supported: Vec<PresentationMode> = native_modes
            .iter()
            .filter_map(|mode| presentation_mode(*mode))
            .collect();
        let selected_mode = choose_presentation_mode(vsync, &supported)?;
        let present_mode = vk_present_mode(selected_mode)?;

        let image_count = if capabilities.maxImageCount == 0 {
            capabilities.minImageCount + 1
        } else {
            (capabilities.minImageCount + 1).min(capabilities.maxImageCount)
        };

        rhi_require(
            capabilities.supportedUsageFlags & sys::VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT != 0,
            "Vulkan surface images do not support color rendering",
        )?;
        rhi_require(
            capabilities.supportedTransforms & sys::VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR != 0,
            "Vulkan surface requires an unsupported pre-rotation transform",
        )?;
        // Callers render in window coordinates, so passing a rotated transform
        // would promise pre-rotated shader output the render-pass contract does not
        // provide. Let the compositor orient it.
        let composite_alpha = capabilities.supportedCompositeAlpha
            & capabilities.supportedCompositeAlpha.wrapping_neg();

        let create_info = sys::VkSwapchainCreateInfoKHR {
            sType: sys::VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
            pNext: core::ptr::null(),
            flags: 0,
            surface,
            minImageCount: image_count,
            imageFormat: chosen.format,
            imageColorSpace: chosen.colorSpace,
            imageExtent: extent,
            imageArrayLayers: 1,
            imageUsage: sys::VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT,
            imageSharingMode: sys::VK_SHARING_MODE_EXCLUSIVE,
            queueFamilyIndexCount: 0,
            pQueueFamilyIndices: core::ptr::null(),
            preTransform: sys::VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR,
            compositeAlpha: composite_alpha,
            presentMode: present_mode,
            clipped: sys::VK_TRUE,
            oldSwapchain: old_swapchain,
        };

        let mut swapchain: sys::VkSwapchainKHR = sys::VK_NULL_HANDLE;
        // SAFETY: `create_info` outlives the call and the out-parameter is a live
        // local.
        let status = unsafe {
            (context.sync.create_swapchain)(
                context.device,
                &create_info,
                core::ptr::null(),
                &mut swapchain,
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && swapchain != sys::VK_NULL_HANDLE,
            "vkCreateSwapchainKHR failed",
        )?;

        let mut actual_count: u32 = 0;
        // SAFETY: the swapchain is live and `actual_count` is a live local.
        let status = unsafe {
            (context.sync.get_swapchain_images)(
                context.device,
                swapchain,
                &mut actual_count,
                core::ptr::null_mut(),
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && actual_count > 0,
            "Vulkan swapchain has no images",
        )?;
        let mut images = vec![sys::VK_NULL_HANDLE; actual_count as usize];
        // SAFETY: `images` was sized from the reported count.
        let status = unsafe {
            (context.sync.get_swapchain_images)(
                context.device,
                swapchain,
                &mut actual_count,
                images.as_mut_ptr(),
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS,
            "Vulkan swapchain image enumeration failed",
        )?;
        images.truncate(actual_count as usize);

        // Wrap each swapchain image as a texture record in the fixed pool.
        let mut texture_ids = Vec::with_capacity(images.len());
        {
            let mut registries = lock(registries);
            for image in &images {
                let id = registries.next_internal_id();
                let mut view_info = sys::VkImageViewCreateInfo {
                    sType: sys::VK_STRUCTURE_TYPE_IMAGE_VIEW_CREATE_INFO,
                    pNext: core::ptr::null(),
                    flags: 0,
                    image: *image,
                    viewType: sys::VK_IMAGE_VIEW_TYPE_2D,
                    format: chosen.format,
                    components: [0; 4],
                    subresourceRange: sys::VkImageSubresourceRange {
                        aspectMask: sys::VK_IMAGE_ASPECT_COLOR_BIT,
                        baseMipLevel: 0,
                        levelCount: 1,
                        baseArrayLayer: 0,
                        layerCount: 1,
                    },
                };
                let mut view: sys::VkImageView = sys::VK_NULL_HANDLE;
                // SAFETY: the image belongs to this live swapchain.
                let status = unsafe {
                    (context.core.create_image_view)(
                        context.device,
                        &view_info,
                        core::ptr::null(),
                        &mut view,
                    )
                };
                if status != sys::VK_SUCCESS || view == sys::VK_NULL_HANDLE {
                    // Release everything already wrapped before reporting failure.
                    for id in texture_ids.drain(..) {
                        if let Some(record) = registries.textures.remove(&id) {
                            // SAFETY: this view was created above and is unused.
                            unsafe {
                                (context.core.destroy_image_view)(
                                    context.device,
                                    record.view,
                                    core::ptr::null(),
                                )
                            };
                        }
                    }
                    // SAFETY: the swapchain has no wrapped images yet.
                    unsafe {
                        (context.sync.destroy_swapchain)(
                            context.device,
                            swapchain,
                            core::ptr::null(),
                        )
                    };
                    return Err(RhiError::OutOfMemory);
                }
                view_info.subresourceRange.levelCount = 1;
                registries.textures.insert(
                    id,
                    VulkanTextureRecord {
                        image: *image,
                        allocation: None,
                        view,
                        format: formats::texture_format(chosen.format),
                        width: extent.width as usize,
                        height: extent.height as usize,
                        depth: 1,
                        dimension: crate::rhi::TextureDimension::Texture2D,
                        layers: 1,
                        mip_levels: 1,
                        usage: crate::rhi::TextureUsage::PRESENT
                            | crate::rhi::TextureUsage::COLOR_TARGET,
                        is_swapchain: true,
                        layout: sys::VK_IMAGE_LAYOUT_UNDEFINED,
                        sample_count: 1,
                        transient_attachment: false,
                        attachment_view: None,
                    },
                );
                texture_ids.push(id);
            }
        }

        Ok(Self {
            surface,
            handle: swapchain,
            format: chosen.format,
            extent,
            presentation_mode: selected_mode,
            images,
            texture_ids,
            vsync,
            requested_color_format: color_format,
            surface_state: SurfaceState::from_capabilities(&capabilities),
        })
    }
}

fn surface_formats(
    context: &VulkanContext,
    surface: sys::VkSurfaceKHR,
) -> RhiResult<Vec<sys::VkSurfaceFormatKHR>> {
    let mut count: u32 = 0;
    // SAFETY: the surface is live and `count` is a live local.
    let status = unsafe {
        (context.instance_commands.get_surface_formats)(
            context.physical_device,
            surface,
            &mut count,
            core::ptr::null_mut(),
        )
    };
    rhi_require(
        status == sys::VK_SUCCESS && count > 0,
        "Vulkan surface has no color formats",
    )?;
    let mut formats = vec![sys::VkSurfaceFormatKHR::default(); count as usize];
    // SAFETY: `formats` was sized from the reported count.
    let status = unsafe {
        (context.instance_commands.get_surface_formats)(
            context.physical_device,
            surface,
            &mut count,
            formats.as_mut_ptr(),
        )
    };
    rhi_require(
        status == sys::VK_SUCCESS,
        "Vulkan surface format enumeration failed",
    )?;
    formats.truncate(count as usize);
    Ok(formats)
}

/// The mode list may grow between the count and data queries, so `VK_INCOMPLETE`
/// is retried and unwritten array entries are never inspected.
fn present_modes(context: &VulkanContext, surface: sys::VkSurfaceKHR) -> RhiResult<Vec<u32>> {
    for _ in 0..4 {
        let mut count: u32 = 0;
        // SAFETY: the surface is live and `count` is a live local.
        let status = unsafe {
            (context.instance_commands.get_surface_present_modes)(
                context.physical_device,
                surface,
                &mut count,
                core::ptr::null_mut(),
            )
        };
        rhi_require(
            status == sys::VK_SUCCESS && count > 0,
            "Vulkan present mode count query failed",
        )?;
        let mut modes = vec![sys::VK_PRESENT_MODE_FIFO_KHR; count as usize];
        // SAFETY: `modes` was sized from the reported count, which the call may
        // lower.
        let result = unsafe {
            (context.instance_commands.get_surface_present_modes)(
                context.physical_device,
                surface,
                &mut count,
                modes.as_mut_ptr(),
            )
        };
        if result == sys::VK_INCOMPLETE {
            continue;
        }
        rhi_require(
            result == sys::VK_SUCCESS && count as usize <= modes.len(),
            "Vulkan present mode enumeration failed",
        )?;
        modes.truncate(count as usize);
        return Ok(modes);
    }
    Err(RhiError::SwapchainAcquireFailed(
        "Vulkan present mode enumeration remained incomplete".into(),
    ))
}

/// The image aspect mask a texture's format needs for barriers.
pub fn image_aspect(format: TextureFormat) -> u32 {
    formats::aspect_flags(format)
}

/// Marks one acquired swapchain image as pending presentation.
pub fn mark_acquired(window: &mut VulkanWindowSwapchain, texture: u32) {
    window.presentation.acquired_id = Some(texture);
    window.presentation.pending_acquire = true;
}

/// Resolves the image index for a previously acquired texture handle.
pub fn image_index(
    window: &VulkanWindowSwapchain,
    image: Swapchain,
    texture: u32,
) -> RhiResult<u32> {
    rhi_require(
        window.chain.texture_ids.iter().any(|id| *id == texture),
        "image was not acquired from this swapchain",
    )?;
    let _ = image;
    window
        .chain
        .texture_ids
        .iter()
        .position(|id| *id == texture)
        .map(|index| index as u32)
        .ok_or_else(|| RhiError::present_failed("image was not acquired from this swapchain"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vsync_prefers_fifo_and_never_immediate() {
        let modes = [PresentationMode::Immediate, PresentationMode::Fifo];
        assert_eq!(
            choose_presentation_mode(true, &modes),
            Ok(PresentationMode::Fifo)
        );
        // An unsynchronized request prefers immediate, then mailbox, then fifo.
        assert_eq!(
            choose_presentation_mode(false, &modes),
            Ok(PresentationMode::Immediate)
        );
        assert_eq!(
            choose_presentation_mode(false, &[PresentationMode::Mailbox, PresentationMode::Fifo]),
            Ok(PresentationMode::Mailbox)
        );
        assert_eq!(
            choose_presentation_mode(false, &[PresentationMode::Fifo]),
            Ok(PresentationMode::Fifo)
        );
        // A compositor-only surface cannot satisfy either policy.
        assert!(choose_presentation_mode(false, &[PresentationMode::Compositor]).is_err());
        assert!(choose_presentation_mode(true, &[]).is_err());
    }

    #[test]
    fn native_modes_round_trip() {
        for (native, expected) in [
            (
                sys::VK_PRESENT_MODE_IMMEDIATE_KHR,
                PresentationMode::Immediate,
            ),
            (sys::VK_PRESENT_MODE_MAILBOX_KHR, PresentationMode::Mailbox),
            (sys::VK_PRESENT_MODE_FIFO_KHR, PresentationMode::Fifo),
        ] {
            assert_eq!(presentation_mode(native), Some(expected));
            assert_eq!(vk_present_mode(expected), Ok(native));
        }
        assert_eq!(presentation_mode(9999), None);
        assert!(vk_present_mode(PresentationMode::Compositor).is_err());
    }

    #[test]
    fn surface_state_compares_extent_and_transform() {
        let mut capabilities = sys::VkSurfaceCapabilitiesKHR::default();
        capabilities.currentExtent = sys::VkExtent2D {
            width: 800,
            height: 600,
        };
        capabilities.currentTransform = sys::VK_SURFACE_TRANSFORM_IDENTITY_BIT_KHR;
        let state = SurfaceState::from_capabilities(&capabilities);
        assert_eq!(state.extent, [800, 600]);
        assert_eq!(state.transform, 1);
        // A resize changes the observed state.
        capabilities.currentExtent.width = 1024;
        assert_ne!(SurfaceState::from_capabilities(&capabilities), state);
    }

    #[test]
    fn an_image_must_belong_to_the_swapchain() {
        let chain = VulkanSwapchain {
            surface: 1,
            handle: 2,
            format: sys::VK_FORMAT_B8G8R8A8_UNORM,
            extent: sys::VkExtent2D {
                width: 1,
                height: 1,
            },
            presentation_mode: PresentationMode::Fifo,
            images: vec![10, 11],
            texture_ids: vec![20, 21],
            vsync: true,
            requested_color_format: TextureFormat::Bgra8Unorm,
            surface_state: SurfaceState {
                extent: [1, 1],
                transform: 1,
            },
        };
        let window = VulkanWindowSwapchain::new(chain, 1);
        assert_eq!(image_index(&window, Swapchain::new(1), 21), Ok(1));
        assert!(image_index(&window, Swapchain::new(1), 99).is_err());
    }
}
