//! Windows backend: a ConPTY pseudo console (Windows 10 1809+).
//!
//! ConPTY keeps its output pipe open after the program exits, so a watcher
//! thread waits for the process, closes the console, and the reader thread then
//! drains the remaining output and sees end-of-file.

#![cfg(windows)]

use super::command::{TerminalCommand, TerminalError};
use super::windows_command_line;
use super::PseudoTerminalSize;
use std::{
    ffi::c_void,
    os::windows::ffi::OsStrExt,
    ptr,
    sync::{Arc, Mutex},
};

type Handle = *mut c_void;
type Hpcon = *mut c_void;

const S_OK: i32 = 0;
const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
const EXTENDED_STARTUPINFO_PRESENT: u32 = 0x0008_0000;
const CREATE_UNICODE_ENVIRONMENT: u32 = 0x0000_0400;
const INFINITE: u32 = 0xFFFF_FFFF;
/// `ProcThreadAttributeValue(22, FALSE, TRUE, FALSE)`; the macro is not
/// importable, so the documented value is used directly.
const PSEUDO_CONSOLE_ATTRIBUTE: usize = 0x0002_0016;

#[repr(C)]
struct StartupInfo {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    x_size: u32,
    y_size: u32,
    x_count_chars: u32,
    y_count_chars: u32,
    fill_attribute: u32,
    flags: u32,
    show_window: u16,
    reserved2_size: u16,
    reserved2: *mut u8,
    std_in: Handle,
    std_out: Handle,
    std_err: Handle,
}

#[repr(C)]
struct StartupInfoEx {
    startup_info: StartupInfo,
    attribute_list: *mut c_void,
}

