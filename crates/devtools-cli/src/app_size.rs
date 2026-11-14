//! Read-only size analysis of the selected runtime's own native executable.
//!
//! Flutter's App Size view attributes Dart AOT snapshots, packages and assets.
//! Cove ships neither, so this module measures the executable the selected
//! runtime is actually running: file bytes, loadable segments, their sections,
//! symbols and the sums of demangled Rust crate groups. Every value comes from
//! the file on disk; a missing or unreadable executable is reported as
//! unavailable instead of being estimated.
//!
//! Segments and sections report file bytes and VM bytes separately, so
//! zero-fill (`.bss`, `__bss`) is never counted as file content. Symbols are
//! deduplicated when several names share one address and section, because a
//! linker may fold identical functions to a single address. The remaining file
//! bytes are reported as `linkeditBytes` (Mach-O linkage metadata) and
//! `unattributedBytes`; no dominator tree, call edge or AOT attribution is
//! invented from a symbol table that cannot prove any of them.
use cove_devtools_protocol::control::Fault;
use object::{Object, ObjectSection, ObjectSegment, ObjectSymbol};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) const DEFAULT_LIMIT: usize = 20;
pub(crate) const MAX_SEGMENTS: usize = 50;
pub(crate) const MAX_SECTIONS: usize = 100;
pub(crate) const MAX_SYMBOLS: usize = 200;
pub(crate) const MAX_GROUPS: usize = 100;
const MAX_GROUP_NAME: usize = 160;
const MAX_ALIAS_NAMES: usize = 3;
const MAX_ALIAS_NAME: usize = 120;
const LINKEDIT_SEGMENT: &str = "__LINKEDIT";

pub(crate) const LABEL: &str = "Native executable of the selected runtime";
pub(crate) const ATTRIBUTION: &str = "This measurement covers the selected runtime's own executable file. Flutter App Size's Dart AOT, package and asset attribution does not apply to Cove.";
pub(crate) const ACCOUNTING_BASIS: &str = "attributedBytes sums symbol extents that fall inside file-backed section bytes; an extent reaches the next symbol in its own section and never past that section, so it is a symbol-table extent, not a proven instruction or data boundary. linkeditBytes is the Mach-O __LINKEDIT segment's file bytes (symbol and string tables, dyld metadata, code signature); other object formats report 0 because they have no linkedit segment. unattributedBytes is the rest of the file: headers, page padding, debug sections, string tables and any bytes no symbol covers. attributedBytes + linkeditBytes + unattributedBytes equals fileBytes. No call edges or dominator relation are implied.";

/// How many rows of each kind to return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Limits {
    pub segments: usize,
    pub sections: usize,
    pub symbols: usize,
    pub groups: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            segments: DEFAULT_LIMIT,
            sections: DEFAULT_LIMIT,
            symbols: DEFAULT_LIMIT,
            groups: DEFAULT_LIMIT,
        }
    }
}

impl Limits {
    fn bounded(self) -> Self {
        Self {
            segments: self.segments.min(MAX_SEGMENTS),
            sections: self.sections.min(MAX_SECTIONS),
            symbols: self.symbols.min(MAX_SYMBOLS),
            groups: self.groups.min(MAX_GROUPS),
        }
    }
}

fn unavailable(message: impl Into<String>) -> Fault {
    Fault::new("unavailable", message)
}

#[cfg(target_os = "macos")]
fn path_bytes(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

/// Resolves a local process ID to the executable that process is running.
#[cfg(target_os = "macos")]
pub(crate) fn executable(pid: u32) -> Result<PathBuf, Fault> {
    let mut buffer = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: `buffer` is valid for writes of its full length, and the kernel
    // writes at most `buffer.len()` bytes. A failed call returns a negative
    // length, which is checked before any byte is read.
    let length = unsafe {
        libc::proc_pidpath(
            pid as libc::c_int,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if length <= 0 {
        return Err(unavailable(format!(
            "macOS could not resolve the executable path for process {pid}: {}",
            std::io::Error::last_os_error()
        )));
    }
    let bytes = &buffer[..length as usize];
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    Ok(path_bytes(&bytes[..end]))
}

/// `/proc/<pid>/exe` is the kernel's own answer for a local process.
#[cfg(target_os = "linux")]
pub(crate) fn executable(pid: u32) -> Result<PathBuf, Fault> {
    let link = format!("/proc/{pid}/exe");
    let path = std::fs::read_link(&link).map_err(|error| {
        unavailable(format!(
            "Linux could not resolve the executable path for process {pid} from {link}: {error}"
        ))
    })?;
    use std::os::unix::ffi::OsStringExt;
    let mut bytes = path.into_os_string().into_vec();
    if bytes.ends_with(b" (deleted)") {
        bytes.truncate(bytes.len() - b" (deleted)".len());
    }
    Ok(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// Opens the process with query-only rights and asks for its image path.
#[cfg(target_os = "windows")]
pub(crate) fn executable(pid: u32) -> Result<PathBuf, Fault> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    // SAFETY: `OpenProcess` takes plain scalar arguments and returns an owned
    // handle or null; null is checked before the handle is used.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return Err(unavailable(format!(
            "Windows could not open process {pid} to resolve its executable: {}",
            std::io::Error::last_os_error()
        )));
    }
    let mut buffer = [0u16; 32_768];
    let mut length = buffer.len() as u32;
    // SAFETY: `buffer` is valid for `length` UTF-16 units and `handle` came
    // from `OpenProcess` above. The call writes at most `length` units.
    let ok = unsafe {
        QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut length)
    };
    // SAFETY: `handle` is owned by this function and is closed exactly once.
    unsafe {
        CloseHandle(handle);
    }
    if ok == 0 {
        return Err(unavailable(format!(
            "Windows could not resolve the executable path for process {pid}: {}",
            std::io::Error::last_os_error()
        )));
    }
    let length = (length as usize).min(buffer.len());
    Ok(PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
}

