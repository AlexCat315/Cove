//! `MediaInput` over a FFmpeg dynamic I/O context, with cooperative cancellation.
use super::{Abort, IO_BUFFER, averror};
use crate::{CancellationToken, MediaError, MediaErrorKind, MediaInput, MediaResult};
use ffmpeg_sys_next as ffi;
use std::ffi::{c_int, c_void};
use std::sync::Arc;

const SEEK_SET: c_int = 0;
const SEEK_CUR: c_int = 1;
const SEEK_END: c_int = 2;

struct Inner {
    input: Box<dyn MediaInput>,
    cancellation: CancellationToken,
    abort: Arc<Abort>,
    position: u64,
}

/// An `AVIOContext` fed by an application-owned [`MediaInput`].
pub(crate) struct CustomIo {
    context: *mut ffi::AVIOContext,
    /// Pinned: `AVIOContext.opaque` points at this allocation for its whole life.
    inner: *mut Inner,
}

impl CustomIo {
    pub(crate) fn new(
        input: Box<dyn MediaInput>,
        cancellation: CancellationToken,
        abort: Arc<Abort>,
    ) -> MediaResult<Self> {
        let seekable = input.is_seekable();
        let length = IO_BUFFER.max(0) as usize + ffi::AVPROBE_PADDING_SIZE as usize;
        let buffer = unsafe {
            // SAFETY: `av_malloc` returns either null or a block of the requested size.
            ffi::av_malloc(length)
        }
        .cast::<u8>();
        if buffer.is_null() {
            return Err(MediaError::new(
                MediaErrorKind::Initialization,
                "FFmpeg could not allocate an I/O buffer",
            ));
        }
        let inner = Box::into_raw(Box::new(Inner {
            input,
            cancellation,
            abort,
            position: 0,
        }));
        let context = unsafe {
            // SAFETY: `buffer` holds the `IO_BUFFER` bytes passed alongside it, and `inner`
            // outlives the context because `Drop` frees the context first.
            ffi::avio_alloc_context(
                buffer,
                IO_BUFFER,
                0,
                inner.cast(),
                Some(Self::read),
                None,
                if seekable { Some(Self::seek) } else { None },
            )
        };
        if context.is_null() {
            unsafe {
                // SAFETY: no context adopted the box, so this is still its only reference.
                drop(Box::from_raw(inner))
            };
            unsafe {
                // SAFETY: the buffer was never adopted by a context.
                ffi::av_free(buffer.cast())
            };
            return Err(MediaError::new(
                MediaErrorKind::Initialization,
                "FFmpeg could not allocate an I/O context",
            ));
        }
        Ok(Self { context, inner })
    }

    /// The context to install as `AVFormatContext::pb`.
    pub(crate) fn context(&self) -> *mut ffi::AVIOContext {
        self.context
    }

    /// Whether a blocking call must give up because the operation was abandoned.
    fn stopped(inner: &Inner) -> bool {
        inner.abort.aborted() || inner.cancellation.is_cancelled()
    }

    unsafe extern "C" fn read(opaque: *mut c_void, buf: *mut u8, buf_size: c_int) -> c_int {
        let inner = unsafe {
            // SAFETY: `opaque` is the box installed by `new` and is only reclaimed in `Drop`.
            &mut *(opaque.cast::<Inner>())
        };
        if Self::stopped(inner) {
            return ffi::AVERROR_EXIT;
        }
        let size = usize::try_from(buf_size.max(0)).unwrap_or(usize::MAX);
        if size == 0 {
            return 0;
        }
        let bytes = unsafe {
            // SAFETY: FFmpeg passes a writable region of exactly `buf_size` bytes.
            std::slice::from_raw_parts_mut(buf, size)
        };
        match inner.input.read(bytes, &inner.cancellation) {
            Ok(0) => ffi::AVERROR_EOF,
            Ok(count) if count <= size => {
                inner.position += count as u64;
                c_int::try_from(count).unwrap_or(buf_size)
            }
            Ok(_) => averror(libc::EIO),
            Err(failure) if failure.kind == MediaErrorKind::Cancelled => ffi::AVERROR_EXIT,
            Err(_) => averror(libc::EIO),
        }
    }

    unsafe extern "C" fn seek(opaque: *mut c_void, offset: i64, whence: c_int) -> i64 {
        let inner = unsafe {
            // SAFETY: `opaque` is the box installed by `new` and is only reclaimed in `Drop`.
            &mut *(opaque.cast::<Inner>())
        };
        if Self::stopped(inner) {
            return ffi::AVERROR_EXIT as i64;
        }
        if whence == ffi::AVSEEK_SIZE {
            return inner
                .input
                .length()
                .and_then(|length| i64::try_from(length).ok())
                .unwrap_or(averror(libc::ENOSYS) as i64);
        }
        if offset < 0 {
            return averror(libc::EINVAL) as i64;
        }
        let base = match whence {
            SEEK_SET => 0,
            SEEK_CUR => inner.position,
            SEEK_END => inner.input.length().unwrap_or(inner.position),
            _ => return averror(libc::EINVAL) as i64,
        };
        let Ok(delta) = u64::try_from(offset) else {
            return averror(libc::EINVAL) as i64;
        };
        let target = base.wrapping_add(delta);
        if inner.input.seek(target, &inner.cancellation).is_err() {
            return averror(libc::EIO) as i64;
        }
        inner.position = target;
        i64::try_from(target).unwrap_or(averror(libc::EINVAL) as i64)
    }
}

impl Drop for CustomIo {
    fn drop(&mut self) {
        // `avio_context_free` leaves the buffer alone and FFmpeg may have
        // reallocated it, so the current pointer is the one that must be freed.
        if !self.context.is_null() {
            let buffer = unsafe {
                // SAFETY: a non-null context is a live allocation owned by this value.
                (*self.context).buffer
            };
            if !buffer.is_null() {
                unsafe {
                    // SAFETY: the buffer belongs to this context and is released with it.
                    ffi::av_free(buffer.cast())
                };
            }
            let mut context = self.context;
            unsafe {
                // SAFETY: the context is owned here and FFmpeg nulls the local copy.
                ffi::avio_context_free(&mut context)
            };
        }
        if !self.inner.is_null() {
            unsafe {
                // SAFETY: this reclaims the box's only reference, after the context that
                // exposed it to the read and seek callbacks has been freed.
                drop(Box::from_raw(self.inner))
            };
        }
    }
}
