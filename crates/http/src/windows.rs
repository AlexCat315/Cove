//! WinHTTP backend. Redirects are followed by [`crate::redirect`] so the
//! policy matches the other backends.
use crate::redirect;
use crate::{Capabilities, ClientConfig, Headers, HttpError, Request, Response, Url};
use std::ffi::c_void;
use std::io::{self, Read};

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
    fn WinHttpSetOption(handle: Handle, option: u32, buffer: *const c_void, length: u32) -> i32;
    fn WinHttpSetTimeouts(
        handle: Handle,
        resolve: i32,
        connect: i32,
        send: i32,
        receive: i32,
    ) -> i32;
    fn WinHttpSendRequest(
        request: Handle,
        headers: *const u16,
        headers_length: u32,
        optional: *const c_void,
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
    fn WinHttpReadData(request: Handle, buffer: *mut c_void, to_read: u32, read: *mut u32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLastError() -> u32;
}

/// `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY`
const ACCESS_AUTOMATIC_PROXY: u32 = 4;
/// `WINHTTP_FLAG_SECURE`
const FLAG_SECURE: u32 = 0x0080_0000;
/// `WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER`
const QUERY_STATUS_CODE: u32 = 19 | 0x2000_0000;
/// `WINHTTP_QUERY_RAW_HEADERS_CRLF`
const QUERY_RAW_HEADERS_CRLF: u32 = 22;
/// `WINHTTP_OPTION_REDIRECT_POLICY` / `WINHTTP_OPTION_REDIRECT_POLICY_NEVER`
const OPTION_REDIRECT_POLICY: u32 = 88;
const REDIRECT_POLICY_NEVER: u32 = 0;
/// `WINHTTP_OPTION_DISABLE_FEATURE` / `WINHTTP_DISABLE_COOKIES`
const OPTION_DISABLE_FEATURE: u32 = 63;
const DISABLE_COOKIES: u32 = 1;

const ERROR_TIMEOUT: u32 = 12002;
const ERROR_NAME_NOT_RESOLVED: u32 = 12007;
const ERROR_CANNOT_CONNECT: u32 = 12029;
const ERROR_SECURE_FAILURE: u32 = 12175;

struct Owned(Handle);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle came from a WinHTTP open call and is closed once.
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

// SAFETY: WinHTTP handles may be used from any thread; each is owned by one
// value and used by one thread at a time.
unsafe impl Send for Owned {}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn failure(call: &str) -> HttpError {
    // SAFETY: reads this thread's last-error value.
    let code = unsafe { GetLastError() };
    match code {
        ERROR_TIMEOUT => HttpError::Timeout,
        ERROR_SECURE_FAILURE => HttpError::Tls(format!("{call} failed: secure channel error")),
        ERROR_NAME_NOT_RESOLVED | ERROR_CANNOT_CONNECT => {
            HttpError::Network(format!("{call} failed: cannot connect (WinHTTP {code})"))
        }
        _ => HttpError::Network(format!("{call} failed: WinHTTP error {code}")),
    }
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        http: true,
        https: true,
        limitation: None,
    }
}

pub fn send(config: &ClientConfig, request: Request, url: Url) -> Result<Response, HttpError> {
    let agent = wide(&config.user_agent);
    // SAFETY: the agent string is NUL terminated; the rest are documented nulls
    // for the automatic proxy configuration.
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
        return Err(failure("WinHttpOpen"));
    }
    let session = Owned(session);
    let timeout = i32::try_from(config.timeout.as_millis())
        .unwrap_or(i32::MAX)
        .max(1);
    // SAFETY: `session` is a live session handle.
    unsafe { WinHttpSetTimeouts(session.0, timeout, timeout, timeout, timeout) };

    let Request {
        mut method,
        mut headers,
        mut body,
        ..
    } = request;
    let mut url = url;
    let mut redirects = 0;
    loop {
        let (connect, handle) = exchange(&session, &method, &url, &headers, &body)?;
        let status = query_number(handle.0, QUERY_STATUS_CODE)
            .and_then(|status| u16::try_from(status).ok())
            .ok_or_else(|| HttpError::InvalidResponse("no status code".into()))?;
        let response_headers = query_headers(handle.0);
        let follow = redirect::Follow {
            method: &mut method,
            headers: &mut headers,
            body: &mut body,
            redirects: &mut redirects,
        };
        let location = response_headers.get("location");
        match redirect::next(status, location, &url, config.max_redirects, follow)? {
            Some(next) => url = next,
            None => {
                let empty = method == "HEAD" || status == 204 || status == 304;
                let body = Body {
                    request: handle,
                    _connect: connect,
                    _session: session,
                    done: empty,
                };
                return Ok(Response::new(
                    status,
                    response_headers,
                    url.to_string(),
                    Box::new(body),
                ));
            }
        }
    }
}