/// Other hosts (mobile, WebAssembly, unsupported Unix) have no local process
/// API this controller can use, so the analysis is explicitly unavailable.
#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub(crate) fn executable(pid: u32) -> Result<PathBuf, Fault> {
    Err(unavailable(format!(
        "App Size reads a local desktop executable; this host cannot resolve process {pid}."
    )))
}

fn demangle(raw: &str) -> String {
    match rustc_demangle::try_demangle(raw) {
        Ok(name) => format!("{name:#}"),
        Err(_) => raw.to_owned(),
    }
}

/// The first path segment of a demangled name, the closest crate/group analogue
/// to Flutter's package attribution. Non-Rust symbols keep their own name.
fn group_name(name: &str) -> String {
    let name = name.strip_prefix('<').unwrap_or(name);
    let head = name.split("::").next().unwrap_or(name);
    let head = head.split(" as ").next().unwrap_or(head).trim();
    if head.is_empty() {
        "(unknown)".into()
    } else {
        head.chars().take(MAX_GROUP_NAME).collect()
    }
}

fn truncated(value: &str, maximum: usize) -> String {
    let mut text: String = value.chars().take(maximum).collect();
    if text.chars().count() < value.chars().count() {
        text.push('…');
    }
    text
}

struct SegmentRow {
    index: usize,
    name: String,
    address: u64,
    vm_size: u64,
    file_offset: u64,
    file_size: u64,
    permissions: String,
    section_count: usize,
    section_vm_bytes: u64,
    section_file_bytes: u64,
    section_zero_fill_bytes: u64,
}

impl SegmentRow {
    fn end_address(&self) -> u64 {
        self.address.saturating_add(self.vm_size)
    }

    fn contains(&self, address: u64) -> bool {
        address >= self.address && address < self.end_address()
    }
}

struct SectionRow {
    index: usize,
    name: String,
    segment: String,
    segment_index: Option<usize>,
    kind: String,
    /// Memory size recorded by the object file (`sh_size`, `vmsize`).
    vm_size: u64,
    /// Bytes this section actually occupies in the file.
    file_size: u64,
    /// Offset of `file_size` bytes in the file, absent for NOBITS sections.
    offset: Option<u64>,
    address: u64,
}

impl SectionRow {
    fn zero_fill_size(&self) -> u64 {
        self.vm_size.saturating_sub(self.file_size)
    }

    fn end_address(&self) -> u64 {
        self.address.saturating_add(self.vm_size)
    }
}

struct SectionExtent {
    segment: String,
    vm_size: u64,
    file_size: u64,
}

struct SymbolRow {
    name: String,
    raw: String,
    kind: String,
    size: u64,
    address: u64,
    section: Option<usize>,
    section_name: Option<String>,
    segment_name: Option<String>,
    inferred: bool,
    /// Additional names collapsed onto this address in the same section.
    aliases: usize,
    alias_names: Vec<String>,
}

impl SymbolRow {
    fn end_address(&self) -> u64 {
        self.address.saturating_add(self.size)
    }
}

/// ELF program headers have no name, so label their `p_type` with the constant
/// a reader would find in the header instead of inventing a section name.
fn elf_segment_name(p_type: object::elf::ProgramType) -> String {
    use object::elf;
    let name = if p_type == elf::PT_LOAD {
        "LOAD"
    } else if p_type == elf::PT_DYNAMIC {
        "DYNAMIC"
    } else if p_type == elf::PT_INTERP {
        "INTERP"
    } else if p_type == elf::PT_NOTE {
        "NOTE"
    } else if p_type == elf::PT_PHDR {
        "PHDR"
    } else if p_type == elf::PT_TLS {
        "TLS"
    } else if p_type == elf::PT_GNU_EH_FRAME {
        "GNU_EH_FRAME"
    } else if p_type == elf::PT_GNU_STACK {
        "GNU_STACK"
    } else if p_type == elf::PT_GNU_RELRO {
        "GNU_RELRO"
    } else {
        return format!("p_type 0x{:x}", p_type.0);
    };
    name.to_owned()
}

fn segment_row(segment: &object::Segment<'_, '_>, index: usize) -> SegmentRow {
    let name = match segment.name() {
        Ok(Some(name)) if !name.is_empty() => name.to_owned(),
        _ => match segment.flags() {
            object::SegmentFlags::Elf { p_type, .. } => elf_segment_name(p_type),
            _ => format!("segment {index}"),
        },
    };
    let (file_offset, file_size) = segment.file_range();
    SegmentRow {
        index,
        name,
        address: segment.address(),
        vm_size: segment.size(),
        file_offset,
        file_size,
        permissions: format!("{:?}", segment.permissions()),
        section_count: 0,
        section_vm_bytes: 0,
        section_file_bytes: 0,
        section_zero_fill_bytes: 0,
    }
}

fn section_row(section: &object::Section<'_, '_>, segments: &[SegmentRow]) -> SectionRow {
    let file_range = section.file_range();
    let mut row = SectionRow {
        index: section.index().0,
        name: section.name().unwrap_or("<unnamed>").to_owned(),
        segment: section
            .segment_name()
            .ok()
            .flatten()
            .unwrap_or("")
            .to_owned(),
        segment_index: None,
        kind: format!("{:?}", section.kind()),
        vm_size: section.size(),
        file_size: file_range.map_or(0, |(_, size)| size),
        offset: file_range.map(|(offset, _)| offset),
        address: section.address(),
    };
    // Mach-O names the owning segment on the section. ELF and PE do not, so
    // use the mapped address; address 0 means the section is not mapped.
    if row.segment.is_empty()
        && row.address != 0
        && let Some(segment) = segments
            .iter()
            .find(|segment| segment.contains(row.address))
    {
        row.segment = segment.name.clone();
    }
    row.segment_index = if row.address != 0 {
        segments
            .iter()
            .position(|segment| segment.contains(row.address))
    } else {
        None
    }
    .or_else(|| {
        segments
            .iter()
            .position(|segment| !row.segment.is_empty() && segment.name == row.segment)
    });
    row
}

