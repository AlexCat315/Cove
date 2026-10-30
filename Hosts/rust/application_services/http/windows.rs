//! Windows image transport built on WinHTTP.
//!
//! WinHTTP is the system HTTP stack, so proxy configuration, certificate trust
//! and HTTP/2 all come from the operating system. The bindings are declared
//! directly against `winhttp.dll`, which every supported Windows release ships.

#![cfg(target_os = "windows")]

use cove_ui_compose::primitives::media::image_data_download::{
    AsyncImageError, ImageDownloadTransport,
};
use std::ffi::c_void;

pub const SCHEME_HTTP: bool = true;
pub const SCHEME_HTTPS: bool = true;

pub struct WindowsImageTransport {
    timeout_ms: i32,
}

impl Default for WindowsImageTransport {
    fn default() -> Self {
        Self { timeout_ms: 15_000 }
    }
}

impl WindowsImageTransport {
    pub fn new(timeout_seconds: f64) -> Self {
        let seconds = if timeout_seconds.is_finite() {
            timeout_seconds.clamp(0.1, 300.0)
        } else {
            15.0
        };
        Self { timeout_ms: (seconds * 1000.0) as i32 }
    }
}

impl ImageDownloadTransport for WindowsImageTransport {
    fn fetch(&self, url: &str, limit: usize) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        runtime::fetch(url, limit, self.timeout_ms)
    }
}

mod runtime {
    use super::*;
    use super::super::core::{self, TransportIoError, Url};

    type Handle = *mut c_void;

    #[link(name = "winhttp")]
    unsafe extern "system" {
        fn WinHttpOpen(
            agent: *const u16,
            access_type: u32,
            proxy: *const u16,
            bypass: *const u16,
            flags: u32,
        ) -> Handle;
        fn WinHttpCloseHandle(handle: Handle) -> i32;
        fn WinHttpConnect(session: Handle, server: *const u16, port: u16, reserved: u32) -> Handle;
        fn WinHttpOpenRequest(
            connect: Handle,
            verb: *const u16,
            object: *const u16,
            version: *const u16,
            referrer: *const u16,
            accept_types: *const *const u16,
            flags: u32,
        ) -> Handle;
        fn WinHttpSendRequest(
            request: Handle,
            headers: *const u16,
            headers_length: u32,
            optional: *mut c_void,
            optional_length: u32,
            total_length: u32,
            context: usize,
        ) -> i32;
        fn WinHttpReceiveResponse(request: Handle, reserved: *mut c_void) -> i32;
        fn WinHttpQueryHeaders(
            request: Handle,
            info_level: u32,
            name: *const u16,
            buffer: *mut c_void,
            buffer_length: *mut u32,
            index: *mut u32,
        ) -> i32;
        fn WinHttpQueryDataAvailable(request: Handle, available: *mut u32) -> i32;
        fn WinHttpReadData(request: Handle, buffer: *mut c_void, to_read: u32, read: *mut u32)
            -> i32;
        fn WinHttpSetTimeouts(
            handle: Handle,
            resolve: i32,
            connect: i32,
            send: i32,
            receive: i32,
        ) -> i32;
    }

