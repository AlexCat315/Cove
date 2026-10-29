#include "CTextSelection.h"
#include <float.h>
#include <stdlib.h>

static int compare_member(const CoveSelectionFrame *a, const CoveSelectionFrame *b) {
    return (a->member > b->member) - (a->member < b->member);
}

static int compare_row(const void *lhs, const void *rhs) {
    const CoveSelectionFrame *a = lhs, *b = rhs;
    if (a->y != b->y) return a->y < b->y ? -1 : 1;
    if (a->x != b->x) return a->x < b->x ? -1 : 1;
    return compare_member(a, b);
}

static int compare_column(const void *lhs, const void *rhs) {
    const CoveSelectionFrame *a = lhs, *b = rhs;
    if (a->x != b->x) return a->x < b->x ? -1 : 1;
    return compare_row(lhs, rhs);
}

void cove_selection_sort(CoveSelectionFrame *frames, size_t count) {
    if (count < 2) return;
    qsort(frames, count, sizeof(*frames), compare_row);
    // Group relative to the row's first Y. A pairwise Y tolerance inside a
    // comparator is not transitive and violates qsort's ordering contract.
    for (size_t start = 0; start < count;) {
        size_t end = start + 1;
        while (end < count && frames[end].y - frames[start].y <= 0.5) ++end;
        if (end - start > 1)
            qsort(frames + start, end - start, sizeof(*frames), compare_column);
        start = end;
    }
}

ptrdiff_t cove_selection_find(const CoveSelectionFrame *frames, size_t count,
                            double x, double y, bool snap, int32_t *edge) {
    *edge = 0;
    ptrdiff_t nearest = -1, next = -1;
    double distance = DBL_MAX;
    for (size_t i = 0; i < count; ++i) {
        const CoveSelectionFrame *f = frames + i;
        if (f->width > 0 && f->height > 0 &&
            x >= f->x && x < f->x + f->width && y >= f->y && y < f->y + f->height)
            return (ptrdiff_t)i;
        if (!snap) continue;
        if (y >= f->y && y < f->y + f->height) {
            double dx = x < f->x ? f->x - x : (x > f->x + f->width ? x - f->x - f->width : 0);
            if (dx < distance) { nearest = (ptrdiff_t)i; distance = dx; }
        }
        if (next < 0 && f->y > y) next = (ptrdiff_t)i;
    }
    if (!snap || count == 0) return -1;
    if (nearest >= 0) return nearest;
    *edge = next >= 0 ? -1 : 1;
    return next >= 0 ? next : (ptrdiff_t)(count - 1);
}
