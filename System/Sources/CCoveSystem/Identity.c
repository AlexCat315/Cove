#include "CoveSystem.h"
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>

static HANDLE open_path(const char *path) {
    int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
    if (!length) return INVALID_HANDLE_VALUE;
    wchar_t *wide = calloc((size_t)length, sizeof(wchar_t));
    if (!wide) return INVALID_HANDLE_VALUE;
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, length);
    HANDLE result = CreateFileW(wide, FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS, NULL);
    free(wide);
    return result;
}

int cove_same_file(const char *lhs, const char *rhs) {
    HANDLE left = open_path(lhs), right = open_path(rhs);
    BY_HANDLE_FILE_INFORMATION a, b;
    int result = -1;
    if (left != INVALID_HANDLE_VALUE && right != INVALID_HANDLE_VALUE &&
        GetFileInformationByHandle(left, &a) && GetFileInformationByHandle(right, &b)) {
        result = a.dwVolumeSerialNumber == b.dwVolumeSerialNumber &&
            a.nFileIndexHigh == b.nFileIndexHigh && a.nFileIndexLow == b.nFileIndexLow;
    }
    if (left != INVALID_HANDLE_VALUE) CloseHandle(left);
    if (right != INVALID_HANDLE_VALUE) CloseHandle(right);
    return result;
}
#else
#include <sys/stat.h>
int cove_same_file(const char *lhs, const char *rhs) {
    struct stat a, b;
    if (stat(lhs, &a) || stat(rhs, &b)) return -1;
    return a.st_dev == b.st_dev && a.st_ino == b.st_ino;
}
#endif
