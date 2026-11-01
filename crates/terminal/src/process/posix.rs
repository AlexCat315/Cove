//! macOS / Linux backend: `forkpty`, drained through EOF by one reader thread.

#![cfg(unix)]

use super::PseudoTerminalSize;
use super::command::{TerminalCommand, TerminalError, string_c_strings};
use std::{
    ffi::CString,
    os::fd::RawFd,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Owns the master descriptor. Every use happens under the lock, so nothing
/// touches the descriptor (or a later reuse of its number) after `close`.
pub(super) struct Channel {
    lock: Mutex<Option<RawFd>>,
}

impl Channel {
    fn new(descriptor: RawFd) -> Self {
        Self {
            lock: Mutex::new(Some(descriptor)),
        }
    }

    fn write(&self, bytes: &[u8]) {
        let guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        let Some(descriptor) = *guard else {
            return;
        };
        let mut offset = 0usize;
        while offset < bytes.len() {
            // SAFETY: the descriptor is open under the lock and the slice is
            // readable for its length.
            let written = unsafe {
                libc::write(
                    descriptor,
                    bytes[offset..].as_ptr().cast(),
                    bytes.len() - offset,
                )
            };
            if written > 0 {
                offset += written as usize;
                continue;
            }
            if written < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return;
            }
            return;
        }
    }

    fn resize(&self, size: PseudoTerminalSize) {
        let guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        let Some(descriptor) = *guard else {
            return;
        };
        let mut window = window_size(size);
        // SAFETY: the descriptor is open and `window` is a live `winsize`.
        unsafe {
            libc::ioctl(descriptor, libc::TIOCSWINSZ, &mut window);
        }
    }

    /// Signals the child while it is still unreaped, so a recycled pid is never
    /// hit.
    fn signal(&self, process_id: i32, signal: i32) {
        let guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        if guard.is_none() {
            return;
        }
        // SAFETY: the pid was recorded from the child this channel belongs to.
        unsafe { libc::kill(process_id, signal) };
    }

    fn close(&self) {
        let mut guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        let Some(descriptor) = guard.take() else {
            return;
        };
        // SAFETY: this is the only place the descriptor is closed, and it is
        // taken out of the slot first so it cannot be closed twice.
        unsafe { libc::close(descriptor) };
    }
}

pub(super) struct PosixPseudoTerminal {
    pub process_id: i32,
    channel: Arc<Channel>,
}

impl PosixPseudoTerminal {
    pub(super) fn spawn(
        command: &TerminalCommand,
        size: PseudoTerminalSize,
        on_output: super::OutputHandler,
        on_exit: super::ExitHandler,
    ) -> Result<Self, TerminalError> {
        let environment = command.resolved_environment();
        let path = command
            .resolved_executable_with(environment.get("PATH").map(String::as_str), None)
            .ok_or_else(|| TerminalError::ExecutableNotFound(command.executable.clone()))?;
        let executable =
            CString::new(path).map_err(|_| TerminalError::SpawnFailed(libc::EINVAL))?;

        // Everything the child touches is allocated before the fork: between
        // fork and exec only async-signal-safe calls are allowed.
        let arguments: Vec<CString> = string_c_strings(
            &std::iter::once(executable.to_string_lossy().into_owned())
                .chain(command.arguments.iter().cloned())
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>(),
        );
        let mut argv: Vec<*const libc::c_char> =
            arguments.iter().map(|value| value.as_ptr()).collect();
        argv.push(std::ptr::null());
        let environment = super::command::environment_c_strings(&environment);
        let mut envp: Vec<*const libc::c_char> =
            environment.iter().map(|value| value.as_ptr()).collect();
        envp.push(std::ptr::null());
        let directory = command
            .working_directory
            .as_ref()
            .and_then(|path| CString::new(path.as_os_str().as_encoded_bytes()).ok());

        let window = window_size(size);
        let mut master: RawFd = -1;
        // SAFETY: `forkpty` returns twice; in the child only async-signal-safe
        // calls are made before `execve`. Linux takes the size by const pointer,
        // Darwin by mutable pointer.
        #[cfg(target_os = "linux")]
        let pid = unsafe {
            libc::forkpty(
                &mut master,
                std::ptr::null_mut(),
                std::ptr::null::<libc::termios>(),
                &window,
            )
        };
        #[cfg(not(target_os = "linux"))]
        // SAFETY: as above.
        let pid = unsafe {
            let mut window = window;
            libc::forkpty(
                &mut master,
                std::ptr::null_mut(),
                std::ptr::null_mut::<libc::termios>(),
                &mut window,
            )
        };
        if pid < 0 {
            return Err(TerminalError::SpawnFailed(
                std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
            ));
        }
        if pid == 0 {
            // SAFETY: this is the child. The host or test runner may ignore or
            // block these signals; a terminal child needs normal shell and
            // job-control behavior.
            unsafe {
                let mut action: libc::sigaction = std::mem::zeroed();
                action.sa_sigaction = libc::SIG_DFL;
                libc::sigemptyset(&mut action.sa_mask);
                for signal in [
                    libc::SIGHUP,
                    libc::SIGINT,
                    libc::SIGQUIT,
                    libc::SIGTERM,
                    libc::SIGPIPE,
                ] {
                    libc::sigaction(signal, &action, std::ptr::null_mut());
                }
                let mut mask: libc::sigset_t = std::mem::zeroed();
                libc::sigemptyset(&mut mask);
                libc::sigprocmask(libc::SIG_SETMASK, &mask, std::ptr::null_mut());
                if let Some(directory) = directory.as_ref() {
                    libc::chdir(directory.as_ptr());
                }
                libc::execve(executable.as_ptr(), argv.as_ptr(), envp.as_ptr());
                libc::_exit(127);
            }
        }

        // SAFETY: the master descriptor belongs to this process now.
        unsafe {
            libc::fcntl(master, libc::F_SETFD, libc::FD_CLOEXEC);
        }
        let channel = Arc::new(Channel::new(master));
        Self::start_reader(pid, master, channel.clone(), on_output, on_exit);
        Ok(Self {
            process_id: pid,
            channel,
        })
    }

