// TLS client over a connected socket, using the system OpenSSL loaded at run
// time. Declaring the few entry points here (instead of including OpenSSL's
// headers) keeps the build free of a target-specific OpenSSL SDK: the same
// object links on Linux, Android and other Unix targets, and the library is
// resolved from the device (Linux distributions) or the app bundle (Android).
//
// Only ABI-stable functions present in OpenSSL 1.1.1 through 4.x are used;
// macros such as SSL_set_tlsext_host_name are expanded to their SSL_ctrl calls.

#include <dirent.h>
#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct ssl_st SSL;
typedef struct ssl_ctx_st SSL_CTX;
typedef struct ssl_method_st SSL_METHOD;
typedef struct x509_st X509;
typedef struct x509_store_st X509_STORE;
typedef struct x509_verify_param_st X509_VERIFY_PARAM;
typedef struct bio_st BIO;

#define SSL_VERIFY_PEER 0x01
#define SSL_ERROR_SSL 1
#define SSL_ERROR_WANT_READ 2
#define SSL_ERROR_WANT_WRITE 3
#define SSL_ERROR_SYSCALL 5
#define SSL_ERROR_ZERO_RETURN 6
#define SSL_CTRL_OPTIONS 32
#define SSL_CTRL_SET_TLSEXT_HOSTNAME 55
#define SSL_CTRL_SET_MIN_PROTO_VERSION 123
#define TLSEXT_NAMETYPE_host_name 0
#define TLS1_2_VERSION 0x0303
#define SSL_OP_IGNORE_UNEXPECTED_EOF (1UL << 7)
#define X509_V_OK 0

static struct {
    void *ssl;
    void *crypto;
    unsigned long version;
    const SSL_METHOD *(*TLS_client_method)(void);
    SSL_CTX *(*SSL_CTX_new)(const SSL_METHOD *);
    int (*SSL_CTX_set_default_verify_paths)(SSL_CTX *);
    int (*SSL_CTX_load_verify_locations)(SSL_CTX *, const char *, const char *);
    void (*SSL_CTX_set_verify)(SSL_CTX *, int, void *);
    long (*SSL_CTX_ctrl)(SSL_CTX *, int, long, void *);
    uint64_t (*SSL_CTX_set_options)(SSL_CTX *, uint64_t);
    X509_STORE *(*SSL_CTX_get_cert_store)(const SSL_CTX *);
    SSL *(*SSL_new)(SSL_CTX *);
    void (*SSL_free)(SSL *);
    int (*SSL_set_fd)(SSL *, int);
    long (*SSL_ctrl)(SSL *, int, long, void *);
    int (*SSL_set1_host)(SSL *, const char *);
    X509_VERIFY_PARAM *(*SSL_get0_param)(SSL *);
    int (*SSL_connect)(SSL *);
    int (*SSL_read)(SSL *, void *, int);
    int (*SSL_write)(SSL *, const void *, int);
    int (*SSL_shutdown)(SSL *);
    int (*SSL_get_error)(const SSL *, int);
    long (*SSL_get_verify_result)(const SSL *);
    unsigned long (*OpenSSL_version_num)(void);
    unsigned long (*ERR_get_error)(void);
    void (*ERR_clear_error)(void);
    void (*ERR_error_string_n)(unsigned long, char *, size_t);
    const char *(*X509_verify_cert_error_string)(long);
    int (*X509_VERIFY_PARAM_set1_ip_asc)(X509_VERIFY_PARAM *, const char *);
    BIO *(*BIO_new_file)(const char *, const char *);
    int (*BIO_free)(BIO *);
    X509 *(*PEM_read_bio_X509)(BIO *, X509 **, void *, void *);
    int (*X509_STORE_add_cert)(X509_STORE *, X509 *);
    void (*X509_free)(X509 *);
    SSL_CTX *context;
    char failure[256];
} api;

