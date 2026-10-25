#include "GuavaSystem.h"
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <aclapi.h>
#include <stdlib.h>

uint32_t guava_restrict_path(const char *path, int directory) {
    int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
    if (!length) return GetLastError();
    wchar_t *wide = calloc((size_t)length, sizeof(wchar_t));
    if (!wide) return ERROR_NOT_ENOUGH_MEMORY;
    MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, length);
    HANDLE token = NULL;
    DWORD error = ERROR_SUCCESS, size = 0;
    TOKEN_USER *user = NULL;
    PACL acl = NULL;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) {
        error = GetLastError(); goto cleanup;
    }
    GetTokenInformation(token, TokenUser, NULL, 0, &size);
    user = malloc(size);
    if (!user) { error = ERROR_NOT_ENOUGH_MEMORY; goto cleanup; }
    if (!GetTokenInformation(token, TokenUser, user, size, &size)) {
        error = GetLastError(); goto cleanup;
    }
    EXPLICIT_ACCESSW access = {0};
    access.grfAccessPermissions = FILE_ALL_ACCESS;
    access.grfAccessMode = SET_ACCESS;
    access.grfInheritance = directory ? SUB_CONTAINERS_AND_OBJECTS_INHERIT : NO_INHERITANCE;
    access.Trustee.TrusteeForm = TRUSTEE_IS_SID;
    access.Trustee.TrusteeType = TRUSTEE_IS_USER;
    access.Trustee.ptstrName = (LPWSTR)user->User.Sid;
    error = SetEntriesInAclW(1, &access, NULL, &acl);
    if (!error) {
        error = SetNamedSecurityInfoW(wide, SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            user->User.Sid, NULL, acl, NULL);
    }
cleanup:
    if (acl) LocalFree(acl);
    if (token) CloseHandle(token);
    free(user);
    free(wide);
    return error;
}
#else
#include <sys/stat.h>
#include <errno.h>
uint32_t guava_restrict_path(const char *path, int directory) {
    return chmod(path, directory ? 0700 : 0600) == 0 ? 0 : (uint32_t)errno;
}
#endif
