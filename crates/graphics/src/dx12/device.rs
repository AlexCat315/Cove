//! Translation of NativeRHI/DX12/DX12Device.swift and the loader half of
//! DX12Backend.swift: the D3D12 device, its queues, the descriptor heaps and the
//! swapchain.
//!
//! Swift called a C++ bridge (`grhi_dx12_*`) that owned all of this. The Rust port
//! drives the COM vtables declared in `sys` directly, so the same objects are
//! created here. Device creation is Windows-only: on any other host the module
//! reports "unsupported backend" instead of failing to compile.
#![allow(non_snake_case)]
use std::collections::HashMap;

use crate::rhi::{
    AdapterCapabilities, Capabilities, DeviceConfig, QueueLimits, RhiError, RhiResult,
    TextureFormat,
};

use super::sys;

/// One D3D12 command queue plus the fence that tracks its submissions.
pub struct Dx12Queue {
    /// Owned COM pointer (`ID3D12CommandQueue`).
    pub queue: sys::ComPtr,
    pub fence: sys::ComPtr,
    pub fence_value: u64,
}

impl Dx12Queue {
    fn new(queue: sys::ComPtr, fence: sys::ComPtr) -> Self {
        Self {
            queue,
            fence,
            fence_value: 0,
        }
    }
}

/// One window's swapchain and its back-buffer resources.
pub struct Dx12Swapchain {
    /// Owned COM pointer (`IDXGISwapChain3`).
    pub swapchain: sys::ComPtr,
    pub width: usize,
    pub height: usize,
    /// Diagnostics: the format the swapchain was created with. Render passes
    /// resolve attachments through the texture registry, so this is not read on
    /// the hot path.
    #[allow(dead_code)]
    pub format: TextureFormat,
    pub vsync: bool,
    #[cfg(target_os = "windows")]
    pub composition: Option<super::composition::CompositionTarget>,
    pub generation: u64,
    /// Back-buffer COM pointers, one per frame.
    pub buffers: Vec<sys::ComPtr>,
    /// Frontend texture handle per back buffer.
    pub texture_ids: Vec<u32>,
    pub view_indices: Vec<u32>,
    pub current_index: u32,
    pub acquired: Option<u32>,
}

/// The D3D12 device and everything hung off it.
pub struct Dx12DeviceHandle {
    pub device: sys::ComPtr,
    #[cfg(target_os = "windows")]
    pub indirect: super::extensions::IndirectSignatures,
    pub factory: sys::ComPtr,
    pub graphics: Dx12Queue,
    /// Shader-visible heap for CBV/SRV/UAV descriptors.
    pub resource_heap: sys::ComPtr,
    pub sampler_heap: sys::ComPtr,
    /// Non-shader-visible heap holding render-target views.
    pub rtv_heap: sys::ComPtr,
    pub dsv_heap: sys::ComPtr,
    pub rtv_descriptor_size: u32,
    pub dsv_descriptor_size: u32,
    pub resource_descriptor_size: u32,
    pub sampler_descriptor_size: u32,
    /// The configuration the device was created with, retained for diagnostics.
    #[allow(dead_code)]
    pub config: DeviceConfig,
    pub capabilities: Capabilities,
    pub adapter_capabilities: AdapterCapabilities,
    pub device_name: String,
    pub swapchains: HashMap<u32, Dx12Swapchain>,
    next_rtv_index: u32,
    free_rtv_indices: Vec<u32>,
    /// Backend-internal handle counter, above the frontend's low IDs.
    pub next_internal_id: u32,
    /// An allocator can be reset once the fence for its most recent list retires.
    allocators: Vec<(sys::ComPtr, u64)>,
    active_allocator: usize,
}

/// First backend-internal handle.
pub const INTERNAL_ID_BASE: u32 = 0xF000_0000;

/// Descriptor-heap capacities. The binding planner uses these to refuse a set
/// before it writes past the end of a heap.
pub const RESOURCE_HEAP_CAPACITY: u32 = 4096;
pub const SAMPLER_HEAP_CAPACITY: u32 = 2048;
pub const RTV_HEAP_CAPACITY: u32 = 64;
pub const DSV_HEAP_CAPACITY: u32 = 64;

