//! `NSURLSession` backend; see `native/apple.m`.
#![cfg_attr(cove_http_socket, allow(dead_code))]

use crate::{Capabilities, ClientConfig, Headers, HttpError, Request, Response, Url};
use std::ffi::{CStr, CString, c_char, c_int, c_long, c_void};
use std::io::{self, Read};

unsafe extern "C" {
    fn cove_http_apple_send(
        method: *const c_char,
        url: *const c_char,
        names: *const *const c_char,
        values: *const *const c_char,
        count: usize,
        body: *const u8,
        body_length: usize,
        timeout: f64,
        maximum_redirects: c_long,
        user_agent: *const c_char,
        status: *mut c_int,
        result: *mut c_int,
        error: *mut *mut c_char,
    ) -> *mut c_void;
    fn cove_http_apple_header_count(handle: *mut c_void) -> usize;
    fn cove_http_apple_header_name(handle: *mut c_void, index: usize) -> *mut c_char;
    fn cove_http_apple_header_value(handle: *mut c_void, index: usize) -> *mut c_char;
    fn cove_http_apple_url(handle: *mut c_void) -> *mut c_char;
    fn cove_http_apple_read(
        handle: *mut c_void,
        buffer: *mut u8,
        length: usize,
        error: *mut *mut c_char,
    ) -> c_long;
    fn cove_http_apple_close(handle: *mut c_void);
    fn cove_http_apple_free(text: *mut c_char);
}

const TIMEOUT: c_long = -2;
const REDIRECTS: c_long = -3;
const BAD_URL: c_long = -4;

/// Takes ownership of a malloc'd C string.
fn take(text: *mut c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    // SAFETY: `text` is a NUL terminated string from `apple.m`, freed once here.
    let value = unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned();
    // SAFETY: allocated with `strdup` by `apple.m`.
    unsafe { cove_http_apple_free(text) };
    Some(value)
}

fn error(code: c_long, message: Option<String>, url: &str) -> HttpError {
    match code {
        TIMEOUT => HttpError::Timeout,
        REDIRECTS => HttpError::TooManyRedirects,
        BAD_URL => HttpError::UnsupportedUrl(url.into()),
        _ => HttpError::Network(message.unwrap_or_else(|| "NSURLSession failed".into())),
    }
}

pub fn capabilities() -> Capabilities {
    Capabilities {
        http: true,
        https: true,
        limitation: None,
    }
}

pub fn send(config: &ClientConfig, request: Request, _: Url) -> Result<Response, HttpError> {
    let invalid = |_| HttpError::InvalidRequest("NUL byte".into());
    let method = CString::new(request.method.as_str()).map_err(invalid)?;
    let url = CString::new(request.url.as_str()).map_err(invalid)?;
    let agent = CString::new(config.user_agent.as_str()).map_err(invalid)?;
    let names = request
        .headers
        .iter()
        .map(|(name, _)| CString::new(name))
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    let values = request
        .headers
        .iter()
        .map(|(_, value)| CString::new(value))
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    let name_pointers: Vec<*const c_char> = names.iter().map(|name| name.as_ptr()).collect();
    let value_pointers: Vec<*const c_char> = values.iter().map(|value| value.as_ptr()).collect();
    let mut status = 0;
    let mut result = 0;
    let mut message = std::ptr::null_mut();
    // SAFETY: every pointer is valid and NUL terminated for the call, the header
    // arrays hold `count` entries and the out parameters are writable.
    let handle = unsafe {
        cove_http_apple_send(
            method.as_ptr(),
            url.as_ptr(),
            name_pointers.as_ptr(),
            value_pointers.as_ptr(),
            names.len(),
            request.body.as_ptr(),
            request.body.len(),
            config.timeout.as_secs_f64().max(0.001),
            c_long::try_from(config.max_redirects).unwrap_or(c_long::MAX),
            agent.as_ptr(),
            &mut status,
            &mut result,
            &mut message,
        )
    };
    if handle.is_null() {
        return Err(error(c_long::from(result), take(message), &request.url));
    }
    let body = Body { handle };
    // SAFETY: `handle` is live until `Body` drops.
    let count = unsafe { cove_http_apple_header_count(handle) };
    let mut headers = Headers::new();
    for index in 0..count {
        // SAFETY: `index < count`; each call returns an owned copy.
        let (name, value) = unsafe {
            (
                take(cove_http_apple_header_name(handle, index)),
                take(cove_http_apple_header_value(handle, index)),
            )
        };
        headers.append(name.unwrap_or_default(), value.unwrap_or_default());
    }
    // SAFETY: as above.
    let final_url = take(unsafe { cove_http_apple_url(handle) }).unwrap_or(request.url);
    let status =
        u16::try_from(status).map_err(|_| HttpError::InvalidResponse(status.to_string()))?;
    Ok(Response::new(status, headers, final_url, Box::new(body)))
}

struct Body {
    handle: *mut c_void,
}

// SAFETY: the task object synchronises its buffer with an `NSCondition`; the
// handle is used by one owner at a time.
unsafe impl Send for Body {}

impl Read for Body {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let mut message = std::ptr::null_mut();
        // SAFETY: `handle` is live and `buffer` is writable for its length.
        let count = unsafe {
            cove_http_apple_read(self.handle, buffer.as_mut_ptr(), buffer.len(), &mut message)
        };
        if count >= 0 {
            return Ok(count as usize);
        }
        Err(error(count, take(message), "").into())
    }
}

impl Drop for Body {
    fn drop(&mut self) {
        // SAFETY: releases the handle returned by `cove_http_apple_send` once.
        unsafe { cove_http_apple_close(self.handle) };
    }
}
