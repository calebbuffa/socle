#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Scheduling context enum exposed to C.
 */
typedef enum orkester_context_t {
  orkester_context_t_ORKESTER_WORKER = 0,
  orkester_context_t_ORKESTER_MAIN = 1,
  orkester_context_t_ORKESTER_IMMEDIATE = 2,
} orkester_context_t;

/**
 * Structured error code exposed to C.
 */
typedef enum orkester_error_code_t {
  orkester_error_code_t_ORKESTER_ERROR_GENERIC = 0,
  orkester_error_code_t_ORKESTER_ERROR_CANCELLED = 1,
  orkester_error_code_t_ORKESTER_ERROR_TIMED_OUT = 2,
  orkester_error_code_t_ORKESTER_ERROR_DROPPED = 3,
} orkester_error_code_t;

/**
 * Opaque async runtime handle exposed to C. Wraps `orkester::AsyncSystem`.
 *
 * cbindgen sees this as an opaque struct so it emits a forward declaration.
 */
typedef struct orkester_async_t {
  uint8_t _opaque;
} orkester_async_t;

/**
 * Callback for continuations — `void(*)(void*)`.
 */
typedef void (*orkester_callback_fn_t)(void*);

/**
 * Dispatch function type for scheduling work on a background thread.
 *
 * The host calls `work(work_data)` on a background thread.
 * Every language can implement this trivially — it's just `void(*)(void*)`.
 */
typedef void (*orkester_dispatch_fn_t)(void *ctx, orkester_callback_fn_t work, void *work_data);

/**
 * Opaque handle to a `Promise<()>` (completion trigger).
 */
typedef void *orkester_promise_t;

/**
 * Opaque handle to a `Future<()>`.
 */
typedef void *orkester_future_t;

/**
 * Opaque handle to a `SharedFuture<()>` (cloneable).
 */
typedef void *orkester_shared_future_t;

/**
 * Opaque handle to a `MainThreadScope`.
 */
typedef void *orkester_main_thread_scope_t;

/**
 * Opaque handle to a `ThreadPool`.
 */
typedef void *orkester_thread_pool_t;

/**
 * Opaque handle to a `CancellationToken`.
 */
typedef void *orkester_cancel_token_t;

/**
 * Opaque handle to a `Semaphore`.
 */
typedef void *orkester_semaphore_t;

/**
 * Opaque handle to a `SemaphorePermit`.
 */
typedef void *orkester_semaphore_permit_t;

/**
 * Attach an error-recovery handler to a future. If the future rejects,
 * the callback is invoked in the given scheduling context. The returned
 * future resolves with `()` either way.
 *
 * The callback receives a pointer to the error message and its byte length.
 * The pointer is valid only for the duration of the callback.
 *
 * Consumes the input future handle.
 */
typedef void (*orkester_catch_fn_t)(void *ctx, const char *error_ptr, uintptr_t error_len);

/**
 * Callback that produces a new future for each retry attempt.
 * Must return an `orkester_future_t`. Returning null is treated as failure.
 */
typedef orkester_future_t (*orkester_retry_fn_t)(void *ctx);

/**
 * Opaque handle to a `JoinSet<()>`.
 */
typedef void *orkester_join_set_t;

/**
 * Opaque handle to a channel `Sender<*mut c_void>`.
 */
typedef void *orkester_sender_t;

/**
 * Opaque handle to a channel `Receiver<*mut c_void>`.
 */
typedef void *orkester_receiver_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Create an `orkester_async_t` from a dispatch function.
 *
 * - `dispatch`: called when orkester needs work scheduled on a background
 *   thread. The implementation must call `work(work_data)` from a background
 *   thread.
 * - `ctx`: user data pointer passed as the first argument to `dispatch`.
 * - `destroy`: optional cleanup function called when the system is dropped.
 *   May be null.
 */
struct orkester_async_t *orkester_async_create(orkester_dispatch_fn_t dispatch,
                                               void *ctx,
                                               void (*destroy)(void*));

/**
 * Destroy an `orkester_async_t`.
 */
void orkester_async_destroy(struct orkester_async_t *ptr);

/**
 * Clone an `orkester_async_t` (cheap Arc clone).
 */