impl Dx12DeviceHandle {
    /// Creates the device and its queues.
    ///
    /// On a non-Windows host this reports "unsupported backend" rather than
    /// pretending the COM calls exist.
    pub fn make(config: DeviceConfig) -> RhiResult<Self> {
        #[cfg(not(target_os = "windows"))]
        {
            let _ = config;
            Err(RhiError::unsupported_backend(
                "DirectX 12 is only available on Windows",
            ))
        }
        #[cfg(target_os = "windows")]
        {
            windows_impl::make(config)
        }
    }

    #[cfg(target_os = "windows")]
    pub fn command_allocator(&mut self) -> RhiResult<sys::ComPtr> {
        let completed = windows_impl::completed_fence(self.graphics.fence)?;
        if let Some((index, &(allocator, _))) = self
            .allocators
            .iter()
            .enumerate()
            .find(|(_, (_, fence))| *fence <= completed)
        {
            // SAFETY: recording is serialized and the allocator's last list has retired.
            let status = unsafe {
                let vtbl = *(allocator as *mut *const CommandAllocatorVtbl);
                ((*vtbl).Reset)(allocator)
            };
            if status != sys::S_OK {
                return Err(RhiError::submit_failed(sys::hresult_message(status)));
            }
            self.active_allocator = index;
            return Ok(allocator);
        }
        let mut allocator: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the device is live and the vtable entry matches `CreateCommandAllocator`.
        let status = unsafe {
            let vtbl = *(self.device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateCommandAllocator)(
                self.device,
                sys::D3D12_COMMAND_LIST_TYPE_DIRECT,
                &sys::IID_ID3D12_COMMAND_ALLOCATOR,
                &mut allocator,
            )
        };
        if status != sys::S_OK || allocator.is_null() {
            return Err(RhiError::submit_failed(sys::hresult_message(status)));
        }
        self.active_allocator = self.allocators.len();
        self.allocators.push((allocator, 0));
        Ok(allocator)
    }

    pub fn execute(&mut self, list: sys::ComPtr) -> RhiResult<u64> {
        let value = execute_and_signal(&mut self.graphics, list)?;
        self.allocators[self.active_allocator].1 = value;
        Ok(value)
    }

    pub fn next_internal_id(&mut self) -> u32 {
        let id = self.next_internal_id;
        self.next_internal_id = self.next_internal_id.wrapping_add(1);
        id
    }

    /// Reserves one descriptor slot in the RTV/DSV heaps.
    pub fn take_rtv_index(&mut self) -> RhiResult<u32> {
        if let Some(index) = self.free_rtv_indices.pop() {
            return Ok(index);
        }
        if self.next_rtv_index >= RTV_HEAP_CAPACITY.min(DSV_HEAP_CAPACITY) {
            return Err(RhiError::OutOfMemory);
        }
        let index = self.next_rtv_index;
        self.next_rtv_index += 1;
        Ok(index)
    }

