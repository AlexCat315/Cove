#ifndef COVE_IMAGE_DECODE_BRIDGE_H
#define COVE_IMAGE_DECODE_BRIDGE_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct CoveImageDecodeResult {
    uint8_t* pixels;
    int32_t width;
    int32_t height;
    char* error_message;
} CoveImageDecodeResult;

bool cove_image_decode_memory(const uint8_t* data,
                               size_t data_size,
                               const char* extension,
                               int32_t target_width,
                               int32_t target_height,
                               CoveImageDecodeResult* out_result);

/// Read intrinsic dimensions without allocating a decoded bitmap.
bool cove_image_dimensions_memory(const uint8_t* data, size_t data_size,
                                   const char* extension, int32_t* width, int32_t* height);

void cove_image_decode_free(CoveImageDecodeResult* result);

/// Every frame of an animated GIF or WebP, composited onto the full canvas
/// and resampled to the target size. `pixels` holds `frame_count` RGBA frames
/// back to back; `durations_ms` one display time per frame. A still image
/// decodes as a single frame.
typedef struct CoveImageAnimationResult {
    uint8_t* pixels;
    int32_t* durations_ms;
    int32_t width;
    int32_t height;
    int32_t frame_count;
    /// 0 repeats forever.
    int32_t loop_count;
    char* error_message;
} CoveImageAnimationResult;

bool cove_image_decode_animation_memory(const uint8_t* data,
                                         size_t data_size,
                                         const char* extension,
                                         int32_t target_width,
                                         int32_t target_height,
                                         int32_t max_frames,
                                         CoveImageAnimationResult* out_result);

void cove_image_animation_free(CoveImageAnimationResult* result);

#ifdef __cplusplus
}
#endif

#endif