    fn start_reader(
        pid: i32,
        master: RawFd,
        channel: Arc<Channel>,
        on_output: super::OutputHandler,
        on_exit: super::ExitHandler,
    ) {
        // The reader owns one reference; the caller keeps another so a failed
        // spawn can still close the descriptor.
        let reader_channel = channel.clone();
        let spawned = std::thread::Builder::new()
            .name("cove.terminal.pty.read".into())
            .spawn(move || {
                // 64 KiB matches the Swift reader and comfortably exceeds a
                // typical pipe buffer, so a burst of output costs one syscall.
                let mut buffer = vec![0u8; 64 * 1024];
                loop {
                    // SAFETY: the descriptor is open (this thread owns the read
                    // side until it closes it) and the buffer is writable.
                    let count =
                        unsafe { libc::read(master, buffer.as_mut_ptr().cast(), buffer.len()) };
                    if count > 0 {
                        on_output(&buffer[..count as usize]);
                        continue;
                    }
                    if count < 0 {
                        let error = std::io::Error::last_os_error();
                        if error.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                    }
                    break;
                }
                reader_channel.close();
                let status = wait_status(pid);
                on_exit(status);
            });
        if spawned.is_err() {
            // Without a reader the session would hang with no output. The exit
            // handler was consumed by the failed closure, so closing the channel
            // is what lets the session observe that nothing more will arrive.
            channel.close();
        }
    }

    pub(super) fn write(&self, bytes: &[u8]) {
        self.channel.write(bytes);
    }

    pub(super) fn resize(&self, size: PseudoTerminalSize) {
        self.channel.resize(size);
    }

    /// Hang up the program, as closing a terminal window does.
    pub(super) fn terminate(&self) {
        self.channel.signal(self.process_id, libc::SIGHUP);
    }
}

impl Drop for PosixPseudoTerminal {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Reaps the child, retrying through signals, and reports a shell-style status.
fn wait_status(pid: i32) -> i32 {
    let mut status: libc::c_int = 0;
    loop {
        // SAFETY: `pid` is this process's child and `status` is writable.
        let result = unsafe { libc::waitpid(pid, &mut status, 0) };
        if result >= 0 {
            break;
        }
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let signal = status & 0x7F;
    if signal == 0 {
        (status >> 8) & 0xFF
    } else {
        128 + signal
    }
}

pub(super) fn window_size(size: PseudoTerminalSize) -> libc::winsize {
    libc::winsize {
        ws_row: clamp_u16(size.rows),
        ws_col: clamp_u16(size.columns),
        ws_xpixel: clamp_u16(size.columns.saturating_mul(size.cell_width)),
        ws_ypixel: clamp_u16(size.rows.saturating_mul(size.cell_height)),
    }
}

fn clamp_u16(value: usize) -> u16 {
    value.min(u16::MAX as usize) as u16
}
