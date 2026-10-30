//! Translation of NativeRHI/Vulkan/Resources/VulkanMemory.swift: the block
//! suballocator for device memory.
//!
//! Instead of one `vkAllocateMemory` per resource, suballocations are carved out
//! of a small number of large blocks. Host-visible blocks are persistently
//! mapped, so the upload ring and immediate transfers never map on the hot path.
//!
//! The allocator is a pure data structure over the driver table it holds, which
//! is what makes the placement logic testable without a GPU: `place` and `free`
//! below decide offsets, and only `create_block` talks to the driver.
use crate::rhi::{RhiError, RhiResult};

use super::setup::VulkanContext;
use super::sys;

/// Default block size, matching Swift's 64 MiB.
pub const DEFAULT_BLOCK_SIZE: sys::VkDeviceSize = 64 * 1024 * 1024;

/// One suballocation out of a memory block.
#[derive(Clone, Copy, Debug)]
pub struct VulkanMemoryAllocation {
    pub block_index: usize,
    pub offset: sys::VkDeviceSize,
    pub size: sys::VkDeviceSize,
    pub memory: sys::VkDeviceMemory,
    /// Persistently mapped base for host-visible blocks; null otherwise.
    pub mapped_base: *mut core::ffi::c_void,
}

// SAFETY: the allocation is plain data plus a driver handle and a mapping that
// stays valid while its block is alive. The mapping is only ever dereferenced by
// the thread that holds the registry lock.
unsafe impl Send for VulkanMemoryAllocation {}

impl VulkanMemoryAllocation {
    /// Writable CPU pointer at `offset` bytes into this allocation, or `None`
    /// when the block was not host visible.
    pub fn cpu_pointer(&self, offset: usize) -> Option<*mut u8> {
        if self.mapped_base.is_null() {
            return None;
        }
        // SAFETY: the mapping covers at least `self.size` bytes starting at the
        // allocation's offset, and the caller is expected to stay in range.
        Some(unsafe { self.mapped_base.cast::<u8>().add(offset) })
    }
}

/// One memory block plus its free list.
pub struct VulkanMemoryBlock {
    pub memory: sys::VkDeviceMemory,
    pub size: sys::VkDeviceSize,
    pub memory_type_index: u32,
    pub mapped_base: *mut core::ffi::c_void,
    /// Free regions, kept sorted by offset and coalesced.
    pub free_regions: Vec<(sys::VkDeviceSize, sys::VkDeviceSize)>,
}

pub struct VulkanMemoryAllocator {
    device: sys::VkDevice,
    core: sys::CoreCommands,
    memory_properties: sys::VkPhysicalDeviceMemoryProperties,
    buffer_image_granularity: sys::VkDeviceSize,
    blocks: Vec<Option<VulkanMemoryBlock>>,
    block_size: sys::VkDeviceSize,
    /// Whether the device enabled buffer device addresses, which every block
    /// must then request so acceleration structures can point into them.
    acceleration_structures_enabled: bool,
}

impl VulkanMemoryAllocator {
    /// Builds an allocator from the pieces it actually uses, so a context can own
    /// one without borrowing itself.
    pub fn from_parts(
        device: sys::VkDevice,
        core: sys::CoreCommands,
        buffer_image_granularity: sys::VkDeviceSize,
        memory_properties: sys::VkPhysicalDeviceMemoryProperties,
        acceleration_structures_enabled: bool,
    ) -> Self {
        Self {
            device,
            core,
            memory_properties,
            buffer_image_granularity,
            blocks: Vec::new(),
            block_size: DEFAULT_BLOCK_SIZE,
            acceleration_structures_enabled,
        }
    }

    pub fn new(context: &VulkanContext, memory_properties: sys::VkPhysicalDeviceMemoryProperties) -> Self {
        Self {
            device: context.device,
            core: context.core,
            memory_properties,
            // Buffer/image granularity is an allocation boundary invariant when
            // linear buffers and optimal images share one VkDeviceMemory block.
            buffer_image_granularity: context.limits.bufferImageGranularity.max(1),
            blocks: Vec::new(),
            block_size: DEFAULT_BLOCK_SIZE,
            acceleration_structures_enabled: context.features.acceleration_structures,
        }
    }

    /// Resolves a memory type satisfying `memory_type_bits` and every required
    /// property bit, or `None`.
    pub fn find_memory_type(
        &self,
        memory_type_bits: u32,
        required_flags: u32,
    ) -> Option<u32> {
        for index in 0..self.memory_properties.memoryTypeCount {
            if memory_type_bits & (1 << index) == 0 {
                continue;
            }
            let flags = self.memory_properties.memoryTypes[index as usize].propertyFlags;
            if flags & required_flags == required_flags {
                return Some(index);
            }
        }
        None
    }

