//! Apple (macOS/iOS) image transport.
//!
//! Swift's image loader owned a `URLSessionDataTask` delegate. The workspace has
//! no `objc2` dependency, so this backend binds the small Foundation surface it
//! needs through a hand-written runtime FFI: `objc_msgSend` against `NSURL` and
//! `NSData`. That keeps the platform HTTP stack (proxy settings, certificate
//! trust, HTTP/2, redirect policy) in use with no new crate, and it is a real
//! platform fetch rather than a socket re-implemented here.
//!
//! The synchronous `NSData dataWithContentsOfURL:options:error:` entry point is
//! used deliberately: it needs no completion block, so there is no block ABI to
//! hand-encode, and the image loader already runs on a bounded worker rather than
//! the UI thread.

#![cfg(any(target_os = "macos", target_os = "ios"))]

use cove_ui_compose::primitives::media::{AsyncImageError, ImageDownloadTransport};
use std::ffi::{CStr, c_char, c_void};

pub const SCHEME_HTTP: bool = true;
pub const SCHEME_HTTPS: bool = true;

pub struct AppleImageTransport {
    timeout: f64,
}

impl Default for AppleImageTransport {
    fn default() -> Self {
        Self { timeout: 15.0 }
    }
}

impl AppleImageTransport {
    pub fn new(timeout_seconds: f64) -> Self {
        let timeout = if timeout_seconds.is_finite() {
            timeout_seconds.clamp(0.1, 300.0)
        } else {
            15.0
        };
        Self { timeout }
    }
}

impl ImageDownloadTransport for AppleImageTransport {
    fn fetch(&self, url: &str, limit: usize) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        runtime::fetch(url, limit, self.timeout)
    }
}

mod runtime {
    use super::*;

