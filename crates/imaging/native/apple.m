#import <Foundation/Foundation.h>
#import <CoreGraphics/CoreGraphics.h>
#import <ImageIO/ImageIO.h>
#include "image_decode_bridge.h"
#include <math.h>
#include <stdlib.h>
#include <string.h>
static bool image_error(CoveImageDecodeResult *out, const char *message) {
    out->error_message = strdup(message); return false;
}
static bool dimensions_valid(double width, double height, uint64_t budget) {
    return isfinite(width) && isfinite(height) && width > 0 && height > 0 && width <= 16384 && height <= 16384 && width * height <= (double)budget;
}
static void fit_size(double sw, double sh, int bw, int bh, bool upscale, int *width, int *height) {
    double scale = fmin((double)bw / sw, (double)bh / sh); if (!upscale) scale = fmin(1, scale);
    *width = (int)fmax(1, round(sw * scale)); *height = (int)fmax(1, round(sh * scale));
}
static CGContextRef make_context(int width, int height, CoveImageDecodeResult *out) {
    size_t length = (size_t)width * height * 4;
    out->pixels = calloc(length, 1); if (!out->pixels) return NULL;
    CGColorSpaceRef space = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef context = CGBitmapContextCreate(out->pixels, width, height, 8, (size_t)width * 4, space, kCGImageAlphaPremultipliedLast | kCGBitmapByteOrder32Big);
    CGColorSpaceRelease(space); if (!context) { free(out->pixels); out->pixels = NULL; return NULL; }
    out->width = width; out->height = height; CGContextSetInterpolationQuality(context, kCGInterpolationHigh); return context;
}
static void unpremultiply(CoveImageDecodeResult *out) {
    size_t length = (size_t)out->width * out->height * 4;
    for (size_t i = 0; i < length; i += 4) {
        unsigned a = out->pixels[i + 3];
        if (!a) memset(out->pixels + i, 0, 3);
        else if (a < 255) for (int c = 0; c < 3; ++c) out->pixels[i + c] = (uint8_t)fmin(255, (out->pixels[i + c] * 255U + a / 2) / a);
    }
}
bool cove_image_apple_decode(const uint8_t *bytes, size_t length, bool pdf, int bw, int bh, uint64_t budget, bool thumbnail, bool partial, CoveImageDecodeResult *out) {
    if (!out) return false; *out = (CoveImageDecodeResult){0};
    if (!bytes || !length || !budget) return image_error(out, "invalid image data");
    @autoreleasepool {
        NSData *data = [NSData dataWithBytesNoCopy:(void *)bytes length:length freeWhenDone:NO];
        if (pdf) {
            CGDataProviderRef provider = CGDataProviderCreateWithCFData((__bridge CFDataRef)data);
            CGPDFDocumentRef document = CGPDFDocumentCreateWithProvider(provider); CGDataProviderRelease(provider);
            if (!document) return image_error(out, "could not decode PDF");
            CGPDFPageRef page = CGPDFDocumentGetPage(document, 1);
            if (!page) { CGPDFDocumentRelease(document); return image_error(out, "PDF has no pages"); }
            CGRect box = CGPDFPageGetBoxRect(page, kCGPDFMediaBox); int rotation = CGPDFPageGetRotationAngle(page);
            double sw = box.size.width, sh = box.size.height; if (rotation % 180) { double t = sw; sw = sh; sh = t; }
            if (!dimensions_valid(sw, sh, budget)) { CGPDFDocumentRelease(document); return image_error(out, "invalid or oversized PDF dimensions"); }
            int width = bw > 0 ? bw : (int)fmax(1, round(sw)), height = bh > 0 ? bh : (int)fmax(1, round(sh));
            if (thumbnail) fit_size(sw, sh, bw, bh, true, &width, &height);
            if (!dimensions_valid(width, height, budget)) { CGPDFDocumentRelease(document); return image_error(out, "oversized PDF target"); }
            CGContextRef context = make_context(width, height, out);
            if (!context) { CGPDFDocumentRelease(document); return image_error(out, "could not allocate PDF bitmap"); }
            CGAffineTransform transform = CGPDFPageGetDrawingTransform(page, kCGPDFMediaBox, CGRectMake(0, 0, width, height), 0, thumbnail);
            CGContextConcatCTM(context, transform); CGContextDrawPDFPage(context, page);
            CGContextRelease(context); CGPDFDocumentRelease(document); unpremultiply(out); return true;
        }
        CGImageSourceRef source = partial ? CGImageSourceCreateIncremental(NULL) : CGImageSourceCreateWithData((__bridge CFDataRef)data, NULL);
        if (!source) return image_error(out, "could not create image source");
        if (partial) CGImageSourceUpdateData(source, (__bridge CFDataRef)data, false);
        if (CGImageSourceGetCount(source) < 1) { CFRelease(source); return image_error(out, "image source has no frames"); }
        NSDictionary *properties = CFBridgingRelease(CGImageSourceCopyPropertiesAtIndex(source, 0, NULL));
        double sw = [properties[(__bridge NSString *)kCGImagePropertyPixelWidth] doubleValue];
        double sh = [properties[(__bridge NSString *)kCGImagePropertyPixelHeight] doubleValue];
        int orientation = [properties[(__bridge NSString *)kCGImagePropertyOrientation] intValue];
        if (!dimensions_valid(sw, sh, budget)) { CFRelease(source); return image_error(out, "invalid or oversized image dimensions"); }
        bool swaps = orientation >= 5 && orientation <= 8;
        if (swaps) { double t = sw; sw = sh; sh = t; }
        int width = bw > 0 ? bw : (int)sw, height = bh > 0 ? bh : (int)sh;
        if (thumbnail) fit_size(sw, sh, bw, bh, false, &width, &height);
        if (!dimensions_valid(width, height, budget)) { CFRelease(source); return image_error(out, "invalid bitmap target"); }
        NSDictionary *options = @{ (__bridge NSString *)kCGImageSourceCreateThumbnailFromImageAlways: @YES,
            (__bridge NSString *)kCGImageSourceCreateThumbnailWithTransform: @YES,
            (__bridge NSString *)kCGImageSourceThumbnailMaxPixelSize: @(MAX(width, height)),
            (__bridge NSString *)kCGImageSourceShouldCacheImmediately: @YES };
        CGImageRef image = CGImageSourceCreateThumbnailAtIndex(source, 0, (__bridge CFDictionaryRef)options); CFRelease(source);
        if (!image) return image_error(out, "could not rasterize image");
        CGContextRef context = make_context(width, height, out);
        if (!context) { CGImageRelease(image); return image_error(out, "could not allocate image bitmap"); }
        CGContextDrawImage(context, CGRectMake(0, 0, width, height), image);
        CGContextRelease(context); CGImageRelease(image); unpremultiply(out); return true;
    }
}