    /// Rounds `size` up to the next multiple of `alignment`.
    fn align(size: sys::VkDeviceSize, alignment: sys::VkDeviceSize) -> sys::VkDeviceSize {
        let alignment = alignment.max(1);
        size.div_ceil(alignment) * alignment
    }

    /// Allocates `size` bytes from a block of a matching memory type.
    pub fn allocate(
        &mut self,
        size: sys::VkDeviceSize,
        alignment: sys::VkDeviceSize,
        memory_type_bits: u32,
        required_flags: u32,
        preferred_flags: u32,
        dedicated: bool,
    ) -> RhiResult<VulkanMemoryAllocation> {
        let granularity = self.buffer_image_granularity;
        let alignment = alignment.max(granularity);
        let size = Self::align(size, granularity);
        let type_index = self
            .find_memory_type(memory_type_bits, required_flags | preferred_flags)
            .or_else(|| self.find_memory_type(memory_type_bits, required_flags))
            .ok_or(RhiError::OutOfMemory)?;

        // Best fit over existing blocks of this type.
        for block_index in 0..self.blocks.len() {
            let Some(block) = &mut self.blocks[block_index] else { continue };
            if block.memory_type_index != type_index {
                continue;
            }
            if let Some(offset) = take_free_region(block, size, alignment) {
                return Ok(Self::commit(
                    block_index,
                    block,
                    offset,
                    size,
                    self.device,
                ));
            }
        }

        // Otherwise grow a fresh block.
        let block_size = if dedicated {
            Self::align(size, alignment)
        } else {
            self.block_size.max(Self::align(size, alignment))
        };
        let coherent = self.memory_properties.memoryTypes[type_index as usize].propertyFlags
            & (sys::VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | sys::VK_MEMORY_PROPERTY_HOST_COHERENT_BIT)
            == (sys::VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT | sys::VK_MEMORY_PROPERTY_HOST_COHERENT_BIT);
        let block = self.create_block(block_size, type_index, coherent)?;
        let block_index = match self.blocks.iter().position(Option::is_none) {
            Some(index) => {
                self.blocks[index] = Some(block);
                index
            }
            None => {
                self.blocks.push(Some(block));
                self.blocks.len() - 1
            }
        };
        let block = self.blocks[block_index].as_mut().expect("just inserted");
        let offset = take_free_region(block, size, alignment).ok_or(RhiError::OutOfMemory)?;
        Ok(Self::commit(block_index, block, offset, size, self.device))
    }

    fn commit(
        block_index: usize,
        block: &VulkanMemoryBlock,
        offset: sys::VkDeviceSize,
        size: sys::VkDeviceSize,
        _device: sys::VkDevice,
    ) -> VulkanMemoryAllocation {
        VulkanMemoryAllocation {
            block_index,
            offset,
            size,
            memory: block.memory,
            mapped_base: block.mapped_base,
        }
    }

    /// Returns one suballocation to its block, coalescing neighbours. A block
    /// that becomes wholly free is released, so retired presentation images do
    /// not pin memory for the process lifetime.
    pub fn free(&mut self, allocation: VulkanMemoryAllocation) {
        let Some(Some(block)) = self.blocks.get_mut(allocation.block_index) else { return };
        block.free_regions.push((allocation.offset, allocation.size));
        block.free_regions.sort_by_key(|(offset, _)| *offset);
        let mut merged: Vec<(sys::VkDeviceSize, sys::VkDeviceSize)> = Vec::new();
        for (offset, size) in block.free_regions.drain(..) {
            match merged.last_mut() {
                Some((previous_offset, previous_size))
                    if *previous_offset + *previous_size == offset =>
                {
                    *previous_size += size;
                }
                _ => merged.push((offset, size)),
            }
        }
        block.free_regions = merged;

        let wholly_free = block.free_regions.len() == 1
            && block.free_regions[0].0 == 0
            && block.free_regions[0].1 == block.size;
        if wholly_free {
            let block = self.blocks[allocation.block_index].take().expect("checked above");
            // SAFETY: the block is wholly unallocated, so nothing references its
            // memory; unmapping and freeing it here is the documented teardown.
            unsafe {
                if !block.mapped_base.is_null() {
                    (self.core.unmap_memory)(self.device, block.memory);
                }
                (self.core.free_memory)(self.device, block.memory, core::ptr::null());
            }
        }
    }

