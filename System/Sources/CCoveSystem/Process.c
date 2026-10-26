#include "CoveSystem.h"
#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>
#include <wchar.h>

typedef struct { HANDLE process; HANDLE job; DWORD pid; } CoveProcess;

void *cove_process_start(const uint16_t *executable, const uint16_t *command,
    const uint16_t *environment, const uint16_t *directory,
    void *input, void *output, void *errors, uint32_t *error) {
    CoveProcess *child = calloc(1, sizeof(CoveProcess));
    if (!child) { *error = ERROR_NOT_ENOUGH_MEMORY; return NULL; }
    HANDLE inherited[3] = {NULL, NULL, NULL};
    HANDLE originals[3] = {(HANDLE)input, (HANDLE)output, (HANDLE)errors};
    PROCESS_INFORMATION information = {0};
    STARTUPINFOEXW startup = {0};
    wchar_t *line = _wcsdup((const wchar_t *)command);
    SIZE_T size = 0;
    DWORD failure = ERROR_SUCCESS;
    if (!line) { failure = ERROR_NOT_ENOUGH_MEMORY; goto cleanup; }
    child->job = CreateJobObjectW(NULL, NULL);
    if (!child->job) { failure = GetLastError(); goto cleanup; }
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits = {0};
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(child->job, JobObjectExtendedLimitInformation, &limits, sizeof(limits))) {
        failure = GetLastError(); goto cleanup;
    }
    for (int i = 0; i < 3; i++) {
        if (!DuplicateHandle(GetCurrentProcess(), originals[i], GetCurrentProcess(), &inherited[i], 0, TRUE, DUPLICATE_SAME_ACCESS)) {
            failure = GetLastError(); goto cleanup;
        }
    }
    InitializeProcThreadAttributeList(NULL, 1, 0, &size);
    startup.lpAttributeList = HeapAlloc(GetProcessHeap(), 0, size);
    if (!startup.lpAttributeList) { failure = ERROR_NOT_ENOUGH_MEMORY; goto cleanup; }
    if (!InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &size)) {
        failure = GetLastError(); HeapFree(GetProcessHeap(), 0, startup.lpAttributeList);
        startup.lpAttributeList = NULL; goto cleanup;
    }
    if (!UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        inherited, sizeof(inherited), NULL, NULL)) { failure = GetLastError(); goto cleanup; }
    startup.StartupInfo.cb = sizeof(startup);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = inherited[0];
    startup.StartupInfo.hStdOutput = inherited[1];
    startup.StartupInfo.hStdError = inherited[2];
    if (!CreateProcessW((LPCWSTR)executable, line, NULL, NULL, TRUE,
        CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
        (LPVOID)environment, (LPCWSTR)directory, &startup.StartupInfo, &information)) {
        failure = GetLastError(); goto cleanup;
    }
    if (!AssignProcessToJobObject(child->job, information.hProcess)) { failure = GetLastError(); goto cleanup; }
    if (ResumeThread(information.hThread) == (DWORD)-1) { failure = GetLastError(); goto cleanup; }
    child->process = information.hProcess;
    child->pid = information.dwProcessId;
cleanup:
    if (startup.lpAttributeList) {
        DeleteProcThreadAttributeList(startup.lpAttributeList);
        HeapFree(GetProcessHeap(), 0, startup.lpAttributeList);
    }
    for (int i = 0; i < 3; i++) if (inherited[i]) CloseHandle(inherited[i]);
    if (information.hThread) CloseHandle(information.hThread);
    free(line);
    *error = failure;
    if (failure) {
        if (information.hProcess) { TerminateProcess(information.hProcess, 1); CloseHandle(information.hProcess); }
        if (child->job) CloseHandle(child->job);
        free(child);
        return NULL;
    }
    return child;
}
int32_t cove_process_pid(void *value) { return (int32_t)((CoveProcess *)value)->pid; }
int cove_process_running(void *value) { return WaitForSingleObject(((CoveProcess *)value)->process, 0) == WAIT_TIMEOUT; }
int32_t cove_process_wait(void *value) {
    CoveProcess *child = value;
    WaitForSingleObject(child->process, INFINITE);
    DWORD code = 1;
    GetExitCodeProcess(child->process, &code);
    return (int32_t)code;
}
void cove_process_terminate(void *value) { TerminateJobObject(((CoveProcess *)value)->job, 1); }
void cove_process_destroy(void *value) {
    CoveProcess *child = value;
    CloseHandle(child->job);
    CloseHandle(child->process);
    free(child);
}
#endif