struct orkester_async_t *orkester_async_clone(const struct orkester_async_t *ptr);

/**
 * Create a promise/future pair.
 */
void orkester_promise_create(const struct orkester_async_t *system,
                             orkester_promise_t *out_promise,
                             orkester_future_t *out_future);

/**
 * Signal completion (resolve the promise). Consumes the handle.
 */
void orkester_promise_resolve(orkester_promise_t promise);

/**
 * Signal failure. Consumes the handle.
 */
void orkester_promise_reject(orkester_promise_t promise,
                             const char *message,
                             uintptr_t message_len);

/**
 * Drop a promise without resolving.
 */
void orkester_promise_drop(orkester_promise_t promise);

/**
 * Check if a future has completed.
 */
bool orkester_future_is_ready(orkester_future_t future);

/**
 * Block until a future completes. Consumes the handle.
 */
bool orkester_future_wait(orkester_future_t future,
                          const char **out_error_ptr,
                          uintptr_t *out_error_len);

/**
 * Block in main thread, dispatching main-thread tasks while waiting. Consumes the handle.
 */
bool orkester_future_wait_in_main(orkester_future_t future,
                                  const char **out_error_ptr,
                                  uintptr_t *out_error_len);

/**
 * Attach a continuation in the given scheduling context. Consumes the input handle.
 */
orkester_future_t orkester_future_then(orkester_future_t future,
                                       enum orkester_context_t context,
                                       orkester_callback_fn_t callback,
                                       void *ctx);

/**
 * Attach a continuation on a worker thread. Consumes the input handle.
 * Compatibility wrapper — prefer `orkester_future_then` with `ORKESTER_WORKER`.
 */
orkester_future_t orkester_future_then_in_worker(orkester_future_t future,
                                                 orkester_callback_fn_t callback,
                                                 void *context);

/**
 * Attach a continuation on the main thread. Consumes the input handle.
 * Compatibility wrapper — prefer `orkester_future_then` with `ORKESTER_MAIN`.
 */
orkester_future_t orkester_future_then_in_main(orkester_future_t future,
                                               orkester_callback_fn_t callback,
                                               void *context);

/**
 * Attach a continuation that runs immediately. Consumes the input handle.
 * Compatibility wrapper — prefer `orkester_future_then` with `ORKESTER_IMMEDIATE`.
 */
orkester_future_t orkester_future_then_immediately(orkester_future_t future,
                                                   orkester_callback_fn_t callback,
                                                   void *context);

/**
 * Convert a future to a shared future. Consumes the future handle.
 */
orkester_shared_future_t orkester_future_share(orkester_future_t future);

/**
 * Drop a future handle.
 */
void orkester_future_drop(orkester_future_t future);

/**
 * Clone a shared future handle.
 */
orkester_shared_future_t orkester_shared_future_clone(orkester_shared_future_t shared);

/**
 * Check if a shared future has completed.
 */
bool orkester_shared_future_is_ready(orkester_shared_future_t shared);

/**
 * Block until a shared future completes. Does NOT consume the handle.
 */
bool orkester_shared_future_wait(orkester_shared_future_t shared,
                                 const char **out_error_ptr,
                                 uintptr_t *out_error_len);

/**
 * Attach a continuation on a shared future in the given scheduling context.
 */
orkester_future_t orkester_shared_future_then(orkester_shared_future_t shared,
                                              enum orkester_context_t context,
                                              orkester_callback_fn_t callback,
                                              void *ctx);

/**
 * Attach a continuation on a shared future (worker thread).
 * Compatibility wrapper — prefer `orkester_shared_future_then` with `ORKESTER_WORKER`.
 */
orkester_future_t orkester_shared_future_then_in_worker(orkester_shared_future_t shared,
                                                        orkester_callback_fn_t callback,
                                                        void *context);

/**
 * Attach a continuation on a shared future (main thread).
 * Compatibility wrapper — prefer `orkester_shared_future_then` with `ORKESTER_MAIN`.
 */
orkester_future_t orkester_shared_future_then_in_main(orkester_shared_future_t shared,
                                                      orkester_callback_fn_t callback,
                                                      void *context);