fn exchange(
    session: &Owned,
    method: &str,
    url: &Url,
    headers: &Headers,
    body: &[u8],
) -> Result<(Owned, Owned), HttpError> {
    let host = wide(&url.host);
    // SAFETY: the host string is NUL terminated and `session` is live.
    let connect = unsafe { WinHttpConnect(session.0, host.as_ptr(), url.port, 0) };
    if connect.is_null() {
        return Err(failure("WinHttpConnect"));
    }
    let connect = Owned(connect);
    let verb = wide(method);
    let object = wide(&url.path);
    // SAFETY: all strings are NUL terminated; HTTPS is a request flag.
    let request = unsafe {
        WinHttpOpenRequest(
            connect.0,
            verb.as_ptr(),
            object.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            if url.secure { FLAG_SECURE } else { 0 },
        )
    };
    if request.is_null() {
        return Err(failure("WinHttpOpenRequest"));
    }
    let request = Owned(request);
    for (option, value) in [
        (OPTION_REDIRECT_POLICY, REDIRECT_POLICY_NEVER),
        (OPTION_DISABLE_FEATURE, DISABLE_COOKIES),
    ] {
        // SAFETY: both options take a `u32` of the given size.
        unsafe {
            WinHttpSetOption(
                request.0,
                option,
                (&value as *const u32).cast(),
                std::mem::size_of::<u32>() as u32,
            )
        };
    }
    let mut block = String::new();
    for (name, value) in headers.iter() {
        block.push_str(&format!("{name}: {value}\r\n"));
    }
    let block = wide(&block);
    let length = u32::try_from(body.len())
        .map_err(|_| HttpError::InvalidRequest("body exceeds 4 GiB".into()))?;
    // SAFETY: `block` is NUL terminated (length -1 means "until NUL") and `body`
    // is readable for `length` bytes for the duration of the call.
    let sent = unsafe {
        WinHttpSendRequest(
            request.0,
            if headers.is_empty() {
                std::ptr::null()
            } else {
                block.as_ptr()
            },
            if headers.is_empty() { 0 } else { u32::MAX },
            if body.is_empty() {
                std::ptr::null()
            } else {
                body.as_ptr().cast()
            },
            length,
            length,
            0,
        )
    };
    if sent == 0 {
        return Err(failure("WinHttpSendRequest"));
    }
    // SAFETY: the request handle is live.
    if unsafe { WinHttpReceiveResponse(request.0, std::ptr::null_mut()) } == 0 {
        return Err(failure("WinHttpReceiveResponse"));
    }
    Ok((connect, request))
}

fn query_number(request: Handle, level: u32) -> Option<u32> {
    let mut value = 0_u32;
    let mut length = std::mem::size_of::<u32>() as u32;
    // SAFETY: numeric query levels write one `u32` into `value`.
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

fn query_headers(request: Handle) -> Headers {
    let mut length = 0_u32;
    // SAFETY: a null buffer with zero length is the documented size query.
    unsafe {
        WinHttpQueryHeaders(
            request,
            QUERY_RAW_HEADERS_CRLF,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
        )
    };
    if length == 0 {
        return Headers::new();
    }
    let mut buffer = vec![0_u16; (length as usize).div_ceil(2)];
    // SAFETY: `buffer` has `length` writable bytes.
    let ok = unsafe {
        WinHttpQueryHeaders(
            request,
            QUERY_RAW_HEADERS_CRLF,
            std::ptr::null(),
            buffer.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Headers::new();
    }
    let units = (length as usize / 2).min(buffer.len());
    String::from_utf16_lossy(&buffer[..units])
        .split("\r\n")
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect()
}

struct Body {
    request: Owned,
    _connect: Owned,
    _session: Owned,
    done: bool,
}

impl Read for Body {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.done || buffer.is_empty() {
            return Ok(0);
        }
        let wanted = u32::try_from(buffer.len()).unwrap_or(u32::MAX);
        let mut read = 0_u32;
        // SAFETY: `buffer` has `wanted` writable bytes and `read` is a valid
        // out parameter; the request handle is live.
        if unsafe {
            WinHttpReadData(
                self.request.0,
                buffer.as_mut_ptr().cast(),
                wanted,
                &mut read,
            )
        } == 0
        {
            return Err(failure("WinHttpReadData").into());
        }
        if read == 0 {
            self.done = true;
        }
        Ok(read as usize)
    }
}
