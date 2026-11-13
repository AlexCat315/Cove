//! Translation of NativeRHI/Vulkan/Setup/{VulkanInstanceSetup,
//! VulkanDeviceSetup}.swift plus VulkanRuntime.swift: instance, physical device
//! and logical device creation, and the real capability probe.
//!
//! Nothing here hardcodes ray tracing or mesh shaders: the reported capability
//! set comes from the enumerated physical-device extensions and the queried
//! feature chain. `VkInstance` is created with only the presentation extensions
//! the loader actually advertises, so a Linux box with Xlib but no Wayland still
//! starts.
use std::collections::HashSet;
use std::ffi::CStr;

use crate::rhi::{RhiError, RhiResult, rhi_require};

use super::sys;

// MARK: - Context

/// One queue handle plus the family it came from.
#[derive(Clone, Copy, Debug)]
pub struct VulkanQueue {
    pub handle: sys::VkQueue,
    pub family: u32,
    pub timestamp_bits: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct VulkanQueues {
    pub graphics: VulkanQueue,
    pub compute: VulkanQueue,
    pub transfer: VulkanQueue,
}

/// Everything a backend operation needs: the live handles, the resolved command
/// tables, the device limits and the enabled feature set.
pub struct VulkanContext {
    #[allow(
        dead_code,
        reason = "keeps the driver library loaded while the context lives"
    )]
    pub loader: std::sync::Arc<sys::VulkanLoader>,
    pub instance_commands: sys::InstanceCommands,
    pub core: sys::CoreCommands,
    pub resources: sys::ResourceCommands,
    pub draw: sys::DrawCommands,
    pub sync: sys::SyncCommands,
    pub advanced: sys::AdvancedCommands,
    pub limits: Box<sys::VkPhysicalDeviceLimits>,
    pub features: sys::FeatureSupport,
    pub instance: sys::VkInstance,
    pub physical_device: sys::VkPhysicalDevice,
    pub device: sys::VkDevice,
    pub queues: VulkanQueues,
    pub device_name: String,
    pub extensions: HashSet<String>,
    /// The block allocator immediate transfers stage through. Separate from the
    /// resource allocator so a transfer can map and release storage without
    /// touching the resource blocks.
    pub staging_allocator: std::sync::Arc<std::sync::Mutex<super::memory::VulkanMemoryAllocator>>,
    /// The one-shot command pool and fence immediate transfers reuse.
    pub scratch: std::sync::Arc<std::sync::Mutex<super::submission::ScratchResources>>,
}

impl VulkanContext {
    /// The render-pass handles this context's device can create passes with.
    pub fn rendering_handles(&self) -> super::rendering::RenderingHandles {
        super::rendering::RenderingHandles::new(self)
    }
}

// MARK: - Runtime probe

/// Reports whether a usable Vulkan driver (ICD) is present.
///
/// Swift cached this probe because creating a throwaway instance is not free.
/// The Rust port does the same with a `OnceLock`.
pub fn is_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(probe_availability)
}

/// Creates a throwaway instance and confirms at least one physical device (ICD)
/// is enumerated. Any failure means the platform has no usable Vulkan driver.
fn probe_availability() -> bool {
    let Some(loader) = sys::VulkanLoader::open() else {
        return false;
    };
    let Ok(instance) = create_instance(&loader, false, false) else {
        return false;
    };
    let Some(commands) = sys::InstanceCommands::load(&loader, instance) else {
        if let Some(destroy) =
            // SAFETY: the instance was created above and has no children yet.
            unsafe {
                loader.load::<sys::PFN_vkDestroyInstance>(instance, c"vkDestroyInstance")
            }
        {
            // SAFETY: same as above; nothing else references the instance.
            unsafe { destroy(instance, core::ptr::null()) };
        }
        return false;
    };
    let mut count: u32 = 0;
    // SAFETY: `commands` was resolved for this instance.
    let result = unsafe {
        (commands.enumerate_physical_devices)(instance, &mut count, core::ptr::null_mut())
    };
    // SAFETY: the instance has no children; destroying it also frees the devices
    // list, which was never read.
    unsafe { (commands.destroy_instance)(instance, core::ptr::null()) };
    result == sys::VK_SUCCESS && count > 0
}

// MARK: - Instance