    /// Opaque Objective-C object handle.
    type Id = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_autoreleasePoolPush() -> *mut c_void;
        fn objc_autoreleasePoolPop(pool: *mut c_void);
    }

    // `objc_msgSend` has one physical symbol and one signature per call site;
    // every call below transmutes it to the exact shape that site needs.
    unsafe extern "C" {
        fn objc_msgSend();
    }

    fn class(name: &CStr) -> Option<Id> {
        // SAFETY: `name` is a NUL terminated class name; the runtime returns the
        // registered class or null.
        let value = unsafe { objc_getClass(name.as_ptr()) };
        (!value.is_null()).then_some(value)
    }

    fn selector(name: &CStr) -> *mut c_void {
        // SAFETY: `name` is a NUL terminated selector name; registration is
        // idempotent and thread-safe.
        unsafe { sel_registerName(name.as_ptr()) }
    }

    /// `[NSString stringWithUTF8String:]`
    fn string(text: &str) -> Option<Id> {
        let c_text = std::ffi::CString::new(text).ok()?;
        let class = class(c"NSString")?;
        // SAFETY: `stringWithUTF8String:` takes one `const char *`.
        let send: unsafe extern "C" fn(Id, *mut c_void, *const c_char) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        // SAFETY: the class object and C string are valid for this call.
        let value = unsafe { send(class, selector(c"stringWithUTF8String:"), c_text.as_ptr()) };
        (!value.is_null()).then_some(value)
    }

    /// `[NSData dataWithContentsOfURL:options:error:]`
    ///
    /// `NSDataReadingMappedIfSafe` is intentionally not requested: the bytes are
    /// copied into a `Vec` anyway.
    pub fn fetch(
        url: &str,
        limit: usize,
        timeout: f64,
    ) -> Result<(Vec<u8>, Option<String>), AsyncImageError> {
        let _pool = AutoreleasePool::new();
        if url.len() > 8192 {
            return Err(AsyncImageError::UnsupportedUrl);
        }
        let _ = timeout;
        let url_string = string(url).ok_or(AsyncImageError::UnsupportedUrl)?;
        let ns_url_class = class(c"NSURL").ok_or(AsyncImageError::HostUnavailable)?;
        // SAFETY: `URLWithString:` takes one object.
        let url_send: unsafe extern "C" fn(Id, *mut c_void, Id) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        // SAFETY: the class object and string are valid.
        let ns_url = unsafe { url_send(ns_url_class, selector(c"URLWithString:"), url_string) };
        if ns_url.is_null() {
            return Err(AsyncImageError::UnsupportedUrl);
        }

        let data_class = class(c"NSData").ok_or(AsyncImageError::HostUnavailable)?;
        let mut error: Id = std::ptr::null_mut();
        // SAFETY: `dataWithContentsOfURL:options:error:` takes (object, usize,
        // *mut Id). Passing options = 0 keeps the default read policy.
        let data_send: unsafe extern "C" fn(Id, *mut c_void, Id, usize, *mut Id) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        // SAFETY: `error` is a valid out-parameter for this call and the class
        // object is live.
        let data = unsafe {
            data_send(
                data_class,
                selector(c"dataWithContentsOfURL:options:error:"),
                ns_url,
                0,
                &mut error,
            )
        };
        if data.is_null() {
            let detail = if error.is_null() {
                "NSData could not read the image URL".to_owned()
            } else {
                describe(error)
            };
            return Err(AsyncImageError::NetworkFailed(detail));
        }

        // `[NSData length]`
        // SAFETY: `length` returns `NSUInteger`.
        let length_send: unsafe extern "C" fn(Id, *mut c_void) -> usize =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        // SAFETY: `data` is a live `NSData`.
        let count = unsafe { length_send(data, selector(c"length")) };
        if count > limit {
            return Err(AsyncImageError::DownloadTooLarge);
        }
        if count == 0 {
            return Ok((Vec::new(), None));
        }
        // `[NSData bytes]`
        // SAFETY: `bytes` returns a borrowed pointer valid while `data` lives.
        let bytes_send: unsafe extern "C" fn(Id, *mut c_void) -> *const u8 =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        // SAFETY: `count` bytes are readable for the lifetime of `data`, which
        // the autorelease pool owns until this function returns.
        let bytes = unsafe { bytes_send(data, selector(c"bytes")) };
        if bytes.is_null() {
            return Err(AsyncImageError::InvalidResponse);
        }
        // SAFETY: `bytes` points to `count` readable bytes owned by `data`.
        let result = unsafe { std::slice::from_raw_parts(bytes, count) }.to_vec();
        Ok((result, content_type(ns_url)))
    }

    // `NSData` carries no MIME type, so the format hint stays absent and
    // `DownloadedImage::new` falls back to the URL's path extension, exactly as
    // the Swift loader did when a response carried no `Content-Type`.
    #[allow(
        clippy::unnecessary_wraps,
        reason = "mirrors the transport contract that may return a MIME type"
    )]
    fn content_type(_url: Id) -> Option<String> {
        None
    }

    /// `[error localizedDescription]`
    fn describe(error: Id) -> String {
        // SAFETY: `localizedDescription` takes no arguments.
        let send: unsafe extern "C" fn(Id, *mut c_void) -> Id =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        // SAFETY: `error` is a live `NSError`.
        let value = unsafe { send(error, selector(c"localizedDescription")) };
        if value.is_null() {
            return "NSData could not read the image URL".to_owned();
        }
        // SAFETY: `UTF8String` returns a borrowed NUL terminated string.
        let utf8: unsafe extern "C" fn(Id, *mut c_void) -> *const c_char =
            unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
        // SAFETY: `value` is a live `NSString`.
        let text = unsafe { utf8(value, selector(c"UTF8String")) };
        if text.is_null() {
            return "NSData could not read the image URL".to_owned();
        }
        // SAFETY: the pointer is NUL terminated and valid while `value` lives.
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    }

    struct AutoreleasePool(Id);

    impl AutoreleasePool {
        fn new() -> Self {
            // SAFETY: pushing a pool on this thread is always valid, and it must
            // be popped on the same thread.
            Self(unsafe { objc_autoreleasePoolPush() })
        }
    }

    impl Drop for AutoreleasePool {
        fn drop(&mut self) {
            // SAFETY: balances this value's own push on this thread.
            unsafe { objc_autoreleasePoolPop(self.0) };
        }
    }
}
