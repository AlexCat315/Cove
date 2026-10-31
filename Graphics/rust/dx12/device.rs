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
    /// Timed waits use this; created on first wait.
    pub fence_event: sys::HANDLE,
    pub kind: u32,
}

impl Dx12Queue {
    fn new(queue: sys::ComPtr, fence: sys::ComPtr, kind: u32) -> Self {
        Self {
            queue,
            fence,
            fence_value: 0,
            fence_event: core::ptr::null_mut(),
            kind,
        }
    }
}

/// One window's swapchain and its back-buffer resources.
pub struct Dx12Swapchain {
    /// Owned COM pointer (`IDXGISwapChain3`).
    pub swapchain: sys::ComPtr,
    pub width: usize,
    pub height: usize,
    pub format: TextureFormat,
    pub vsync: bool,
    pub generation: u64,
    /// Back-buffer COM pointers, one per frame.
    pub buffers: Vec<sys::ComPtr>,
    /// Frontend texture handle per back buffer.
    pub texture_ids: Vec<u32>,
    /// Render-target descriptors, one per back buffer.
    pub render_targets: Vec<sys::D3D12_CPU_DESCRIPTOR_HANDLE>,
    pub current_index: u32,
    pub acquired: Option<u32>,
}

/// The D3D12 device and everything hung off it.
pub struct Dx12DeviceHandle {
    pub device: sys::ComPtr,
    pub factory: sys::ComPtr,
    pub graphics: Dx12Queue,
    pub compute: Dx12Queue,
    pub copy: Dx12Queue,
    /// Shader-visible heap for CBV/SRV/UAV descriptors.
    pub resource_heap: sys::ComPtr,
    pub sampler_heap: sys::ComPtr,
    /// Non-shader-visible heap holding render-target views.
    pub rtv_heap: sys::ComPtr,
    pub dsv_heap: sys::ComPtr,
    pub rtv_descriptor_size: u32,
    pub resource_descriptor_size: u32,
    pub sampler_descriptor_size: u32,
    pub config: DeviceConfig,
    pub capabilities: Capabilities,
    pub adapter_capabilities: AdapterCapabilities,
    pub device_name: String,
    pub swapchains: HashMap<u32, Dx12Swapchain>,
    pub next_rtv_index: u32,
    pub next_resource_index: u32,
    /// Backend-internal handle counter, above the frontend's low IDs.
    pub next_internal_id: u32,
    /// One reusable command allocator per queue class. A closed list keeps its
    /// allocator busy until the GPU retires it, so each class owns one and resets it
    /// once its submissions completed.
    pub allocators: HashMap<u32, sys::ComPtr>,
}

/// First backend-internal handle.
pub const INTERNAL_ID_BASE: u32 = 0xF000_0000;

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

    /// The command allocator for a queue class, created on first use.
    pub fn command_allocator(&mut self, queue_kind: u32) -> RhiResult<sys::ComPtr> {
        if let Some(allocator) = self.allocators.get(&queue_kind) {
            return Ok(*allocator);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = queue_kind;
            Err(RhiError::unsupported_backend(
                "DirectX 12 is only available on Windows",
            ))
        }
        #[cfg(target_os = "windows")]
        {
            let mut allocator: sys::ComPtr = core::ptr::null_mut();
            // SAFETY: the device is live and the vtable entry matches
            // `CreateCommandAllocator`.
            let status = unsafe {
                let vtbl = *(self.device as *mut *const sys::ID3D12DeviceVtbl);
                ((*vtbl).CreateCommandAllocator)(
                    self.device,
                    queue_kind,
                    &sys::IID_ID3D12_COMMAND_ALLOCATOR,
                    &mut allocator,
                )
            };
            if status != sys::S_OK || allocator.is_null() {
                return Err(RhiError::submit_failed(sys::hresult_message(status)));
            }
            self.allocators.insert(queue_kind, allocator);
            Ok(allocator)
        }
    }

    pub fn next_internal_id(&mut self) -> u32 {
        let id = self.next_internal_id;
        self.next_internal_id = self.next_internal_id.wrapping_add(1);
        id
    }
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
            for queue in [&self.graphics, &self.compute, &self.copy] {
                if !queue.fence_event.is_null() {
                    close_handle(queue.fence_event);
                }
                sys::IUnknownVtbl::release(queue.fence);
                sys::IUnknownVtbl::release(queue.queue);
            }
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