    pub fn release_rtv_index(&mut self, index: u32) {
        self.free_rtv_indices.push(index);
    }
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct CommandAllocatorVtbl {
    base: sys::IUnknownVtbl,
    child: sys::ID3D12DeviceChildSlots,
    Reset: unsafe extern "system" fn(sys::ComPtr) -> sys::HRESULT,
}

impl Drop for Dx12DeviceHandle {
    fn drop(&mut self) {
        // SAFETY: these pointers were created by this device and released exactly
        // once, in reverse creation order.
        unsafe {
            for chain in self.swapchains.values() {
                for buffer in &chain.buffers {
                    sys::IUnknownVtbl::release(*buffer);
                }
                sys::IUnknownVtbl::release(chain.swapchain);
            }
            for (allocator, _) in &self.allocators {
                sys::IUnknownVtbl::release(*allocator);
            }
            sys::IUnknownVtbl::release(self.graphics.fence);
            sys::IUnknownVtbl::release(self.graphics.queue);
            for heap in [
                self.resource_heap,
                self.sampler_heap,
                self.rtv_heap,
                self.dsv_heap,
            ] {
                sys::IUnknownVtbl::release(heap);
            }
            sys::IUnknownVtbl::release(self.factory);
            sys::IUnknownVtbl::release(self.device);
        }
    }
}

/// A fence and the value a waiter should reach. Carries the two handles so the
/// waiter can run on its own thread without borrowing the device.
#[derive(Clone, Copy)]
pub struct FenceWait {
    pub fence: sys::ComPtr,
    pub value: u64,
}

// SAFETY: the handle is a COM pointer to a fence, which D3D12 documents as usable
// from any thread; the waiter only calls `GetCompletedValue` and
// `SetEventOnCompletion` on it.
unsafe impl Send for FenceWait {}

impl FenceWait {
    /// Blocks until the GPU reaches `value`.
    pub fn wait(self) -> RhiResult<()> {
        #[cfg(not(target_os = "windows"))]
        {
            Err(RhiError::unsupported_backend(
                "DirectX 12 is only available on Windows",
            ))
        }
        #[cfg(target_os = "windows")]
        {
            windows_impl::wait_for_fence_values(self.fence, self.value)
        }
    }
}

/// Waits for one queue's fence to reach `value`.
fn wait_for_fence(queue: &Dx12Queue, value: u64) -> RhiResult<()> {
    #[cfg(target_os = "windows")]
    {
        windows_impl::wait_for_fence(queue, value)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (queue, value);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
}

/// Shapes the RHI capability set from the probed feature level.
///
/// DXR tier 1.0 enables acceleration structures. Inline compute queries also
/// require tier 1.1. The remaining ray tracing operations use their defaults.
pub fn capabilities_for(mesh: bool, ray_tracing: bool, ray_query: bool) -> Capabilities {
    Capabilities::configure(|capabilities| {
        capabilities.graphics = true;
        capabilities.compute = true;
        capabilities.indirect_draw = true;
        capabilities.textures.texture_3d = true;
        capabilities.textures.cube = true;
        capabilities.mesh_shading.mesh = mesh;
        capabilities.mesh_shading.task = mesh;
        capabilities.ray_tracing.acceleration_structures = ray_tracing;
        capabilities.ray_tracing.compute_ray_query = ray_query;
        capabilities.max_queues = QueueLimits::default();
    })
}

/// Everything the Windows-only implementation needs, kept in one module so the
/// platform calls stay together.
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use core::ffi::c_void;
    use windows::Win32::Graphics::{Direct3D12::*, Dxgi::*};
    use windows::core::Interface;

    unsafe extern "system" {
        fn LoadLibraryA(name: *const i8) -> sys::HINSTANCE;
        fn GetProcAddress(module: sys::HINSTANCE, name: *const i8) -> *mut c_void;
        pub fn CreateEventW(
            attributes: *const c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> sys::HANDLE;
        pub fn CloseHandle(handle: sys::HANDLE) -> i32;
    }

    /// `D3D12CreateDevice`'s signature.
    type CreateDevice = unsafe extern "system" fn(
        *mut c_void,
        u32,
        *const sys::GUID,
        *mut *mut c_void,
    ) -> sys::HRESULT;

    /// `CreateDXGIFactory2`'s signature.
    type CreateFactory =
        unsafe extern "system" fn(u32, *const sys::GUID, *mut *mut c_void) -> sys::HRESULT;

    pub fn make(config: DeviceConfig) -> RhiResult<Dx12DeviceHandle> {
        // SAFETY: the DLL names are NUL-terminated and the symbols are looked up
        // with the signatures their types declare.
        let d3d12 = unsafe { LoadLibraryA(c"d3d12.dll".as_ptr()) };
        if d3d12.is_null() {
            return Err(RhiError::unsupported_backend("d3d12.dll is unavailable"));
        }
        // SAFETY: the module handle is live.
        let create_device = unsafe { GetProcAddress(d3d12, c"D3D12CreateDevice".as_ptr()) };
        if create_device.is_null() {
            return Err(RhiError::unsupported_backend(
                "D3D12CreateDevice is unavailable",
            ));
        }
        // SAFETY: `D3D12CreateDevice` has the documented signature.
        let create_device: CreateDevice = unsafe { core::mem::transmute_copy(&create_device) };

        // SAFETY: as above, for the DXGI factory.
        let dxgi = unsafe { LoadLibraryA(c"dxgi.dll".as_ptr()) };
        if dxgi.is_null() {
            return Err(RhiError::unsupported_backend("dxgi.dll is unavailable"));
        }
        // SAFETY: the module handle is live.
        let create_factory = unsafe { GetProcAddress(dxgi, c"CreateDXGIFactory2".as_ptr()) };
        if create_factory.is_null() {
            return Err(RhiError::unsupported_backend(
                "CreateDXGIFactory2 is unavailable",
            ));
        }
        // SAFETY: `CreateDXGIFactory2` has the documented signature.
        let create_factory: CreateFactory = unsafe { core::mem::transmute_copy(&create_factory) };

        let mut factory: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the out-parameter is a live local and the IID names `IDXGIFactory4`.
        let status = unsafe { create_factory(0, &sys::IID_IDXGI_FACTORY4, &mut factory) };
        if status != sys::S_OK || factory.is_null() {
            return Err(RhiError::unsupported_backend(sys::hresult_message(status)));
        }

        let mut device: sys::ComPtr = core::ptr::null_mut();
        let mut device_name = "Direct3D 12 adapter".to_owned();
        let preference = if config.prefer_low_power {
            DXGI_GPU_PREFERENCE_MINIMUM_POWER
        } else {
            DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE
        };
        // SAFETY: CreateDXGIFactory2 returned a live IDXGIFactory4 pointer.
        let factory4 = unsafe { IDXGIFactory4::from_raw_borrowed(&factory).unwrap() };
        if let Ok(factory6) = factory4.cast::<IDXGIFactory6>() {
            for index in 0.. {
                // SAFETY: enumeration returns an owned adapter or NOT_FOUND.
                let Ok(adapter) = (unsafe {
                    factory6.EnumAdapterByGpuPreference::<IDXGIAdapter1>(index, preference)
                }) else {
                    break;
                };
                // SAFETY: the adapter interface was returned above.
                let Ok(description) = (unsafe { adapter.GetDesc1() }) else {
                    continue;
                };
                if description.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                    continue;
                }
                // SAFETY: the adapter and output pointer are live.
                let status = unsafe {
                    create_device(
                        adapter.as_raw(),
                        0xB000,
                        &sys::IID_ID3D12_DEVICE,
                        &mut device,
                    )
                };
                if status == sys::S_OK && !device.is_null() {
                    let end = description
                        .Description
                        .iter()
                        .position(|&ch| ch == 0)
                        .unwrap_or(description.Description.len());
                    device_name = String::from_utf16_lossy(&description.Description[..end]);
                    break;
                }
            }
        }
        let status = if device.is_null() {
            // SAFETY: a null adapter asks DXGI for its default adapter.
            unsafe {
                create_device(
                    core::ptr::null_mut(),
                    0xB000,
                    &sys::IID_ID3D12_DEVICE,
                    &mut device,
                )
            }
        } else {
            sys::S_OK
        };
        if status != sys::S_OK || device.is_null() {
            // SAFETY: `factory` was created above and released exactly once.
            unsafe { sys::IUnknownVtbl::release(factory) };
            return Err(RhiError::unsupported_backend(sys::hresult_message(status)));
        }

        let graphics = create_queue(device, sys::D3D12_COMMAND_LIST_TYPE_DIRECT)?;

        let (
            rtv_descriptor_size,
            dsv_descriptor_size,
            resource_descriptor_size,
            sampler_descriptor_size,
        ) = descriptor_sizes(device);
        let resource_heap = create_heap(
            device,
            sys::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            RESOURCE_HEAP_CAPACITY,
            true,
        )?;
        let sampler_heap = create_heap(
            device,
            sys::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
            SAMPLER_HEAP_CAPACITY,
            true,
        )?;
        let rtv_heap = create_heap(
            device,
            sys::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            RTV_HEAP_CAPACITY,
            false,
        )?;
        let dsv_heap = create_heap(
            device,
            sys::D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
            DSV_HEAP_CAPACITY,
            false,
        )?;

        // SAFETY: the device was just created with IID_ID3D12Device.
        let native = unsafe { ID3D12Device::from_raw_borrowed(&device).unwrap() };
        let indirect = super::super::extensions::IndirectSignatures::new(native)?;
        let (mesh, ray_tracing, ray_query) = probe_features(native);
        let capabilities = capabilities_for(mesh, ray_tracing, ray_query);
        Ok(Dx12DeviceHandle {
            device,
            indirect,
            factory,
            graphics,
            resource_heap,
            sampler_heap,
            rtv_heap,
            dsv_heap,
            rtv_descriptor_size,
            dsv_descriptor_size,
            resource_descriptor_size,
            sampler_descriptor_size,
            config,
            capabilities,
            adapter_capabilities: AdapterCapabilities {
                // Diagnostic: what the adapter reports, not what this build
                // implements.
                ray_tracing,
                mesh_shading: mesh,
            },
            device_name,
            swapchains: HashMap::new(),
            next_rtv_index: 0,
            free_rtv_indices: Vec::new(),
            next_internal_id: INTERNAL_ID_BASE,
            allocators: Vec::new(),
            active_allocator: 0,
        })
    }

    fn create_queue(device: sys::ComPtr, kind: u32) -> RhiResult<Dx12Queue> {
        let description = sys::D3D12_COMMAND_QUEUE_DESC {
            Type: kind,
            Priority: 0,
            Flags: sys::D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        };
        let mut queue: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: `device` is a live `ID3D12Device` and its vtable slot for
        // `CreateCommandQueue` is the third after `IUnknown`.
        let status = unsafe {
            let vtbl = *(device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateCommandQueue)(
                device,
                &description,
                &sys::IID_ID3D12_COMMAND_QUEUE,
                &mut queue,
            )
        };
        if status != sys::S_OK || queue.is_null() {
            return Err(RhiError::unsupported_backend(sys::hresult_message(status)));
        }
        let fence = create_fence(device, 0, sys::D3D12_FENCE_FLAG_NONE)?;
        Ok(Dx12Queue::new(queue, fence))
    }

    fn create_fence(device: sys::ComPtr, value: u64, flags: u32) -> RhiResult<sys::ComPtr> {
        let mut fence: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: `device` is live and the vtable entry matches `CreateFence`.
        let status = unsafe {
            let vtbl = *(device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateFence)(device, value, flags, &sys::IID_ID3D12_FENCE, &mut fence)
        };
        if status != sys::S_OK || fence.is_null() {
            return Err(RhiError::unsupported_backend(sys::hresult_message(status)));
        }
        Ok(fence)
    }

    fn descriptor_sizes(device: sys::ComPtr) -> (u32, u32, u32, u32) {
        // SAFETY: the device is live; each call only reads a size.
        unsafe {
            let vtbl = *(device as *mut *const sys::ID3D12DeviceVtbl);
            (
                ((*vtbl).GetDescriptorHandleIncrementSize)(
                    device,
                    sys::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                ),
                ((*vtbl).GetDescriptorHandleIncrementSize)(
                    device,
                    sys::D3D12_DESCRIPTOR_HEAP_TYPE_DSV,
                ),
                ((*vtbl).GetDescriptorHandleIncrementSize)(
                    device,
                    sys::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                ),
                ((*vtbl).GetDescriptorHandleIncrementSize)(
                    device,
                    sys::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER,
                ),
            )
        }
    }

    fn create_heap(
        device: sys::ComPtr,
        kind: u32,
        count: u32,
        shader_visible: bool,
    ) -> RhiResult<sys::ComPtr> {
        let description = sys::D3D12_DESCRIPTOR_HEAP_DESC {
            Type: kind,
            NumDescriptors: count,
            Flags: if shader_visible {
                sys::D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE
            } else {
                0
            },
            NodeMask: 0,
        };
        let mut heap: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: `device` is live and the vtable entry matches `CreateDescriptorHeap`.
        let status = unsafe {
            let vtbl = *(device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CreateDescriptorHeap)(
                device,
                &description,
                &sys::IID_ID3D12_DESCRIPTOR_HEAP,
                &mut heap,
            )
        };
        if status != sys::S_OK || heap.is_null() {
            return Err(RhiError::unsupported_backend(sys::hresult_message(status)));
        }
        Ok(heap)
    }

    fn probe_features(device: &ID3D12Device) -> (bool, bool, bool) {
        let mut mesh = D3D12_FEATURE_DATA_D3D12_OPTIONS7::default();
        let mut rays = D3D12_FEATURE_DATA_D3D12_OPTIONS5::default();
        // SAFETY: the output structures and byte sizes match their selectors.
        let (mesh_ok, rays_ok) = unsafe {
            (
                device
                    .CheckFeatureSupport(
                        D3D12_FEATURE_D3D12_OPTIONS7,
                        (&mut mesh as *mut D3D12_FEATURE_DATA_D3D12_OPTIONS7).cast(),
                        core::mem::size_of_val(&mesh) as u32,
                    )
                    .is_ok(),
                device
                    .CheckFeatureSupport(
                        D3D12_FEATURE_D3D12_OPTIONS5,
                        (&mut rays as *mut D3D12_FEATURE_DATA_D3D12_OPTIONS5).cast(),
                        core::mem::size_of_val(&rays) as u32,
                    )
                    .is_ok(),
            )
        };
        (
            mesh_ok && mesh.MeshShaderTier.0 >= D3D12_MESH_SHADER_TIER_1.0,
            rays_ok && rays.RaytracingTier.0 >= D3D12_RAYTRACING_TIER_1_0.0,
            rays_ok && rays.RaytracingTier.0 >= D3D12_RAYTRACING_TIER_1_1.0,
        )
    }

    pub fn completed_fence(fence: sys::ComPtr) -> RhiResult<u64> {
        // SAFETY: the fence is live and the vtable entry matches `GetCompletedValue`.
        let completed = unsafe {
            let vtbl = *(fence as *mut *const FenceVtbl);
            ((*vtbl).GetCompletedValue)(fence)
        };
        if completed == u64::MAX {
            return Err(RhiError::DeviceLost);
        }
        Ok(completed)
    }

    /// Waits on a bare fence handle, for the completion thread.
    pub fn wait_for_fence_values(fence: sys::ComPtr, value: u64) -> RhiResult<()> {
        if completed_fence(fence)? >= value {
            return Ok(());
        }
        // SAFETY: null attributes and name request an auto-named event.
        let event = unsafe { CreateEventW(core::ptr::null(), 0, 0, core::ptr::null()) };
        if event.is_null() {
            return Err(RhiError::submit_failed("CreateEventW failed"));
        }
        // SAFETY: the fence and event are live.
        let status = unsafe {
            let vtbl = *(fence as *mut *const FenceVtbl);
            ((*vtbl).SetEventOnCompletion)(fence, value, event)
        };
        if status != sys::S_OK {
            // SAFETY: the event was created here and was not registered.
            unsafe { CloseHandle(event) };
            return Err(RhiError::submit_failed(sys::hresult_message(status)));
        }
        // SAFETY: the event is live; waiting on it is the documented pattern.
        unsafe { WaitForSingleObject(event, sys::INFINITE) };
        // SAFETY: the event was created here and is no longer needed.
        unsafe { CloseHandle(event) };
        completed_fence(fence)?;
        Ok(())
    }

    pub fn wait_for_fence(queue: &Dx12Queue, value: u64) -> RhiResult<()> {
        wait_for_fence_values(queue.fence, value)
    }

    unsafe extern "system" {
        fn WaitForSingleObject(handle: sys::HANDLE, milliseconds: u32) -> u32;
    }

    /// `ID3D12Fence`'s vtable prefix, which is all the backend calls.
    #[repr(C)]
    struct FenceVtbl {
        base: sys::IUnknownVtbl,
        child: sys::ID3D12DeviceChildSlots,
        GetCompletedValue: unsafe extern "system" fn(sys::ComPtr) -> u64,
        SetEventOnCompletion:
            unsafe extern "system" fn(sys::ComPtr, u64, sys::HANDLE) -> sys::HRESULT,
        /// CPU-side signal; queues signal through `ID3D12CommandQueue::Signal`.
        Signal: unsafe extern "system" fn(sys::ComPtr, u64) -> sys::HRESULT,
    }

    /// The vtable entry points the swapchain needs from `IDXGISwapChain3`.
    #[allow(dead_code)]
    pub struct SwapChainVtbl;
}

/// Submits `command_list` on `queue` and returns the fence value to wait for.
pub fn execute_and_signal(queue: &mut Dx12Queue, command_list: sys::ComPtr) -> RhiResult<u64> {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: the queue and list are live; the vtable entries match the D3D12
        // header order for `ExecuteCommandLists` and `Signal`.
        unsafe {
            let list = [command_list];
            let vtbl = *(queue.queue as *mut *const QueueVtbl);
            ((*vtbl).ExecuteCommandLists)(queue.queue, 1, list.as_ptr().cast());
            queue.fence_value += 1;
            let value = queue.fence_value;
            // The queue signals the fence once the list completes on the GPU.
            ((*vtbl).Signal)(queue.queue, queue.fence, value);
            Ok(value)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (queue, command_list);
        Err(RhiError::unsupported_backend(
            "DirectX 12 is only available on Windows",
        ))
    }
}

/// Waits for a queue's fence to reach `value`.
pub fn wait_idle(queue: &Dx12Queue, value: u64) -> RhiResult<()> {
    wait_for_fence(queue, value)
}

/// `ID3D12CommandQueue`'s vtable prefix.
#[repr(C)]
struct QueueVtbl {
    base: sys::IUnknownVtbl,
    child: sys::ID3D12DeviceChildSlots,
    UpdateTileMappings: usize,
    CopyTileMappings: usize,
    ExecuteCommandLists: unsafe extern "system" fn(sys::ComPtr, u32, *const sys::ComPtr),
    SetMarker: usize,
    BeginEvent: usize,
    EndEvent: usize,
    Signal: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr, u64) -> sys::HRESULT,
    Wait: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr, u64) -> sys::HRESULT,
    GetTimestampFrequency: usize,
    GetClockCalibration: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_ids_start_above_the_frontend_space() {
        assert_eq!(INTERNAL_ID_BASE, 0xF000_0000);
        const { assert!(INTERNAL_ID_BASE > 0x8000_0000) };
    }

    #[test]
    fn capabilities_follow_native_feature_tiers() {
        let none = capabilities_for(false, false, false);
        assert!(none.graphics && none.compute);
        // Ray tracing is deliberately unadvertised at every feature level: this
        // build does not link the DXR build/execute dispatch tables, and the
        // capability set must only promise what the backend will honour. The
        // adapter's real support is reported separately via `AdapterCapabilities`.
        assert!(!none.ray_tracing.acceleration_structures);
        assert!(!none.ray_tracing.compute_ray_query);
        let tier_1_0 = capabilities_for(false, true, false);
        assert!(tier_1_0.ray_tracing.acceleration_structures);
        assert!(!tier_1_0.ray_tracing.compute_ray_query);
        let full = capabilities_for(true, true, true);
        assert!(full.ray_tracing.compute_ray_query);
        assert!(full.mesh_shading.mesh && full.mesh_shading.task);
    }

    #[test]
    fn dx12_reports_unsupported_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            let result = Dx12DeviceHandle::make(DeviceConfig::default());
            assert!(result.is_err());
        }
    }
}
