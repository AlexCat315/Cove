#include "CRenderKernels.h"
#include <math.h>
#include <string.h>

_Static_assert(sizeof(CoveRenderVertex) == 20, "UI vertex ABI must remain 20 bytes");

void cove_render_translate(void *vertices, size_t count, float x, float y) {
    unsigned char *bytes = vertices;
    for (size_t i = 0; i < count; ++i) {
        CoveRenderVertex vertex;
        memcpy(&vertex, bytes + i * sizeof(vertex), sizeof(vertex));
        vertex.x += x;
        vertex.y += y;
        memcpy(bytes + i * sizeof(vertex), &vertex, sizeof(vertex));
    }
}

void cove_render_indices(uint32_t *indices, size_t count, uint32_t offset) {
    for (size_t i = 0; i < count; ++i) indices[i] += offset;
}

void cove_render_opacity(void *vertices, size_t count, float opacity) {
    unsigned char *bytes = vertices;
    for (size_t i = 0; i < count; ++i) {
        uint32_t color;
        unsigned char *address = bytes + i * sizeof(CoveRenderVertex) + offsetof(CoveRenderVertex, color);
        memcpy(&color, address, sizeof(color));
        uint32_t alpha = (uint32_t)roundf((float)(color >> 24) * opacity);
        color = (color & 0x00ffffffu) | (alpha << 24);
        memcpy(address, &color, sizeof(color));
    }
}

void cove_render_bounds(const void *vertices, size_t count, float *bounds) {
    if (count == 0) return;
    const unsigned char *bytes = vertices;
    CoveRenderVertex vertex;
    memcpy(&vertex, bytes, sizeof(vertex));
    float min_x = vertex.x, max_x = vertex.x, min_y = vertex.y, max_y = vertex.y;
    for (size_t i = 1; i < count; ++i) {
        memcpy(&vertex, bytes + i * sizeof(vertex), sizeof(vertex));
        // Match Swift min/max, including their handling of non-finite inputs.
        min_x = vertex.x < min_x ? vertex.x : min_x;
        max_x = max_x < vertex.x ? vertex.x : max_x;
        min_y = vertex.y < min_y ? vertex.y : min_y;
        max_y = max_y < vertex.y ? vertex.y : max_y;
    }
    bounds[0] = min_x; bounds[1] = min_y;
    bounds[2] = max_x - min_x; bounds[3] = max_y - min_y;
}

static uint8_t cove_image_byte(double value) {
    return (uint8_t)fmax(0.0, fmin(255.0, round(value)));
}

void cove_image_reduced_mip(const uint8_t *pixels, size_t width, size_t height, uint8_t *output) {
    const size_t w = width > 1 ? width / 2 : 1;
    const size_t h = height > 1 ? height / 2 : 1;
    const double dx = (double)width / (double)w;
    const double dy = (double)height / (double)h;
    for (size_t y = 0; y < h; ++y) {
        const double low_y = (double)y * dy, high_y = (double)(y + 1) * dy;
        for (size_t x = 0; x < w; ++x) {
            const double low_x = (double)x * dx, high_x = (double)(x + 1) * dx;
            double red = 0.0, green = 0.0, blue = 0.0, alpha = 0.0, area = 0.0;
            const size_t max_y = (size_t)fmin((double)height, ceil(high_y));
            const size_t max_x = (size_t)fmin((double)width, ceil(high_x));
            for (size_t sy = (size_t)low_y; sy < max_y; ++sy) {
                const double wy = fmin(high_y, (double)(sy + 1)) - fmax(low_y, (double)sy);
                for (size_t sx = (size_t)low_x; sx < max_x; ++sx) {
                    const double weight = wy * (fmin(high_x, (double)(sx + 1)) - fmax(low_x, (double)sx));
                    const size_t index = (sy * width + sx) * 4;
                    const double a = (double)pixels[index + 3] * weight;
                    red += (double)pixels[index] * a;
                    green += (double)pixels[index + 1] * a;
                    blue += (double)pixels[index + 2] * a;
                    alpha += a; area += weight;
                }
            }
            const size_t index = (y * w + x) * 4;
            output[index] = alpha > 0.0 ? cove_image_byte(red / alpha) : 0;
            output[index + 1] = alpha > 0.0 ? cove_image_byte(green / alpha) : 0;
            output[index + 2] = alpha > 0.0 ? cove_image_byte(blue / alpha) : 0;
            output[index + 3] = cove_image_byte(alpha / fmax(area, 0.00001));
        }
    }
}