    /// `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY`
    const ACCESS_AUTOMATIC_PROXY: u32 = 4;
    /// `WINHTTP_FLAG_SECURE`
    const FLAG_SECURE: u32 = 0x0080_0000;
    /// `WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER`
    const QUERY_STATUS_CODE: u32 = 19 | 0x2000_0000;
    /// `WINHTTP_QUERY_CONTENT_TYPE`
    const QUERY_CONTENT_TYPE: u32 = 5;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn fetch(
        url: &str,
        limit: usize,
        timeout_ms: i32,
    ) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        let parsed = Url::parse(url)?;
        let agent = wide("CoveHost/1");
        // SAFETY: the agent string is NUL terminated and the remaining arguments
        // are documented nulls/constants for the automatic proxy configuration.
        let session = unsafe {
            WinHttpOpen(
                agent.as_ptr(),
                ACCESS_AUTOMATIC_PROXY,
                std::ptr::null(),
                std::ptr::null(),
                0,
            )
        };
        if session.is_null() {
            return Err(AsyncImageError::HostUnavailable);
        }
        let result = fetch_with_session(session, &parsed, limit, timeout_ms);
        // SAFETY: `session` came from `WinHttpOpen` and is closed exactly once.
        unsafe { WinHttpCloseHandle(session) };
        result
    }

    fn fetch_with_session(
        session: Handle,
        url: &Url,
        limit: usize,
        timeout_ms: i32,
    ) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        // SAFETY: `session` is a live WinHTTP session handle.
        unsafe { WinHttpSetTimeouts(session, timeout_ms, timeout_ms, timeout_ms, timeout_ms) };

        let host = wide(&url.host);
        // SAFETY: the host string is NUL terminated and `session` is live.
        let connect = unsafe { WinHttpConnect(session, host.as_ptr(), url.port, 0) };
        if connect.is_null() {
            return Err(AsyncImageError::HostUnavailable);
        }
        struct Close(Handle);
        impl Drop for Close {
            fn drop(&mut self) {
                // SAFETY: this handle came from a WinHTTP open call and is closed
                // exactly once by its owner.
                unsafe { WinHttpCloseHandle(self.0) };
            }
        }
        let _connect = Close(connect);

        let verb = wide("GET");
        let object = wide(&url.path);
        // SAFETY: all strings are NUL terminated, the verb is GET, and HTTPS is a
        // request flag rather than a separate call.
        let request = unsafe {
            WinHttpOpenRequest(
                connect,
                verb.as_ptr(),
                object.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0 | if url.secure { FLAG_SECURE } else { 0 },
            )
        };
        if request.is_null() {
            return Err(AsyncImageError::HostUnavailable);
        }
        let _request = Close(request);

        // SAFETY: the request handle is live and no optional body is supplied.
        let sent = unsafe {
            WinHttpSendRequest(
                request,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                0,
                0,
            )
        };
        if sent == 0 {
            return Err(TransportIoError::network("WinHttpSendRequest failed"));
        }
        // SAFETY: the request handle is live.
        if unsafe { WinHttpReceiveResponse(request, std::ptr::null_mut()) } == 0 {
            return Err(TransportIoError::network("WinHttpReceiveResponse failed"));
        }

        let status = query_number(request, QUERY_STATUS_CODE).unwrap_or(0);
        core::validate_status(u16::try_from(status).unwrap_or(0))?;
        let content_type = query_string(request, QUERY_CONTENT_TYPE);

        let mut body = Vec::new();
        loop {
            let mut available = 0_u32;
            // SAFETY: the request handle is live and `available` is a valid
            // out-parameter.
            if unsafe { WinHttpQueryDataAvailable(request, &mut available) } == 0 {
                return Err(TransportIoError::network("WinHttpQueryDataAvailable failed"));
            }
            if available == 0 {
                break;
            }
            if body.len() + available as usize > limit {
                return Err(AsyncImageError::DownloadTooLarge);
            }
            let mut chunk = vec![0_u8; available as usize];
            let mut read = 0_u32;
            // SAFETY: `chunk` has `available` writable bytes and `read` is a valid
            // out-parameter.
            if unsafe {
                WinHttpReadData(request, chunk.as_mut_ptr().cast(), available, &mut read)
            } == 0
            {
                return Err(TransportIoError::network("WinHttpReadData failed"));
            }
            chunk.truncate(read as usize);
            body.extend_from_slice(&chunk);
            if body.len() > limit {
                return Err(AsyncImageError::DownloadTooLarge);
            }
        }
        Ok((body, content_type))
    }

    fn query_number(request: Handle, level: u32) -> Option<u32> {
        let mut value = 0_u32;
        let mut length = std::mem::size_of::<u32>() as u32;
        // SAFETY: `value` is a `u32` out-parameter of the declared size, and the
        // numeric query levels write exactly that width.
        let ok = unsafe {
            WinHttpQueryHeaders(
                request,
                level,
                std::ptr::null(),
                (&mut value as *mut u32).cast(),
                &mut length,
                std::ptr::null_mut(),
            )
        };
        (ok != 0).then_some(value)
    }

    fn query_string(request: Handle, level: u32) -> Option<String> {
        let mut length = 0_u32;
        // SAFETY: a null buffer with a zero length is the documented size query.
        unsafe {
            WinHttpQueryHeaders(
                request,
                level,
                std::ptr::null(),
                std::ptr::null_mut(),
                &mut length,
                std::ptr::null_mut(),
            )
        };
        if length == 0 {
            return None;
        }
        let mut buffer = vec![0_u16; (length as usize).div_ceil(2)];
        // SAFETY: `buffer` has `length` writable bytes.
        let ok = unsafe {
            WinHttpQueryHeaders(
                request,
                level,
                std::ptr::null(),
                buffer.as_mut_ptr().cast(),
                &mut length,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return None;
        }
        let units = length as usize / 2;
        let text = String::from_utf16_lossy(&buffer[..units.min(buffer.len())]);
        Some(text.trim_end_matches('\0').to_owned())
    }
}
