#ifndef COVE_THREAD_LOCAL_H
#define COVE_THREAD_LOCAL_H

#include <stddef.h>

/// Invoked once per thread when the slot still holds a value as the thread exits.
///
/// Only platforms that can run a destructor on thread exit invoke this; the
/// caller must treat it as best-effort and must not depend on it for correctness.
typedef void (*cove_thread_local_destructor)(void *_Nullable value);

/// Create the platform thread-local slot.
///
/// Idempotent and safe to call from concurrent threads. Passing a different
/// `destructor` on a later call has no effect — the first one wins — so callers
/// should always pass the same function.
void cove_thread_local_init(cove_thread_local_destructor _Nullable destructor);

/// Raw thread-local load. Returns `NULL` when the slot is empty.
void *_Nullable cove_thread_local_get(void);

/// Raw thread-local store. Passing `NULL` clears the slot.
void cove_thread_local_set(void *_Nullable value);

#endif /* COVE_THREAD_LOCAL_H */
