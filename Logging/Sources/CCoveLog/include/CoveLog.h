#pragma once
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

typedef enum { COVE_LOG_TRACE, COVE_LOG_DEBUG, COVE_LOG_INFO, COVE_LOG_NOTICE,
    COVE_LOG_WARNING, COVE_LOG_ERROR, COVE_LOG_CRITICAL } CoveLogLevel;
typedef void (*CoveLogCallback)(int32_t level, const char *label, const char *message,
    const char *file, const char *function, uint32_t line);
typedef void (*CoveLogFlushCallback)(void);
void cove_log_set_handler(CoveLogCallback handler, CoveLogFlushCallback flush);
void cove_log_flush(void);
void cove_log_write(int32_t level, const char *label, const char *message,
    const char *file, const char *function, uint32_t line);
void cove_log_printf(int32_t level, const char *label, const char *file, const char *function, uint32_t line, const char *format, ...);
void cove_log_console_write(int32_t level, const char *message);
intptr_t cove_log_lock_file(const char *path);
void cove_log_unlock_file(intptr_t handle);
#define COVE_LOGF(level, label, ...) cove_log_printf(level, label, __FILE__, __func__, __LINE__, __VA_ARGS__)
#define COVE_LOG(level, label, message) cove_log_write(level, label, message, __FILE__, __func__, __LINE__)
#ifdef __cplusplus
}
#endif
