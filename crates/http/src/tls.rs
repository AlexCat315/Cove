//! Rust side of `native/openssl.c`.
use crate::HttpError;
use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::os::fd::AsRawFd;

unsafe extern "C" {
    fn cove_http_tls_available(error: *mut c_char, length: usize) -> c_int;
    fn cove_http_tls_connect(
        fd: c_int,
        host: *const c_char,
        ip_literal: c_int,
        error: *mut c_char,
        length: usize,
        timed_out: *mut c_int,
    ) -> *mut c_void;
    fn cove_http_tls_read(
        ssl: *mut c_void,
        buffer: *mut c_void,
        length: usize,
        error: *mut c_char,
        error_length: usize,
    ) -> c_long;
    fn cove_http_tls_write(
        ssl: *mut c_void,
        buffer: *const c_void,
        length: usize,
        error: *mut c_char,
        error_length: usize,
    ) -> c_long;
    fn cove_http_tls_close(ssl: *mut c_void);
}

fn message(buffer: &[u8]) -> String {
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    String::from_utf8_lossy(&buffer[..end]).into_owned()
}

/// `Err` names why OpenSSL cannot be used.
pub fn available() -> Result<(), String> {
    let mut error = [0u8; 256];
    // SAFETY: `error` is writable for its length; the call fills a C string.
    let ok = unsafe { cove_http_tls_available(error.as_mut_ptr().cast(), error.len()) };
    if ok == 1 {
        Ok(())
    } else {
        Err(message(&error))
    }
}

/// A TLS session that owns its socket.
pub struct TlsStream {
    ssl: *mut c_void,
    _socket: TcpStream,
}

// SAFETY: the `SSL` object is owned by this value alone and OpenSSL permits
// using one `SSL` from any single thread at a time.
unsafe impl Send for TlsStream {}

impl TlsStream {
    pub fn connect(socket: TcpStream, host: &str, ip_literal: bool) -> Result<Self, HttpError> {
        let host = CString::new(host).map_err(|_| HttpError::UnsupportedUrl(host.into()))?;
        let mut error = [0u8; 256];
        let mut timed_out = 0;
        // SAFETY: the descriptor stays open for the lifetime of the session (the
        // socket moves into `Self`), `host` is NUL terminated and the out
        // parameters are valid.
        let ssl = unsafe {
            cove_http_tls_connect(
                socket.as_raw_fd(),
                host.as_ptr(),
                c_int::from(ip_literal),
                error.as_mut_ptr().cast(),
                error.len(),
                &mut timed_out,
            )
        };
        if ssl.is_null() {
            return Err(if timed_out != 0 {
                HttpError::Timeout
            } else {
                HttpError::Tls(message(&error))
            });
        }
        Ok(Self {
            ssl,
            _socket: socket,
        })
    }
}

fn result(count: c_long, error: &[u8]) -> io::Result<usize> {
    match count {
        -2 => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "TLS operation timed out",
        )),
        count if count < 0 => Err(io::Error::other(HttpError::Tls(message(error)))),
        count => Ok(count as usize),
    }
}

impl Read for TlsStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut error = [0u8; 256];
        // SAFETY: `ssl` is live and `buffer` is writable for its length.
        let count = unsafe {
            cove_http_tls_read(
                self.ssl,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                error.as_mut_ptr().cast(),
                error.len(),
            )
        };
        result(count, &error)
    }
}

impl Write for TlsStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut error = [0u8; 256];
        // SAFETY: `ssl` is live and `buffer` is readable for its length.
        let count = unsafe {
            cove_http_tls_write(
                self.ssl,
                buffer.as_ptr().cast(),
                buffer.len(),
                error.as_mut_ptr().cast(),
                error.len(),
            )
        };
        result(count, &error)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for TlsStream {
    fn drop(&mut self) {
        // SAFETY: `ssl` came from `cove_http_tls_connect` and is freed once,
        // before the socket closes.
        unsafe { cove_http_tls_close(self.ssl) };
    }
}