/**
 * Attach a continuation on a shared future (immediately).
 * Compatibility wrapper — prefer `orkester_shared_future_then` with `ORKESTER_IMMEDIATE`.
 */
orkester_future_t orkester_shared_future_then_immediately(orkester_shared_future_t shared,
                                                          orkester_callback_fn_t callback,
                                                          void *context);

/**
 * Drop a shared future handle.
 */
void orkester_shared_future_drop(orkester_shared_future_t shared);

/**
 * Convert a shared future into a unique future. Consumes the shared handle.
 *
 * Creates a `Future<()>` that completes when the shared future completes.
 * The shared handle is consumed (dropped); other clones remain valid.
 */
orkester_future_t orkester_shared_future_into_unique(orkester_shared_future_t shared);

/**
 * Create an already-resolved future.
 */
orkester_future_t orkester_future_create_resolved(const struct orkester_async_t *system);

/**
 * Schedule a callback in the given context. Returns a future.
 */
orkester_future_t orkester_async_run(const struct orkester_async_t *system,
                                     enum orkester_context_t context,
                                     orkester_callback_fn_t callback,
                                     void *ctx);

/**
 * Schedule a callback on a worker thread. Returns a future.
 * Compatibility wrapper — prefer `orkester_async_run` with `ORKESTER_WORKER`.
 */
orkester_future_t orkester_async_run_in_worker(const struct orkester_async_t *system,
                                               orkester_callback_fn_t callback,
                                               void *context);

/**
 * Schedule a callback on the main thread. Returns a future.
 * Compatibility wrapper — prefer `orkester_async_run` with `ORKESTER_MAIN`.
 */
orkester_future_t orkester_async_run_in_main(const struct orkester_async_t *system,
                                             orkester_callback_fn_t callback,
                                             void *context);

/**
 * Dispatch all queued main-thread tasks. Returns how many were dispatched.
 */
uintptr_t orkester_async_dispatch(const struct orkester_async_t *system);

/**
 * Dispatch a single main-thread task. Returns true if one was dispatched.
 */
bool orkester_async_dispatch_one(const struct orkester_async_t *system);

/**
 * Free a string previously returned by orkester FFI error functions.
 */
void orkester_string_drop(const char *ptr, uintptr_t len);

/**
 * Enter main-thread scope: the calling thread is treated as the main thread
 * until `orkester_main_thread_scope_drop` is called on the returned handle.
 */
orkester_main_thread_scope_t orkester_main_thread_scope_create(const struct orkester_async_t *system);

/**
 * Leave main-thread scope. Consumes the handle.
 */
void orkester_main_thread_scope_drop(orkester_main_thread_scope_t scope);

/**
 * Create a thread pool with the given number of threads.
 */
orkester_thread_pool_t orkester_thread_pool_create(uintptr_t num_threads);

/**
 * Drop a thread pool handle. Consumes the handle.
 */
void orkester_thread_pool_drop(orkester_thread_pool_t pool);

/**
 * Clone a thread pool handle (cheap Arc clone).
 */
orkester_thread_pool_t orkester_thread_pool_clone(orkester_thread_pool_t pool);

/**
 * Schedule a callback in a thread pool. Returns a future.
 */
orkester_future_t orkester_async_run_in_pool(const struct orkester_async_t *system,
                                             orkester_thread_pool_t pool,
                                             orkester_callback_fn_t callback,
                                             void *context);

/**
 * Attach a continuation in a thread pool. Consumes the input handle.
 */
orkester_future_t orkester_future_then_in_pool(orkester_future_t future,
                                               orkester_thread_pool_t pool,
                                               orkester_callback_fn_t callback,
                                               void *context);

/**
 * Attach a continuation on a shared future in a thread pool.
 */
orkester_future_t orkester_shared_future_then_in_pool(orkester_shared_future_t shared,
                                                      orkester_thread_pool_t pool,
                                                      orkester_callback_fn_t callback,
                                                      void *context);

/**
 * Create an `orkester_async_t` with a built-in thread pool task processor.
 * No vtable needed — orkester manages its own threads.
 */
struct orkester_async_t *orkester_async_create_default(uintptr_t num_threads);

/**
 * Wait for all futures to complete. Consumes all input handles.
 * Returns a single future that resolves when every input has resolved.
 * If any input future rejects, the output future rejects with the first error.
 */
