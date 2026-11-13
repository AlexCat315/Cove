use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::rhi::{Completion, RhiError, RhiResult, TimedCompletion};

#[link(wasm_import_module = "cove_webgpu")]
unsafe extern "C" {
    fn request(message: *const u8, message_len: usize, bytes: *const u8, bytes_len: usize) -> i32;
}

struct Pending {
    callback: Option<TimedCompletion>,
    count: Arc<AtomicUsize>,
}

impl Drop for Pending {
    fn drop(&mut self) {
        self.count.fetch_sub(1, Ordering::Relaxed);
    }
}

thread_local! {
    static COMPLETIONS: RefCell<HashMap<u32, Pending>> = RefCell::default();
}

/// Called by the browser on the event loop after the WebGPU queue completes.
pub fn complete(token: u32, nanos: Option<u64>) {
    let pending = COMPLETIONS.with(|completions| completions.borrow_mut().remove(&token));
    if let Some(mut pending) = pending
        && let Some(callback) = pending.callback.take()
    {
        callback(nanos);
    }
}

#[derive(Clone)]
pub(super) struct Host {
    id: u32,
    pending: Arc<AtomicUsize>,
}

impl Host {
    pub fn open() -> RhiResult<Self> {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        let host = Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            pending: Arc::default(),
        };
        host.call("open", Value::Null, &[])?;
        Ok(host)
    }

    pub fn call(&self, op: &str, args: Value, data: &[u8]) -> RhiResult<()> {
        let message = json!({ "device": self.id, "op": op, "args": args }).to_string();
        // SAFETY: both buffers are readable for the synchronous import. The
        // host copies any data it will use after the import returns.
        let status = unsafe { request(message.as_ptr(), message.len(), data.as_ptr(), data.len()) };
        match status {
            0 => Ok(()),
            -1 => Err(RhiError::unsupported_backend(
                "WebGPU is unavailable in this browser",
            )),
            -2 => Err(RhiError::DeviceLost),
            _ => Err(RhiError::invalid_argument(format!("WebGPU rejected {op}"))),
        }
    }

    pub fn queue(&self, op: &str, args: Value, callback: Completion) -> RhiResult<()> {
        self.queue_timed(op, args, Box::new(move |_| callback()))
    }

    pub fn queue_timed(&self, op: &str, mut args: Value, callback: TimedCompletion) -> RhiResult<()> {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        let token = NEXT.fetch_add(1, Ordering::Relaxed);
        self.pending.fetch_add(1, Ordering::Relaxed);
        COMPLETIONS.with(|completions| {
            completions.borrow_mut().insert(
                token,
                Pending {
                    callback: Some(callback),
                    count: self.pending.clone(),
                },
            );
        });
        args["completion"] = json!(token);
        let result = self.call(op, args, &[]);
        if result.is_err() {
            COMPLETIONS.with(|completions| completions.borrow_mut().remove(&token));
        }
        result
    }

    pub fn idle(&self) -> bool {
        self.pending.load(Ordering::Relaxed) == 0
    }
}
