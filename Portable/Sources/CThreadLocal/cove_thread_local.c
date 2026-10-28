// A minimal, dependency-free cross-platform thread-local slot.
//
// The composition runtime records `@State` reads on the calling thread while a
// view body runs. That lookup happens once per field read, so it has to be a raw
// thread-local load rather than a Foundation `Thread.threadDictionary` access
// (which hashes a string on every call).
//
// Keeping the platform branches here means the Swift side sees one uniform API
// and needs no `canImport` sprawl of its own.

#include "cove_thread_local.h"

#if defined(__wasi__)

// This WASI build is single-threaded, so one process-wide slot is equivalent to
// a thread-local one — and it avoids depending on any threading shim.
static void *cove_slot = NULL;

void cove_thread_local_init(cove_thread_local_destructor destructor) {
    (void)destructor; // slot lives for the whole process; nothing to reclaim
}

void *cove_thread_local_get(void) { return cove_slot; }

void cove_thread_local_set(void *value) { cove_slot = value; }

#elif defined(_WIN32)

#include <windows.h>

// FLS rather than TLS: it is the only Windows thread-local facility that runs
// a callback when a thread exits, which is how the retained value is released.
static DWORD cove_index = FLS_OUT_OF_INDEXES;
static INIT_ONCE cove_once = INIT_ONCE_STATIC_INIT;
static cove_thread_local_destructor cove_dtor = NULL;

static void NTAPI cove_destroy(PVOID value) {
    cove_thread_local_destructor dtor = cove_dtor;
    if (dtor != NULL && value != NULL) { dtor(value); }
}

static BOOL CALLBACK cove_create(PINIT_ONCE once, PVOID param, PVOID *context) {
    (void)once;
    (void)param;
    (void)context;
    cove_index = FlsAlloc(cove_destroy);
    return TRUE;
}

void cove_thread_local_init(cove_thread_local_destructor destructor) {
    cove_dtor = destructor;
    InitOnceExecuteOnce(&cove_once, cove_create, NULL, NULL);
}

void *cove_thread_local_get(void) {
    return cove_index == FLS_OUT_OF_INDEXES ? NULL : FlsGetValue(cove_index);
}

void cove_thread_local_set(void *value) {
    if (cove_index != FLS_OUT_OF_INDEXES) { (void)FlsSetValue(cove_index, value); }
}

#else

#include <pthread.h>

static pthread_key_t cove_key;
static pthread_once_t cove_once = PTHREAD_ONCE_INIT;
static cove_thread_local_destructor cove_dtor = NULL;

static void cove_destroy(void *value) {
    cove_thread_local_destructor dtor = cove_dtor;
    if (dtor != NULL && value != NULL) { dtor(value); }
}

static void cove_create(void) { (void)pthread_key_create(&cove_key, cove_destroy); }

void cove_thread_local_init(cove_thread_local_destructor destructor) {
    cove_dtor = destructor;
    (void)pthread_once(&cove_once, cove_create);
}

void *cove_thread_local_get(void) { return pthread_getspecific(cove_key); }

void cove_thread_local_set(void *value) { (void)pthread_setspecific(cove_key, value); }

#endif
