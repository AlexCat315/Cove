/* Cove native LLDB bridge: a plain C API over the LLDB SB (C++) API.
 *
 * The Rust debugger crate loads the compiled shim at runtime; nothing in the
 * controller links liblldb directly. When liblldb or the shim is missing the
 * backend reports `COVE_LLDB_UNAVAILABLE` and the DAP adapter remains the
 * fallback. Every SB object lives behind an opaque handle owned by the shim.
 */
#ifndef COVE_LLDB_H
#define COVE_LLDB_H

#include <stddef.h>
#include <stdint.h>

/* MSVC rejects a dllexport definition whose prior extern "C" declaration lacked
 * the attribute (C2375), so the export must sit on the declarations too. */
#if defined(_WIN32)
#define COVE_LLDB_API __declspec(dllexport)
#else
#define COVE_LLDB_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define COVE_LLDB_OK 0
#define COVE_LLDB_ERR 1
#define COVE_LLDB_UNAVAILABLE 2

/* Session states, mirroring the wire model's SessionState. */
enum {
  COVE_STATE_STARTING = 0,
  COVE_STATE_ATTACHED = 1,
  COVE_STATE_RUNNING = 2,
  COVE_STATE_STOPPED = 3,
  COVE_STATE_EXITED = 4,
  COVE_STATE_TERMINATED = 5,
  COVE_STATE_CLOSED = 6
};

/* Why the selected thread stopped. */
enum {
  COVE_STOP_NONE = 0,
  COVE_STOP_BREAKPOINT = 1,
  COVE_STOP_STEP = 2,
  COVE_STOP_SIGNAL = 3,
  COVE_STOP_EXCEPTION = 4,
  COVE_STOP_WATCHPOINT = 5,
  COVE_STOP_OTHER = 6
};

/* Step kinds for cove_lldb_step. */
enum { COVE_STEP_OVER = 0, COVE_STEP_IN = 1, COVE_STEP_OUT = 2 };

/* Output categories for cove_lldb_drain_output. */
enum {
  COVE_OUTPUT_CONSOLE = 0,
  COVE_OUTPUT_STDOUT = 1,
  COVE_OUTPUT_STDERR = 2,
  COVE_OUTPUT_LOG = 3
};

typedef void *cove_session;
/* Opaque frame/value handle. 0 means "no handle / no children". */
typedef uint64_t cove_handle;

/* Global lifecycle. cove_lldb_initialize is idempotent and must succeed before
 * any session is created. cove_lldb_version returns a static string. */
COVE_LLDB_API int cove_lldb_initialize(void);
COVE_LLDB_API const char *cove_lldb_version(void);
/* Thread-local detail for the most recent failure on this thread. */
COVE_LLDB_API const char *cove_lldb_last_error(void);

COVE_LLDB_API cove_session cove_lldb_session_create(void);
COVE_LLDB_API void cove_lldb_session_destroy(cove_session session);

/* Attach by local PID, or to a gdb-remote stub URL (connect://127.0.0.1:1234).
 * `program` may be NULL. Both return once the attach is under way; the first
 * stop arrives through cove_lldb_wait_event. */
COVE_LLDB_API int cove_lldb_attach(cove_session session, uint32_t pid);
COVE_LLDB_API int cove_lldb_attach_remote(cove_session session, const char *url,
                            const char *program);
/* Detach without terminating the debuggee. */
COVE_LLDB_API int cove_lldb_detach(cove_session session);
/* Force cleanup; safe to call repeatedly and after detach. */
COVE_LLDB_API void cove_lldb_abort(cove_session session);

COVE_LLDB_API int cove_lldb_continue(cove_session session);
COVE_LLDB_API int cove_lldb_pause(cove_session session);
COVE_LLDB_API int cove_lldb_step(cove_session session, uint64_t thread_id, int kind);

typedef struct {
  int state;
  int stop_reason;
  uint64_t thread_id;
  int exit_code;
  int has_exit_code;
  uint64_t breakpoint_ids[8];
  int num_breakpoints;
  char description[512];
  char process_name[256];
  uint32_t pid;
  /* Incremented for every stop folded into the state, so a caller can tell a
   * new stop from the one it started with. */
  uint64_t stop_count;
} cove_state;

/* Copies the current observed state. Only the event thread mutates it. */
COVE_LLDB_API int cove_lldb_get_state(cove_session session, cove_state *out);
/* Blocks up to timeout_ms for one process event, folds it into the observed
 * state, and returns 1 when an event was consumed or 0 on timeout. */
COVE_LLDB_API int cove_lldb_wait_event(cove_session session, uint32_t timeout_ms);
/* Makes a blocked cove_lldb_wait_event return promptly. */
COVE_LLDB_API void cove_lldb_wake(cove_session session);

typedef struct {
  int category;
  char text[1024];
} cove_output;
/* Moves queued log point and process output into buf, oldest first. */
COVE_LLDB_API int cove_lldb_drain_output(cove_session session, cove_output *buf, int max,
                           int *out_count);

typedef struct {
  uint64_t id;
  char name[128];
} cove_thread;
COVE_LLDB_API int cove_lldb_threads(cove_session session, cove_thread *buf, int max,
                      int *out_count);

typedef struct {
  cove_handle id;
  char name[256];
  char path[512];
  uint32_t line;
  uint32_t column;
  char pc[32];
} cove_frame;
COVE_LLDB_API int cove_lldb_stack(cove_session session, uint64_t thread_id, uint64_t start,
                    uint64_t levels, cove_frame *buf, int max,
                    uint64_t *out_total);

typedef struct {
  char name[64];
  cove_handle ref;
  int expensive;
} cove_scope;
COVE_LLDB_API int cove_lldb_scopes(cove_session session, cove_handle frame, cove_scope *buf,
                     int max, int *out_count);

typedef struct {
  char name[128];
  char value[1024];
  char type[128];
  char evaluate_name[256];
  cove_handle ref;
  uint64_t num_children;
  uint64_t address;
  int has_address;
  int value_truncated;
} cove_variable;
COVE_LLDB_API int cove_lldb_variables(cove_session session, cove_handle ref, uint64_t start,
                        uint64_t count, cove_variable *buf, int max,
                        uint64_t *out_total);

COVE_LLDB_API int cove_lldb_evaluate(cove_session session, const char *expression,
                       cove_handle frame, uint32_t timeout_ms,
                       cove_variable *out);

/* Creates one source breakpoint. condition/log_message may be NULL. */
COVE_LLDB_API int cove_lldb_breakpoint_create(cove_session session, const char *path,
                                uint32_t line, const char *condition,
                                const char *log_message, uint64_t *out_id,
                                int *out_verified, uint32_t *out_resolved_line);
COVE_LLDB_API int cove_lldb_breakpoint_delete(cove_session session, uint64_t id);
/* Current resolution of a breakpoint; a pending one resolves on module load. */
COVE_LLDB_API int cove_lldb_breakpoint_info(cove_session session, uint64_t id,
                              int *out_verified, uint32_t *out_resolved_line);

/* Enables or disables one exception filter: cpp_throw, cpp_catch, objc_throw,
 * objc_catch or rust_panic. */
COVE_LLDB_API int cove_lldb_set_exception_filter(cove_session session, const char *filter,
                                   int enabled);

#ifdef __cplusplus
}
#endif

#endif /* COVE_LLDB_H */