orkester_future_t orkester_future_all(const struct orkester_async_t *system,
                                      orkester_future_t *futures,
                                      uintptr_t count);

/**
 * Reject a promise with an integer error code and message. Consumes the handle.
 * The code is prepended to the message as "[code] message".
 */
void orkester_promise_reject_with_code(orkester_promise_t promise,
                                       int32_t code,
                                       const char *message,
                                       uintptr_t message_len);

/**
 * Wait for a future to complete and retrieve the error code if it failed.
 * Returns true on success, false on error.
 * On error, writes the error code, message pointer, and message length.
 */
bool orkester_future_wait_with_code(orkester_future_t future,
                                    enum orkester_error_code_t *out_code,
                                    const char **out_error_ptr,
                                    uintptr_t *out_error_len);

/**
 * Create a new cancellation token.
 */
orkester_cancel_token_t orkester_cancel_token_create(void);

/**
 * Clone a cancellation token handle.
 */
orkester_cancel_token_t orkester_cancel_token_clone(orkester_cancel_token_t token);

/**
 * Signal cancellation.
 */
void orkester_cancel_token_cancel(orkester_cancel_token_t token);

/**
 * Check if a cancellation token has been signalled.
 */
bool orkester_cancel_token_is_cancelled(orkester_cancel_token_t token);

/**
 * Drop a cancellation token handle.
 */
void orkester_cancel_token_drop(orkester_cancel_token_t token);

/**
 * Attach a cancellation token to a future. Consumes the future handle.
 * If the token is signalled before the future completes, the returned
 * future rejects with error code CANCELLED.
 */
orkester_future_t orkester_future_with_cancellation(orkester_future_t future,
                                                    orkester_cancel_token_t token);

/**
 * Create a future that completes after `millis` milliseconds.
 */
orkester_future_t orkester_delay(const struct orkester_async_t *system, uint64_t millis);

/**
 * Wrap a future with a timeout. If the future doesn't complete within
 * `millis` milliseconds, the returned future rejects with TIMED_OUT.
 * Consumes the input future handle.
 */
orkester_future_t orkester_timeout(const struct orkester_async_t *system,
                                   orkester_future_t future,
                                   uint64_t millis);

/**
 * Race multiple futures. Returns a future that resolves with the first
 * to complete. Consumes all input future handles.
 */
orkester_future_t orkester_race(const struct orkester_async_t *system,
                                orkester_future_t *futures,
                                uintptr_t count);

/**
 * Spawn a detached task in the given context. Fire-and-forget — there is
 * no future to observe the result.
 */
void orkester_spawn(const struct orkester_async_t *system,
                    enum orkester_context_t context,
                    orkester_callback_fn_t callback,
                    void *ctx);

/**
 * Create a counting semaphore with `permits` available slots.
 */
orkester_semaphore_t orkester_semaphore_create(const struct orkester_async_t *system,
                                               uintptr_t permits);

/**
 * Acquire a semaphore permit (blocking). Returns a permit handle.
 */
orkester_semaphore_permit_t orkester_semaphore_acquire(orkester_semaphore_t sem);

/**
 * Try to acquire a semaphore permit without blocking.
 * Returns null if no permit is available.
 */
orkester_semaphore_permit_t orkester_semaphore_try_acquire(orkester_semaphore_t sem);

/**
 * Return the number of available permits.
 */
uintptr_t orkester_semaphore_available(orkester_semaphore_t sem);

/**
 * Drop a semaphore permit (releases the slot back to the semaphore).
 */
void orkester_semaphore_permit_drop(orkester_semaphore_permit_t permit);

/**
 * Drop a semaphore handle.
 */
void orkester_semaphore_drop(orkester_semaphore_t sem);

orkester_future_t orkester_future_catch(orkester_future_t future,
                                        enum orkester_context_t context,
                                        orkester_catch_fn_t callback,
                                        void *ctx);

/**
 * Attach an error-recovery handler on a shared future. Does NOT consume
 * the shared future handle.
 */
orkester_future_t orkester_shared_future_catch(orkester_shared_future_t shared,
                                               enum orkester_context_t context,
                                               orkester_catch_fn_t callback,
                                               void *ctx);