/// Creates the instance with the presentation extensions the loader supports.
pub fn create_instance(
    loader: &sys::VulkanLoader,
    enable_validation: bool,
    presentation: bool,
) -> RhiResult<sys::VkInstance> {
    // SAFETY: both names are the exact global entry points their types declare.
    let create: sys::PFN_vkCreateInstance =
        // SAFETY: see above.
        unsafe { loader.load(sys::VK_NULL_INSTANCE, c"vkCreateInstance") }
            .ok_or_else(|| RhiError::unsupported_backend("Vulkan loader is missing vkCreateInstance"))?;
    // SAFETY: as above.
    let enumerate: sys::PFN_vkEnumerateInstanceExtensionProperties =
        // SAFETY: as above.
        unsafe { loader.load(sys::VK_NULL_INSTANCE, c"vkEnumerateInstanceExtensionProperties") }
            .ok_or_else(|| {
                RhiError::unsupported_backend(
                    "Vulkan loader is missing vkEnumerateInstanceExtensionProperties",
                )
            })?;

    let supported = instance_extensions(enumerate)?;
    let mut names: Vec<&CStr> = Vec::new();
    if presentation {
        rhi_require(
            supported.iter().any(|name| name == "VK_KHR_surface"),
            "Vulkan loader does not support VK_KHR_surface",
        )?;
        names.push(c"VK_KHR_surface");
        #[cfg(target_os = "windows")]
        {
            rhi_require(
                supported.iter().any(|name| name == "VK_KHR_win32_surface"),
                "Vulkan loader does not support Win32 presentation",
            )?;
            names.push(c"VK_KHR_win32_surface");
        }
        #[cfg(target_os = "android")]
        {
            rhi_require(
                supported
                    .iter()
                    .any(|name| name == "VK_KHR_android_surface"),
                "Vulkan loader does not support Android presentation",
            )?;
            names.push(c"VK_KHR_android_surface");
        }
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        {
            // Either WSI extension may be absent on a Linux installation.
            for candidate in ["VK_KHR_xlib_surface", "VK_KHR_wayland_surface"] {
                if supported.iter().any(|name| name == candidate) {
                    names.push(if candidate == "VK_KHR_xlib_surface" {
                        c"VK_KHR_xlib_surface"
                    } else {
                        c"VK_KHR_wayland_surface"
                    });
                }
            }
        }
    }

    // Query the loader's API version, then cap it at 1.3.
    let mut version: u32 = sys::api_version(1, 0);
    // SAFETY: `vkEnumerateInstanceVersion` is optional before 1.1; when absent
    // the loader's version stays at the 1.0 default, which the check below
    // rejects with a clear message.
    if let Some(enumerate_version) = unsafe {
        loader.load::<sys::PFN_vkEnumerateInstanceVersion>(
            sys::VK_NULL_INSTANCE,
            c"vkEnumerateInstanceVersion",
        )
    } {
        // SAFETY: the pointer refers to a live local and the call only writes it.
        unsafe { enumerate_version(&mut version) };
    }
    rhi_require(
        version >= sys::api_version(1, 1),
        "NativeRHI requires a Vulkan 1.1 loader",
    )?;

    let application_name = c"NativeRHI";
    let engine_name = c"NativeRHI";
    let application = sys::VkApplicationInfo {
        sType: sys::VK_STRUCTURE_TYPE_APPLICATION_INFO,
        pNext: core::ptr::null(),
        pApplicationName: application_name.as_ptr(),
        applicationVersion: 0,
        pEngineName: engine_name.as_ptr(),
        engineVersion: 0,
        apiVersion: version.min(sys::api_version(1, 3)),
    };

    let extension_pointers: Vec<*const core::ffi::c_char> =
        names.iter().map(|name| name.as_ptr()).collect();
    let layers: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
    // Validation is a development aid: a device without the layer installed,
    // such as any Android phone, runs without it instead of failing to start.
    let validation_available = enable_validation
        && instance_layers(loader)
            .iter()
            .any(|name| name == "VK_LAYER_KHRONOS_validation");
    let layer_pointers: Vec<*const core::ffi::c_char> = if validation_available {
        layers.iter().map(|name| name.as_ptr()).collect()
    } else {
        Vec::new()
    };

    let create_info = sys::VkInstanceCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
        pNext: core::ptr::null(),
        flags: 0,
        pApplicationInfo: &application,
        enabledLayerCount: layer_pointers.len() as u32,
        ppEnabledLayerNames: layer_pointers.as_ptr(),
        enabledExtensionCount: extension_pointers.len() as u32,
        ppEnabledExtensionNames: extension_pointers.as_ptr(),
    };

    let mut instance: sys::VkInstance = sys::VK_NULL_INSTANCE;
    // SAFETY: `create_info` and every array/string it points at are live for the
    // duration of the call, and the out-parameter is a live local.
    let result = unsafe { create(&create_info, core::ptr::null(), &mut instance) };
    if result != sys::VK_SUCCESS || instance.is_null() {
        return Err(RhiError::invalid_argument(format!(
            "vkCreateInstance failed: VkResult {result}, API {}.{}, extensions {:?}",
            version >> 22,
            (version >> 12) & 0x3ff,
            names
        )));
    }
    Ok(instance)
}

