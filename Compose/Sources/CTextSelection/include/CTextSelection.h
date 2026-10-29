#ifndef COVE_TEXT_SELECTION_H
#define COVE_TEXT_SELECTION_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct {
    double x, y, width, height;
    size_t member;
} CoveSelectionFrame;

/// Sorts once by row, then by horizontal position within each row.
void cove_selection_sort(CoveSelectionFrame *frames, size_t count);

/// Returns a sorted frame index, or -1. Edge is -1 for the paragraph start,
/// 1 for its end, and 0 when the caller should hit-test its glyphs.
ptrdiff_t cove_selection_find(const CoveSelectionFrame *frames, size_t count,
                            double x, double y, bool snap, int32_t *edge);

#endif
