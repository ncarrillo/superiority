#ifndef STIMPAK_H
#define STIMPAK_H

#include <stdbool.h>
#include <stdint.h>

#if defined(STIMPAK_STATIC)
#define STIMPAK_API
#elif defined(_WIN32)
#define STIMPAK_API __declspec(dllimport)
#else
#define STIMPAK_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define STIMPAK_OK 0
#define STIMPAK_ERR_INVALID_ARGUMENT -1
#define STIMPAK_ERR_DISCONNECTED -2
#define STIMPAK_ERR_NO_SUCH_AUTH -3
#define STIMPAK_ERR_PANIC -99

#define STIMPAK_ABI_VERSION 3u
#define STIMPAK_EVENT_SCHEMA_VERSION 2u

typedef struct StimpakClient StimpakClient;

/* Every string argument is UTF-8 and valid only for the duration of the call. */
/* application_id is a stable namespace such as "com.example.ExampleBot". */
STIMPAK_API StimpakClient *stimpak_client_open(const char *application_id);
/* Advanced override; most applications should let Stimpak choose the path. */
STIMPAK_API StimpakClient *stimpak_client_open_at_path(const char *credential_path);
STIMPAK_API void stimpak_client_close(StimpakClient *client);

STIMPAK_API int32_t stimpak_client_connect(
    StimpakClient *client,
    bool force_interactive);
/*
 * channels_json is an array of:
 *   {"kind":"public","id":1028}
 *   {"kind":"private","name":"Bots"}
 *   {"kind":"group","club_id":535241}
 * expected_account_id=0 disables the account guard. [] joins General.
 */
STIMPAK_API int32_t stimpak_client_connect_configured(
    StimpakClient *client,
    bool force_interactive,
    uint64_t expected_account_id,
    const char *channels_json);
STIMPAK_API int32_t stimpak_client_disconnect(StimpakClient *client);
STIMPAK_API int32_t stimpak_client_sign_out(StimpakClient *client);

STIMPAK_API int32_t stimpak_client_join_public(
    StimpakClient *client,
    uint16_t channel_id);
STIMPAK_API int32_t stimpak_client_join_private(
    StimpakClient *client,
    const char *name);
STIMPAK_API int32_t stimpak_client_join_group(
    StimpakClient *client,
    uint32_t club_id);
STIMPAK_API int32_t stimpak_client_search_groups(
    StimpakClient *client,
    const char *query);
STIMPAK_API int32_t stimpak_client_leave(
    StimpakClient *client,
    uint8_t channel_index);

STIMPAK_API int32_t stimpak_client_send_message(
    StimpakClient *client,
    uint8_t channel_index,
    const char *body);
STIMPAK_API int32_t stimpak_client_send_whisper(
    StimpakClient *client,
    const char *name,
    const char *body);
STIMPAK_API int32_t stimpak_client_answer_group_invitation(
    StimpakClient *client,
    uint32_t club_id,
    bool accept);
STIMPAK_API int32_t stimpak_client_answer_party_invitation(
    StimpakClient *client,
    uint8_t channel_index,
    bool accept);

STIMPAK_API int32_t stimpak_client_submit_auth(
    StimpakClient *client,
    uint64_t auth_id,
    const char *token);
STIMPAK_API int32_t stimpak_client_cancel_auth(
    StimpakClient *client,
    uint64_t auth_id);

/*
 * Returns an owned UTF-8 JSON object, or NULL on timeout. Only one thread may
 * poll a client at a time. Release non-NULL results with stimpak_string_free.
 */
STIMPAK_API char *stimpak_client_poll(StimpakClient *client, uint32_t timeout_ms);
STIMPAK_API void stimpak_string_free(char *value);

STIMPAK_API uint16_t stimpak_default_public_channel(void);
STIMPAK_API uint32_t stimpak_abi_version(void);
STIMPAK_API uint32_t stimpak_event_schema_version(void);
/* Static UTF-8 storage owned by Stimpak; do not free. */
STIMPAK_API const char *stimpak_version(void);

#ifdef __cplusplus
}
#endif

#endif
