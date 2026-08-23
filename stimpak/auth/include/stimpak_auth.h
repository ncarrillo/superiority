#ifndef STIMPAK_AUTH_H
#define STIMPAK_AUTH_H

#include <stdbool.h>
#include <stdint.h>

#if defined(_WIN32)
#define STIMPAK_AUTH_API __declspec(dllimport)
#else
#define STIMPAK_AUTH_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define STIMPAK_AUTH_ABI_VERSION 1u
#define STIMPAK_AUTH_COMPLETED 0
#define STIMPAK_AUTH_CANCELLED 1
#define STIMPAK_AUTH_FAILED 2

typedef struct StimpakAuthSession StimpakAuthSession;

/* `detail` is borrowed UTF-8 valid only during this callback. It is the
 * Battle.net session token for COMPLETED, NULL for CANCELLED, and an error
 * message for FAILED. The callback runs on the host UI thread. */
typedef void (*StimpakAuthCallback)(
    void *context,
    int32_t status,
    const char *detail);

/* Starts an in-process native WebView and returns immediately. Call from a UI
 * thread whose AppKit or Win32 loop will remain active. */
STIMPAK_AUTH_API StimpakAuthSession *stimpak_auth_present(
    const char *url,
    bool fresh_account,
    StimpakAuthCallback callback,
    void *context);

/* Cancel and close on the same UI thread used for present. Close exactly once. */
STIMPAK_AUTH_API void stimpak_auth_cancel(StimpakAuthSession *session);
STIMPAK_AUTH_API void stimpak_auth_close(StimpakAuthSession *session);

STIMPAK_AUTH_API uint32_t stimpak_auth_abi_version(void);
/* Static UTF-8 storage owned by Stimpak.Auth; do not free. */
STIMPAK_AUTH_API const char *stimpak_auth_version(void);

#ifdef __cplusplus
}
#endif

#endif
