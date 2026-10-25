#pragma once
#include <stdint.h>

// Return zero on success, otherwise an errno or Win32 error code.
uint32_t guava_restrict_path(const char *path, int directory);
int guava_same_file(const char *lhs, const char *rhs);

#if defined(_WIN32)
void *guava_process_start(const uint16_t *executable, const uint16_t *command,
    const uint16_t *environment, const uint16_t *directory,
    void *input, void *output, void *errors, uint32_t *error);
int32_t guava_process_pid(void *value);
int guava_process_running(void *value);
int32_t guava_process_wait(void *value);
void guava_process_terminate(void *value);
void guava_process_destroy(void *value);
#endif
