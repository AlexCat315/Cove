use crate::process::ProcessCommand;
use os_pipe::{PipeReader, PipeWriter};
use std::io;

pub(crate) fn read_pipe(
    pipe: &mut PipeReader,
    buffer: &mut [u8],
    control: &std::sync::atomic::AtomicU8,
) -> io::Result<usize> {
    use std::{io::Read, sync::atomic::Ordering};
    loop {
        if control.load(Ordering::Acquire) != 0 {
            return Ok(0);
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let mut fd = libc::pollfd {
                fd: pipe.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: poll receives one valid descriptor and a live writable pollfd.
            let ready = unsafe { libc::poll(&mut fd, 1, 10) };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if ready == 0 {
                continue;
            }
            return pipe.read(buffer);
        }
        #[cfg(windows)]
        {
            use std::{ffi::c_void, os::windows::io::AsRawHandle};
            #[link(name = "kernel32")]
            unsafe extern "system" {
                fn PeekNamedPipe(
                    pipe: *mut c_void,
                    buffer: *mut c_void,
                    size: u32,
                    read: *mut u32,
                    available: *mut u32,
                    remaining: *mut u32,
                ) -> i32;
            }
            let mut available = 0;
            // SAFETY: only queries a live pipe handle; all optional buffers are null.
            let result = unsafe {
                PeekNamedPipe(
                    pipe.as_raw_handle(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if result == 0 {
                let error = io::Error::last_os_error();
                return if error.kind() == io::ErrorKind::BrokenPipe {
                    Ok(0)
                } else {
                    Err(error)
                };
            }
            if available == 0 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }
            let count = buffer.len().min(available as usize);
            return pipe.read(&mut buffer[..count]);
        }
    }
}

pub(crate) fn write_pipe(
    pipe: &mut PipeWriter,
    mut bytes: &[u8],
    control: &std::sync::atomic::AtomicU8,
) -> io::Result<()> {
    use std::{io::Write, sync::atomic::Ordering};
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let fd = pipe.as_raw_fd();
        // SAFETY: the live writer exclusively owns this descriptor. Its flags are not shared
        // with the child, which owns the read end of this pipe.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: F_SETFL receives the previously returned flags with nonblocking I/O enabled.
        if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    while !bytes.is_empty() {
        if control.load(Ordering::Acquire) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "process cancelled or deadline reached",
            ));
        }
        match pipe.write(bytes) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "process stdin stopped accepting input",
                ));
            }
            Ok(count) => bytes = &bytes[count..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(5))
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) struct Spawned {
    pub child: NativeChild,
    pub input: PipeWriter,
    pub output: PipeReader,
    pub errors: PipeReader,
}

#[cfg(unix)]
pub(crate) struct NativeChild {
    child: std::process::Child,
    pid: i32,
}

#[cfg(unix)]
pub(crate) fn spawn(spec: &ProcessCommand) -> io::Result<Spawned> {
    use std::os::unix::process::CommandExt;
    let (input_read, input) = os_pipe::pipe()?;
    let (output, output_write) = os_pipe::pipe()?;
    let (errors, error_write) = os_pipe::pipe()?;
    let mut command = std::process::Command::new(&spec.executable);
    command
        .args(&spec.arguments)
        .env_clear()
        .envs(&spec.environment)
        .current_dir(&spec.directory)
        .process_group(0)
        .stdin(input_read)
        .stdout(output_write)
        .stderr(error_write);
    let child = command.spawn()?;
    let pid = child.id() as i32;
    drop(command);
    Ok(Spawned {
        child: NativeChild { child, pid },
        input,
        output,
        errors,
    })
}