/// The loader's instance layers; empty when they cannot be enumerated.
fn instance_layers(loader: &sys::VulkanLoader) -> Vec<String> {
    // SAFETY: the name is the global entry point its type declares.
    let Some(enumerate) = (unsafe {
        loader.load::<sys::PFN_vkEnumerateInstanceLayerProperties>(
            sys::VK_NULL_INSTANCE,
            c"vkEnumerateInstanceLayerProperties",
        )
    }) else {
        return Vec::new();
    };
    for _ in 0..4 {
        let mut count: u32 = 0;
        // SAFETY: a null array asks only for the count.
        if unsafe { enumerate(&mut count, core::ptr::null_mut()) } != sys::VK_SUCCESS {
            return Vec::new();
        }
        let mut values = vec![sys::VkLayerProperties::default(); count as usize];
        // SAFETY: `values` holds `count` entries and `count` bounds the write.
        let result = unsafe { enumerate(&mut count, values.as_mut_ptr()) };
        if result == sys::VK_INCOMPLETE {
            continue;
        }
        if result != sys::VK_SUCCESS {
            return Vec::new();
        }
        values.truncate(count as usize);
        return values.iter().map(sys::VkLayerProperties::name).collect();
    }
    Vec::new()
}

/// Enumerates the loader's instance extensions, retrying `VK_INCOMPLETE`.
fn instance_extensions(
    enumerate: sys::PFN_vkEnumerateInstanceExtensionProperties,
) -> RhiResult<Vec<String>> {
    for _ in 0..4 {
        let mut count: u32 = 0;
        // SAFETY: a null layer name means "all layers".
        let status = unsafe { enumerate(core::ptr::null(), &mut count, core::ptr::null_mut()) };
        rhi_require(
            status == sys::VK_SUCCESS && count > 0,
            "Vulkan instance extension enumeration failed",
        )?;
        let mut values = vec![sys::VkExtensionProperties::default(); count as usize];
        // SAFETY: `values` was sized from the count the driver just reported, and
        // `count` is both in- and out-parameter bounded by that size.
        let result = unsafe { enumerate(core::ptr::null(), &mut count, values.as_mut_ptr()) };
        if result == sys::VK_INCOMPLETE {
            continue;
        }
        rhi_require(
            result == sys::VK_SUCCESS,
            "Vulkan instance extension enumeration failed",
        )?;
        return Ok(values
            .iter()
            .take(count as usize)
            .map(|value| value.name())
            .collect());
    }
    Err(RhiError::unsupported_backend(
        "Vulkan instance extensions changed repeatedly during enumeration",
    ))
}

// MARK: - Device

struct QueueFamilies {
    graphics: u32,
    compute: u32,
    transfer: u32,
    timestamp_bits: Vec<u32>,
}

/// Builds a full context, throwing a clear error if any stage fails.
pub fn make_context(
    loader: sys::VulkanLoader,
    enable_validation: bool,
) -> RhiResult<VulkanContext> {
    let loader = std::sync::Arc::new(loader);
    let instance = create_instance(&loader, enable_validation, true)?;
    let Some(instance_commands) = sys::InstanceCommands::load(&loader, instance) else {
        // SAFETY: the instance has no children yet, so it can be destroyed as-is.
        unsafe {
            (loader
                .load::<sys::PFN_vkDestroyInstance>(instance, c"vkDestroyInstance")
                .expect("vkDestroyInstance is a required entry point"))(
                instance, core::ptr::null()
            )
        };
        return Err(RhiError::unsupported_backend(
            "Vulkan loader is missing instance entry points",
        ));
    };

    // Every failure path releases whatever was created so far.
    let mut owned_device: sys::VkDevice = core::ptr::null_mut();
    let outcome = build_context(&loader, instance, &instance_commands, &mut owned_device);
    if outcome.is_err() {
        // SAFETY: `owned_device` is either null or a live device this function
        // created; the instance still owns no children afterwards.
        unsafe {
            if !owned_device.is_null() {
                (instance_commands.destroy_device)(owned_device, core::ptr::null());
            }
            (instance_commands.destroy_instance)(instance, core::ptr::null());
        }
    }
    outcome
}

