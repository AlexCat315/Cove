/* Translation of the hot resolution/reordering passes in CoveUICore/Bidi.swift. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>

enum { L, R, AL, EN, ES, ET, AN, CS, NSM, BN, B, S, WS, ON };
static int strong_direction(uint8_t value) {
    if (value == L) return L;
    if (value == R || value == EN || value == AN) return R;
    return -1;
}
void cove_bidi_resolve(const uint8_t *source, uint8_t *types, size_t count, uint8_t base, uint8_t *levels) {
    if (!count) return;
    memcpy(types, source, count);
    const uint8_t sos = (base & 1) ? R : L;
    for (size_t i = 0; i < count; ++i) {
        if (types[i] == NSM || types[i] == BN) types[i] = i ? types[i - 1] : sos;
    }
    uint8_t last_strong = sos;
    for (size_t i = 0; i < count; ++i) {
        if (types[i] == L || types[i] == R || types[i] == AL) last_strong = types[i];
        else if (types[i] == EN && last_strong == AL) types[i] = AN;
    }
    for (size_t i = 0; i < count; ++i) if (types[i] == AL) types[i] = R;
    for (size_t i = 1; i + 1 < count; ++i) {
        const uint8_t previous = types[i - 1], next = types[i + 1];
        if (types[i] == ES && previous == EN && next == EN) types[i] = EN;
        else if (types[i] == CS && previous == next && (previous == EN || previous == AN)) types[i] = previous;
    }
    size_t i = 0;
    while (i < count) {
        if (types[i] != ET) { ++i; continue; }
        size_t end = i;
        while (end + 1 < count && types[end + 1] == ET) ++end;
        const int touches = (i && types[i - 1] == EN) || (end + 1 < count && types[end + 1] == EN);
        if (touches) for (size_t j = i; j <= end; ++j) types[j] = EN;
        i = end + 1;
    }
    for (i = 0; i < count; ++i) if (types[i] == ES || types[i] == ET || types[i] == CS) types[i] = ON;
    last_strong = sos;
    for (i = 0; i < count; ++i) {
        if (types[i] == L || types[i] == R) last_strong = types[i];
        else if (types[i] == EN && last_strong == L) types[i] = L;
    }
    i = 0;
    while (i < count) {
        if (strong_direction(types[i]) >= 0) { ++i; continue; }
        size_t end = i;
        while (end + 1 < count && strong_direction(types[end + 1]) < 0) ++end;
        const int before = i ? strong_direction(types[i - 1]) : sos;
        const int after = end + 1 < count ? strong_direction(types[end + 1]) : sos;
        const uint8_t resolved = (uint8_t)(before == after ? before : sos);
        for (size_t j = i; j <= end; ++j) if (source[j] != B && source[j] != S) types[j] = resolved;
        i = end + 1;
    }
    for (i = 0; i < count; ++i) {
        const uint8_t type = types[i];
        if (source[i] == B || source[i] == S) levels[i] = base;
        else if (!(base & 1)) levels[i] = type == R ? base + 1 : (type == AN || type == EN) ? base + 2 : base;
        else levels[i] = (type == L || type == EN || type == AN) ? base + 1 : base;
    }
    size_t tail = count;
    while (tail) {
        const uint8_t original = source[tail - 1];
        if (original != WS && original != B && original != S && original != BN) break;
        levels[--tail] = base;
    }
}
void cove_bidi_visual_order(const uint8_t *levels, size_t count, size_t *order) {
    uint8_t highest = 0, lowest_odd = 255;
    for (size_t i = 0; i < count; ++i) {
        order[i] = i;
        if (levels[i] > highest) highest = levels[i];
        if ((levels[i] & 1) && levels[i] < lowest_odd) lowest_odd = levels[i];
    }
    if (lowest_odd == 255) return;
    for (int level = highest; level >= lowest_odd; --level) {
        size_t i = 0;
        while (i < count) {
            if (levels[order[i]] < level) { ++i; continue; }
            size_t end = i;
            while (end + 1 < count && levels[order[end + 1]] >= level) ++end;
            size_t lo = i, hi = end;
            while (lo < hi) { const size_t saved = order[lo]; order[lo++] = order[hi]; order[hi--] = saved; }
            i = end + 1;
        }
    }
}