#[repr(C)]
struct ProcessInformation {
    process: Handle,
    thread: Handle,
    process_id: u32,
    thread_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Coord {
    x: i16,
    y: i16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreatePipe(read: *mut Handle, write: *mut Handle, attributes: *mut c_void, size: u32) -> i32;
    fn CloseHandle(handle: Handle) -> i32;
    fn ReadFile(handle: Handle, buffer: *mut c_void, length: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
    fn WriteFile(handle: Handle, buffer: *const c_void, length: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
    fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
    fn GetExitCodeProcess(process: Handle, code: *mut u32) -> i32;
    fn GetLastError() -> u32;
    fn InitializeProcThreadAttributeList(list: *mut c_void, count: u32, flags: u32, size: *mut usize) -> i32;
    fn UpdateProcThreadAttribute(
        list: *mut c_void,
        flags: u32,
        attribute: usize,
        value: *const c_void,
        size: usize,
        previous: *mut c_void,
        return_size: *mut usize,
    ) -> i32;
    fn DeleteProcThreadAttributeList(list: *mut c_void);
    fn CreateProcessW(
        application: *const u16,
        command_line: *mut u16,
        process_attributes: *mut c_void,
        thread_attributes: *mut c_void,
        inherit_handles: i32,
        flags: u32,
        environment: *const c_void,
        directory: *const u16,
        startup_info: *mut StartupInfo,
        information: *mut ProcessInformation,
    ) -> i32;
    fn CreatePseudoConsole(size: Coord, input: Handle, output: Handle, flags: u32, console: *mut Hpcon) -> i32;
    fn ResizePseudoConsole(console: Hpcon, size: Coord) -> i32;
    fn ClosePseudoConsole(console: Hpcon);
}

pub(super) struct ConPty {
    pub process_id: i32,
    state: Arc<ConPtyState>,
}

struct ConPtyState {
    console: Mutex<Option<Hpcon>>,
    input: Handle,
    process: Handle,
}

// SAFETY: the handles are owned by this state and every use is serialised; the
// console handle is behind a mutex so its close is never concurrent with a use.
unsafe impl Send for ConPtyState {}
unsafe impl Sync for ConPtyState {}

impl ConPty {
    pub(super) fn spawn(
        command: &TerminalCommand,
        size: PseudoTerminalSize,
        on_output: super::OutputHandler,
        on_exit: super::ExitHandler,
    ) -> Result<Self, TerminalError> {
        let environment = command.resolved_environment();
        let path = command
            .resolved_executable_with(
                environment.get("Path").or_else(|| environment.get("PATH")).map(String::as_str),
                std::env::var("PATHEXT").ok().as_deref(),
            )
            .ok_or_else(|| TerminalError::ExecutableNotFound(command.executable.clone()))?;

        // SAFETY: every handle produced here is either closed before return or
        // stored in the returned value, and every pointer argument is live.
        unsafe {
            let mut input_read: Handle = ptr::null_mut();
            let mut input_write: Handle = ptr::null_mut();
            let mut output_read: Handle = ptr::null_mut();
            let mut output_write: Handle = ptr::null_mut();
            if CreatePipe(&mut input_read, &mut input_write, ptr::null_mut(), 0) == 0
                || CreatePipe(&mut output_read, &mut output_write, ptr::null_mut(), 0) == 0
            {
                return Err(TerminalError::SpawnFailed(GetLastError() as i32));
            }
            // Keep the console's pipe ends alive until the client is attached.
            let mut console: Hpcon = ptr::null_mut();
            let created = CreatePseudoConsole(coord(size), input_read, output_write, 0, &mut console);
            if created != S_OK || console.is_null() {
                CloseHandle(input_read);
                CloseHandle(input_write);
                CloseHandle(output_read);
                CloseHandle(output_write);
                return Err(TerminalError::SpawnFailed(created));
            }
            CloseHandle(input_read);
            CloseHandle(output_write);

            let mut list_size: usize = 0;
            InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut list_size);
            let storage = vec![0u8; list_size];
            let attributes = storage.as_ptr() as *mut c_void;
            let mut failure = None;
            if InitializeProcThreadAttributeList(attributes, 1, 0, &mut list_size) == 0
                || UpdateProcThreadAttribute(
                    attributes,
                    0,
                    PSEUDO_CONSOLE_ATTRIBUTE,
                    console,
                    std::mem::size_of::<Hpcon>(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                ) == 0
            {
                failure = Some(GetLastError() as i32);
            }
            if let Some(error) = failure {
                DeleteProcThreadAttributeList(attributes);
                ClosePseudoConsole(console);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(TerminalError::SpawnFailed(error));
            }

            let mut startup: StartupInfoEx = std::mem::zeroed();
            startup.startup_info.cb = std::mem::size_of::<StartupInfoEx>() as u32;
            // Otherwise Windows duplicates redirected parent stdio even with
            // handle inheritance disabled, bypassing the pseudoconsole.
            startup.startup_info.flags = STARTF_USESTDHANDLES;
            startup.attribute_list = attributes;
            let mut information: ProcessInformation = std::mem::zeroed();
            let launch_line = windows_command_line::command_line_string(
                &std::iter::once(path).chain(command.arguments.iter().cloned()).collect::<Vec<_>>(),
            );
            let mut command_line: Vec<u16> = launch_line.encode_utf16().collect();
            command_line.push(0);
            let mut environment_block = windows_command_line::environment_block_utf16(&environment);
            let directory: Option<Vec<u16>> = command.working_directory.as_ref().map(|path| {
                let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
                wide.push(0);
                wide
            });
            let flags = EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT;
            let launched = CreateProcessW(
                ptr::null(),
                command_line.as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                flags,
                environment_block.as_mut_ptr().cast(),
                directory.as_ref().map_or(ptr::null(), |wide| wide.as_ptr()),
                &mut startup.startup_info,
                &mut information,
            );
            DeleteProcThreadAttributeList(attributes);
            if launched == 0 || information.process.is_null() {
                let error = GetLastError() as i32;
                ClosePseudoConsole(console);
                CloseHandle(input_write);
                CloseHandle(output_read);
                return Err(TerminalError::SpawnFailed(error));
            }
            if !information.thread.is_null() {
                CloseHandle(information.thread);
            }

            let state = Arc::new(ConPtyState {
                console: Mutex::new(Some(console)),
                input: input_write,
                process: information.process,
            });
            Self::start_reading(state.clone(), output_read, on_output, on_exit);
            Ok(Self { process_id: information.process_id as i32, state })
        }
    }

    fn start_reading(
        state: Arc<ConPtyState>,
        output: Handle,
        on_output: super::OutputHandler,
        on_exit: super::ExitHandler,
    ) {
        let reader_state = state.clone();
        let _ = std::thread::Builder::new().name("cove.terminal.conpty.read".into()).spawn(move || {
            let mut buffer = vec![0u8; 64 * 1024];
            loop {
                let mut count: u32 = 0;
                // SAFETY: the output pipe handle is owned by this reader until
                // it closes it below.
                let ok = unsafe {
                    ReadFile(
                        output,
                        buffer.as_mut_ptr().cast(),
                        buffer.len() as u32,
                        &mut count,
                        ptr::null_mut(),
                    )
                };
                if ok == 0 || count == 0 {
                    break;
                }
                on_output(&buffer[..count as usize]);
            }
            // SAFETY: the handle is closed exactly once, here.
            unsafe { CloseHandle(output) };
            let mut code: u32 = 0;
            // SAFETY: the process handle is held by `reader_state` for the
            // duration of both calls.
            unsafe {
                WaitForSingleObject(reader_state.process, INFINITE);
                GetExitCodeProcess(reader_state.process, &mut code);
            }
            reader_state.close_console();
            on_exit(code as i32);
        });

        let watch_state = state;
        let _ = std::thread::Builder::new().name("cove.terminal.conpty.exit".into()).spawn(move || {
            // Closing emits the final console frame; the reader remains alive
            // until that frame is drained and the pipe reaches EOF.
            // SAFETY: the process handle is held by `watch_state`.
            unsafe { WaitForSingleObject(watch_state.process, INFINITE) };
            watch_state.close_console();
        });
    }

    pub(super) fn write(&self, bytes: &[u8]) {
        if self.state.console.lock().unwrap_or_else(|error| error.into_inner()).is_none() {
            return;
        }
        let mut offset = 0usize;
        while offset < bytes.len() {
            let mut written: u32 = 0;
            // SAFETY: the input pipe handle is owned by the state and the slice
            // is readable for the requested length.
            let ok = unsafe {
                WriteFile(
                    self.state.input,
                    bytes[offset..].as_ptr().cast(),
                    (bytes.len() - offset) as u32,
                    &mut written,
                    ptr::null_mut(),
                )
            };
            if ok == 0 || written == 0 {
                return;
            }
            offset += written as usize;
        }
    }

    pub(super) fn resize(&self, size: PseudoTerminalSize) {
        let guard = self.state.console.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(console) = *guard {
            // SAFETY: the console handle is live while the lock is held.
            unsafe { ResizePseudoConsole(console, coord(size)) };
        }
    }

    /// Closing the console sends console programs CTRL_CLOSE_EVENT, as closing a
    /// console window does.
    pub(super) fn terminate(&self) {
        self.state.close_console();
    }
}

impl ConPtyState {
    fn close_console(&self) {
        let closing = {
            let mut guard = self.console.lock().unwrap_or_else(|error| error.into_inner());
            guard.take()
        };
        if let Some(console) = closing {
            // SAFETY: the console handle was taken out of the slot, so it is
            // closed exactly once.
            unsafe { ClosePseudoConsole(console) };
        }
    }
}

impl Drop for ConPty {
    fn drop(&mut self) {
        // SAFETY: each handle is released once; the console is taken by value.
        unsafe {
            CloseHandle(self.state.input);
            CloseHandle(self.state.process);
        }
        self.state.close_console();
    }
}

fn coord(size: PseudoTerminalSize) -> Coord {
    Coord { x: size.columns.clamp(1, i16::MAX as usize) as i16, y: size.rows.clamp(1, i16::MAX as usize) as i16 }
}
