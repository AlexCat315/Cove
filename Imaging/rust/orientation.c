#include <stddef.h>
#include <stdint.h>
#include <string.h>
void cove_image_orient_rgba(const uint8_t *input, size_t w, size_t h, int orientation, uint8_t *output) {
    const size_t ow = orientation >= 5 ? h : w, oh = orientation >= 5 ? w : h;
    for (size_t y = 0; y < oh; ++y) for (size_t x = 0; x < ow; ++x) {
        size_t sx = x, sy = y;
        switch (orientation) {
            case 2: sx = w - 1 - x; break;
            case 3: sx = w - 1 - x; sy = h - 1 - y; break;
            case 4: sy = h - 1 - y; break;
            case 5: sx = y; sy = x; break;
            case 6: sx = y; sy = h - 1 - x; break;
            case 7: sx = w - 1 - y; sy = h - 1 - x; break;
            case 8: sx = w - 1 - y; sy = x; break;
        }
        memcpy(output + (y * ow + x) * 4, input + (sy * w + sx) * 4, 4);
    }
}
