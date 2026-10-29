#ifndef COVE_RENDER_KERNELS_H
#define COVE_RENDER_KERNELS_H

#include <stddef.h>
#include <stdint.h>

// Matches UIVertex's renderer ABI. Byte copies avoid aliasing Swift storage.
typedef struct {
    float x, y, u, v;
    uint32_t color;
} CoveRenderVertex;

void cove_render_translate(void *vertices, size_t count, float x, float y);
void cove_render_indices(uint32_t *indices, size_t count, uint32_t offset);
void cove_render_opacity(void *vertices, size_t count, float opacity);
void cove_render_bounds(const void *vertices, size_t count, float *bounds);

#endif
