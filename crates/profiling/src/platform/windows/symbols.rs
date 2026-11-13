//! PDB and DWARF symbolization through dbghelp.
//!
//! dbghelp is process-global and not thread-safe, so every call is serialized
//! and the first symbolizer initializes it while the last one cleans up.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows_sys::Win32::Foundation::{GetLastError, HANDLE};
use windows_sys::Win32::System::Diagnostics::Debug::{
    IMAGEHLP_LINEW64, SYMOPT_DEFERRED_LOADS, SYMOPT_LOAD_LINES, SYMOPT_UNDNAME, SYMBOL_INFOW,
    SymCleanup, SymFromAddrW, SymGetLineFromAddrW64, SymInitializeW, SymRefreshModuleList,
    SymSetOptions,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

use crate::platform::fp::canonical_address;
use crate::{SourceLocation, SymbolizedFrame};

use super::unwind::ADDRESS_MASK;

/// Addresses cached before the symbol cache is reset.
const SYMBOL_CACHE_LIMIT: usize = 65_536;
/// Room for one demangled name, matching dbghelp's own `MAX_SYM_NAME`.
const MAX_SYMBOL_NAME: usize = 2048;
/// Bound on a wide-string scan so a missing terminator cannot run away.
const MAX_PATH_CHARS: usize = 32_768;

static DBGHELP: Mutex<()> = Mutex::new(());
static USERS: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Symbolizer {
    /// Stored as an integer so the symbolizer stays `Send`.
    process: isize,
    cache: HashMap<u64, SymbolizedFrame>,
}

impl Symbolizer {
    pub(crate) fn new() -> Self {
        let process = acquire();
        Self {
            process,
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

    fn resolve_uncached(&self, address: u64) -> SymbolizedFrame {
        let _guard = lock();
        let process = self.process as HANDLE;
        if let Some(frame) = lookup(process, address) {
            return frame;
        }
        // A module may have been loaded after this symbolizer was created.
        // SAFETY: dbghelp is initialized and the lock serializes this call.
        unsafe { SymRefreshModuleList(process) };
        lookup(process, address).unwrap_or(SymbolizedFrame {
            address,
            function: None,
            location: None,
        })
    }
}

impl Drop for Symbolizer {
    fn drop(&mut self) {
        release(self.process as HANDLE);
    }
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    DBGHELP.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Initializes dbghelp for the first symbolizer and returns the process handle.
fn acquire() -> isize {
    let _guard = lock();
    // SAFETY: GetCurrentProcess returns a pseudo-handle for this process.
    let process = unsafe { GetCurrentProcess() };
    if USERS.fetch_add(1, Ordering::AcqRel) == 0 {
        // SAFETY: the option flags are a valid dbghelp bitmask.
        unsafe { SymSetOptions(SYMOPT_DEFERRED_LOADS | SYMOPT_LOAD_LINES | SYMOPT_UNDNAME) };
        // SAFETY: a null search path uses the default; invading the process
        // makes dbghelp enumerate its loaded modules.
        unsafe { SymInitializeW(process, std::ptr::null(), 1) };
    }
    process as isize
}

fn release(process: HANDLE) {
    let _guard = lock();
    if USERS.fetch_sub(1, Ordering::AcqRel) == 1 {
        // SAFETY: this is the last symbolizer, so dbghelp can be torn down.
        unsafe { SymCleanup(process) };
    }
}

/// `SYMBOL_INFOW` ends in a one-element name array; the rest follows it.
#[repr(C)]
struct SymbolBuffer {
    info: SYMBOL_INFOW,
    name: [u16; MAX_SYMBOL_NAME],
}

fn lookup(process: HANDLE, address: u64) -> Option<SymbolizedFrame> {
    let mut buffer = SymbolBuffer {
        // SAFETY: SYMBOL_INFOW is plain old data; the fields dbghelp reads are
        // set below before the call.
        info: unsafe { std::mem::zeroed::<SYMBOL_INFOW>() },
        name: [0; MAX_SYMBOL_NAME],
    };
    buffer.info.SizeOfStruct = std::mem::size_of::<SYMBOL_INFOW>() as u32;
    buffer.info.MaxNameLen = MAX_SYMBOL_NAME as u32;
    let mut displacement = 0_u64;
    // SAFETY: the buffer is large enough for MaxNameLen characters after the
    // fixed struct, and dbghelp is initialized under the lock.
    let found = unsafe {
        SymFromAddrW(
            process,
            address,
            &mut displacement,
            std::ptr::addr_of_mut!(buffer.info),
        )
    } != 0;
    let function = if found {
        let length = (buffer.info.NameLen as usize).min(MAX_SYMBOL_NAME);
        // SAFETY: dbghelp wrote `NameLen` characters starting at `Name`, which
        // is contiguous with the trailing `name` array in this repr(C) buffer.
        let chars = unsafe { std::slice::from_raw_parts(buffer.info.Name.as_ptr(), length) };
        let name = String::from_utf16_lossy(chars);
        (!name.is_empty()).then(|| addr2line::demangle_auto(Cow::Owned(name), None).into_owned())
    } else {
        None
    };

    let mut line = IMAGEHLP_LINEW64 {
        SizeOfStruct: std::mem::size_of::<IMAGEHLP_LINEW64>() as u32,
        Key: std::ptr::null_mut(),
        LineNumber: 0,
        FileName: std::ptr::null_mut(),
        Address: 0,
    };
    let mut line_displacement = 0_u32;
    // SAFETY: `line` is a valid IMAGEHLP_LINEW64 with its size set; dbghelp
    // fills FileName with a pointer it owns.
    let has_line =
        unsafe { SymGetLineFromAddrW64(process, address, &mut line_displacement, &mut line) } != 0;
    let location = if has_line {
        wide_string(line.FileName).map(|file| SourceLocation {
            file,
            line: (line.LineNumber > 0).then_some(line.LineNumber),
        })
    } else {
        None
    };

    if function.is_none() && location.is_none() {
        return None;
    }
    Some(SymbolizedFrame {
        address,
        function,
        location,
    })
}

/// Reads a NUL-terminated UTF-16 string owned by dbghelp.
fn wide_string(pointer: *const u16) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    let mut length = 0;
    while length < MAX_PATH_CHARS {
        // SAFETY: the scan stops at the terminator or at MAX_PATH_CHARS, and
        // dbghelp guarantees a terminated string for a successful lookup.
        if unsafe { *pointer.add(length) } == 0 {
            break;
        }
        length += 1;
    }
    if length == 0 {
        return None;
    }
    // SAFETY: `length` characters were read from the same pointer.
    let chars = unsafe { std::slice::from_raw_parts(pointer, length) };
    Some(String::from_utf16_lossy(chars))
}

/// Reported when dbghelp could not be initialized at all.
pub(crate) fn initialization_error() -> i32 {
    // SAFETY: GetLastError only reads the calling thread's last-error code.
    unsafe { GetLastError() as i32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_string_stops_at_the_terminator() {
        let text = [u16::from(b'c'), u16::from(b'o'), u16::from(b'v'), u16::from(b'e'), 0];
        assert_eq!(wide_string(text.as_ptr()).as_deref(), Some("cove"));
        assert_eq!(wide_string(std::ptr::null()), None);
        let empty = [0_u16];
        assert_eq!(wide_string(empty.as_ptr()), None);
    }

    #[test]
    fn symbol_buffer_keeps_room_for_the_name() {
        let buffer = SymbolBuffer {
            // SAFETY: plain old data, zeroed for a layout assertion only.
            info: unsafe { std::mem::zeroed::<SYMBOL_INFOW>() },
            name: [0; MAX_SYMBOL_NAME],
        };
        let base = std::ptr::addr_of!(buffer.info.Name) as usize;
        let end = std::ptr::addr_of!(buffer.name) as usize;
        assert_eq!(end - base, 2);
    }
}
