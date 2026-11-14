/* Fallback shim compiled when LLDB development files are absent at build time.
 * It keeps the C ABI so the loader always succeeds, but every operation reports
 * that the native backend is unavailable. The DAP adapter remains the fallback.
 */
#include "cove_lldb.h"

#include <string.h>

static const char *kUnavailable =
    "The native LLDB backend was not built: LLDB development files were not "
    "found at build time. Install LLVM (headers and liblldb) or set "
    "COVE_LLDB_DIR, then rebuild.";

static void clear(void *out, size_t size) {
  if (out) {
    memset(out, 0, size);
  }
}

COVE_LLDB_API int cove_lldb_initialize(void) { return COVE_LLDB_UNAVAILABLE; }
COVE_LLDB_API const char *cove_lldb_version(void) { return "unavailable"; }
COVE_LLDB_API const char *cove_lldb_last_error(void) { return kUnavailable; }

COVE_LLDB_API cove_session cove_lldb_session_create(void) { return NULL; }
COVE_LLDB_API void cove_lldb_session_destroy(cove_session session) { (void)session; }

COVE_LLDB_API int cove_lldb_attach(cove_session s, uint32_t p) {
  (void)s; (void)p; return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_attach_remote(cove_session s, const char *u,
                                        const char *p) {
  (void)s; (void)u; (void)p; return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_detach(cove_session s) { (void)s; return COVE_LLDB_UNAVAILABLE; }
COVE_LLDB_API void cove_lldb_abort(cove_session s) { (void)s; }

COVE_LLDB_API int cove_lldb_continue(cove_session s) { (void)s; return COVE_LLDB_UNAVAILABLE; }
COVE_LLDB_API int cove_lldb_pause(cove_session s) { (void)s; return COVE_LLDB_UNAVAILABLE; }
COVE_LLDB_API int cove_lldb_step(cove_session s, uint64_t t, int k) {
  (void)s; (void)t; (void)k; return COVE_LLDB_UNAVAILABLE;
}

COVE_LLDB_API int cove_lldb_get_state(cove_session s, cove_state *out) {
  (void)s; clear(out, sizeof(*out));
  if (out) out->state = COVE_STATE_CLOSED;
  return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_wait_event(cove_session s, uint32_t t) {
  (void)s; (void)t; return 0;
}
COVE_LLDB_API void cove_lldb_wake(cove_session s) { (void)s; }
COVE_LLDB_API int cove_lldb_drain_output(cove_session s, cove_output *b, int m, int *c) {
  (void)s; (void)b; (void)m; if (c) *c = 0; return COVE_LLDB_UNAVAILABLE;
}

COVE_LLDB_API int cove_lldb_threads(cove_session s, cove_thread *b, int m, int *c) {
  (void)s; (void)b; (void)m; if (c) *c = 0; return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_stack(cove_session s, uint64_t t, uint64_t st,
                                uint64_t l, cove_frame *b, int m, uint64_t *tot) {
  (void)s; (void)t; (void)st; (void)l; (void)b; (void)m;
  if (tot) *tot = 0; return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_scopes(cove_session s, cove_handle f, cove_scope *b,
                                 int m, int *c) {
  (void)s; (void)f; (void)b; (void)m; if (c) *c = 0; return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_variables(cove_session s, cove_handle r, uint64_t st,
                                    uint64_t ct, cove_variable *b, int m,
                                    uint64_t *tot) {
  (void)s; (void)r; (void)st; (void)ct; (void)b; (void)m;
  if (tot) *tot = 0; return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_evaluate(cove_session s, const char *e, cove_handle f,
                                   uint32_t t, cove_variable *out) {
  (void)s; (void)e; (void)f; (void)t; clear(out, sizeof(*out));
  return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_breakpoint_create(cove_session s, const char *p,
                                            uint32_t l, const char *c,
                                            const char *lm, uint64_t *id,
                                            int *v, uint32_t *rl) {
  (void)s; (void)p; (void)l; (void)c; (void)lm;
  if (id) *id = 0; if (v) *v = 0; if (rl) *rl = 0;
  return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_breakpoint_delete(cove_session s, uint64_t id) {
  (void)s; (void)id; return COVE_LLDB_UNAVAILABLE;
}COVE_LLDB_API int cove_lldb_breakpoint_info(cove_session s, uint64_t id, int *v,
                                          uint32_t *rl) {
  (void)s; (void)id; if (v) *v = 0; if (rl) *rl = 0; return COVE_LLDB_UNAVAILABLE;
}
COVE_LLDB_API int cove_lldb_set_exception_filter(cove_session s, const char *f, int e) {
  (void)s; (void)f; (void)e; return COVE_LLDB_UNAVAILABLE;
}