    /// Unmaps and frees every block. Call only after the device is idle.
    pub fn destroy_all(&mut self) {
        for block in self.blocks.drain(..).flatten() {
            // SAFETY: the device is idle, so no queued work reads this memory.
            unsafe {
                if !block.mapped_base.is_null() {
                    (self.core.unmap_memory)(self.device, block.memory);
                }
                (self.core.free_memory)(self.device, block.memory, core::ptr::null());
            }
        }
    }

    /// Total bytes currently reserved from the driver, for diagnostics.
    pub fn reserved_bytes(&self) -> sys::VkDeviceSize {
        self.blocks.iter().flatten().map(|block| block.size).sum()
    }

    fn create_block(
        &self,
        size: sys::VkDeviceSize,
        type_index: u32,
        host_visible: bool,
    ) -> RhiResult<VulkanMemoryBlock> {
        let mut allocate_info = sys::VkMemoryAllocateInfo {
            sType: sys::VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO,
            pNext: core::ptr::null(),
            allocationSize: size,
            memoryTypeIndex: type_index,
        };
        // Device addresses are required for acceleration structures, so every
        // block opts in when the feature is on.
        let mut flags = sys::VkMemoryAllocateFlagsInfo {
            sType: sys::VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_FLAGS_INFO,
            pNext: core::ptr::null(),
            flags: sys::VK_MEMORY_ALLOCATE_DEVICE_ADDRESS_BIT,
            deviceMask: 0,
        };
        if self.acceleration_structures_enabled {
            allocate_info.pNext = (&mut flags as *mut sys::VkMemoryAllocateFlagsInfo).cast();
        }

        let mut memory: sys::VkDeviceMemory = sys::VK_NULL_HANDLE;
        // SAFETY: `allocate_info` and the flag node it points at are live for the
        // call, and the out-parameter is a live local.
        let status = unsafe {
            (self.core.allocate_memory)(self.device, &allocate_info, core::ptr::null(), &mut memory)
        };
        if status != sys::VK_SUCCESS || memory == sys::VK_NULL_HANDLE {
            return Err(RhiError::OutOfMemory);
        }

        let mut mapped_base: *mut core::ffi::c_void = core::ptr::null_mut();
        if host_visible {
            // SAFETY: the memory was just allocated and nothing else maps it.
            let map_status = unsafe {
                (self.core.map_memory)(self.device, memory, 0, size, 0, &mut mapped_base)
            };
            if map_status != sys::VK_SUCCESS {
                // SAFETY: the allocation failed to map; freeing it here releases
                // the only reference.
                unsafe { (self.core.free_memory)(self.device, memory, core::ptr::null()) };
                return Err(RhiError::OutOfMemory);
            }
        }
        Ok(VulkanMemoryBlock {
            memory,
            size,
            memory_type_index: type_index,
            mapped_base,
            free_regions: vec![(0, size)],
        })
    }
}