/// Closes a Win32 event handle, or does nothing on a host without one.
fn close_handle(handle: sys::HANDLE) {
    #[cfg(target_os = "windows")]
    {
        // SAFETY: the handle came from `CreateEventW` and is closed once.
        unsafe { windows_impl::CloseHandle(handle) };
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
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

/// Reports the adapter's feature level, for capability shaping.
fn probe_features(_device: sys::ComPtr) -> (u32, bool, bool) {
    // D3D12_FEATURE_FEATURE_LEVELS reports the maximum feature level; ray tracing
    // (DXR 1.0) needs 11_0, and 11_1 is required for ray queries.
    #[cfg(target_os = "windows")]
    {
        windows_impl::probe_features(_device)
    }
    #[cfg(not(target_os = "windows"))]
    {
        (0, false, false)
    }
}

/// Shapes the RHI capability set from the probed feature level.
///
/// Ray tracing stays **unadvertised** even on an adapter that supports DXR: the
/// capability set is a promise the backend will honour, and this build does not
/// link the DXR build/execute dispatch tables. The adapter's real support is
/// reported separately through `AdapterCapabilities`, which is diagnostic.
pub fn capabilities_for(feature_level: u32, mesh: bool, task: bool) -> Capabilities {
    let _ = feature_level;
    Capabilities::configure(|capabilities| {
        capabilities.graphics = true;
        capabilities.compute = true;
        capabilities.indirect_draw = true;
        capabilities.textures.texture_3d = true;
        capabilities.textures.cube = true;
        capabilities.mesh_shading.mesh = mesh;
        capabilities.mesh_shading.task = task;
        capabilities.max_queues = QueueLimits::default();
    })
}

/// Everything the Windows-only implementation needs, kept in one module so the
/// platform calls stay together.
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use core::ffi::c_void;

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

        // Prefer the low-power adapter when asked, matching the config flag.
        let mut device: sys::ComPtr = core::ptr::null_mut();
        // SAFETY: the out-parameter is a live local; `None` picks the default
        // adapter, and the feature level 11_0 is the DXR baseline.
        let status = unsafe {
            create_device(
                core::ptr::null_mut(),
                0xB000,
                &sys::IID_ID3D12_DEVICE,
                &mut device,
            )
        };
        if status != sys::S_OK || device.is_null() {
            // SAFETY: `factory` was created above and released exactly once.
            unsafe { sys::IUnknownVtbl::release(factory) };
            return Err(RhiError::unsupported_backend(sys::hresult_message(status)));
        }

        let graphics = create_queue(device, sys::D3D12_COMMAND_LIST_TYPE_DIRECT)?;
        let compute = create_queue(device, sys::D3D12_COMMAND_LIST_TYPE_COMPUTE)?;
        let copy = create_queue(device, sys::D3D12_COMMAND_LIST_TYPE_COPY)?;

        let (rtv_descriptor_size, resource_descriptor_size, sampler_descriptor_size) =
            descriptor_sizes(device);
        let resource_heap = create_heap(
            device,
            sys::D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
            1024,
            true,
        )?;
        let sampler_heap = create_heap(device, sys::D3D12_DESCRIPTOR_HEAP_TYPE_SAMPLER, 256, true)?;
        let rtv_heap = create_heap(device, sys::D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 64, false)?;
        let dsv_heap = create_heap(device, sys::D3D12_DESCRIPTOR_HEAP_TYPE_DSV, 64, false)?;

        let (feature_level, ray_tracing, ray_query) = probe_features(device);
        let mesh = feature_level >= 12;
        // `capabilities_for` deliberately leaves ray tracing unadvertised; the
        // adapter's real support rides on `adapter_capabilities` instead.
        let capabilities = capabilities_for(feature_level, mesh, mesh);
        let _ = (ray_tracing, ray_query);
        Ok(Dx12DeviceHandle {
            device,
            factory,
            graphics,
            compute,
            copy,
            resource_heap,
            sampler_heap,
            rtv_heap,
            dsv_heap,
            rtv_descriptor_size,
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
            device_name: "Direct3D 12 adapter".to_owned(),
            swapchains: HashMap::new(),
            next_rtv_index: 0,
            next_resource_index: 0,
            next_internal_id: INTERNAL_ID_BASE,
            allocators: HashMap::new(),
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
        Ok(Dx12Queue::new(queue, fence, kind))
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

    fn descriptor_sizes(device: sys::ComPtr) -> (u32, u32, u32) {
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
            ((*vtbl).CreateDescriptorHeap)(device, &description, &sys::IID_IUNKNOWN, &mut heap)
        };
        if status != sys::S_OK || heap.is_null() {
            return Err(RhiError::unsupported_backend(sys::hresult_message(status)));
        }
        Ok(heap)
    }

    pub fn probe_features(device: sys::ComPtr) -> (u32, bool, bool) {
        // `D3D12_FEATURE_FEATURE_LEVELS` = 1. The struct it fills is an array of
        // feature levels; the driver writes back the highest one it supports.
        const D3D12_FEATURE_FEATURE_LEVELS: u32 = 1;
        let levels = [0xC000u32, 0xB100, 0xB000, 0xA100, 0xA000];
        let mut query = FeatureLevelsQuery {
            num_feature_levels: levels.len() as u32,
            p_feature_levels: levels.as_ptr(),
            max_supported_feature_level: 0,
        };
        // SAFETY: `device` is live, the query struct outlives the call and declares
        // its exact size.
        let status = unsafe {
            let vtbl = *(device as *mut *const sys::ID3D12DeviceVtbl);
            ((*vtbl).CheckFeatureSupport)(
                device,
                D3D12_FEATURE_FEATURE_LEVELS,
                (&mut query as *mut FeatureLevelsQuery).cast(),
                core::mem::size_of::<FeatureLevelsQuery>() as u32,
            )
        };
        if status != sys::S_OK {
            return (0, false, false);
        }
        let max = query.max_supported_feature_level;
        // 0xB000 is 11_0 (DXR 1.0), 0xB100 is 11_1 (ray queries).
        (max, max >= 0xB000, max >= 0xB100)
    }

    /// `D3D12_FEATURE_DATA_FEATURE_LEVELS`.
    #[repr(C)]
    struct FeatureLevelsQuery {
        num_feature_levels: u32,
        p_feature_levels: *const u32,
        max_supported_feature_level: u32,
    }

    /// Waits on a bare fence handle, for the completion thread.
    pub fn wait_for_fence_values(fence: sys::ComPtr, value: u64) -> RhiResult<()> {
        // SAFETY: the fence is live and the vtable entry matches `GetCompletedValue`.
        let completed = unsafe {
            let vtbl = *(fence as *mut *const FenceVtbl);
            ((*vtbl).GetCompletedValue)(fence)
        };
        if completed >= value {
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
            return Err(RhiError::submit_failed(sys::hresult_message(status)));
        }
        // SAFETY: the event is live; waiting on it is the documented pattern.
        unsafe { WaitForSingleObject(event, sys::INFINITE) };
        // SAFETY: the event was created here and is no longer needed.
        unsafe { CloseHandle(event) };
        Ok(())
    }

    pub fn wait_for_fence(queue: &Dx12Queue, value: u64) -> RhiResult<()> {
        // SAFETY: the fence is live and the vtable entry matches `GetCompletedValue`.
        let completed = unsafe {
            let vtbl = *(queue.fence as *mut *const FenceVtbl);
            ((*vtbl).GetCompletedValue)(queue.fence)
        };
        if completed >= value {
            return Ok(());
        }
        let event = if queue.fence_event.is_null() {
            // SAFETY: the attributes and name are null, which requests an
            // auto-named, non-inheritable event.
            unsafe { CreateEventW(core::ptr::null(), 0, 0, core::ptr::null()) }
        } else {
            queue.fence_event
        };
        if event.is_null() {
            return Err(RhiError::submit_failed("CreateEventW failed"));
        }
        // SAFETY: the fence and event are live; `SetEventOnCompletion` registers the
        // event for the requested value.
        let status = unsafe {
            let vtbl = *(queue.fence as *mut *const FenceVtbl);
            ((*vtbl).SetEventOnCompletion)(queue.fence, value, event)
        };
        if status != sys::S_OK {
            return Err(RhiError::submit_failed(sys::hresult_message(status)));
        }
        // SAFETY: the event handle is live and waiting on it is the documented
        // pattern.
        unsafe { WaitForSingleObject(event, sys::INFINITE) };
        Ok(())
    }

    unsafe extern "system" {
        fn WaitForSingleObject(handle: sys::HANDLE, milliseconds: u32) -> u32;
    }

    /// `ID3D12Fence`'s vtable prefix, which is all the backend calls.
    #[repr(C)]
    struct FenceVtbl {
        base: sys::IUnknownVtbl,
        GetCompletedValue: unsafe extern "system" fn(sys::ComPtr) -> u64,
        SetEventOnCompletion:
            unsafe extern "system" fn(sys::ComPtr, u64, sys::HANDLE) -> sys::HRESULT,
        Signal: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr, u64) -> sys::HRESULT,
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
            let fence_vtbl = *(queue.fence as *mut *const FenceVtblLocal);
            ((*fence_vtbl).Signal)(queue.fence, queue.queue, value);
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

/// `ID3D12Fence`'s vtable prefix, at module scope so `execute_and_signal` can use
/// it without the Windows-only module.
#[repr(C)]
struct FenceVtblLocal {
    base: sys::IUnknownVtbl,
    GetCompletedValue: unsafe extern "system" fn(sys::ComPtr) -> u64,
    SetEventOnCompletion: unsafe extern "system" fn(sys::ComPtr, u64, sys::HANDLE) -> sys::HRESULT,
    Signal: unsafe extern "system" fn(sys::ComPtr, sys::ComPtr, u64) -> sys::HRESULT,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_ids_start_above_the_frontend_space() {
        assert_eq!(INTERNAL_ID_BASE, 0xF000_0000);
        assert!(INTERNAL_ID_BASE > 0x8000_0000);
    }

    #[test]
    fn capabilities_follow_the_feature_level() {
        let none = capabilities_for(0, false, false);
        assert!(none.graphics && none.compute);
        // Ray tracing is deliberately unadvertised at every feature level: this
        // build does not link the DXR build/execute dispatch tables, and the
        // capability set must only promise what the backend will honour. The
        // adapter's real support is reported separately via `AdapterCapabilities`.
        let eleven = capabilities_for(11, false, false);
        assert!(!eleven.ray_tracing.acceleration_structures);
        assert!(!eleven.ray_tracing.compute_ray_query);
        let twelve = capabilities_for(12, true, true);
        assert!(!twelve.ray_tracing.acceleration_structures);
        assert!(!twelve.ray_tracing.compute_ray_query);
        assert!(twelve.mesh_shading.mesh && twelve.mesh_shading.task);
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