#[cfg(unix)]
impl NativeChild {
    pub fn pid(&self) -> u32 {
        self.pid as u32
    }
    pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
        use std::os::unix::process::ExitStatusExt;
        Ok(self
            .child
            .try_wait()?
            .map(|s| s.code().unwrap_or(128 + s.signal().unwrap_or(0))))
    }
    pub fn terminate(&mut self) {
        // SAFETY: a positive pid is recorded from this child, which owns its process group.
        unsafe {
            libc::kill(-self.pid, libc::SIGKILL);
        }
        let _ = self.child.kill();
    }
    pub fn wait(&mut self) -> io::Result<i32> {
        use std::os::unix::process::ExitStatusExt;
        let status = self.child.wait()?;
        Ok(status.code().unwrap_or(128 + status.signal().unwrap_or(0)))
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        ffi::{OsStr, c_void},
        os::windows::{ffi::OsStrExt, io::AsRawHandle},
    };
    unsafe extern "C" {
        fn cove_process_start(
            executable: *const u16,
            command: *const u16,
            environment: *const u16,
            directory: *const u16,
            input: *mut c_void,
            output: *mut c_void,
            errors: *mut c_void,
            error: *mut u32,
        ) -> *mut c_void;
        fn cove_process_pid(value: *mut c_void) -> i32;
        fn cove_process_running(value: *mut c_void) -> i32;
        fn cove_process_wait(value: *mut c_void) -> i32;
        fn cove_process_terminate(value: *mut c_void);
        fn cove_process_destroy(value: *mut c_void);
    }
    pub(crate) struct NativeChild {
        handle: *mut c_void,
    }
    // SAFETY: the handle is moved to one supervisor thread and accessed only there.
    unsafe impl Send for NativeChild {}

    fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
        let mut result: Vec<_> = value.encode_wide().collect();
        if result.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "NUL in process argument",
            ));
        }
        result.push(0);
        Ok(result)
    }
    /// Microsoft CRT escaping, including backslashes before quotes and a trailing backslash.
    fn quote(value: &OsStr) -> io::Result<Vec<u16>> {
        let value = wide(value)?;
        let mut result = vec![34];
        let mut slashes = 0;
        for &unit in &value[..value.len() - 1] {
            if unit == 92 {
                slashes += 1;
                continue;
            }
            result.extend(std::iter::repeat_n(
                92,
                slashes * if unit == 34 { 2 } else { 1 },
            ));
            if unit == 34 {
                result.push(92);
            }
            result.push(unit);
            slashes = 0;
        }
        result.extend(std::iter::repeat_n(92, slashes * 2));
        result.push(34);
        Ok(result)
    }
    pub(crate) fn spawn(spec: &ProcessCommand) -> io::Result<Spawned> {
        let (input_read, input) = os_pipe::pipe()?;
        let (output, output_write) = os_pipe::pipe()?;
        let (errors, error_write) = os_pipe::pipe()?;
        let executable = wide(spec.executable.as_os_str())?;
        let directory = wide(spec.directory.as_os_str())?;
        let mut command = quote(spec.executable.as_os_str())?;
        for argument in &spec.arguments {
            command.push(32);
            command.extend(quote(argument)?);
        }
        command.push(0);
        let mut entries: Vec<_> = spec.environment.iter().collect();
        entries.sort_by_key(|(key, _)| key.to_string_lossy().to_uppercase());
        let mut environment = Vec::new();
        for (key, value) in entries {
            if key.is_empty() || key.to_string_lossy().contains('=') {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid environment key",
                ));
            }
            let mut entry = key.clone();
            entry.push("=");
            entry.push(value);
            environment.extend(wide(&entry)?);
        }
        if environment.is_empty() {
            environment.push(0);
        }
        environment.push(0);
        let mut error = 0;
        // SAFETY: all UTF-16 strings and pipe handles are live; C duplicates only the three pipes,
        // assigns the suspended child to a kill-on-close Job Object, then resumes it.
        let handle = unsafe {
            cove_process_start(
                executable.as_ptr(),
                command.as_ptr(),
                environment.as_ptr(),
                directory.as_ptr(),
                input_read.as_raw_handle(),
                output_write.as_raw_handle(),
                error_write.as_raw_handle(),
                &mut error,
            )
        };
        if handle.is_null() {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        Ok(Spawned {
            child: NativeChild { handle },
            input,
            output,
            errors,
        })
    }
    impl NativeChild {
        pub fn pid(&self) -> u32 {
            // SAFETY: valid, exclusively owned native process object.
            unsafe { cove_process_pid(self.handle) as u32 }
        }
        pub fn try_wait(&mut self) -> io::Result<Option<i32>> {
            // SAFETY: the supervisor exclusively owns the live native process object.
            if unsafe { cove_process_running(self.handle) } != 0 {
                Ok(None)
            } else {
                self.wait().map(Some)
            }
        }
        pub fn terminate(&mut self) {
            // SAFETY: valid job/process object, terminated but not destroyed here.
            unsafe {
                cove_process_terminate(self.handle);
            }
        }
        pub fn wait(&mut self) -> io::Result<i32> {
            // SAFETY: valid process handle, kept alive until Drop.
            Ok(unsafe { cove_process_wait(self.handle) })
        }
    }
    impl Drop for NativeChild {
        fn drop(&mut self) {
            // SAFETY: native allocation is released exactly once; closing the job reaps descendants.
            unsafe {
                cove_process_destroy(self.handle);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn crt_quotes_preserve_quotes_and_trailing_slashes() {
            let render =
                |value: &str| String::from_utf16(&quote(OsStr::new(value)).unwrap()).unwrap();
            assert_eq!(render(""), "\"\"");
            assert_eq!(render("a b\\"), "\"a b\\\\\"");
            assert_eq!(render("a\"b"), "\"a\\\"b\"");
        }
    }
}
#[cfg(windows)]
pub(crate) use windows::{NativeChild, spawn};