fn build_context(
    loader: &std::sync::Arc<sys::VulkanLoader>,
    instance: sys::VkInstance,
    instance_commands: &sys::InstanceCommands,
    owned_device: &mut sys::VkDevice,
) -> RhiResult<VulkanContext> {
    let mut memory_properties = sys::VkPhysicalDeviceMemoryProperties::default();
    let physical_device = select_physical_device(instance_commands, instance)?
        .ok_or_else(|| RhiError::unsupported_backend("no Vulkan physical device (ICD) found"))?;

    let mut properties = sys::VkPhysicalDeviceProperties::default();
    // SAFETY: the physical device is live and the out-parameter is a live local.
    unsafe { (instance_commands.get_physical_device_properties)(physical_device, &mut properties) };
    // The memory-type table is what the block allocator searches.
    // SAFETY: as above.
    unsafe {
        (instance_commands.get_physical_device_memory_properties)(
            physical_device,
            &mut memory_properties,
        )
    };
    rhi_require(
        properties.apiVersion >= sys::api_version(1, 1),
        "NativeRHI requires Vulkan 1.1",
    )?;
    let device_name = properties.device_name();

    let extensions = device_extensions(instance_commands, physical_device);
    let families = find_queue_families(instance_commands, physical_device);
    rhi_require(
        families.graphics != u32::MAX,
        "no graphics-capable Vulkan queue family",
    )?;

    let unique_families = unique_family_list(&families);
    let (device, features) = create_logical_device(
        instance_commands,
        physical_device,
        &families,
        &unique_families,
        &extensions,
        properties.apiVersion,
    )?;
    *owned_device = device;

    // Device entry points are resolved through vkGetDeviceProcAddr.
    // SAFETY: `vkGetDeviceProcAddr` is an instance-level entry point with the
    // documented signature, and the device is live.
    let get_device_proc_addr: sys::PFN_vkGetDeviceProcAddr =
        unsafe { loader.load(instance, c"vkGetDeviceProcAddr") }
            .ok_or_else(|| RhiError::unsupported_backend("vkGetDeviceProcAddr unavailable"))?;
    let resolve = |name: &CStr| -> sys::PFN_vkVoidFunction {
        // SAFETY: the device is live and `name` is NUL-terminated.
        unsafe { get_device_proc_addr(device, name.as_ptr()) }
    };

    // SAFETY: every table below resolves entry points for this live device, which
    // is the contract `load` requires.
    let (core, resources, draw, sync) = unsafe {
        (
            sys::CoreCommands::load(&resolve).ok_or_else(|| {
                RhiError::unsupported_backend("device is missing core Vulkan entry points")
            })?,
            sys::ResourceCommands::load(&resolve).ok_or_else(|| {
                RhiError::unsupported_backend("device is missing resource Vulkan entry points")
            })?,
            sys::DrawCommands::load(&resolve).ok_or_else(|| {
                RhiError::unsupported_backend("device is missing draw Vulkan entry points")
            })?,
            sys::SyncCommands::load(&resolve).ok_or_else(|| {
                RhiError::unsupported_backend(
                    "device is missing swapchain entry points; is VK_KHR_swapchain enabled?",
                )
            })?,
        )
    };

    // SAFETY: as above; the optional table only resolves what the features allow.
    let advanced = unsafe {
        sys::AdvancedCommands::load(
            &resolve,
            &features,
            loader,
            instance,
            physical_device,
            instance_commands,
        )
    };

    let mut graphics_queue: sys::VkQueue = core::ptr::null_mut();
    // SAFETY: the family index came from the device's own queue-family list.
    unsafe {
        (instance_commands.get_device_queue)(device, families.graphics, 0, &mut graphics_queue)
    };
    let mut compute_queue = graphics_queue;
    if families.compute != families.graphics {
        // SAFETY: the family index came from the device's own queue-family list.
        unsafe {
            (instance_commands.get_device_queue)(device, families.compute, 0, &mut compute_queue)
        };
    }
    let mut transfer_queue = graphics_queue;
    if families.transfer != families.graphics {
        // SAFETY: the family index came from the device's own queue-family list.
        unsafe {
            (instance_commands.get_device_queue)(device, families.transfer, 0, &mut transfer_queue)
        };
    }

    Ok(VulkanContext {
        loader: std::sync::Arc::clone(loader),
        instance_commands: *instance_commands,
        core,
        resources,
        draw,
        sync,
        advanced,
        limits: Box::new(properties.limits),
        features,
        instance,
        physical_device,
        device,
        queues: VulkanQueues {
            graphics: VulkanQueue {
                handle: graphics_queue,
                family: families.graphics,
                timestamp_bits: families.timestamp_bits[families.graphics as usize],
            },
            compute: VulkanQueue {
                handle: compute_queue,
                family: families.compute,
                timestamp_bits: families.timestamp_bits[families.compute as usize],
            },
            transfer: VulkanQueue {
                handle: transfer_queue,
                family: families.transfer,
                timestamp_bits: families.timestamp_bits[families.transfer as usize],
            },
        },
        device_name,
        extensions,
        // The staging allocator only needs the device, its entry points and the
        // granularity the driver reports, so it is built from those directly.
        staging_allocator: std::sync::Arc::new(std::sync::Mutex::new(
            super::memory::VulkanMemoryAllocator::from_parts(
                device,
                core,
                properties.limits.bufferImageGranularity.max(1),
                memory_properties,
                features.acceleration_structures,
            ),
        )),
        scratch: std::sync::Arc::new(std::sync::Mutex::new(
            super::submission::ScratchResources::new(),
        )),
    })
}