static pthread_once_t once = PTHREAD_ONCE_INIT;

static void *open_first(const char *directory, const char *const *names) {
    char path[1024];
    for (size_t index = 0; names[index]; index++) {
        const char *name = names[index];
        if (directory && *directory) {
            snprintf(path, sizeof path, "%s/%s", directory, name);
            name = path;
        }
        void *handle = dlopen(name, RTLD_NOW | RTLD_LOCAL);
        if (handle) return handle;
    }
    return NULL;
}

#define BIND(library, name)                                                          \
    do {                                                                             \
        *(void **)(&api.name) = dlsym(api.library, #name);                           \
        if (!api.name) {                                                             \
            snprintf(api.failure, sizeof api.failure, "OpenSSL lacks %s", #name);     \
            return;                                                                  \
        }                                                                            \
    } while (0)

static int add_directory(X509_STORE *store, const char *directory) {
    DIR *entries = opendir(directory);
    if (!entries) return 0;
    int added = 0;
    struct dirent *entry;
    char path[1024];
    while ((entry = readdir(entries))) {
        if (entry->d_name[0] == '.') continue;
        snprintf(path, sizeof path, "%s/%s", directory, entry->d_name);
        BIO *file = api.BIO_new_file(path, "r");
        if (!file) continue;
        X509 *certificate;
        while ((certificate = api.PEM_read_bio_X509(file, NULL, NULL, NULL))) {
            if (api.X509_STORE_add_cert(store, certificate) == 1) added++;
            api.X509_free(certificate);
        }
        api.BIO_free(file);
    }
    closedir(entries);
    api.ERR_clear_error();
    return added;
}

static void load_trust(SSL_CTX *context) {
    // SSL_CERT_FILE / SSL_CERT_DIR and the build's own default locations.
    api.SSL_CTX_set_default_verify_paths(context);
    static const char *const bundles[] = {
        "/etc/ssl/certs/ca-certificates.crt",
        "/etc/pki/tls/certs/ca-bundle.crt",
        "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
        "/etc/ssl/ca-bundle.pem",
        "/etc/ssl/cert.pem",
        "/usr/local/share/certs/ca-root-nss.crt",
        NULL,
    };
    for (size_t index = 0; bundles[index]; index++) {
        if (api.SSL_CTX_load_verify_locations(context, bundles[index], NULL) == 1) break;
    }
    // Android keeps one PEM file per certificate, named by a hash OpenSSL's
    // directory lookup does not use, so the files are loaded one by one.
    static const char *const directories[] = {
        "/apex/com.android.conscrypt/cacerts",
        "/system/etc/security/cacerts",
        NULL,
    };
    X509_STORE *store = api.SSL_CTX_get_cert_store(context);
    for (size_t index = 0; directories[index]; index++) {
        if (add_directory(store, directories[index]) > 0) break;
    }
    api.ERR_clear_error();
}

static void initialize(void) {
    const char *directory = getenv("COVE_OPENSSL_LIB_DIR");
#if defined(__APPLE__)
    static const char *const ssl_names[] = {"libssl.3.dylib", "libssl.4.dylib", "libssl.dylib", NULL};
    static const char *const crypto_names[] = {"libcrypto.3.dylib", "libcrypto.4.dylib", "libcrypto.dylib", NULL};
#elif defined(__ANDROID__)
    static const char *const ssl_names[] = {"libssl.so", "libssl.so.3", NULL};
    static const char *const crypto_names[] = {"libcrypto.so", "libcrypto.so.3", NULL};
#else
    static const char *const ssl_names[] = {"libssl.so.3", "libssl.so.4", "libssl.so.1.1", "libssl.so", NULL};
    static const char *const crypto_names[] = {"libcrypto.so.3", "libcrypto.so.4", "libcrypto.so.1.1", "libcrypto.so", NULL};
#endif
    // libcrypto first so libssl's dependency resolves to the same copy.
    api.crypto = open_first(directory, crypto_names);
    api.ssl = api.crypto ? open_first(directory, ssl_names) : NULL;
    if (!api.ssl) {
        snprintf(api.failure, sizeof api.failure, "OpenSSL (libssl) is not installed");
        return;
    }
    BIND(crypto, OpenSSL_version_num);
    api.version = api.OpenSSL_version_num();
    if (api.version < 0x10101000UL) {
        snprintf(api.failure, sizeof api.failure, "OpenSSL %lx is older than 1.1.1", api.version);
        return;
    }
    BIND(ssl, TLS_client_method);
    BIND(ssl, SSL_CTX_new);
    BIND(ssl, SSL_CTX_set_default_verify_paths);
    BIND(ssl, SSL_CTX_load_verify_locations);
    BIND(ssl, SSL_CTX_set_verify);
    BIND(ssl, SSL_CTX_ctrl);
    BIND(ssl, SSL_CTX_get_cert_store);
    BIND(ssl, SSL_new);
    BIND(ssl, SSL_free);
    BIND(ssl, SSL_set_fd);
    BIND(ssl, SSL_ctrl);
    BIND(ssl, SSL_set1_host);
    BIND(ssl, SSL_get0_param);
    BIND(ssl, SSL_connect);
    BIND(ssl, SSL_read);
    BIND(ssl, SSL_write);
    BIND(ssl, SSL_shutdown);
    BIND(ssl, SSL_get_error);
    BIND(ssl, SSL_get_verify_result);
    BIND(crypto, ERR_get_error);
    BIND(crypto, ERR_clear_error);
    BIND(crypto, ERR_error_string_n);
    BIND(crypto, X509_verify_cert_error_string);
    BIND(crypto, X509_VERIFY_PARAM_set1_ip_asc);
    BIND(crypto, BIO_new_file);
    BIND(crypto, BIO_free);
    BIND(crypto, PEM_read_bio_X509);
    BIND(crypto, X509_STORE_add_cert);
    BIND(crypto, X509_free);
    // 3.0 widened SSL_CTX_set_options to uint64_t; 1.1.1 takes unsigned long.
    if (api.version >= 0x30000000UL) BIND(ssl, SSL_CTX_set_options);

    SSL_CTX *context = api.SSL_CTX_new(api.TLS_client_method());
    if (!context) {
        snprintf(api.failure, sizeof api.failure, "SSL_CTX_new failed");
        return;
    }
    api.SSL_CTX_ctrl(context, SSL_CTRL_SET_MIN_PROTO_VERSION, TLS1_2_VERSION, NULL);
    // Servers that close without close_notify are common with Connection:
    // close; body framing still detects truncation.
    if (api.SSL_CTX_set_options) {
        api.SSL_CTX_set_options(context, SSL_OP_IGNORE_UNEXPECTED_EOF);
    }
    api.SSL_CTX_set_verify(context, SSL_VERIFY_PEER, NULL);
    load_trust(context);
    api.context = context;
}

static void describe(char *error, size_t length, const char *prefix) {
    unsigned long code = api.ERR_get_error();
    if (code) {
        char detail[200];
        api.ERR_error_string_n(code, detail, sizeof detail);
        snprintf(error, length, "%s: %s", prefix, detail);
    } else {
        snprintf(error, length, "%s", prefix);
    }
    api.ERR_clear_error();
}

/// 1 when TLS is usable; otherwise 0 with the reason in `error`.
int cove_http_tls_available(char *error, size_t length) {
    pthread_once(&once, initialize);
    if (api.context) return 1;
    snprintf(error, length, "%s", api.failure[0] ? api.failure : "OpenSSL is unavailable");
    return 0;
}

/// Handshakes on the connected blocking socket `fd`, verifying the chain and
/// `host` (a DNS name or IP literal). Returns NULL with `error` filled; sets
/// `*timed_out` when the socket timeout expired.
SSL *cove_http_tls_connect(int fd, const char *host, int ip_literal, char *error, size_t length,
                           int *timed_out) {
    *timed_out = 0;
    if (!cove_http_tls_available(error, length)) return NULL;
    api.ERR_clear_error();
    SSL *ssl = api.SSL_new(api.context);
    if (!ssl) {
        describe(error, length, "SSL_new failed");
        return NULL;
    }
    int configured = api.SSL_set_fd(ssl, fd) == 1;
    if (configured && ip_literal) {
        configured = api.X509_VERIFY_PARAM_set1_ip_asc(api.SSL_get0_param(ssl), host) == 1;
    } else if (configured) {
        configured = api.SSL_ctrl(ssl, SSL_CTRL_SET_TLSEXT_HOSTNAME, TLSEXT_NAMETYPE_host_name,
                                  (void *)host) == 1 &&
                     api.SSL_set1_host(ssl, host) == 1;
    }
    if (!configured) {
        describe(error, length, "TLS setup failed");
        api.SSL_free(ssl);
        return NULL;
    }
    errno = 0;
    int result = api.SSL_connect(ssl);
    if (result != 1) {
        int reason = api.SSL_get_error(ssl, result);
        long verify = api.SSL_get_verify_result(ssl);
        if (verify != X509_V_OK) {
            snprintf(error, length, "certificate verification failed: %s",
                     api.X509_verify_cert_error_string(verify));
            api.ERR_clear_error();
        } else if ((reason == SSL_ERROR_SYSCALL || reason == SSL_ERROR_WANT_READ ||
                    reason == SSL_ERROR_WANT_WRITE) &&
                   (errno == EAGAIN || errno == EWOULDBLOCK)) {
            *timed_out = 1;
            snprintf(error, length, "TLS handshake timed out");
            api.ERR_clear_error();
        } else {
            describe(error, length, "TLS handshake failed");
        }
        api.SSL_free(ssl);
        return NULL;
    }
    return ssl;
}

/// Bytes read (> 0), 0 at end of stream, -1 on error, -2 on timeout.
long cove_http_tls_read(SSL *ssl, void *buffer, size_t length, char *error, size_t error_length) {
    int count = length > 0x40000000 ? 0x40000000 : (int)length;
    errno = 0;
    int result = api.SSL_read(ssl, buffer, count);
    if (result > 0) return result;
    int reason = api.SSL_get_error(ssl, result);
    if (reason == SSL_ERROR_ZERO_RETURN) return 0;
    if ((reason == SSL_ERROR_SYSCALL || reason == SSL_ERROR_WANT_READ) &&
        (errno == EAGAIN || errno == EWOULDBLOCK)) {
        api.ERR_clear_error();
        return -2;
    }
    if (reason == SSL_ERROR_SYSCALL && api.ERR_get_error() == 0 && errno == 0) return 0;
    describe(error, error_length, "TLS read failed");
    return -1;
}

/// Bytes written (> 0), -1 on error, -2 on timeout.
long cove_http_tls_write(SSL *ssl, const void *buffer, size_t length, char *error,
                         size_t error_length) {
    int count = length > 0x40000000 ? 0x40000000 : (int)length;
    errno = 0;
    int result = api.SSL_write(ssl, buffer, count);
    if (result > 0) return result;
    int reason = api.SSL_get_error(ssl, result);
    if ((reason == SSL_ERROR_SYSCALL || reason == SSL_ERROR_WANT_WRITE) &&
        (errno == EAGAIN || errno == EWOULDBLOCK)) {
        api.ERR_clear_error();
        return -2;
    }
    describe(error, error_length, "TLS write failed");
    return -1;
}

void cove_http_tls_close(SSL *ssl) {
    if (!ssl) return;
    api.SSL_shutdown(ssl);
    api.ERR_clear_error();
    api.SSL_free(ssl);
}
