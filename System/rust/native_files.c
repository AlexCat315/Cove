#include <stdint.h>
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>

static wchar_t *cove_wide_path(const char *path) {
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
    if (!count) return NULL;
    wchar_t *wide = calloc((size_t)count, sizeof(wchar_t));
    if (!wide) { SetLastError(ERROR_NOT_ENOUGH_MEMORY); return NULL; }
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, count)) {
        DWORD error = GetLastError(); free(wide); SetLastError(error); return NULL;
    }
    return wide;
}
uint32_t cove_replace_file(const char *source, const char *destination) {
    wchar_t *from = cove_wide_path(source);
    if (!from) return GetLastError();
    wchar_t *to = cove_wide_path(destination);
    if (!to) { DWORD error = GetLastError(); free(from); return error; }
    DWORD error = ERROR_SUCCESS;
    for (int attempt = 0; attempt < 5; ++attempt) {
        if (MoveFileExW(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) { error = ERROR_SUCCESS; break; }
        error = GetLastError();
        if (attempt == 4 || (error != ERROR_SHARING_VIOLATION && error != ERROR_LOCK_VIOLATION)) break;
        Sleep((DWORD)(25 << attempt));
    }
    free(from); free(to); return error;
}
#else
#include <stdio.h>
#include <errno.h>
uint32_t cove_replace_file(const char *source, const char *destination) {
    return rename(source, destination) == 0 ? 0 : (uint32_t)errno;
}
#endif