/// Prefers a discrete GPU, then an integrated one, otherwise the first device.
fn select_physical_device(
    commands: &sys::InstanceCommands,
    instance: sys::VkInstance,
) -> RhiResult<Option<sys::VkPhysicalDevice>> {
    let mut count: u32 = 0;
    // SAFETY: `count` is a live local used as the out-parameter.
    let status = unsafe {
        (commands.enumerate_physical_devices)(instance, &mut count, core::ptr::null_mut())
    };
    if status != sys::VK_SUCCESS || count == 0 {
        return Ok(None);
    }
    let mut devices = vec![core::ptr::null_mut(); count as usize];
    // SAFETY: `devices` was sized from the count the driver just reported.
    if unsafe { (commands.enumerate_physical_devices)(instance, &mut count, devices.as_mut_ptr()) }
        != sys::VK_SUCCESS
    {
        return Ok(None);
    }
    devices.truncate(count as usize);
    let mut chosen: Option<sys::VkPhysicalDevice> = None;
    for device in &devices {
        let mut properties = sys::VkPhysicalDeviceProperties::default();
        // SAFETY: the device came from the enumeration above.
        unsafe { (commands.get_physical_device_properties)(*device, &mut properties) };
        if properties.deviceType == sys::VK_PHYSICAL_DEVICE_TYPE_DISCRETE_GPU {
            chosen = Some(*device);
            break;
        }
        if properties.deviceType == sys::VK_PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU && chosen.is_none()
        {
            chosen = Some(*device);
        }
    }
    Ok(chosen.or_else(|| devices.first().copied()))
}

fn device_extensions(
    commands: &sys::InstanceCommands,
    physical_device: sys::VkPhysicalDevice,
) -> HashSet<String> {
    let mut count: u32 = 0;
    // SAFETY: a null layer name means "all layers".
    unsafe {
        (commands.enumerate_device_extension_properties)(
            physical_device,
            core::ptr::null(),
            &mut count,
            core::ptr::null_mut(),
        )
    };
    if count == 0 {
        return HashSet::new();
    }
    let mut properties = vec![sys::VkExtensionProperties::default(); count as usize];
    // SAFETY: `properties` was sized from the reported count.
    let status = unsafe {
        (commands.enumerate_device_extension_properties)(
            physical_device,
            core::ptr::null(),
            &mut count,
            properties.as_mut_ptr(),
        )
    };
    if status != sys::VK_SUCCESS {
        return HashSet::new();
    }
    properties
        .iter()
        .take(count as usize)
        .map(|value| value.name())
        .collect()
}

