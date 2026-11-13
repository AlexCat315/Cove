//! Loaded-image discovery from `/proc/self/maps` and DWARF symbolization.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use addr2line::Loader;

use crate::platform::fp::canonical_address;
use crate::{SourceLocation, SymbolizedFrame};

use super::unwind::ADDRESS_MASK;

/// Addresses cached before the symbol cache is reset.
const SYMBOL_CACHE_LIMIT: usize = 65_536;
/// `PT_LOAD`, the only program header that places file bytes in memory.
const PT_LOAD: u32 = 1;
/// Refuse implausible program-header tables rather than reading far.
const MAX_PROGRAM_HEADERS: u16 = 256;

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
        let address = canonical_address(address, ADDRESS_MASK);
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
        let Some((path, bias)) = image else {
            return dladdr_frame(address);
        };
        // Runtime address = file virtual address + load bias.
        let file_address = address.wrapping_sub(bias);
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
            .map(|image| (image.path.clone(), image.bias))
    }
}

struct Image {
    path: PathBuf,
    /// Added to a file virtual address to obtain its runtime address.
    bias: u64,
    /// Executable mapping ranges in the live address space.
    ranges: Vec<(u64, u64)>,
}

/// One parsed line of `/proc/self/maps`.
struct Mapping {
    start: u64,
    end: u64,
    offset: u64,
    executable: bool,
    path: PathBuf,
}

/// Reads the executable file-backed images mapped into this process.
fn scan_images() -> Vec<Image> {
    let Ok(text) = std::fs::read_to_string("/proc/self/maps") else {
        return Vec::new();
    };
    let mut by_path: HashMap<PathBuf, Vec<Mapping>> = HashMap::new();
    for line in text.lines() {
        if let Some(mapping) = parse_mapping(line) {
            by_path
                .entry(mapping.path.clone())
                .or_default()
                .push(mapping);
        }
    }
    let mut images = Vec::new();
    for (path, mappings) in by_path {
        // The mapping at file offset zero is where the ELF header landed, so
        // it fixes the load bias for every other segment of the same image.
        let Some(base) = mappings.iter().find(|mapping| mapping.offset == 0) else {
            continue;
        };
        let Some(lowest_vaddr) = lowest_load_address(&path) else {
            continue;
        };
        let bias = base.start.wrapping_sub(lowest_vaddr);
        let ranges = mappings
            .iter()
            .filter(|mapping| mapping.executable)
            .map(|mapping| (mapping.start, mapping.end))
            .collect::<Vec<_>>();
        if !ranges.is_empty() {
            images.push(Image { path, bias, ranges });
        }
    }
    images
}

fn parse_mapping(line: &str) -> Option<Mapping> {
    // The path is the sixth field and may itself contain spaces, so the first
    // five fields are consumed by scanning rather than by `split_whitespace`.
    let mut rest = line.trim_start();
    let mut fields = [""; 5];
    for field in &mut fields {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        *field = &rest[..end];
        rest = rest[end..].trim_start();
    }
    let path = rest.trim_end();
    if !path.starts_with('/') {
        return None;
    }
    let path = PathBuf::from(path.strip_suffix(" (deleted)").unwrap_or(path));
    let (start, end) = fields[0].split_once('-')?;
    let start = u64::from_str_radix(start, 16).ok()?;
    let end = u64::from_str_radix(end, 16).ok()?;
    let offset = u64::from_str_radix(fields[2], 16).ok()?;
    (end > start).then_some(Mapping {
        start,
        end,
        offset,
        executable: fields[1].as_bytes().get(2) == Some(&b'x'),
        path,
    })
}

/// Lowest `p_vaddr` among the ELF image's `PT_LOAD` segments.
fn lowest_load_address(path: &Path) -> Option<u64> {
    let mut file = File::open(path).ok()?;
    let mut header = [0_u8; 64];
    file.read_exact(&mut header).ok()?;
    if &header[..4] != b"\x7fELF" || header[4] != 2 {
        return None;
    }
    let program_offset = u64::from_le_bytes(header[32..40].try_into().ok()?);
    let entry_size = u16::from_le_bytes(header[54..56].try_into().ok()?);
    let entry_count = u16::from_le_bytes(header[56..58].try_into().ok()?);
    if entry_size < 56 || entry_count == 0 || entry_count > MAX_PROGRAM_HEADERS {
        return None;
    }
    file.seek(SeekFrom::Start(program_offset)).ok()?;
    let mut entry = vec![0_u8; usize::from(entry_size)];
    let mut lowest = None;
    for _ in 0..entry_count {
        file.read_exact(&mut entry).ok()?;
        let kind = u32::from_le_bytes(entry[0..4].try_into().ok()?);
        if kind != PT_LOAD {
            continue;
        }
        let vaddr = u64::from_le_bytes(entry[16..24].try_into().ok()?);
        lowest = Some(lowest.map_or(vaddr, |current: u64| current.min(vaddr)));
    }
    lowest
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

/// Last resort for images without debug information or a readable file.
fn dladdr_frame(address: u64) -> SymbolizedFrame {
    let mut info = std::mem::MaybeUninit::<libc::Dl_info>::zeroed();
    // SAFETY: dladdr reads the dynamic linker's image table and writes at most
    // one `Dl_info` into the provided storage.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_executable_file_mapping() {
        let mapping =
            parse_mapping("7f0000000000-7f0000001000 r-xp 00002000 08:01 4242 /usr/lib/libcove.so")
                .expect("mapping");
        assert_eq!(mapping.start, 0x7f0000000000);
        assert_eq!(mapping.end, 0x7f0000001000);
        assert_eq!(mapping.offset, 0x2000);
        assert!(mapping.executable);
        assert_eq!(mapping.path, Path::new("/usr/lib/libcove.so"));
    }

    #[test]
    fn skips_anonymous_and_special_mappings() {
        assert!(parse_mapping("7f0000000000-7f0000001000 rw-p 00000000 00:00 0").is_none());
        assert!(parse_mapping("7f0000000000-7f0000001000 r-xp 00000000 00:00 0 [vdso]").is_none());
    }

    #[test]
    fn keeps_a_path_that_contains_spaces() {
        let mapping =
            parse_mapping("7f0000000000-7f0000001000 r-xp 00000000 08:01 7 /opt/my app/bin/cove")
                .expect("mapping");
        assert_eq!(mapping.path, Path::new("/opt/my app/bin/cove"));
    }
}
