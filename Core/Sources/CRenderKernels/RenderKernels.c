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