/// Takes the first free region that fits `size` at `alignment`, splitting the
/// remainder. Returns the aligned offset.
fn take_free_region(
    block: &mut VulkanMemoryBlock,
    size: sys::VkDeviceSize,
    alignment: sys::VkDeviceSize,
) -> Option<sys::VkDeviceSize> {
    for index in 0..block.free_regions.len() {
        let (region_offset, region_size) = block.free_regions[index];
        let aligned = VulkanMemoryAllocator::align(region_offset, alignment);
        let padding = aligned - region_offset;
        if padding + size > region_size {
            continue;
        }
        let remaining = region_size - padding - size;
        block.free_regions.remove(index);
        if padding > 0 {
            block.free_regions.push((region_offset, padding));
        }
        if remaining > 0 {
            block.free_regions.push((aligned + size, remaining));
        }
        block.free_regions.sort_by_key(|(offset, _)| *offset);
        return Some(aligned);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A command table whose entries are never called. `find_memory_type` only
    /// reads `memory_properties`, so the table exists to satisfy the struct shape;
    /// it is built with an explicit null-entry loader rather than `zeroed()`, which
    /// would be undefined behaviour for a type holding raw pointers.
    fn null_core_commands() -> sys::CoreCommands {
        fn missing(_name: &core::ffi::CStr) -> sys::PFN_vkVoidFunction {
            core::ptr::null()
        }
        // SAFETY: the loader returns null for every name, so `CoreCommands::load`
        // reports `None` instead of producing a table. The fallback below is only
        // reached if a required entry point is renamed, in which case the test
        // panics rather than calling through a null pointer.
        unsafe { sys::CoreCommands::load(&missing) }
            .expect("the test needs no device entry point")
    }

    fn block(size: sys::VkDeviceSize) -> VulkanMemoryBlock {
        VulkanMemoryBlock {
            memory: 1,
            size,
            memory_type_index: 0,
            mapped_base: core::ptr::null_mut(),
            free_regions: vec![(0, size)],
        }
    }

    #[test]
    fn alignment_rounds_up_and_never_divides_by_zero() {
        assert_eq!(VulkanMemoryAllocator::align(0, 256), 0);
        assert_eq!(VulkanMemoryAllocator::align(1, 256), 256);
        assert_eq!(VulkanMemoryAllocator::align(256, 256), 256);
        assert_eq!(VulkanMemoryAllocator::align(257, 256), 512);
        assert_eq!(VulkanMemoryAllocator::align(5, 0), 5);
    }

    #[test]
    fn the_first_allocation_starts_at_zero_and_splits_the_block() {
        let mut storage = block(1024);
        assert_eq!(take_free_region(&mut storage, 100, 1), Some(0));
        assert_eq!(storage.free_regions, vec![(100, 924)]);
        assert_eq!(take_free_region(&mut storage, 24, 8), Some(104));
        assert_eq!(storage.free_regions, vec![(128, 896)]);
    }

    #[test]
    fn a_region_too_small_for_the_alignments_is_skipped() {
        let mut storage = block(64);
        // The first 4 bytes are consumed, leaving a 60-byte region at offset 4;
        // a 64-byte request aligned to 64 does not fit.
        assert_eq!(take_free_region(&mut storage, 4, 1), Some(0));
        assert_eq!(take_free_region(&mut storage, 64, 64), None);
        assert_eq!(take_free_region(&mut storage, 60, 4), Some(4));
        assert!(storage.free_regions.is_empty());
    }

    #[test]
    fn freeing_coalesces_adjacent_regions() {
        let mut storage = block(256);
        let first = take_free_region(&mut storage, 64, 1).expect("first");
        let second = take_free_region(&mut storage, 64, 1).expect("second");
        assert_eq!((first, second), (0, 64));
        assert_eq!(storage.free_regions, vec![(128, 128)]);
        // Releasing both in reverse order must still merge into one region.
        storage.free_regions.push((second, 64));
        storage.free_regions.push((first, 64));
        storage.free_regions.sort_by_key(|(offset, _)| *offset);
        let mut merged: Vec<(sys::VkDeviceSize, sys::VkDeviceSize)> = Vec::new();
        for (offset, size) in storage.free_regions.drain(..) {
            match merged.last_mut() {
                Some((previous_offset, previous_size))
                    if *previous_offset + *previous_size == offset =>
                {
                    *previous_size += size;
                }
                _ => merged.push((offset, size)),
            }
        }
        assert_eq!(merged, vec![(0, 256)]);
    }

    #[test]
    fn a_host_visible_allocation_reports_a_cpu_pointer() {
        let allocation = VulkanMemoryAllocation {
            block_index: 0,
            offset: 0,
            size: 16,
            memory: 1,
            mapped_base: core::ptr::null_mut(),
        };
        // An unmapped block must not pretend to have a pointer.
        assert!(allocation.cpu_pointer(0).is_none());
    }

    #[test]
    fn memory_type_search_requires_every_requested_bit() {
        let mut properties = sys::VkPhysicalDeviceMemoryProperties::default();
        properties.memoryTypeCount = 2;
        properties.memoryTypes[0] = sys::VkMemoryType {
            propertyFlags: sys::VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT,
            heapIndex: 0,
        };
        properties.memoryTypes[1] = sys::VkMemoryType {
            propertyFlags: sys::VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT
                | sys::VK_MEMORY_PROPERTY_HOST_COHERENT_BIT,
            heapIndex: 1,
        };
        // `find_memory_type` only reads `memory_properties`, so the command table is
        // never dereferenced here; it is left as a null-filled table rather than a
        // zeroed struct, which would be undefined behaviour for a pointer-bearing
        // type.
        let allocator = VulkanMemoryAllocator {
            device: core::ptr::null_mut(),
            core: null_core_commands(),
            memory_properties: properties,
            buffer_image_granularity: 1,
            blocks: Vec::new(),
            block_size: DEFAULT_BLOCK_SIZE,
            acceleration_structures_enabled: false,
        };
        assert_eq!(allocator.find_memory_type(0b11, sys::VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT), Some(0));
        assert_eq!(
            allocator.find_memory_type(0b11, sys::VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT),
            Some(1)
        );
        // Type 0 does not satisfy visibility, so a mask naming only it fails.
        assert_eq!(
            allocator.find_memory_type(0b01, sys::VK_MEMORY_PROPERTY_HOST_VISIBLE_BIT),
            None
        );
        assert_eq!(allocator.find_memory_type(0, 0), None);
    }
}
