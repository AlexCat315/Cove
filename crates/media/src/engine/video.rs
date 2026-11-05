//! Decoder output to tightly packed RGBA, with a persistent scaling context.
use crate::{MediaError, MediaErrorKind, MediaResult, VideoDynamicRange, VideoStorage};
use ffmpeg_sys_next as ffi;
use std::any::Any;
use std::ffi::{CStr, c_char, c_int};

/// One converted frame: `width * height * 4` bytes, row stride `width * 4`.
pub(crate) struct Rgba {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub colorimetry: String,
    pub dynamic_range: VideoDynamicRange,
    pub aspect_ratio: (u32, u32),
}

/// The single CPU plane behind a published frame.
pub struct FrameStorage {
    bytes: Vec<u8>,
}

impl FrameStorage {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl VideoStorage for FrameStorage {
    fn copy_planes(&self) -> MediaResult<Vec<Vec<u8>>> {
        Ok(vec![self.bytes.clone()])
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Converts any CPU pixel format to RGBA8. The scaling context is reused across frames.
pub(crate) struct Converter {
    context: *mut ffi::SwsContext,
}

impl Converter {
    pub(crate) fn new() -> Self {
        Self {
            context: std::ptr::null_mut(),
        }
    }

    /// Scales `source` into `bytes`, which must be at least `width * height * 4`.
    /// The destination is a borrowed buffer: the scaler writes into it, we own the copy.
    pub(crate) fn convert(
        &mut self,
        source: *mut ffi::AVFrame,
        mut bytes: Vec<u8>,
    ) -> MediaResult<Rgba> {
        // SAFETY: `source` is a live decoded frame owned by the pump thread for this call.
        let (width, height, transfer, aspect, colorspace) = unsafe {
            (
                (*source).width,
                (*source).height,
                (*source).color_trc,
                (*source).sample_aspect_ratio,
                (*source).colorspace,
            )
        };
        if width <= 0 || height <= 0 {
            return Err(MediaError::new(
                MediaErrorKind::Negotiation,
                "decoded video has no dimensions",
            ));
        }
        let stride = usize::try_from(width)
            .ok()
            .and_then(|w| w.checked_mul(4))
            .and_then(|w| w.checked_mul(usize::try_from(height).unwrap_or(usize::MAX - 1)))
            .ok_or_else(|| {
                MediaError::new(MediaErrorKind::Negotiation, "video frame is too large")
            })?;
        bytes.clear();
        bytes.resize(stride, 0);
        let format = unsafe {
            // SAFETY: `AV_PIX_FMT_*` values are exactly the discriminants of the generated
            // `AVPixelFormat` enum, and a decoded frame carries one of them.
            std::mem::transmute::<c_int, ffi::AVPixelFormat>((*source).format)
        };
        let context = unsafe {
            // SAFETY: the source frame is live for the call, and the cached context is
            // either null or one allocated by `sws_getCachedContext` and freed in `drop`.
            ffi::sws_getCachedContext(
                self.context,
                width,
                height,
                format,
                width,
                height,
                ffi::AVPixelFormat::AV_PIX_FMT_RGBA,
                ffi::SwsFlags::SWS_BILINEAR as c_int,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        self.context = context;
        if context.is_null() {
            return Err(MediaError::new(
                MediaErrorKind::Negotiation,
                "FFmpeg could not convert the decoded pixel format to RGBA",
            ));
        }
        let row = c_int::try_from(width * 4)
            .map_err(|_| MediaError::new(MediaErrorKind::Negotiation, "video row overflows"))?;
        let mut data = [
            bytes.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ];
        let mut lines = [row, 0, 0, 0];
        let scaled = unsafe {
            // SAFETY: the source frame is live and planar pointers come from FFmpeg; the
            // destination holds `stride` bytes per row for `height` rows.
            ffi::sws_scale(
                context,
                (*source).data.as_ptr().cast(),
                (*source).linesize.as_ptr(),
                0,
                height,
                data.as_mut_ptr(),
                lines.as_mut_ptr(),
            )
        };
        if scaled <= 0 {
            return Err(MediaError::new(
                MediaErrorKind::Decode,
                "FFmpeg did not convert the decoded video frame",
            ));
        }
        Ok(Rgba {
            bytes,
            width: width as u32,
            height: height as u32,
            colorimetry: colorimetry(transfer, colorspace),
            dynamic_range: match transfer {
                ffi::AVColorTransferCharacteristic::AVCOL_TRC_SMPTE2084 => VideoDynamicRange::Pq,
                ffi::AVColorTransferCharacteristic::AVCOL_TRC_ARIB_STD_B67 => {
                    VideoDynamicRange::Hlg
                }
                ffi::AVColorTransferCharacteristic::AVCOL_TRC_UNSPECIFIED => {
                    VideoDynamicRange::Unknown
                }
                _ => VideoDynamicRange::Sdr,
            },
            aspect_ratio: (aspect.num.max(0) as u32, aspect.den.max(1) as u32),
        })
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        if !self.context.is_null() {
            // SAFETY: the context is uniquely owned by this converter.
            unsafe { ffi::sws_freeContext(self.context) };
            self.context = std::ptr::null_mut();
        }
    }
}

fn colorimetry(transfer: ffi::AVColorTransferCharacteristic, space: ffi::AVColorSpace) -> String {
    let name = |pointer: *const c_char| -> String {
        if pointer.is_null() {
            "unknown".to_owned()
        } else {
            // SAFETY: FFmpeg returns static NUL-terminated names for these enums.
            unsafe { CStr::from_ptr(pointer) }
                .to_string_lossy()
                .into_owned()
        }
    };
    let transfer = name(unsafe {
        // SAFETY: the enum value comes from a decoded frame and the name is static.
        ffi::av_color_transfer_name(transfer)
    });
    let space = name(unsafe {
        // SAFETY: the enum value comes from a decoded frame and the name is static.
        ffi::av_color_space_name(space)
    });
    if transfer == "unknown" && space == "unknown" {
        "unknown".to_owned()
    } else {
        format!("{space}/{transfer}")
    }
}

/// A NUL-free description of a codec, for `TrackInfo` and frame formats.
pub(crate) fn codec_name(id: ffi::AVCodecID) -> String {
    // SAFETY: the codec id names are static strings in FFmpeg's codec table.
    let pointer = unsafe { ffi::avcodec_get_name(id) };
    if pointer.is_null() {
        return "unknown".to_owned();
    }
    // SAFETY: the name is a static NUL-terminated string owned by FFmpeg.
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}
