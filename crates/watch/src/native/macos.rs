//! FSEvents: recursive, coalesced notifications for a folder tree.

use std::ffi::{c_char, c_void};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

type CFIndex = isize;
type FSEventStreamRef = *mut c_void;

#[repr(C)]
struct FSEventStreamContext {
    version: CFIndex,
    info: *mut c_void,
    retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
    release: Option<extern "C" fn(*const c_void)>,
    copy_description: Option<extern "C" fn(*const c_void) -> *const c_void>,
}

type FSEventStreamCallback = extern "C" fn(
    stream: *const c_void,
    info: *mut c_void,
    count: usize,
    paths: *mut c_void,
    flags: *const u32,
    ids: *const u64,
);

const UTF8: u32 = 0x0800_0100;
const SINCE_NOW: u64 = u64::MAX;
const NO_DEFER: u32 = 0x02;
const FILE_EVENTS: u32 = 0x10;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFTypeArrayCallBacks: u8;
    fn CFStringCreateWithBytes(
        allocator: *const c_void,
        bytes: *const u8,
        length: CFIndex,
        encoding: u32,
        external: u8,
    ) -> *const c_void;
    fn CFArrayCreate(
        allocator: *const c_void,
        values: *const *const c_void,
        count: CFIndex,
        callbacks: *const c_void,
    ) -> *const c_void;
    fn CFRelease(object: *const c_void);
}

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn FSEventStreamCreate(
        allocator: *const c_void,
        callback: FSEventStreamCallback,
        context: *const FSEventStreamContext,
        paths: *const c_void,
        since: u64,
        latency: f64,
        flags: u32,
    ) -> FSEventStreamRef;
    fn FSEventStreamSetDispatchQueue(stream: FSEventStreamRef, queue: *mut c_void);
    fn FSEventStreamStart(stream: FSEventStreamRef) -> u8;
    fn FSEventStreamStop(stream: FSEventStreamRef);
    fn FSEventStreamInvalidate(stream: FSEventStreamRef);
    fn FSEventStreamRelease(stream: FSEventStreamRef);
}

unsafe extern "C" {
    fn dispatch_queue_create(label: *const c_char, attributes: *const c_void) -> *mut c_void;
    fn dispatch_release(object: *mut c_void);
}

/// Shared with the stream through its context; FSEvents retains and releases
/// it, so it outlives any callback still running on the queue.
type Shared = Mutex<Option<Sender<()>>>;

extern "C" fn retain(info: *const c_void) -> *const c_void {
    // SAFETY: `info` came from `Arc::into_raw` of a `Shared` and is still
    // owned by at least one strong count.
    unsafe { Arc::increment_strong_count(info.cast::<Shared>()) };
    info
}

extern "C" fn release(info: *const c_void) {
    // SAFETY: balances one `retain` (or the initial `into_raw`).
    unsafe { Arc::decrement_strong_count(info.cast::<Shared>()) };
}

extern "C" fn callback(
    _stream: *const c_void,
    info: *mut c_void,
    _count: usize,
    _paths: *mut c_void,
    _flags: *const u32,
    _ids: *const u64,
) {
    // SAFETY: FSEvents keeps `info` retained for the stream's lifetime.
    let shared = unsafe { &*info.cast::<Shared>() };
    if let Ok(sender) = shared.lock()
        && let Some(sender) = sender.as_ref()
    {
        let _ = sender.send(());
    }
}

pub struct NativeWatch {
    stream: FSEventStreamRef,
    queue: *mut c_void,
    shared: Arc<Shared>,
}

// SAFETY: the stream and queue are only touched in `start` and `drop`; FSEvents
// and libdispatch objects are thread-safe to stop and release.
unsafe impl Send for NativeWatch {}

impl NativeWatch {
    pub fn start(root: &Path, wake: Sender<()>, _stop: Arc<AtomicBool>) -> Option<Self> {
        let path = root.to_str()?;
        let shared: Arc<Shared> = Arc::new(Mutex::new(Some(wake)));
        let info = Arc::into_raw(shared.clone()) as *mut c_void;
        let context = FSEventStreamContext {
            version: 0,
            info,
            retain: Some(retain),
            release: Some(release),
            copy_description: None,
        };
        // SAFETY: plain CoreFoundation/CoreServices calls with valid
        // arguments; every created object is released below or in `drop`.
        unsafe {
            let string = CFStringCreateWithBytes(
                std::ptr::null(),
                path.as_ptr(),
                path.len() as CFIndex,
                UTF8,
                0,
            );
            if string.is_null() {
                release(info);
                return None;
            }
            let values = [string];
            let paths = CFArrayCreate(
                std::ptr::null(),
                values.as_ptr(),
                1,
                (&raw const kCFTypeArrayCallBacks).cast(),
            );
            CFRelease(string);
            if paths.is_null() {
                release(info);
                return None;
            }
            let stream = FSEventStreamCreate(
                std::ptr::null(),
                callback,
                &context,
                paths,
                SINCE_NOW,
                0.1,
                NO_DEFER | FILE_EVENTS,
            );
            CFRelease(paths);
            // The stream retained `info` through the context; drop the
            // reference taken for the context itself.
            release(info);
            if stream.is_null() {
                return None;
            }
            let queue = dispatch_queue_create(c"com.cove.watch".as_ptr(), std::ptr::null());
            FSEventStreamSetDispatchQueue(stream, queue);
            if FSEventStreamStart(stream) == 0 {
                FSEventStreamInvalidate(stream);
                FSEventStreamRelease(stream);
                dispatch_release(queue);
                return None;
            }
            Some(Self {
                stream,
                queue,
                shared,
            })
        }
    }
}

impl Drop for NativeWatch {
    fn drop(&mut self) {
        if let Ok(mut sender) = self.shared.lock() {
            sender.take();
        }
        // SAFETY: the stream was started in `start` and is torn down once.
        unsafe {
            FSEventStreamStop(self.stream);
            FSEventStreamInvalidate(self.stream);
            FSEventStreamRelease(self.stream);
            dispatch_release(self.queue);
        }
    }
}
