//! Loaded-image discovery and DWARF symbolization for the macOS sampler.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use addr2line::Loader;

use crate::{SourceLocation, SymbolizedFrame};

use super::unwind::canonical_address;

/// `MH_MAGIC_64`, the only header layout a 64-bit macOS process has.
const MH_MAGIC_64: u32 = 0xfeed_facf;
/// `LC_SEGMENT_64`.
const LC_SEGMENT_64: u32 = 0x19;
/// `VM_PROT_EXECUTE`.
const VM_PROT_EXECUTE: u32 = 1 << 2;
/// Refuse implausible load-command blocks rather than reading far.
const MAX_LOAD_COMMAND_BYTES: usize = 0x10000;
/// Addresses cached before the symbol cache is reset.
const SYMBOL_CACHE_LIMIT: usize = 65_536;

pub(crate) struct Symbolizer {
    images: Vec<Image>,
    loaders: HashMap<PathBuf, Option<Loader>>,
    cache: HashMap<u64, SymbolizedFrame>,
}

impl Symbolizer {
    pub(crate) fn new() -> Self {
        Self {
            images: scan_images(),
            loaders: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    /// Resolves one address, caching the answer including misses.
    pub(crate) fn resolve(&mut self, address: u64) -> SymbolizedFrame {
        let address = canonical_address(address);
        if let Some(frame) = self.cache.get(&address) {
            return frame.clone();
        }
        let frame = self.resolve_uncached(address);
        if self.cache.len() >= SYMBOL_CACHE_LIMIT {
            self.cache.clear();
        }
        self.cache.insert(address, frame.clone());
        frame
    }

    fn resolve_uncached(&mut self, address: u64) -> SymbolizedFrame {
        let mut image = self.image_for(address);
        if image.is_none() {
            // A library may have been loaded after this symbolizer was created.
            self.images = scan_images();
            image = self.image_for(address);
        }
        let Some((path, slide)) = image else {
            return dladdr_frame(address);
        };
        // Runtime address = file virtual address + slide, so the loader wants
        // the file address. A negative slide wraps correctly.
        let file_address = address.wrapping_sub(slide);
        let loader = self
            .loaders
            .entry(path.clone())
            .or_insert_with(|| Loader::new(&path).ok());
        if let Some(loader) = loader.as_ref()
            && let Some(frame) = resolve_with_loader(loader, file_address, address)
        {
            return frame;
        }
        dladdr_frame(address)
    }

    fn image_for(&self, address: u64) -> Option<(PathBuf, u64)> {
        self.images
            .iter()
            .find(|image| {
                image
                    .ranges
                    .iter()
                    .any(|(start, end)| address >= *start && address < *end)
            })
            .map(|image| (image.path.clone(), image.slide as u64))
    }
}

struct Image {
    path: PathBuf,
    slide: i64,
    /// Executable segment ranges in the live address space.
    ranges: Vec<(u64, u64)>,
}

/// Reads the images dyld has loaded into this process.
#[allow(deprecated)] // libc's `mach_header` type is deprecated; only the pointer is used.
fn scan_images() -> Vec<Image> {
    // SAFETY: dyld image accessors only read dyld's process-wide image table.
    let count = unsafe { libc::_dyld_image_count() };
    let mut images = Vec::new();
    for index in 0..count {
        // SAFETY: `index` is below the count just read from dyld.
        let header = unsafe { libc::_dyld_get_image_header(index) }.cast::<u8>();
        if header.is_null() {
            continue;
        }
        // SAFETY: same valid index; dyld returns the slide it applied.
        let slide = unsafe { libc::_dyld_get_image_vmaddr_slide(index) } as i64;
        // SAFETY: the name is a NUL-terminated string owned by dyld for the
        // lifetime of the loaded image.
        let name = unsafe { libc::_dyld_get_image_name(index) };
        if name.is_null() {
            continue;
        }
        let path = PathBuf::from(OsStr::from_bytes(
            // SAFETY: non-null NUL-terminated string from dyld.
            unsafe { CStr::from_ptr(name) }.to_bytes(),
        ));
        let ranges = executable_segments(header)
            .into_iter()
            .filter_map(|(vmaddr, vmsize)| {
                let start = add_slide(vmaddr, slide)?;
                let end = start.checked_add(vmsize)?;
                (end > start).then_some((start, end))
            })
            .collect::<Vec<_>>();
        if !ranges.is_empty() {
            images.push(Image {
                path,
                slide,
                ranges,
            });
        }
    }
    images
}

fn add_slide(value: u64, slide: i64) -> Option<u64> {
    u64::try_from(i128::from(value) + i128::from(slide)).ok()
}

/// Returns the file virtual ranges of executable segments in one live image.
fn executable_segments(header: *const u8) -> Vec<(u64, u64)> {
    let mut segments = Vec::new();
    if header.is_null() {
        return segments;
    }
    // SAFETY: `header` points at a mapped Mach-O header.
    if unsafe { read_u32(header, 0) } != MH_MAGIC_64 {
        return segments;
    }
    // SAFETY: field offset of `sizeofcmds` in `mach_header_64`.
    let sizeofcmds = unsafe { read_u32(header, 20) } as usize;
    if sizeofcmds == 0 || sizeofcmds > MAX_LOAD_COMMAND_BYTES {
        return segments;
    }
    let commands_end = 32 + sizeofcmds;
    let mut offset = 32;
    while offset + 8 <= commands_end {
        // SAFETY: `offset` + 8 stays inside the load-command block.
        let command = unsafe { read_u32(header, offset) };
        // SAFETY: same bounds.
        let size = unsafe { read_u32(header, offset + 4) } as usize;
        if size < 8 || offset + size > commands_end {
            break;
        }
        if command == LC_SEGMENT_64 && size >= 72 {
            // SAFETY: a segment command is 72 bytes; all reads stay inside it.
            let (vmaddr, vmsize, initprot) = unsafe {
                (
                    read_u64(header, offset + 24),
                    read_u64(header, offset + 32),
                    read_u32(header, offset + 60),
                )
            };
            if initprot & VM_PROT_EXECUTE != 0 && vmsize > 0 {
                segments.push((vmaddr, vmsize));
            }
        }
        offset += size;
    }
    segments
}

/// SAFETY: the caller guarantees `base + offset + 4` is inside one mapped
/// Mach-O header or load-command block.
unsafe fn read_u32(base: *const u8, offset: usize) -> u32 {
    // SAFETY: upheld by the caller.
    unsafe { base.add(offset).cast::<u32>().read_unaligned() }
}

/// SAFETY: the caller guarantees `base + offset + 8` is inside one mapped
/// Mach-O header or load-command block.
unsafe fn read_u64(base: *const u8, offset: usize) -> u64 {
    // SAFETY: upheld by the caller.
    unsafe { base.add(offset).cast::<u64>().read_unaligned() }
}

fn resolve_with_loader(
    loader: &Loader,
    file_address: u64,
    address: u64,
) -> Option<SymbolizedFrame> {
    let mut function = None;
    let mut location = None;
    if let Ok(mut frames) = loader.find_frames(file_address)
        && let Ok(Some(frame)) = frames.next()
    {
        if let Some(name) = frame.function.as_ref() {
            function = name
                .demangle()
                .ok()
                .map(Cow::into_owned)
                .or_else(|| name.raw_name().ok().map(Cow::into_owned));
        }
        if let Some(frame_location) = frame.location
            && let Some(file) = frame_location.file
        {
            location = Some(SourceLocation {
                file: file.to_owned(),
                line: frame_location.line,
            });
        }
    }
    if function.is_none()
        && let Some(name) = loader.find_symbol(file_address)
    {
        function = Some(addr2line::demangle_auto(Cow::Borrowed(name), None).into_owned());
    }
    if function.is_none() && location.is_none() {
        return None;
    }
    Some(SymbolizedFrame {
        address,
        function,
        location,
    })
}

/// Last resort for dyld-cache images whose file is not on disk.
fn dladdr_frame(address: u64) -> SymbolizedFrame {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::zeroed();
    // SAFETY: dladdr reads the current image table and writes at most one
    // `Dl_info` into the provided storage.
    let found = unsafe { libc::dladdr(address as *const libc::c_void, info.as_mut_ptr()) } != 0;
    let function = if found {
        // SAFETY: a successful dladdr initialized the struct; `dli_sname` is
        // null or a NUL-terminated name in the image's string table.
        let info = unsafe { info.assume_init() };
        if info.dli_sname.is_null() {
            None
        } else {
            // SAFETY: non-null NUL-terminated symbol name.
            let name = unsafe { CStr::from_ptr(info.dli_sname) }.to_string_lossy();
            Some(addr2line::demangle_auto(name, None).into_owned())
        }
    } else {
        None
    };
    SymbolizedFrame {
        address,
        function,
        location: None,
    }
}