/// Finds one family per queue class. When no dedicated compute or transfer
/// family exists, both fall back to the graphics family.
fn find_queue_families(
    commands: &sys::InstanceCommands,
    physical_device: sys::VkPhysicalDevice,
) -> QueueFamilies {
    let mut count: u32 = 0;
    // SAFETY: `count` is a live local used as the out-parameter.
    unsafe {
        (commands.get_physical_device_queue_family_properties)(
            physical_device,
            &mut count,
            core::ptr::null_mut(),
        )
    };
    let mut families = QueueFamilies {
        graphics: u32::MAX,
        compute: u32::MAX,
        transfer: u32::MAX,
        timestamp_bits: Vec::new(),
    };
    if count == 0 {
        return families;
    }
    let mut properties = vec![sys::VkQueueFamilyProperties::default(); count as usize];
    // SAFETY: `properties` was sized from the reported count.
    unsafe {
        (commands.get_physical_device_queue_family_properties)(
            physical_device,
            &mut count,
            properties.as_mut_ptr(),
        )
    };
    for (index, family) in properties.iter().take(count as usize).enumerate() {
        families.timestamp_bits.push(family.timestampValidBits);
        let index = index as u32;
        let has_graphics = family.queueFlags & sys::VK_QUEUE_GRAPHICS_BIT != 0;
        if has_graphics && families.graphics == u32::MAX {
            families.graphics = index;
        }
        if family.queueFlags & sys::VK_QUEUE_COMPUTE_BIT != 0
            && !has_graphics
            && families.compute == u32::MAX
        {
            families.compute = index;
        }
        if family.queueFlags & sys::VK_QUEUE_TRANSFER_BIT != 0
            && !has_graphics
            && families.transfer == u32::MAX
        {
            families.transfer = index;
        }
    }
    if families.compute == u32::MAX {
        families.compute = families.graphics;
    }
    if families.transfer == u32::MAX {
        families.transfer = families.graphics;
    }
    families
}

fn unique_family_list(families: &QueueFamilies) -> Vec<u32> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for family in [families.graphics, families.compute, families.transfer] {
        if family == u32::MAX || !seen.insert(family) {
            continue;
        }
        result.push(family);
    }
    result
}