fn push_symbol(symbol: &object::Symbol<'_, '_>, rows: &mut Vec<SymbolRow>) {
    if !symbol.is_definition() {
        return;
    }
    let Ok(raw) = symbol.name() else {
        return;
    };
    if raw.is_empty() {
        return;
    }
    rows.push(SymbolRow {
        name: demangle(raw),
        raw: raw.to_owned(),
        kind: format!("{:?}", symbol.kind()),
        size: symbol.size(),
        address: symbol.address(),
        section: symbol.section_index().map(|index| index.0),
        section_name: None,
        segment_name: None,
        inferred: false,
        aliases: 0,
        alias_names: Vec::new(),
    });
}

/// Collapses symbols that share one section and address. Linkers fold identical
/// functions and objects, and Mach-O lists every folded name; keeping all of
/// them would charge one extent once per alias. The first name stays the row,
/// the rest are reported as alias names, and the largest recorded size wins.
fn dedupe_aliases(symbols: Vec<SymbolRow>) -> (Vec<SymbolRow>, usize) {
    let mut positions: HashMap<(usize, u64), usize> = HashMap::new();
    let mut kept: Vec<SymbolRow> = Vec::with_capacity(symbols.len());
    let mut collapsed = 0usize;
    for symbol in symbols {
        if symbol.address != 0
            && let Some(section) = symbol.section
            && let Some(&position) = positions.get(&(section, symbol.address))
        {
            collapsed += 1;
            let existing = &mut kept[position];
            existing.aliases += 1;
            if existing.alias_names.len() < MAX_ALIAS_NAMES {
                existing
                    .alias_names
                    .push(truncated(&symbol.raw, MAX_ALIAS_NAME));
            }
            if symbol.size > existing.size {
                existing.size = symbol.size;
                existing.inferred = symbol.inferred;
            }
            continue;
        }
        if symbol.address != 0
            && let Some(section) = symbol.section
        {
            positions.insert((section, symbol.address), kept.len());
        }
        kept.push(symbol);
    }
    (kept, collapsed)
}

