#pragma once
#include <stdint.h>

// Return zero on success, otherwise an errno or Win32 error code.
uint32_t cove_restrict_path(const char *path, int directory);
int cove_same_file(const char *lhs, const char *rhs);
intptr_t cove_file_lock(const char *path, uint32_t *error);
void cove_file_unlock(intptr_t handle);

#if defined(_WIN32)
void *cove_process_start(const uint16_t *executable, const uint16_t *command,
    const uint16_t *environment, const uint16_t *directory,
    void *input, void *output, void *errors, uint32_t *error);
int32_t cove_process_pid(void *value);
int cove_process_running(void *value);
int32_t cove_process_wait(void *value);
void cove_process_terminate(void *value);
void cove_process_destroy(void *value);
#endif