/// Creates the logical device and reports which optional features were enabled.
fn create_logical_device(
    commands: &sys::InstanceCommands,
    physical_device: sys::VkPhysicalDevice,
    families: &QueueFamilies,
    unique_families: &[u32],
    extensions: &HashSet<String>,
    api_version: u32,
) -> RhiResult<(sys::VkDevice, sys::FeatureSupport)> {
    let core_1_2 = api_version >= sys::api_version(1, 2);
    let core_1_3 = api_version >= sys::api_version(1, 3);
    let maintenance = core_1_3 || extensions.contains("VK_KHR_maintenance4");
    rhi_require(
        core_1_2 || extensions.contains("VK_KHR_timeline_semaphore"),
        "NativeRHI requires timeline semaphores (Vulkan 1.2 or VK_KHR_timeline_semaphore)",
    )?;

    // Query only the structures this device can consume: a 1.1 driver cannot read
    // the aggregate 1.2/1.3 feature structures.
    let mut mesh = sys::VkPhysicalDeviceMeshShaderFeaturesEXT::default();
    let mut ray_query = sys::VkPhysicalDeviceRayQueryFeaturesKHR::default();
    let mut structure = sys::VkPhysicalDeviceAccelerationStructureFeaturesKHR::default();
    let mut address = sys::VkPhysicalDeviceBufferDeviceAddressFeatures::default();
    let mut local_size = sys::VkPhysicalDeviceMaintenance4Features::default();
    let mut timeline = sys::VkPhysicalDeviceTimelineSemaphoreFeatures::default();
    let mut draw_parameters = sys::VkPhysicalDeviceShaderDrawParametersFeatures::default();

    // Build the pNext chain tail-first so each `pNext` points at the next node.
    let mut tail: *mut core::ffi::c_void = core::ptr::null_mut();
    if extensions.contains("VK_EXT_mesh_shader") {
        mesh.pNext = tail;
        tail = (&mut mesh as *mut sys::VkPhysicalDeviceMeshShaderFeaturesEXT).cast();
    }
    if core_1_2 && extensions.contains("VK_KHR_ray_query") {
        ray_query.pNext = tail;
        tail = (&mut ray_query as *mut sys::VkPhysicalDeviceRayQueryFeaturesKHR).cast();
    }
    if core_1_2 && extensions.contains("VK_KHR_acceleration_structure") {
        structure.pNext = tail;
        tail =
            (&mut structure as *mut sys::VkPhysicalDeviceAccelerationStructureFeaturesKHR).cast();
    }
    if core_1_2 {
        address.pNext = tail;
        tail = (&mut address as *mut sys::VkPhysicalDeviceBufferDeviceAddressFeatures).cast();
    }
    if maintenance {
        local_size.pNext = tail;
        tail = (&mut local_size as *mut sys::VkPhysicalDeviceMaintenance4Features).cast();
    }
    timeline.pNext = tail;
    draw_parameters.pNext =
        (&mut timeline as *mut sys::VkPhysicalDeviceTimelineSemaphoreFeatures).cast();

    let mut supported = sys::VkPhysicalDeviceFeatures2 {
        pNext: (&mut draw_parameters as *mut sys::VkPhysicalDeviceShaderDrawParametersFeatures)
            .cast(),
        ..Default::default()
    };
    // SAFETY: every node in the chain outlives the call, which only writes into
    // them.
    unsafe { (commands.get_physical_device_features2)(physical_device, &mut supported) };

    rhi_require(
        timeline.timelineSemaphore != 0,
        "Vulkan timeline semaphore feature is unavailable",
    )?;
    rhi_require(
        draw_parameters.shaderDrawParameters != 0
            && supported.features.drawIndirectFirstInstance != 0,
        "NativeRHI requires shader draw parameters and indirect firstInstance",
    )?;

    let mut features = sys::FeatureSupport {
        spirv_version: if core_1_3 {
            sys::SPIRV_1_6
        } else if core_1_2 {
            sys::SPIRV_1_5
        } else if extensions.contains("VK_KHR_spirv_1_4") {
            sys::SPIRV_1_4
        } else {
            sys::SPIRV_1_3
        },
        maintenance4: local_size.maintenance4 != 0,
        non_solid_fill: supported.features.fillModeNonSolid != 0,
        ..sys::FeatureSupport::default()
    };
    features.acceleration_structures = core_1_2
        && address.bufferDeviceAddress != 0
        && structure.accelerationStructure != 0
        && extensions.contains("VK_KHR_acceleration_structure")
        && extensions.contains("VK_KHR_deferred_host_operations");
    features.ray_query = features.acceleration_structures
        && ray_query.rayQuery != 0
        && extensions.contains("VK_KHR_ray_query");
    features.mesh = mesh.meshShader != 0 && extensions.contains("VK_EXT_mesh_shader");
    features.task = features.mesh && mesh.taskShader != 0;

    // Clear the queried bits, then enable only the implemented subset.
    let mut tail: *mut core::ffi::c_void = core::ptr::null_mut();
    mesh.meshShader = u32::from(features.mesh);
    mesh.taskShader = u32::from(features.task);
    mesh.pNext = tail;
    if features.mesh {
        tail = (&mut mesh as *mut sys::VkPhysicalDeviceMeshShaderFeaturesEXT).cast();
    }
    ray_query.rayQuery = u32::from(features.ray_query);
    ray_query.pNext = tail;
    if features.ray_query {
        tail = (&mut ray_query as *mut sys::VkPhysicalDeviceRayQueryFeaturesKHR).cast();
    }
    if features.acceleration_structures {
        structure = sys::VkPhysicalDeviceAccelerationStructureFeaturesKHR {
            pNext: tail,
            accelerationStructure: sys::VK_TRUE,
            ..Default::default()
        };
        tail =
            (&mut structure as *mut sys::VkPhysicalDeviceAccelerationStructureFeaturesKHR).cast();
        address = sys::VkPhysicalDeviceBufferDeviceAddressFeatures {
            pNext: tail,
            bufferDeviceAddress: sys::VK_TRUE,
            ..Default::default()
        };
        tail = (&mut address as *mut sys::VkPhysicalDeviceBufferDeviceAddressFeatures).cast();
    }
    local_size.maintenance4 = u32::from(features.maintenance4);
    local_size.pNext = tail;
    if features.maintenance4 {
        tail = (&mut local_size as *mut sys::VkPhysicalDeviceMaintenance4Features).cast();
    }
    timeline.timelineSemaphore = sys::VK_TRUE;
    timeline.pNext = tail;
    draw_parameters.shaderDrawParameters = sys::VK_TRUE;
    draw_parameters.pNext =
        (&mut timeline as *mut sys::VkPhysicalDeviceTimelineSemaphoreFeatures).cast();
    let enabled = sys::VkPhysicalDeviceFeatures {
        fillModeNonSolid: supported.features.fillModeNonSolid,
        drawIndirectFirstInstance: sys::VK_TRUE,
        ..Default::default()
    };

    let priority: f32 = 1.0;
    let queue_infos: Vec<sys::VkDeviceQueueCreateInfo> = unique_families
        .iter()
        .map(|family| sys::VkDeviceQueueCreateInfo {
            sType: sys::VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
            pNext: core::ptr::null(),
            flags: 0,
            queueFamilyIndex: *family,
            queueCount: 1,
            pQueuePriorities: &priority,
        })
        .collect();

    let mut names: Vec<&CStr> = Vec::new();
    if !core_1_2 {
        names.push(c"VK_KHR_timeline_semaphore");
        if extensions.contains("VK_KHR_spirv_1_4") {
            names.push(c"VK_KHR_spirv_1_4");
            names.push(c"VK_KHR_shader_float_controls");
        }
    }
    if features.maintenance4 && !core_1_3 {
        names.push(c"VK_KHR_maintenance4");
    }
    if extensions.contains("VK_KHR_swapchain") {
        names.push(c"VK_KHR_swapchain");
    }
    if extensions.contains("VK_KHR_portability_subset") {
        names.push(c"VK_KHR_portability_subset");
    }
    if features.acceleration_structures {
        names.push(c"VK_KHR_acceleration_structure");
        names.push(c"VK_KHR_deferred_host_operations");
    }
    if features.ray_query {
        names.push(c"VK_KHR_ray_query");
    }
    if features.mesh {
        names.push(c"VK_EXT_mesh_shader");
    }
    let name_pointers: Vec<*const core::ffi::c_char> =
        names.iter().map(|name| name.as_ptr()).collect();

    let create_info = sys::VkDeviceCreateInfo {
        sType: sys::VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
        pNext: (&mut draw_parameters as *mut sys::VkPhysicalDeviceShaderDrawParametersFeatures)
            .cast(),
        flags: 0,
        queueCreateInfoCount: queue_infos.len() as u32,
        pQueueCreateInfos: queue_infos.as_ptr(),
        enabledLayerCount: 0,
        ppEnabledLayerNames: core::ptr::null(),
        enabledExtensionCount: name_pointers.len() as u32,
        ppEnabledExtensionNames: name_pointers.as_ptr(),
        pEnabledFeatures: &enabled,
    };

    let mut device: sys::VkDevice = core::ptr::null_mut();
    // SAFETY: every pointer in `create_info` refers to a live local that outlives
    // the call, and the out-parameter is a live local.
    let status = unsafe {
        (commands.create_device)(
            physical_device,
            &create_info,
            core::ptr::null(),
            &mut device,
        )
    };
    rhi_require(
        status == sys::VK_SUCCESS && !device.is_null(),
        "vkCreateDevice failed",
    )?;
    let _ = families;
    Ok((device, features))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_packs_major_and_minor() {
        assert_eq!(sys::api_version(1, 0), 1 << 22);
        assert_eq!(sys::api_version(1, 1), (1 << 22) | (1 << 12));
        assert_eq!(sys::api_version(1, 3), (1 << 22) | (3 << 12));
        assert!(sys::api_version(1, 3) > sys::api_version(1, 2));
    }

    #[test]
    fn queue_family_list_deduplicates_and_skips_missing_families() {
        let families = QueueFamilies {
            graphics: 0,
            compute: 0,
            transfer: 0,
            timestamp_bits: Vec::new(),
        };
        assert_eq!(unique_family_list(&families), vec![0]);
        let families = QueueFamilies {
            graphics: 0,
            compute: 1,
            transfer: 2,
            timestamp_bits: Vec::new(),
        };
        assert_eq!(unique_family_list(&families), vec![0, 1, 2]);
        let families = QueueFamilies {
            graphics: u32::MAX,
            compute: u32::MAX,
            transfer: u32::MAX,
            timestamp_bits: Vec::new(),
        };
        assert!(unique_family_list(&families).is_empty());
    }

    #[test]
    fn feature_support_defaults_to_no_optional_graphics_features() {
        let features = sys::FeatureSupport::default();
        assert!(!features.acceleration_structures);
        assert!(!features.ray_query);
        assert!(!features.mesh);
        assert!(!features.task);
        assert!(!features.maintenance4);
        assert!(!features.non_solid_fill);
        assert_eq!(features.spirv_version, 0);
    }
}
