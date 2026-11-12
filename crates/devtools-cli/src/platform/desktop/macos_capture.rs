use super::{Fault, Rect, RgbaImage};
use block2::RcBlock;
use objc2::{runtime::AnyClass, sel};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo,
    CGImageByteOrderInfo, CGPreflightScreenCaptureAccess, kCGColorSpaceSRGB,
};
use objc2_screen_capture_kit::SCScreenshotManager;
use std::{sync::mpsc, time::Duration};

pub(super) fn capture(bounds: Rect) -> Result<RgbaImage, Fault> {
    if !CGPreflightScreenCaptureAccess() {
        return Err(Fault::new(
            "capture_denied",
            "Grant Screen Recording access to the DevTools controller.",
        ));
    }
    let has_region_capture = AnyClass::get(c"SCScreenshotManager")
        .and_then(|class| class.class_method(sel!(captureImageInRect:completionHandler:)))
        .is_some();
    if !has_region_capture {
        return Err(Fault::new(
            "unavailable",
            "This macOS version lacks ScreenCaptureKit display-region capture.",
        ));
    }
    let (sender, result) = mpsc::sync_channel(1);
    let handler = RcBlock::new(
        move |image: *mut CGImage, _error: *mut objc2_foundation::NSError| {
            // SAFETY: a non-null image delivered to this completion block is valid until it returns.
            let result = unsafe { image.as_ref() }
                .ok_or_else(capture_failed)
                .and_then(pixels);
            let _ = sender.try_send(result);
        },
    );
    let rect = CGRect::new(
        CGPoint::new(bounds.x as f64, bounds.y as f64),
        CGSize::new(bounds.width as f64, bounds.height as f64),
    );
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: ScreenCaptureKit copies the completion block before returning.
        unsafe { SCScreenshotManager::captureImageInRect_completionHandler(rect, Some(&handler)) };
    });
    result
        .recv_timeout(Duration::from_secs(6))
        .map_err(|_| capture_failed())?
}

fn capture_failed() -> Fault {
    Fault::new(
        "capture_failed",
        "ScreenCaptureKit did not return a screenshot.",
    )
}

fn pixels(image: &CGImage) -> Result<RgbaImage, Fault> {
    let width = CGImage::width(Some(image));
    let height = CGImage::height(Some(image));
    let pixel_count = width
        .checked_mul(height)
        .filter(|size| *size <= 40_000_000 && *size > 0)
        .ok_or_else(|| {
            Fault::new(
                "capture_size",
                "ScreenCaptureKit image exceeds the pixel budget.",
            )
        })?;
    let mut rgba = vec![0; pixel_count * 4];
    // SAFETY: sRGB is a constant provided by CoreGraphics.
    let color =
        CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })).ok_or_else(capture_failed)?;
    // SAFETY: rgba backs a tightly packed bitmap and outlives the context, which writes it synchronously.
    let context = unsafe {
        CGBitmapContextCreate(
            rgba.as_mut_ptr().cast(),
            width,
            height,
            8,
            width * 4,
            Some(&color),
            CGImageByteOrderInfo::Order32Big.0 | CGImageAlphaInfo::PremultipliedLast.0,
        )
    }
    .ok_or_else(capture_failed)?;
    CGContext::draw_image(
        Some(&context),
        CGRect::new(CGPoint::ZERO, CGSize::new(width as f64, height as f64)),
        Some(image),
    );
    drop(context);
    RgbaImage::from_raw(width as u32, height as u32, rgba).ok_or_else(capture_failed)
}