/// Mach-O records no symbol size. Size each size-less text symbol by the
/// distance to the next text symbol in its section, which is the extent
/// `nm`-style tools report, and never past the end of that section. Data
/// symbols are left at their recorded size because the next data symbol can be
/// separated by arbitrarily large anonymous gaps. ELF and PE sizes stay
/// untouched.
fn infer_sizes(symbols: &mut [SymbolRow], sections: &HashMap<usize, (u64, u64)>) {
    let mut by_section: HashMap<usize, Vec<usize>> = HashMap::new();
    for (index, symbol) in symbols.iter().enumerate() {
        if let Some(section) = symbol.section
            && symbol.kind == "Text"
        {
            by_section.entry(section).or_default().push(index);
        }
    }
    for (section, mut indices) in by_section {
        let Some(&(start, end)) = sections.get(&section) else {
            continue;
        };
        indices.sort_by_key(|&index| symbols[index].address);
        for position in 0..indices.len() {
            let current = indices[position];
            let address = symbols[current].address;
            if symbols[current].size > 0 || address < start || address >= end {
                continue;
            }
            let mut next = position + 1;
            while next < indices.len() && symbols[indices[next]].address == address {
                next += 1;
            }
            let limit = indices
                .get(next)
                .map_or(end, |&index| symbols[index].address.min(end));
            if limit > address {
                symbols[current].size = limit - address;
                symbols[current].inferred = true;
            }
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Accounting {
    attributed_file_bytes: u64,
    attributed_vm_bytes: u64,
    linkedit_bytes: u64,
    unattributed_bytes: u64,
    unmapped_symbol_bytes: u64,
}

/// Splits the file into symbol-covered bytes, Mach-O linkedit metadata and the
/// remainder. A symbol is charged at most the file and VM extent of its own
/// section, so zero-fill symbols raise VM attribution only and a section never
/// contributes more file bytes than it holds.
fn accounting(
    file_bytes: u64,
    segments: &[SegmentRow],
    sections: &HashMap<usize, SectionExtent>,
    symbols: &[SymbolRow],
) -> Accounting {
    let linkedit_bytes = segments
        .iter()
        .filter(|segment| segment.name == LINKEDIT_SEGMENT)
        .fold(0u64, |sum, segment| sum.saturating_add(segment.file_size));

    let mut per_section: HashMap<usize, u64> = HashMap::new();
    let mut unmapped_symbol_bytes = 0u64;
    for symbol in symbols {
        if symbol.size == 0 {
            continue;
        }
        match symbol.section {
            Some(index) if sections.contains_key(&index) => {
                let bytes = per_section.entry(index).or_insert(0);
                *bytes = bytes.saturating_add(symbol.size);
            }
            _ => unmapped_symbol_bytes = unmapped_symbol_bytes.saturating_add(symbol.size),
        }
    }

    let mut attributed_file_bytes = 0u64;
    let mut attributed_vm_bytes = 0u64;
    for (index, symbol_bytes) in per_section {
        let Some(extent) = sections.get(&index) else {
            continue;
        };
        if extent.segment == LINKEDIT_SEGMENT {
            unmapped_symbol_bytes = unmapped_symbol_bytes.saturating_add(symbol_bytes);
            continue;
        }
        attributed_file_bytes =
            attributed_file_bytes.saturating_add(symbol_bytes.min(extent.file_size));
        attributed_vm_bytes = attributed_vm_bytes.saturating_add(symbol_bytes.min(extent.vm_size));
    }
    let attributed_file_bytes =
        attributed_file_bytes.min(file_bytes.saturating_sub(linkedit_bytes));
    let unattributed_bytes = file_bytes
        .saturating_sub(attributed_file_bytes)
        .saturating_sub(linkedit_bytes);
    Accounting {
        attributed_file_bytes,
        attributed_vm_bytes,
        linkedit_bytes,
        unattributed_bytes,
        unmapped_symbol_bytes,
    }
}

/// Reads the executable and returns a bounded, self-describing measurement.
pub(crate) fn analyze(pid: u32, executable: &Path, limits: Limits) -> Result<Value, Fault> {
    let bytes = std::fs::read(executable).map_err(|error| {
        unavailable(format!(
            "Cannot read the selected runtime's executable {}: {error}",
            executable.display()
        ))
    })?;
    let file = object::File::parse(bytes.as_slice()).map_err(|error| {
        unavailable(format!(
            "The selected runtime's executable {} is not a readable object file: {error}",
            executable.display()
        ))
    })?;
    let limits = limits.bounded();
    let file_bytes = bytes.len() as u64;

    let mut segments: Vec<SegmentRow> = file
        .segments()
        .enumerate()
        .map(|(index, segment)| segment_row(&segment, index))
        .collect();
    let mut sections: Vec<SectionRow> = file
        .sections()
        .map(|section| section_row(&section, &segments))
        .collect();

    for section in &sections {
        if let Some(index) = section.segment_index
            && let Some(segment) = segments.get_mut(index)
        {
            segment.section_count += 1;
            segment.section_vm_bytes = segment.section_vm_bytes.saturating_add(section.vm_size);
            segment.section_file_bytes =
                segment.section_file_bytes.saturating_add(section.file_size);
            segment.section_zero_fill_bytes = segment
                .section_zero_fill_bytes
                .saturating_add(section.zero_fill_size());
        }
    }

    let segment_count = segments.len();
    let segment_vm_bytes = segments
        .iter()
        .fold(0u64, |sum, segment| sum.saturating_add(segment.vm_size));
    let segment_file_bytes = segments
        .iter()
        .fold(0u64, |sum, segment| sum.saturating_add(segment.file_size));

    let section_count = sections.len();
    let section_vm_bytes = sections
        .iter()
        .fold(0u64, |sum, section| sum.saturating_add(section.vm_size));
    let section_file_bytes = sections
        .iter()
        .fold(0u64, |sum, section| sum.saturating_add(section.file_size));
    let section_zero_fill_bytes = sections.iter().fold(0u64, |sum, section| {
        sum.saturating_add(section.zero_fill_size())
    });
    let section_ranges: HashMap<usize, (u64, u64)> = sections
        .iter()
        .map(|section| (section.index, (section.address, section.end_address())))
        .collect();
    let section_extents: HashMap<usize, SectionExtent> = sections
        .iter()
        .map(|section| {
            (
                section.index,
                SectionExtent {
                    segment: section.segment.clone(),
                    vm_size: section.vm_size,
                    file_size: section.file_size,
                },
            )
        })
        .collect();
    let section_names: HashMap<usize, (String, String)> = sections
        .iter()
        .map(|section| {
            (
                section.index,
                (section.name.clone(), section.segment.clone()),
            )
        })
        .collect();

    let mut symbols = Vec::new();
    for symbol in file.symbols() {
        push_symbol(&symbol, &mut symbols);
    }
    if symbols.is_empty() {
        for symbol in file.dynamic_symbols() {
            push_symbol(&symbol, &mut symbols);
        }
    }
    let symbol_entries = symbols.len();
    let (mut symbols, alias_count) = dedupe_aliases(symbols);
    infer_sizes(&mut symbols, &section_ranges);
    for symbol in &mut symbols {
        if let Some(index) = symbol.section
            && let Some((name, segment)) = section_names.get(&index)
        {
            symbol.section_name = Some(name.clone());
            symbol.segment_name = (!segment.is_empty()).then(|| segment.clone());
        }
    }

    let symbol_count = symbols.len();
    let symbol_bytes = symbols
        .iter()
        .fold(0u64, |sum, symbol| sum.saturating_add(symbol.size));
    let aliased_symbols = symbols.iter().filter(|symbol| symbol.aliases > 0).count();
    let split = accounting(file_bytes, &segments, &section_extents, &symbols);

    let mut groups: HashMap<String, (u64, u64)> = HashMap::new();
    for symbol in &symbols {
        let group = groups.entry(group_name(&symbol.name)).or_insert((0, 0));
        group.0 = group.0.saturating_add(symbol.size);
        group.1 += 1;
    }
    let group_count = groups.len();
    let mut groups: Vec<_> = groups
        .into_iter()
        .map(|(name, (bytes, symbols))| (name, bytes, symbols))
        .collect();
    groups.sort_by(|one, two| two.1.cmp(&one.1).then_with(|| one.0.cmp(&two.0)));

    sections.sort_by(|one, two| {
        two.vm_size
            .cmp(&one.vm_size)
            .then(one.address.cmp(&two.address))
    });
    symbols.sort_by(|one, two| two.size.cmp(&one.size).then(one.address.cmp(&two.address)));
    segments.sort_by(|one, two| {
        two.vm_size
            .cmp(&one.vm_size)
            .then(one.address.cmp(&two.address))
    });

    let segments_truncated = segment_count > limits.segments;
    let sections_truncated = section_count > limits.sections;
    let symbols_truncated = symbol_count > limits.symbols;
    let groups_truncated = group_count > limits.groups;

    let segment_rows: Vec<_> = segments
        .iter()
        .take(limits.segments)
        .map(|segment| {
            json!({
                "index": segment.index,
                "name": segment.name,
                "address": segment.address,
                "end_address": segment.end_address(),
                "vm_size": segment.vm_size,
                "file_size": segment.file_size,
                "offset": segment.file_offset,
                "permissions": segment.permissions,
                "section_count": segment.section_count,
                "section_vm_bytes": segment.section_vm_bytes,
                "section_file_bytes": segment.section_file_bytes,
                "section_zero_fill_bytes": segment.section_zero_fill_bytes,
            })
        })
        .collect();
    let section_rows: Vec<_> = sections
        .iter()
        .take(limits.sections)
        .map(|section| {
            json!({
                "index": section.index,
                "name": section.name,
                "segment": section.segment,
                "segment_index": section.segment_index,
                "kind": section.kind,
                "size": section.vm_size,
                "vm_size": section.vm_size,
                "file_size": section.file_size,
                "zero_fill_size": section.zero_fill_size(),
                "offset": section.offset,
                "address": section.address,
                "end_address": section.end_address(),
            })
        })
        .collect();
    let symbol_rows: Vec<_> = symbols
        .iter()
        .take(limits.symbols)
        .map(|symbol| {
            json!({
                "name": symbol.name,
                "raw": symbol.raw,
                "kind": symbol.kind,
                "size": symbol.size,
                "address": symbol.address,
                "end_address": symbol.end_address(),
                "inferred_size": symbol.inferred,
                "section": symbol.section_name,
                "segment": symbol.segment_name,
                "section_index": symbol.section,
                "aliases": symbol.aliases,
                "alias_names": symbol.alias_names,
            })
        })
        .collect();
    let group_rows: Vec<_> = groups
        .iter()
        .take(limits.groups)
        .map(|(name, bytes, symbols)| json!({"name": name, "bytes": bytes, "symbols": symbols}))
        .collect();

    let mut truncated = Vec::new();
    if segments_truncated {
        truncated.push("segments");
    }
    if sections_truncated {
        truncated.push("sections");
    }
    if symbols_truncated {
        truncated.push("symbols");
    }
    if groups_truncated {
        truncated.push("groups");
    }

    // One flat `json!` literal this wide exceeds serde_json's macro recursion
    // limit, so the report is assembled from its own sections.
    let mut report = json!({
        "available": true,
        "measurement": "native_executable",
        "label": LABEL,
        "attribution": ATTRIBUTION,
        "pid": pid,
        "executable": executable.display().to_string(),
        "format": format!("{:?}", file.format()),
        "kind": format!("{:?}", file.kind()),
        "architecture": format!("{:?}", file.architecture()),
        "file_bytes": file_bytes,
        "segment_count": segment_count,
        "segments_returned": segment_rows.len(),
        "segments_truncated": segments_truncated,
        "segment_file_bytes": segment_file_bytes,
        "segment_vm_bytes": segment_vm_bytes,
        "segments": segment_rows,
    });
    let sections = json!({
        "section_bytes": section_vm_bytes,
        "section_vm_bytes": section_vm_bytes,
        "section_file_bytes": section_file_bytes,
        "section_zero_fill_bytes": section_zero_fill_bytes,
        "section_count": section_count,
        "sections_returned": section_rows.len(),
        "sections_truncated": sections_truncated,
        "sections": section_rows,
    });
    let symbols = json!({
        "symbol_bytes": symbol_bytes,
        "symbol_size_basis": "Symbol sizes come from the object symbol table. Mach-O records no sizes, so a size-less text symbol is sized to the next text symbol in its own section and marked inferred_size; data symbols keep their recorded size. Symbols that share one section and address are aliases: they are reported on one row with an alias count instead of being charged twice.",
        "symbol_count": symbol_count,
        "symbol_entries": symbol_entries,
        "alias_count": alias_count,
        "aliased_symbols": aliased_symbols,
        "symbols_returned": symbol_rows.len(),
        "symbols_truncated": symbols_truncated,
        "symbols": symbol_rows,
    });
    let groups = json!({
        "group_count": group_count,
        "groups_returned": group_rows.len(),
        "groups_truncated": groups_truncated,
        "groups": group_rows,
    });
    let totals = json!({
        "attributed_bytes": split.attributed_file_bytes,
        "attributed_vm_bytes": split.attributed_vm_bytes,
        "linkedit_bytes": split.linkedit_bytes,
        "unattributed_bytes": split.unattributed_bytes,
        "unmapped_symbol_bytes": split.unmapped_symbol_bytes,
        "accounting": {
            "file_bytes": file_bytes,
            "attributed_bytes": split.attributed_file_bytes,
            "attributed_vm_bytes": split.attributed_vm_bytes,
            "linkedit_bytes": split.linkedit_bytes,
            "unattributed_bytes": split.unattributed_bytes,
            "unmapped_symbol_bytes": split.unmapped_symbol_bytes,
            "section_file_bytes": section_file_bytes,
            "section_vm_bytes": section_vm_bytes,
            "section_zero_fill_bytes": section_zero_fill_bytes,
            "segment_file_bytes": segment_file_bytes,
            "segment_vm_bytes": segment_vm_bytes,
            "basis": ACCOUNTING_BASIS,
        },
        "limits": {
            "segments": limits.segments,
            "sections": limits.sections,
            "symbols": limits.symbols,
            "groups": limits.groups,
        },
        "truncated": truncated,
    });
    let object = report.as_object_mut().expect("the report is an object");
    for section in [sections, symbols, groups, totals] {
        object.extend(
            section
                .as_object()
                .expect("each section is an object")
                .clone(),
        );
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_executable() -> PathBuf {
        std::env::current_exe().expect("the test binary has a path")
    }

    fn analyze_current(limits: Limits) -> Value {
        analyze(std::process::id(), &current_executable(), limits).unwrap()
    }

    #[test]
    fn groups_use_the_first_demangled_path_segment() {
        assert_eq!(group_name("cove_ui::Frame::paint"), "cove_ui");
        assert_eq!(
            group_name("<cove_ui::Frame as core::fmt::Debug>::fmt"),
            "cove_ui"
        );
        assert_eq!(group_name("main"), "main");
        assert_eq!(group_name(""), "(unknown)");
    }

    #[test]
    fn the_current_test_executable_yields_real_sections_and_bounded_rows() {
        let value = analyze_current(Limits {
            segments: 3,
            sections: 3,
            symbols: 3,
            groups: 3,
        });
        assert_eq!(value["available"], true);
        assert_eq!(value["measurement"], "native_executable");
        assert_eq!(value["label"], LABEL);
        assert!(value["file_bytes"].as_u64().unwrap() > 0, "file bytes");
        let segments = value["segments"].as_array().unwrap();
        assert!(!segments.is_empty(), "a test executable has segments");
        assert!(segments.len() <= 3);
        assert!(segments[0]["name"].as_str().is_some());
        assert!(segments[0]["vm_size"].as_u64().is_some());
        assert_eq!(
            value["segments_truncated"].as_bool().unwrap(),
            value["segment_count"].as_u64().unwrap() > 3
        );
        let sections = value["sections"].as_array().unwrap();
        assert!(!sections.is_empty(), "a test executable has sections");
        assert!(sections.len() <= 3);
        assert!(sections[0]["name"].as_str().is_some());
        assert!(sections[0]["kind"].as_str().is_some());
        assert!(sections[0]["size"].as_u64().is_some());
        assert!(sections[0]["address"].as_u64().is_some());
        assert_eq!(
            value["sections_truncated"].as_bool().unwrap(),
            value["section_count"].as_u64().unwrap() > 3
        );
        assert!(value["symbols"].as_array().unwrap().len() <= 3);
        assert!(value["groups"].as_array().unwrap().len() <= 3);
        assert!(value["section_count"].as_u64().unwrap() >= sections.len() as u64);
        if value["symbol_count"].as_u64().unwrap() > 0 {
            assert!(
                value["symbol_bytes"].as_u64().unwrap() > 0,
                "a debug test executable has sized symbols"
            );
            assert!(value["symbols"][0]["size"].as_u64().unwrap() > 0);
        }
        assert_eq!(value["accounting"]["basis"], ACCOUNTING_BASIS);
        assert_eq!(
            value["limits"],
            json!({"segments": 3, "sections": 3, "symbols": 3, "groups": 3})
        );
    }

    #[test]
    fn the_current_test_executable_reports_containment_and_balanced_accounting() {
        let value = analyze_current(Limits {
            segments: MAX_SEGMENTS,
            sections: MAX_SECTIONS,
            symbols: MAX_SYMBOLS,
            groups: MAX_GROUPS,
        });
        let file_bytes = value["file_bytes"].as_u64().unwrap();
        let segments = value["segments"].as_array().unwrap();
        assert!(!segments.is_empty(), "segments");
        let mut segment_file = 0u64;
        let mut segment_vm = 0u64;
        for segment in segments {
            let vm_size = segment["vm_size"].as_u64().unwrap();
            let file_size = segment["file_size"].as_u64().unwrap();
            let offset = segment["offset"].as_u64().unwrap();
            let section_vm = segment["section_vm_bytes"].as_u64().unwrap();
            let section_file = segment["section_file_bytes"].as_u64().unwrap();
            assert!(
                offset.saturating_add(file_size) <= file_bytes,
                "{} file range outside the file",
                segment["name"]
            );
            assert!(
                section_file <= file_size,
                "{} sections claim more file bytes than the segment holds",
                segment["name"]
            );
            assert!(
                section_vm <= vm_size,
                "{} sections claim more VM bytes than the segment maps",
                segment["name"]
            );
            assert_eq!(
                segment["section_zero_fill_bytes"].as_u64().unwrap(),
                section_vm.saturating_sub(section_file),
                "{} zero fill",
                segment["name"]
            );
            segment_file = segment_file.saturating_add(file_size);
            segment_vm = segment_vm.saturating_add(vm_size);
        }
        assert_eq!(value["segment_file_bytes"], segment_file);
        assert_eq!(value["segment_vm_bytes"], segment_vm);

        let sections = value["sections"].as_array().unwrap();
        assert!(!sections.is_empty(), "sections");
        let mut section_vm = 0u64;
        let mut section_file = 0u64;
        let mut section_zero_fill = 0u64;
        for section in sections {
            let vm_size = section["vm_size"].as_u64().unwrap();
            let size = section["size"].as_u64().unwrap();
            let file_size = section["file_size"].as_u64().unwrap();
            let zero_fill = section["zero_fill_size"].as_u64().unwrap();
            assert_eq!(vm_size, size, "size keeps its VM meaning");
            assert_eq!(
                section["end_address"],
                section["address"].as_u64().unwrap() + vm_size
            );
            assert_eq!(zero_fill, vm_size.saturating_sub(file_size));
            if file_size <= vm_size {
                assert_eq!(file_size + zero_fill, vm_size, "{}", section["name"]);
            } else {
                assert_eq!(zero_fill, 0, "{}", section["name"]);
            }
            assert!(file_size <= file_bytes);
            if let Some(offset) = section["offset"].as_u64() {
                assert!(offset.saturating_add(file_size) <= file_bytes);
            }
            if section["segment"] == "__TEXT" {
                assert!(
                    section["segment_index"].as_u64().is_some(),
                    "a __TEXT section names its segment"
                );
            }
            section_vm = section_vm.saturating_add(vm_size);
            section_file = section_file.saturating_add(file_size);
            section_zero_fill = section_zero_fill.saturating_add(zero_fill);
        }
        assert_eq!(value["section_bytes"], section_vm);
        assert_eq!(value["section_vm_bytes"], section_vm);
        assert_eq!(value["section_file_bytes"], section_file);
        assert_eq!(value["section_zero_fill_bytes"], section_zero_fill);
        #[cfg(target_os = "macos")]
        {
            assert!(
                sections
                    .iter()
                    .any(|section| section["segment"] == "__TEXT")
            );
            assert!(
                segments
                    .iter()
                    .any(|segment| segment["name"] == "__LINKEDIT"),
                "a Mach-O executable has a __LINKEDIT segment"
            );
            assert!(
                value["linkedit_bytes"].as_u64().unwrap() > 0,
                "Mach-O linkage metadata is reported separately"
            );
        }
        let attributed = value["attributed_bytes"].as_u64().unwrap();
        let linkedit = value["linkedit_bytes"].as_u64().unwrap();
        let unattributed = value["unattributed_bytes"].as_u64().unwrap();
        assert_eq!(attributed + linkedit + unattributed, file_bytes);
        if value["symbol_count"] == 0 {
            assert_eq!(
                attributed, 0,
                "a symbol-free image attributes no file bytes"
            );
        } else {
            assert!(attributed > 0, "the symbol table covers real bytes");
        }
        assert_eq!(value["accounting"]["attributed_bytes"], attributed);
        assert_eq!(value["accounting"]["linkedit_bytes"], linkedit);
        assert_eq!(value["accounting"]["unattributed_bytes"], unattributed);
        assert_eq!(
            value["symbol_entries"].as_u64().unwrap(),
            value["symbol_count"].as_u64().unwrap() + value["alias_count"].as_u64().unwrap()
        );
        assert!(
            value["aliased_symbols"].as_u64().unwrap() <= value["alias_count"].as_u64().unwrap()
        );
        if value["alias_count"].as_u64().unwrap() == 0 {
            assert_eq!(value["aliased_symbols"], 0);
        }
        for symbol in value["symbols"].as_array().unwrap() {
            let address = symbol["address"].as_u64().unwrap();
            let size = symbol["size"].as_u64().unwrap();
            assert_eq!(symbol["end_address"], address + size);
            assert!(
                symbol["aliases"].as_u64().unwrap() + 1
                    >= symbol["alias_names"].as_array().unwrap().len() as u64
            );
        }
    }

    #[test]
    fn truncation_bounds_segments_sections_symbols_and_groups() {
        let value = analyze_current(Limits {
            segments: 1,
            sections: 1,
            symbols: 1,
            groups: 1,
        });
        for (name, count, truncated, returned) in [
            (
                "segments",
                "segment_count",
                "segments_truncated",
                "segments_returned",
            ),
            (
                "sections",
                "section_count",
                "sections_truncated",
                "sections_returned",
            ),
            (
                "symbols",
                "symbol_count",
                "symbols_truncated",
                "symbols_returned",
            ),
            (
                "groups",
                "group_count",
                "groups_truncated",
                "groups_returned",
            ),
        ] {
            let rows = value[name].as_array().unwrap();
            assert!(rows.len() <= 1, "{name}");
            assert_eq!(value[returned].as_u64().unwrap(), rows.len() as u64);
            assert_eq!(
                value[truncated].as_bool().unwrap(),
                value[count].as_u64().unwrap() > rows.len() as u64
            );
            if value[truncated].as_bool().unwrap() {
                assert!(
                    value["truncated"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|entry| *entry == name),
                    "{name} is listed in the truncated array"
                );
            }
        }
        let value = analyze_current(Limits {
            segments: MAX_SEGMENTS,
            sections: MAX_SECTIONS,
            symbols: MAX_SYMBOLS,
            groups: MAX_GROUPS,
        });
        let sections = value["sections"].as_array().unwrap();
        if !value["sections_truncated"].as_bool().unwrap() {
            assert_eq!(
                sections
                    .iter()
                    .map(|section| section["size"].as_u64().unwrap())
                    .sum::<u64>(),
                value["section_bytes"].as_u64().unwrap()
            );
        }
    }

    fn symbol(
        name: &str,
        kind: &str,
        address: u64,
        size: u64,
        section: Option<usize>,
    ) -> SymbolRow {
        SymbolRow {
            name: name.into(),
            raw: name.into(),
            kind: kind.into(),
            size,
            address,
            section,
            section_name: None,
            segment_name: None,
            inferred: false,
            aliases: 0,
            alias_names: Vec::new(),
        }
    }

    #[test]
    fn missing_symbol_sizes_are_inferred_within_their_section() {
        let mut symbols = vec![
            symbol("outside", "Text", 5, 0, Some(0)),
            symbol("a", "Text", 10, 0, Some(0)),
            symbol("b", "Text", 30, 0, Some(0)),
            symbol("data", "Data", 40, 0, Some(0)),
            symbol("c", "Text", 50, 4, Some(0)),
            symbol("no_section", "Text", 10, 0, None),
        ];
        let sections = HashMap::from([(0usize, (10u64, 60u64))]);
        infer_sizes(&mut symbols, &sections);
        assert_eq!((symbols[1].size, symbols[1].inferred), (20, true));
        assert_eq!((symbols[2].size, symbols[2].inferred), (20, true));
        assert_eq!(symbols[3].size, 0, "data is not inferred");
        assert_eq!((symbols[4].size, symbols[4].inferred), (4, false));
        assert_eq!(symbols[0].size, 0, "an address outside the section");
        assert_eq!(symbols[5].size, 0, "a symbol without a section");
    }

    #[test]
    fn aliases_at_one_address_are_collapsed_and_counted() {
        let symbols = vec![
            symbol("first", "Text", 0x1000, 0, Some(2)),
            symbol("second", "Text", 0x1000, 0, Some(2)),
            symbol("third", "Text", 0x1000, 8, Some(2)),
            symbol("next", "Text", 0x1010, 0, Some(2)),
            symbol("other_section", "Text", 0x1000, 0, Some(3)),
            symbol("absolute", "Unknown", 0x1000, 0, None),
            symbol("absolute_again", "Unknown", 0x1000, 0, None),
            symbol("zero_address", "Unknown", 0, 0, Some(4)),
            symbol("zero_address_alias", "Unknown", 0, 0, Some(4)),
        ];
        let (mut kept, collapsed) = dedupe_aliases(symbols);
        assert_eq!(collapsed, 2, "two names folded onto the first address");
        assert_eq!(kept.len(), 7);
        assert_eq!(kept[0].name, "first");
        assert_eq!(kept[0].aliases, 2);
        assert_eq!(kept[0].size, 8, "the largest recorded size wins");
        assert_eq!(
            kept[0].alias_names,
            vec!["second".to_owned(), "third".to_owned()]
        );
        assert_eq!(kept[3].aliases, 0, "symbols without a section are kept");
        assert_eq!(kept[4].aliases, 0, "the same name without a section");
        assert_eq!(kept[5].aliases, 0, "address 0 is not treated as aliasable");
        assert_eq!(kept[6].aliases, 0, "address 0 aliases stay separate");

        let sections = HashMap::from([(2usize, (0x1000u64, 0x1100u64))]);
        infer_sizes(&mut kept, &sections);
        assert_eq!(kept[0].size, 8, "a recorded alias size is kept");
        assert_eq!(kept[1].size, 0xF0, "the next text symbol reaches the end");
    }

    #[test]
    fn accounting_charges_file_vm_and_linkedit_bytes_separately() {
        let segments = vec![
            SegmentRow {
                index: 0,
                name: "__TEXT".into(),
                address: 0x1000,
                vm_size: 600,
                file_offset: 0,
                file_size: 600,
                permissions: "R-X".into(),
                section_count: 1,
                section_vm_bytes: 500,
                section_file_bytes: 500,
                section_zero_fill_bytes: 0,
            },
            SegmentRow {
                index: 1,
                name: "__DATA".into(),
                address: 0x2000,
                vm_size: 200,
                file_offset: 600,
                file_size: 100,
                permissions: "RW-".into(),
                section_count: 2,
                section_vm_bytes: 200,
                section_file_bytes: 100,
                section_zero_fill_bytes: 100,
            },
            SegmentRow {
                index: 2,
                name: "__LINKEDIT".into(),
                address: 0x3000,
                vm_size: 200,
                file_offset: 700,
                file_size: 200,
                permissions: "R--".into(),
                section_count: 1,
                section_vm_bytes: 200,
                section_file_bytes: 200,
                section_zero_fill_bytes: 0,
            },
        ];
        let sections = HashMap::from([
            (
                0usize,
                SectionExtent {
                    segment: "__TEXT".into(),
                    vm_size: 500,
                    file_size: 500,
                },
            ),
            (
                1usize,
                SectionExtent {
                    segment: "__DATA".into(),
                    vm_size: 100,
                    file_size: 100,
                },
            ),
            (
                2usize,
                SectionExtent {
                    segment: "__DATA".into(),
                    vm_size: 100,
                    file_size: 0,
                },
            ),
            (
                3usize,
                SectionExtent {
                    segment: "__LINKEDIT".into(),
                    vm_size: 200,
                    file_size: 200,
                },
            ),
        ]);
        let symbols = vec![
            symbol("text_over", "Text", 0x1000, 900, Some(0)),
            symbol("data", "Data", 0x2100, 80, Some(1)),
            symbol("bss", "Data", 0x2200, 30, Some(2)),
            symbol("unmapped", "Unknown", 0x3000, 10, None),
            symbol("linkedit", "Unknown", 0x3100, 10, Some(3)),
            symbol("zero", "Unknown", 0x3200, 0, Some(3)),
        ];
        let split = accounting(1000, &segments, &sections, &symbols);
        assert_eq!(split.attributed_file_bytes, 580);
        assert_eq!(split.attributed_vm_bytes, 610);
        assert_eq!(split.linkedit_bytes, 200);
        assert_eq!(split.unattributed_bytes, 220);
        assert_eq!(split.unmapped_symbol_bytes, 20);
        assert_eq!(
            split.attributed_file_bytes + split.linkedit_bytes + split.unattributed_bytes,
            1000
        );

        let no_linkedit = accounting(
            1000,
            &segments[..2],
            &sections,
            &[symbol("text", "Text", 0x1000, 400, Some(0))],
        );
        assert_eq!(no_linkedit.linkedit_bytes, 0, "no __LINKEDIT segment");
        assert_eq!(no_linkedit.attributed_file_bytes, 400);
        assert_eq!(no_linkedit.unattributed_bytes, 600);
    }

    #[test]
    fn process_id_resolves_to_the_current_executable_where_the_os_allows_it() {
        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        {
            let resolved = executable(std::process::id()).unwrap();
            let resolved = std::fs::canonicalize(resolved).unwrap();
            let current = std::fs::canonicalize(current_executable()).unwrap();
            assert_eq!(resolved, current);
        }
    }

    #[test]
    fn an_invalid_executable_is_explicitly_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("not-an-executable");
        std::fs::write(&path, b"not an object file, not even close").unwrap();
        let fault = analyze(std::process::id(), &path, Limits::default()).unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(
            fault.message.contains("not a readable object file"),
            "{}",
            fault.message
        );
        let missing = directory.path().join("missing");
        let fault = analyze(std::process::id(), &missing, Limits::default()).unwrap_err();
        assert_eq!(fault.code, "unavailable");
        assert!(fault.message.contains("Cannot read"), "{}", fault.message);
    }

    #[test]
    fn symbol_names_are_demangled() {
        let demangled = demangle("_ZN4core3fmt5write17h1234567890abcdefE");
        assert_eq!(demangled, "core::fmt::write");
        assert_eq!(demangle("memcpy"), "memcpy");
    }
}
