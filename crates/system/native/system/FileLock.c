#include "CoveSystem.h"
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>

intptr_t cove_file_lock(const char *path, uint32_t *error) {
    int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
    if (!length) { *error = GetLastError(); return -1; }
    wchar_t *wide = calloc((size_t)length, sizeof(wchar_t));
    if (!wide) { *error = ERROR_NOT_ENOUGH_MEMORY; return -1; }
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, length);
    HANDLE handle = CreateFileW(wide, GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    free(wide);
    if (handle == INVALID_HANDLE_VALUE) { *error = GetLastError(); return -1; }
    OVERLAPPED offset = {0};
    if (!LockFileEx(handle, LOCKFILE_EXCLUSIVE_LOCK, 0, MAXDWORD, MAXDWORD, &offset)) {
        *error = GetLastError(); CloseHandle(handle); return -1;
    }
    return (intptr_t)handle;
}

intptr_t cove_file_try_lock(const char *path) {
    int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
    if (!length) { return -1; }
    wchar_t *wide = calloc((size_t)length, sizeof(wchar_t));
    if (!wide) { return -1; }
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, length);
    HANDLE handle = CreateFileW(wide, GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_ALWAYS,
        FILE_ATTRIBUTE_NORMAL, NULL);
    free(wide);
    if (handle == INVALID_HANDLE_VALUE) { return -1; }
    OVERLAPPED offset = {0};
    if (!LockFileEx(handle, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY, 0, MAXDWORD, MAXDWORD, &offset)) {
        CloseHandle(handle); return -1;
    }
    return (intptr_t)handle;
}

void cove_file_unlock(intptr_t value) {
    OVERLAPPED offset = {0};
    HANDLE handle = (HANDLE)value;
    UnlockFileEx(handle, 0, MAXDWORD, MAXDWORD, &offset);
    CloseHandle(handle);
}
#else
#include <errno.h>
#include <fcntl.h>
#include <sys/file.h>
#include <unistd.h>

intptr_t cove_file_lock(const char *path, uint32_t *error) {
    int fd = open(path, O_CREAT | O_RDWR | O_CLOEXEC, 0600);
    if (fd < 0) { *error = (uint32_t)errno; return -1; }
    while (flock(fd, LOCK_EX)) {
        if (errno == EINTR) continue;
        *error = (uint32_t)errno; close(fd); return -1;
    }
    return fd;
}

intptr_t cove_file_try_lock(const char *path) {
    int fd = open(path, O_CREAT | O_RDWR | O_CLOEXEC, 0600);
    if (fd < 0) { return -1; }
    if (flock(fd, LOCK_EX | LOCK_NB) != 0) { close(fd); return -1; }
    return (intptr_t)fd;
}

void cove_file_unlock(intptr_t handle) {
    flock((int)handle, LOCK_UN);
    close((int)handle);
}
#endif
