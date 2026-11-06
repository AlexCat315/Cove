//! Translation of `System/Sources/CoveProcess/Platform.swift`.
use os_pipe::PipeWriter;
use std::io::{self, Write};

pub enum ProcessPlatform {}

impl ProcessPlatform {
    /// Prevents `SIGPIPE` delivery for the parent end of stdin on macOS.
    pub fn prepare_input(_handle: &PipeWriter) {
        #[cfg(target_os = "macos")]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: the writer owns a live descriptor for the duration of the call.
            unsafe {
                libc::fcntl(_handle.as_raw_fd(), libc::F_SETNOSIGPIPE, 1);
            }
        }
    }

    /// Linux blocks `SIGPIPE` for the write and consumes a signal it caused, so
    /// a closed child never terminates the host process.
    pub fn write(data: &[u8], handle: &mut PipeWriter) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: sigset values are initialized before use; the previous
            // mask is restored before returning.
            unsafe {
                let mut blocked: libc::sigset_t = std::mem::zeroed();
                let mut original: libc::sigset_t = std::mem::zeroed();
                let mut pending: libc::sigset_t = std::mem::zeroed();
                libc::sigemptyset(&mut blocked);
                libc::sigaddset(&mut blocked, libc::SIGPIPE);
                libc::sigpending(&mut pending);
                let already_pending = libc::sigismember(&pending, libc::SIGPIPE) == 1;
                libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut original);
                let result = handle.write_all(data);
                if !already_pending && libc::sigismember(&original, libc::SIGPIPE) != 1 {
                    let timeout = libc::timespec {
                        tv_sec: 0,
                        tv_nsec: 0,
                    };
                    libc::sigtimedwait(&blocked, std::ptr::null_mut(), &timeout);
                }
                libc::pthread_sigmask(libc::SIG_SETMASK, &original, std::ptr::null_mut());
                result
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            handle.write_all(data)
        }
    }
}
