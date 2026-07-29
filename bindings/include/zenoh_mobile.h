#ifndef ZENOH_MOBILE_H
#define ZENOH_MOBILE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Callback type for async operations.
 * @param key   The Zenoh key (topic) string
 * @param value The payload value string
 * @param ctx   User-provided context pointer
 */
typedef void (*zenoh_callback_t)(const char* key, const char* value, void* ctx);

/**
 * Open a Zenoh session.
 * @param config_str JSON configuration string
 * @return 0 on success, -1 on failure
 */
int z_open(const char* config_str);

/**
 * Put a key/value pair.
 * @param key   The key string
 * @param value The value string
 * @return 0 on success, -1 on failure
 */
int z_put(const char* key, const char* value);

/**
 * Subscribe to a key expression.
 * The callback will be invoked for each received sample.
 * @param key      The key expression to subscribe to
 * @param callback Callback function invoked on each sample
 * @param ctx      User-provided context passed to callback
 * @return A non-zero handle on success, 0 on failure
 */
uint64_t z_subscribe(const char* key, zenoh_callback_t callback, void* ctx);

/**
 * Cancel a subscription.
 * @param handle The handle returned by z_subscribe
 * @return 0 on success, -1 on failure
 */
int z_unsubscribe(uint64_t handle);

/**
 * Get (query) a key expression.
 * The callback is invoked once with the first reply.
 * @param key      The key expression to query
 * @param callback Callback function invoked with the reply
 * @param ctx      User-provided context passed to callback
 * @return 0 on success, -1 on failure
 */
int z_get(const char* key, zenoh_callback_t callback, void* ctx);

/**
 * Declare a publisher for a key expression.
 * @param key The key expression to publish on
 * @return A non-zero handle on success, 0 on failure
 */
uint64_t z_declare_publisher(const char* key);

/**
 * Publish a value using a publisher handle.
 * @param pub   The publisher handle from z_declare_publisher
 * @param value The value string to publish
 * @return 0 on success, -1 on failure
 */
int z_publisher_put(uint64_t pub, const char* value);

/**
 * Undeclare (drop) a publisher.
 * @param pub The publisher handle from z_declare_publisher
 * @return 0 on success, -1 on failure
 */
int z_undeclare_publisher(uint64_t pub);

/**
 * Close the Zenoh session.
 * @return 0 always
 */
int z_close(void);

/**
 * Get the Zenoh session ID as a hex string.
 * Caller must free with z_free_string.
 * @return A NUL-terminated hex string, or NULL
 */
char* z_zid(void);

/**
 * Free a string returned by a z_* function.
 */
void z_free_string(char* ptr);

#ifdef __cplusplus
}
#endif

#endif /* ZENOH_MOBILE_H */