/**
 * Retry an operation up to `max_attempts` times with exponential back-off.
 *
 * On each attempt, `factory` is called to produce a new future. If that
 * future resolves, the retry future resolves. If it rejects and attempts
 * remain, the next attempt is scheduled after a back-off delay.
 */
orkester_future_t orkester_retry(const struct orkester_async_t *system,
                                 uint32_t max_attempts,
                                 orkester_retry_fn_t factory,
                                 void *ctx);

/**
 * Create a new JoinSet.
 */
orkester_join_set_t orkester_join_set_create(const struct orkester_async_t *system);

/**
 * Push a future into the JoinSet. Consumes the future handle.
 */
void orkester_join_set_push(orkester_join_set_t js, orkester_future_t future);

/**
 * Return the number of futures in the JoinSet.
 */
uintptr_t orkester_join_set_len(orkester_join_set_t js);

/**
 * Wait for all futures in the JoinSet. Consumes the handle.
 * Returns the number of futures that resolved successfully.
 * `out_total` receives the total count. Failures are counted as
 * `*out_total - return_value`.
 */
uintptr_t orkester_join_set_join_all(orkester_join_set_t js, uintptr_t *out_total);

/**
 * Wait for the next future to complete. Returns true if a result was
 * obtained, false if the JoinSet is empty.
 * `out_ok` is set to true if the future resolved, false if it rejected.
 */
bool orkester_join_set_join_next(orkester_join_set_t js, bool *out_ok);

/**
 * Drop a JoinSet without waiting. Consumes the handle.
 */
void orkester_join_set_drop(orkester_join_set_t js);

/**
 * Create a bounded mpsc channel with the given capacity.
 * Writes sender and receiver handles to the output pointers.
 */
void orkester_channel_create(uintptr_t capacity,
                             orkester_sender_t *out_sender,
                             orkester_receiver_t *out_receiver);

/**
 * Create a one-shot channel (capacity 1).
 */
void orkester_channel_create_oneshot(orkester_sender_t *out_sender,
                                     orkester_receiver_t *out_receiver);

/**
 * Clone a sender handle.
 */
orkester_sender_t orkester_sender_clone(orkester_sender_t sender);

/**
 * Send a value through the channel (blocking if full).
 * Returns true on success, false if the receiver has been dropped.
 * On failure, `out_value` receives the unsent value back.
 */
bool orkester_sender_send(orkester_sender_t sender, void *value, void **out_value);

/**
 * Try to send without blocking. Returns 0 on success, 1 if full, 2 if closed.
 * On failure, `out_value` receives the unsent value back.
 */
uint32_t orkester_sender_try_send(orkester_sender_t sender, void *value, void **out_value);

/**
 * Send with a timeout in milliseconds. Returns 0 on success, 1 if timed
 * out (full), 2 if closed. On failure, `out_value` receives the value back.
 */
uint32_t orkester_sender_send_timeout(orkester_sender_t sender,
                                      void *value,
                                      uint64_t timeout_ms,
                                      void **out_value);

/**
 * Check if the receiver has been dropped.
 */
bool orkester_sender_is_closed(orkester_sender_t sender);

/**
 * Drop a sender handle.
 */
void orkester_sender_drop(orkester_sender_t sender);

/**
 * Receive a value (blocking). Returns true if a value was received.
 * `out_value` receives the pointer. Returns false when all senders are
 * dropped and the buffer is empty.
 */
bool orkester_receiver_recv(orkester_receiver_t receiver, void **out_value);

/**
 * Non-blocking receive. Returns true if a value was available.
 */
bool orkester_receiver_try_recv(orkester_receiver_t receiver, void **out_value);

/**
 * Receive with a timeout in milliseconds. Returns true if a value arrived
 * before the deadline.
 */
bool orkester_receiver_recv_timeout(orkester_receiver_t receiver,
                                    uint64_t timeout_ms,
                                    void **out_value);

/**
 * Check if a receiver's channel is closed and empty.
 */
bool orkester_receiver_is_closed(orkester_receiver_t receiver);

/**
 * Drop a receiver handle.
 */
void orkester_receiver_drop(orkester_receiver_t receiver);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus
