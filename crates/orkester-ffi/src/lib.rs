//! C-ABI FFI for orkester scheduling primitives.
//!
//! Futures carry an optional **payload** (`*mut c_void` + destructor).
//! Values travel inside the future through Rust — no external bookkeeping
//! needed on the host side.

// C-style snake_case naming is intentional for FFI types.
#![allow(non_camel_case_types)]

use orkester::channel::{self, Receiver, Sender};
use orkester::{
    AsyncSystem, CancellationToken, Context, Executor, Future, JoinSet, MainThreadScope,
    Semaphore, ThreadPool,
};
use std::ffi::{c_char, c_void};

/// Opaque async runtime handle. Wraps `orkester::AsyncSystem`.
#[repr(C)]
pub struct orkester_async_t {
    _opaque: u8,
}

/// Opaque handle to a `Future<Payload>`.
pub type orkester_future_t = *mut c_void;

/// Opaque handle to a `SharedFuture<Payload>` (cloneable).
pub type orkester_shared_future_t = *mut c_void;

/// Opaque handle to a `Promise<Payload>` (completion trigger).
pub type orkester_promise_t = *mut c_void;

/// Opaque handle to a `MainThreadScope`.
pub type orkester_main_thread_scope_t = *mut c_void;

/// Opaque handle to a `ThreadPool`.
pub type orkester_thread_pool_t = *mut c_void;

/// Callback for fire-and-forget operations — `void(*)(void*)`.
pub type orkester_callback_fn_t = unsafe extern "C" fn(*mut c_void);

/// Value destructor callback — `void(*)(void*)`.
pub type orkester_destroy_fn_t = unsafe extern "C" fn(*mut c_void);

/// Value transform callback for continuations.
/// Receives `(ctx, input_value)`, returns the output value pointer.
/// The callback takes ownership of `input_value` and must either
/// reuse it or destroy it.
pub type orkester_map_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, value: *mut c_void) -> *mut c_void;

/// Async-transform callback for continuations that return futures.
/// Receives `(ctx, input_value)`, returns a new `orkester_future_t`.
/// The callback takes ownership of `input_value`.
/// Returning null is treated as resolving with an empty payload.
pub type orkester_then_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, value: *mut c_void) -> orkester_future_t;

/// Error-recovery callback that receives an opaque error pointer and
/// returns a recovery value. The callback does NOT take ownership of
/// `error` — copy it if needed beyond the callback lifetime.
pub type orkester_catch_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, error: *mut c_void) -> *mut c_void;

/// Scheduling context enum exposed to C.
#[repr(C)]
pub enum orkester_context_t {
    ORKESTER_CONTEXT_BACKGROUND = 0,
    ORKESTER_CONTEXT_MAIN = 1,
    ORKESTER_CONTEXT_IMMEDIATE = 2,
}

impl orkester_context_t {
    fn to_context(self) -> Context {
        match self {
            orkester_context_t::ORKESTER_CONTEXT_BACKGROUND => Context::BACKGROUND,
            orkester_context_t::ORKESTER_CONTEXT_MAIN => Context::MAIN,
            orkester_context_t::ORKESTER_CONTEXT_IMMEDIATE => Context::IMMEDIATE,
        }
    }
}

/// Result code for channel send operations.
#[repr(C)]
pub enum orkester_send_result_t {
    /// Value was sent successfully.
    ORKESTER_SEND_OK = 0,
    /// Channel is full (try_send / send_timeout).
    ORKESTER_SEND_FULL = 1,
    /// Channel is closed (receiver dropped).
    ORKESTER_SEND_CLOSED = 2,
}

/// Dispatch function type for scheduling work on a background thread.
///
/// The host calls `work(work_data)` on a background thread.
pub type orkester_dispatch_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, work: orkester_callback_fn_t, work_data: *mut c_void);

/// Create an `orkester_async_t` from a dispatch function.
///
/// - `dispatch`: called when orkester needs work scheduled on a background
///   thread. The implementation must call `work(work_data)` from a background
///   thread.
/// - `ctx`: user data pointer passed as the first argument to `dispatch`.
/// - `destroy`: optional cleanup function called when the system is dropped.
///   May be null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_create(
    dispatch: orkester_dispatch_fn_t,
    ctx: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) -> *mut orkester_async_t {
    let executor = FfiExecutor {
        dispatch,
        ctx: SendCtx(ctx),
        destroy,
    };
    let system = AsyncSystem::new(executor);
    Box::into_raw(Box::new(system)) as *mut orkester_async_t
}

/// Destroy an `orkester_async_t`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_destroy(ptr: *mut orkester_async_t) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr as *mut AsyncSystem)) };
    }
}

/// Clone an `orkester_async_t` (cheap Arc clone).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_clone(
    ptr: *const orkester_async_t,
) -> *mut orkester_async_t {
    let system = unsafe { &*(ptr as *const AsyncSystem) };
    Box::into_raw(Box::new(system.clone())) as *mut orkester_async_t
}

struct FfiExecutor {
    dispatch: orkester_dispatch_fn_t,
    ctx: SendCtx,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Send for FfiExecutor {}
unsafe impl Sync for FfiExecutor {}

impl Executor for FfiExecutor {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        // Package the Rust closure as a callback + data pointer.
        let work_data = Box::into_raw(Box::new(task)) as *mut c_void;

        /// Static trampoline: called by the host from a background thread.
        unsafe extern "C" fn trampoline(data: *mut c_void) {
            let boxed: Box<Box<dyn FnOnce() + Send + 'static>> =
                unsafe { Box::from_raw(data as *mut Box<dyn FnOnce() + Send + 'static>) };
            (*boxed)();
        }

        unsafe { (self.dispatch)(self.ctx.0, trampoline, work_data) };
    }
}

impl Drop for FfiExecutor {
    fn drop(&mut self) {
        if let Some(drop_fn) = self.destroy {
            unsafe { drop_fn(self.ctx.0) };
        }
    }
}

struct SendCallback {
    func: orkester_callback_fn_t,
    context: SendCtx,
}
unsafe impl Send for SendCallback {}

impl SendCallback {
    unsafe fn call(&self) {
        unsafe { (self.func)(self.context.0) };
    }
}

#[derive(Clone, Copy)]
struct SendCtx(*mut c_void);
unsafe impl Send for SendCtx {}
unsafe impl Sync for SendCtx {}

/// Wrapper for value-transform callbacks to satisfy `Send`.
struct SendTransform {
    func: orkester_map_fn_t,
    ctx: SendCtx,
    result_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}
unsafe impl Send for SendTransform {}

/// Wrapper for async-transform callbacks to satisfy `Send`.
struct SendThenAsync {
    func: orkester_then_fn_t,
    ctx: SendCtx,
}
unsafe impl Send for SendThenAsync {}

/// Wrapper for opaque-error catch callbacks to satisfy `Send`.
struct SendCatchOpaque {
    func: orkester_catch_fn_t,
    ctx: SendCtx,
    result_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}
unsafe impl Send for SendCatchOpaque {}

/// An opaque error object that can be stored inside `AsyncError` via
/// `AsyncError::new(OpaqueError { .. })`. The host boxes its error type
/// (e.g. C++ `exception_ptr`) to `void*`, and on catch the pointer is
/// extracted via `downcast_ref::<OpaqueError>()`.
struct OpaqueError {
    ptr: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Send for OpaqueError {}
unsafe impl Sync for OpaqueError {}

impl std::fmt::Display for OpaqueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "opaque FFI error")
    }
}

impl std::fmt::Debug for OpaqueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpaqueError")
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl std::error::Error for OpaqueError {}

impl Drop for OpaqueError {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy {
            if !self.ptr.is_null() {
                unsafe { destroy(self.ptr) };
            }
        }
    }
}

/// A collected array of payload values returned by `orkester_future_all_with_values`.
/// Each element's value pointer can be extracted by index; the caller takes
/// ownership of extracted values.
struct PayloadArray {
    payloads: Vec<Payload>,
}

/// A value payload carried by futures across the FFI boundary.
///
/// On resolve, the host provides a `value` pointer and an optional `destroy`
/// function. On continuation, the payload flows to the next stage. When the
/// future is dropped or waited, the destructor is called (if present) to
/// clean up the value.
struct Payload {
    value: *mut c_void,
    clone_fn: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    destroy_fn: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Send for Payload {}

impl Payload {
    fn empty() -> Self {
        Payload {
            value: std::ptr::null_mut(),
            clone_fn: None,
            destroy_fn: None,
        }
    }

    fn new(
        value: *mut c_void,
        destroy_fn: Option<unsafe extern "C" fn(*mut c_void)>,
    ) -> Self {
        Payload {
            value,
            clone_fn: None,
            destroy_fn,
        }
    }

    /// Attach a clone function (needed before sharing).
    fn with_clone(
        mut self,
        clone_fn: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    ) -> Self {
        self.clone_fn = clone_fn;
        self
    }

    /// Extract the value pointer, preventing the destructor from running.
    fn take_value(&mut self) -> *mut c_void {
        let val = self.value;
        self.value = std::ptr::null_mut();
        val
    }
}

impl Clone for Payload {
    fn clone(&self) -> Self {
        if self.value.is_null() {
            return Self {
                value: std::ptr::null_mut(),
                clone_fn: self.clone_fn,
                destroy_fn: self.destroy_fn,
            };
        }
        match self.clone_fn {
            Some(f) => Self {
                value: unsafe { f(self.value) },
                clone_fn: self.clone_fn,
                destroy_fn: self.destroy_fn,
            },
            None => Self {
                value: std::ptr::null_mut(),
                clone_fn: None,
                destroy_fn: None,
            },
        }
    }
}

impl Drop for Payload {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy_fn {
            if !self.value.is_null() {
                unsafe { destroy(self.value) };
            }
        }
    }
}

unsafe fn write_ffi_error(
    error: orkester::AsyncError,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) {
    let msg = error.to_string();
    if !out_error_ptr.is_null() {
        let leaked = msg.into_bytes().into_boxed_slice();
        unsafe {
            *out_error_len = leaked.len();
            *out_error_ptr = Box::into_raw(leaked) as *const c_char;
        }
    }
}

/// Create a promise/future pair.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_create(
    system: *const orkester_async_t,
    out_promise: *mut orkester_promise_t,
    out_future: *mut orkester_future_t,
) {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let (promise, future) = system.promise::<Payload>();
    unsafe {
        *out_promise = Box::into_raw(Box::new(promise)) as orkester_promise_t;
        *out_future = Box::into_raw(Box::new(future)) as orkester_future_t;
    }
}

/// Signal completion (resolve the promise) with no payload. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_resolve(promise: orkester_promise_t) {
    if !promise.is_null() {
        let promise = unsafe { Box::from_raw(promise as *mut orkester::Promise<Payload>) };
        promise.resolve(Payload::empty());
    }
}

/// Signal completion with a value payload. Consumes the promise handle.
///
/// - `value`: the value pointer to carry. Null is valid.
/// - `destroy`: optional destructor called when the payload is dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_resolve_with(
    promise: orkester_promise_t,
    value: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) {
    if !promise.is_null() {
        let promise = unsafe { Box::from_raw(promise as *mut orkester::Promise<Payload>) };
        promise.resolve(Payload::new(value, destroy));
    }
}

/// Signal failure. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_reject(
    promise: orkester_promise_t,
    message: *const c_char,
    message_len: usize,
) {
    if promise.is_null() {
        return;
    }
    let promise = unsafe { Box::from_raw(promise as *mut orkester::Promise<Payload>) };
    let msg = if message.is_null() {
        "FFI promise rejected".to_string()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(message as *const u8, message_len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    promise.reject(orkester::AsyncError::msg(msg));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_drop(promise: orkester_promise_t) {
    if !promise.is_null() {
        unsafe { drop(Box::from_raw(promise as *mut orkester::Promise<Payload>)) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_is_ready(future: orkester_future_t) -> bool {
    let future = unsafe { &*(future as *const Future<Payload>) };
    future.is_ready()
}

/// Block until a future completes. Consumes the handle.
/// On success, writes the payload value to `out_value` (if non-null).
/// The caller takes ownership of the returned value and must destroy it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_block(
    future: orkester_future_t,
    out_value: *mut *mut c_void,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    if future.is_null() {
        return false;
    }
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    match future.block() {
        Ok(mut payload) => {
            if !out_value.is_null() {
                unsafe { *out_value = payload.take_value() };
            }
            true
        }
        Err(e) => {
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Block in main thread, dispatching main-thread tasks while waiting. Consumes the handle.
/// On success, writes the payload value to `out_value` (if non-null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_block_with_main(
    future: orkester_future_t,
    out_value: *mut *mut c_void,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    if future.is_null() {
        return false;
    }
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    match future.block_with_main() {
        Ok(mut payload) => {
            if !out_value.is_null() {
                unsafe { *out_value = payload.take_value() };
            }
            true
        }
        Err(e) => {
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Attach a synchronous value-transform continuation in the given scheduling
/// context. The `transform` callback receives `(ctx, input_value)` and returns
/// a new value pointer. The callback takes ownership of `input_value`.
/// `result_destroy` is called to clean up the returned value when dropped.
/// Consumes the input future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_map(
    future: orkester_future_t,
    context: orkester_context_t,
    transform: orkester_map_fn_t,
    ctx: *mut c_void,
    result_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    let tx = SendTransform {
        func: transform,
        ctx: SendCtx(ctx),
        result_destroy,
    };
    let next = future.then(context.to_context(), move |mut payload| {
        let tx = &tx; // force whole-struct capture
        let input_value = payload.take_value();
        let output_value = unsafe { (tx.func)(tx.ctx.0, input_value) };
        Payload::new(output_value, tx.result_destroy)
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Convert a future to a shared future. Consumes the future handle.
///
/// `clone_fn` is called to clone the payload when the shared future is
/// blocked or continued. Pass null if sharing is only used for fan-out
/// without accessing the value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_share(
    future: orkester_future_t,
    clone_fn: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
) -> orkester_shared_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    // Attach clone_fn to the payload before sharing
    let future = if clone_fn.is_some() {
        future.map(move |payload| payload.with_clone(clone_fn))
    } else {
        future
    };
    let shared = future.share();
    Box::into_raw(Box::new(shared)) as orkester_shared_future_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_drop(future: orkester_future_t) {
    if !future.is_null() {
        unsafe { drop(Box::from_raw(future as *mut Future<Payload>)) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_clone(
    shared: orkester_shared_future_t,
) -> orkester_shared_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<Payload>) };
    Box::into_raw(Box::new(shared.clone())) as orkester_shared_future_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_is_ready(shared: orkester_shared_future_t) -> bool {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<Payload>) };
    shared.is_ready()
}

/// Block until a shared future completes. Does NOT consume the handle.
/// On success, writes a clone of the payload value to `out_value` (if non-null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_block(
    shared: orkester_shared_future_t,
    out_value: *mut *mut c_void,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<Payload>) };
    match shared.block() {
        Ok(mut payload) => {
            if !out_value.is_null() {
                unsafe { *out_value = payload.take_value() };
            }
            true
        }
        Err(e) => {
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_drop(shared: orkester_shared_future_t) {
    if !shared.is_null() {
        unsafe {
            drop(Box::from_raw(
                shared as *mut orkester::SharedFuture<Payload>,
            ))
        };
    }
}

/// Convert a shared future into a unique future. Consumes the shared handle.
/// Other clones remain valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_into_unique(
    shared: orkester_shared_future_t,
) -> orkester_future_t {
    let shared = unsafe { Box::from_raw(shared as *mut orkester::SharedFuture<Payload>) };
    let unique = shared.map(|p| p);
    Box::into_raw(Box::new(unique)) as orkester_future_t
}

/// Create an already-resolved future with an empty payload.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_create_resolved(
    system: *const orkester_async_t,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let future = system.resolved(Payload::empty());
    Box::into_raw(Box::new(future)) as orkester_future_t
}

/// Create an already-resolved future carrying a value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_create_resolved_with(
    system: *const orkester_async_t,
    value: *mut c_void,
    destroy_fn: Option<unsafe extern "C" fn(*mut c_void)>,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let future = system.resolved(Payload::new(value, destroy_fn));
    Box::into_raw(Box::new(future)) as orkester_future_t
}

/// Dispatch all queued main-thread tasks. Returns how many were dispatched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_dispatch(system: *const orkester_async_t) -> usize {
    let system = unsafe { &*(system as *const AsyncSystem) };
    system.flush_main()
}

/// Dispatch a single main-thread task. Returns true if one was dispatched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_dispatch_one(system: *const orkester_async_t) -> bool {
    let system = unsafe { &*(system as *const AsyncSystem) };
    system.flush_main_one()
}

/// Free a string previously returned by orkester FFI error functions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_string_drop(ptr: *const c_char, len: usize) {
    if !ptr.is_null() {
        let _ = unsafe {
            Box::from_raw(std::slice::from_raw_parts_mut(ptr as *mut u8, len) as *mut [u8])
        };
    }
}

/// Enter main-thread scope: the calling thread is treated as the main thread
/// until `orkester_main_thread_scope_drop` is called on the returned handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_main_thread_scope_create(
    system: *const orkester_async_t,
) -> orkester_main_thread_scope_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let scope = system.main_scope();
    Box::into_raw(Box::new(scope)) as orkester_main_thread_scope_t
}

/// Leave main-thread scope. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_main_thread_scope_drop(scope: orkester_main_thread_scope_t) {
    if !scope.is_null() {
        unsafe { drop(Box::from_raw(scope as *mut MainThreadScope)) };
    }
}

/// Create a thread pool with the given number of threads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_thread_pool_create(num_threads: usize) -> orkester_thread_pool_t {
    let pool = ThreadPool::new(num_threads);
    Box::into_raw(Box::new(pool)) as orkester_thread_pool_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_thread_pool_drop(pool: orkester_thread_pool_t) {
    if !pool.is_null() {
        unsafe { drop(Box::from_raw(pool as *mut ThreadPool)) };
    }
}

/// Clone a thread pool handle (cheap Arc clone).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_thread_pool_clone(
    pool: orkester_thread_pool_t,
) -> orkester_thread_pool_t {
    let pool = unsafe { &*(pool as *const ThreadPool) };
    Box::into_raw(Box::new(pool.clone())) as orkester_thread_pool_t
}

/// Create an `orkester_async_t` with a built-in thread pool.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_create_default(
    num_threads: usize,
) -> *mut orkester_async_t {
    let system = AsyncSystem::with_threads(num_threads);
    Box::into_raw(Box::new(system)) as *mut orkester_async_t
}

/// Wait for all futures to complete. Consumes all input handles.
/// Returns a single future that resolves when every input has resolved.
/// If any input future rejects, the output future rejects with the first error.
/// The combined future carries an empty payload (individual payloads are dropped).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_all(
    system: *const orkester_async_t,
    futures: *mut orkester_future_t,
    count: usize,
) -> orkester_future_t {
    let system_ref = unsafe { &*(system as *const AsyncSystem) };

    if count == 0 {
        let resolved = system_ref.resolved(Payload::empty());
        return Box::into_raw(Box::new(resolved)) as orkester_future_t;
    }

    let mut futs: Vec<Future<Payload>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *futures.add(i) };
        futs.push(unsafe { *Box::from_raw(handle as *mut Future<Payload>) });
    }

    let combined = system_ref.join_all(futs);
    // all() returns Future<Vec<Payload>>; map to Future<Payload> with empty payload
    let signal = combined.then(Context::IMMEDIATE, |_| Payload::empty());
    Box::into_raw(Box::new(signal)) as orkester_future_t
}

/// Opaque handle to a `CancellationToken`.
pub type orkester_cancel_token_t = *mut c_void;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_cancel_token_create() -> orkester_cancel_token_t {
    Box::into_raw(Box::new(CancellationToken::new())) as orkester_cancel_token_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_cancel_token_clone(
    token: orkester_cancel_token_t,
) -> orkester_cancel_token_t {
    let token = unsafe { &*(token as *const CancellationToken) };
    Box::into_raw(Box::new(token.clone())) as orkester_cancel_token_t
}

/// Signal cancellation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_cancel_token_cancel(token: orkester_cancel_token_t) {
    if !token.is_null() {
        let token = unsafe { &*(token as *const CancellationToken) };
        token.cancel();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_cancel_token_is_cancelled(
    token: orkester_cancel_token_t,
) -> bool {
    if token.is_null() {
        return false;
    }
    let token = unsafe { &*(token as *const CancellationToken) };
    token.is_cancelled()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_cancel_token_drop(token: orkester_cancel_token_t) {
    if !token.is_null() {
        unsafe { drop(Box::from_raw(token as *mut CancellationToken)) };
    }
}

/// Attach a cancellation token to a future. Consumes the future handle.
/// If the token is signalled before the future completes, the returned
/// future rejects with error code CANCELLED.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_with_cancellation(
    future: orkester_future_t,
    token: orkester_cancel_token_t,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    let token = unsafe { &*(token as *const CancellationToken) };
    let result = future.with_cancellation(token);
    Box::into_raw(Box::new(result)) as orkester_future_t
}

/// Attach a cancellation token to a shared future. Does NOT consume the shared handle.
/// If the token is signalled before the future completes, the returned
/// future rejects with error code CANCELLED.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_with_cancellation(
    shared: orkester_shared_future_t,
    token: orkester_cancel_token_t,
) -> orkester_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<Payload>) };
    let token = unsafe { &*(token as *const CancellationToken) };
    let result = shared.with_cancellation(token);
    Box::into_raw(Box::new(result)) as orkester_future_t
}

/// Create a future that completes after `millis` milliseconds.
/// Resolves with an empty payload.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_delay(
    system: *const orkester_async_t,
    millis: u64,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let future = orkester::delay(system, std::time::Duration::from_millis(millis))
        .map(|()| Payload::empty());
    Box::into_raw(Box::new(future)) as orkester_future_t
}

/// Wrap a future with a timeout. If the future doesn't complete within
/// `millis` milliseconds, the returned future rejects with TIMED_OUT.
/// Consumes the input future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_timeout(
    system: *const orkester_async_t,
    future: orkester_future_t,
    millis: u64,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    let result = orkester::timeout(system, future, std::time::Duration::from_millis(millis));
    Box::into_raw(Box::new(result)) as orkester_future_t
}

/// Race multiple futures. Returns a future that resolves with the first
/// to complete. Consumes all input future handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_race(
    system: *const orkester_async_t,
    futures: *mut orkester_future_t,
    count: usize,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let mut futs: Vec<Future<Payload>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *futures.add(i) };
        if !handle.is_null() {
            futs.push(unsafe { *Box::from_raw(handle as *mut Future<Payload>) });
        }
    }
    let result = orkester::race(system, futs);
    Box::into_raw(Box::new(result)) as orkester_future_t
}

/// Spawn a detached task in the given context. Fire-and-forget — there is
/// no future to observe the result.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_spawn(
    system: *const orkester_async_t,
    context: orkester_context_t,
    callback: orkester_callback_fn_t,
    ctx: *mut c_void,
) {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(ctx),
    };
    system.spawn_detached(context.to_context(), move || {
        unsafe { cb.call() };
    });
}

/// Opaque handle to a `Semaphore`.
pub type orkester_semaphore_t = *mut c_void;

/// Opaque handle to a `SemaphorePermit`.
pub type orkester_semaphore_permit_t = *mut c_void;

/// Create a counting semaphore with `permits` available slots.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_create(
    system: *const orkester_async_t,
    permits: usize,
) -> orkester_semaphore_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let sem = Semaphore::new(system, permits.max(1));
    Box::into_raw(Box::new(sem)) as orkester_semaphore_t
}

/// Acquire a semaphore permit (blocking). Returns a permit handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_acquire(
    sem: orkester_semaphore_t,
) -> orkester_semaphore_permit_t {
    let sem = unsafe { &*(sem as *const Semaphore) };
    let permit = sem.acquire();
    Box::into_raw(Box::new(permit)) as orkester_semaphore_permit_t
}

/// Try to acquire a semaphore permit without blocking.
/// Returns null if no permit is available.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_try_acquire(
    sem: orkester_semaphore_t,
) -> orkester_semaphore_permit_t {
    let sem = unsafe { &*(sem as *const Semaphore) };
    match sem.try_acquire() {
        Some(permit) => Box::into_raw(Box::new(permit)) as orkester_semaphore_permit_t,
        None => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_available(sem: orkester_semaphore_t) -> usize {
    let sem = unsafe { &*(sem as *const Semaphore) };
    sem.available_permits()
}

/// Drop a semaphore permit (releases the slot back to the semaphore).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_permit_drop(permit: orkester_semaphore_permit_t) {
    if !permit.is_null() {
        unsafe { drop(Box::from_raw(permit as *mut orkester::SemaphorePermit)) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_drop(sem: orkester_semaphore_t) {
    if !sem.is_null() {
        unsafe { drop(Box::from_raw(sem as *mut Semaphore)) };
    }
}

/// Reject a promise with an opaque error object. Consumes the promise handle.
///
/// The `error` pointer is stored inside a Rust `AsyncError`. When the error
/// is eventually observed (via `orkester_future_catch` or
/// `orkester_future_block`), the pointer is passed back to the
/// host. `destroy` is called when the error is dropped.
///
/// This enables host languages to flow typed error objects (e.g. C++
/// `exception_ptr`) through the Rust scheduling pipeline without
/// serialization.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_reject_with(
    promise: orkester_promise_t,
    error: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) {
    if promise.is_null() {
        return;
    }
    let promise = unsafe { Box::from_raw(promise as *mut orkester::Promise<Payload>) };
    promise.reject(orkester::AsyncError::new(OpaqueError {
        ptr: error,
        destroy,
    }));
}

/// Attach an async-transform continuation. The callback receives the input
/// value and returns a new `orkester_future_t` (or null for an empty-payload
/// resolved future). The returned inner future is **flattened** — the
/// output future resolves when the inner future resolves.
///
/// This is the FFI equivalent of Rust's `future.then(ctx, |v| -> Future<U>)`
/// with automatic unwrapping.
///
/// Consumes the input future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_then(
    future: orkester_future_t,
    context: orkester_context_t,
    transform: orkester_then_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    let system = future.system();
    let tx = SendThenAsync {
        func: transform,
        ctx: SendCtx(ctx),
    };
    // The callback returns Future<Payload>. We use turbofish to select the
    // auto-flatten ResolveOutput impl (Future<Payload> → Payload).
    let next =
        future.then::<Payload, _, Future<Payload>>(context.to_context(), move |mut payload| {
            let tx = &tx;
            let input_value = payload.take_value();
            let inner_handle = unsafe { (tx.func)(tx.ctx.0, input_value) };
            if inner_handle.is_null() {
                // Null return → resolved with empty payload
                return system.resolved(Payload::empty());
            }
            unsafe { *Box::from_raw(inner_handle as *mut Future<Payload>) }
        });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach an async-transform continuation on a shared future.
/// Does NOT consume the shared future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_then(
    shared: orkester_shared_future_t,
    context: orkester_context_t,
    transform: orkester_then_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<Payload>) };
    let system = shared.system();
    let tx = SendThenAsync {
        func: transform,
        ctx: SendCtx(ctx),
    };
    let next =
        shared.then::<Payload, _, Future<Payload>>(context.to_context(), move |mut payload| {
            let tx = &tx;
            let input_value = payload.take_value();
            let inner_handle = unsafe { (tx.func)(tx.ctx.0, input_value) };
            if inner_handle.is_null() {
                return system.resolved(Payload::empty());
            }
            unsafe { *Box::from_raw(inner_handle as *mut Future<Payload>) }
        });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach an async-transform continuation in a thread pool.
/// Consumes the input future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_then_in_pool(
    future: orkester_future_t,
    pool: orkester_thread_pool_t,
    transform: orkester_then_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    let system = future.system();
    let pool = unsafe { &*(pool as *const ThreadPool) };
    let tx = SendThenAsync {
        func: transform,
        ctx: SendCtx(ctx),
    };
    let next = future.then_in_pool::<Payload, _, Future<Payload>>(pool, move |mut payload| {
        let tx = &tx;
        let input_value = payload.take_value();
        let inner_handle = unsafe { (tx.func)(tx.ctx.0, input_value) };
        if inner_handle.is_null() {
            return system.resolved(Payload::empty());
        }
        unsafe { *Box::from_raw(inner_handle as *mut Future<Payload>) }
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach an error-recovery handler that receives an opaque error pointer
/// and returns a recovery value. If the future resolves, the callback is
/// skipped and the value passes through unchanged.
///
/// The `callback` receives `(ctx, error_ptr)` where `error_ptr` is the
/// pointer previously passed to `orkester_promise_reject_with`, or null
/// if the error was not opaque. The callback returns a `void*` recovery
/// value.
///
/// Consumes the input future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_catch(
    future: orkester_future_t,
    context: orkester_context_t,
    callback: orkester_catch_fn_t,
    ctx: *mut c_void,
    result_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    let cb = SendCatchOpaque {
        func: callback,
        ctx: SendCtx(ctx),
        result_destroy,
    };
    let next = future.catch(context.to_context(), move |error| {
        let cb = &cb;
        let opaque_ptr = error
            .downcast_ref::<OpaqueError>()
            .map(|e| e.ptr)
            .unwrap_or(std::ptr::null_mut());
        let recovery_value = unsafe { (cb.func)(cb.ctx.0, opaque_ptr) };
        Payload::new(recovery_value, cb.result_destroy)
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach an error-recovery handler on a shared future that receives an
/// opaque error pointer and returns a recovery value.
/// Does NOT consume the shared future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_catch(
    shared: orkester_shared_future_t,
    context: orkester_context_t,
    callback: orkester_catch_fn_t,
    ctx: *mut c_void,
    result_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) -> orkester_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<Payload>) };
    let cb = SendCatchOpaque {
        func: callback,
        ctx: SendCtx(ctx),
        result_destroy,
    };
    let next = shared.catch(context.to_context(), move |error| {
        let cb = &cb;
        let opaque_ptr = error
            .downcast_ref::<OpaqueError>()
            .map(|e| e.ptr)
            .unwrap_or(std::ptr::null_mut());
        let recovery_value = unsafe { (cb.func)(cb.ctx.0, opaque_ptr) };
        Payload::new(recovery_value, cb.result_destroy)
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Wait for all futures to complete, preserving per-element payloads.
/// Consumes all input future handles.
///
/// Returns a single future whose payload is a `PayloadArray`. Use
/// `orkester_payload_array_len` and `orkester_payload_array_get` to
/// extract individual values from the result.
///
/// If any input future rejects, the output future rejects with the first
/// error. Successfully resolved payloads are dropped in that case.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_all_with_values(
    system: *const orkester_async_t,
    futures: *mut orkester_future_t,
    count: usize,
) -> orkester_future_t {
    let system_ref = unsafe { &*(system as *const AsyncSystem) };

    if count == 0 {
        let arr = PayloadArray {
            payloads: Vec::new(),
        };
        let resolved = system_ref.resolved(Payload::new(
            Box::into_raw(Box::new(arr)) as *mut c_void,
            Some(payload_array_destroy),
        ));
        return Box::into_raw(Box::new(resolved)) as orkester_future_t;
    }

    let mut futs: Vec<Future<Payload>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *futures.add(i) };
        futs.push(unsafe { *Box::from_raw(handle as *mut Future<Payload>) });
    }

    let combined = system_ref.join_all(futs);
    // join_all returns Future<Vec<Payload>>; wrap into a PayloadArray payload
    let result = combined.map(|payloads| {
        let arr = PayloadArray { payloads };
        Payload::new(
            Box::into_raw(Box::new(arr)) as *mut c_void,
            Some(payload_array_destroy),
        )
    });
    Box::into_raw(Box::new(result)) as orkester_future_t
}

/// Destructor for a `PayloadArray`. Called automatically when the
/// wrapping future is dropped or the caller finishes extracting values.
unsafe extern "C" fn payload_array_destroy(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr as *mut PayloadArray)) };
    }
}

/// Get the number of elements in a payload array.
/// `array_payload` is the `out_value` pointer obtained from blocking a
/// future returned by `orkester_future_all_with_values`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_payload_array_len(array_payload: *const c_void) -> usize {
    if array_payload.is_null() {
        return 0;
    }
    let arr = unsafe { &*(array_payload as *const PayloadArray) };
    arr.payloads.len()
}

/// Extract the value pointer at `index` from a payload array.
/// Transfers ownership of the value to the caller. Each index should
/// only be extracted once. Returns null if the index is out of bounds
/// or the value was already extracted.
///
/// `array_payload` is the `out_value` obtained from blocking the future
/// returned by `orkester_future_all_with_values`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_payload_array_get(
    array_payload: *mut c_void,
    index: usize,
) -> *mut c_void {
    if array_payload.is_null() {
        return std::ptr::null_mut();
    }
    let arr = unsafe { &mut *(array_payload as *mut PayloadArray) };
    if index >= arr.payloads.len() {
        return std::ptr::null_mut();
    }
    arr.payloads[index].take_value()
}

/// Destroy a payload array and all remaining (un-extracted) values.
/// Call this after you've extracted the values you need.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_payload_array_drop(array_payload: *mut c_void) {
    if !array_payload.is_null() {
        unsafe { drop(Box::from_raw(array_payload as *mut PayloadArray)) };
    }
}

/// Block until a shared future completes, dispatching main-thread tasks
/// while waiting. Does NOT consume the handle.
/// On success, writes a clone of the payload value to `out_value` (if non-null).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_block_with_main(
    shared: orkester_shared_future_t,
    out_value: *mut *mut c_void,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<Payload>) };
    match shared.block_with_main() {
        Ok(mut payload) => {
            if !out_value.is_null() {
                unsafe { *out_value = payload.take_value() };
            }
            true
        }
        Err(e) => {
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Callback that produces a new future for each retry attempt.
/// Must return an `orkester_future_t`. Returning null is treated as failure.
pub type orkester_retry_fn_t = unsafe extern "C" fn(ctx: *mut c_void) -> orkester_future_t;

struct SendRetry {
    func: orkester_retry_fn_t,
    context: SendCtx,
}
unsafe impl Send for SendRetry {}
unsafe impl Sync for SendRetry {}

/// Retry an operation up to `max_attempts` times with exponential back-off.
///
/// On each attempt, `factory` is called to produce a new future. If that
/// future resolves, the retry future resolves. If it rejects and attempts
/// remain, the next attempt is scheduled after a back-off delay.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_retry(
    system: *const orkester_async_t,
    max_attempts: u32,
    factory: orkester_retry_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let system_ref = unsafe { &*(system as *const AsyncSystem) };
    let cb = SendRetry {
        func: factory,
        context: SendCtx(ctx),
    };
    let system_clone = system_ref.clone();
    let result = orkester::retry(system_ref, max_attempts, Default::default(), move || {
        let cb = &cb; // force whole-struct capture
        let handle = unsafe { (cb.func)(cb.context.0) };
        if handle.is_null() {
            return system_clone.resolved(Err(orkester::AsyncError::msg(
                "retry factory returned null",
            )));
        }
        let future = unsafe { *Box::from_raw(handle as *mut Future<Payload>) };
        future.then(Context::IMMEDIATE, |payload| Ok(payload))
    });
    Box::into_raw(Box::new(result)) as orkester_future_t
}

/// Opaque handle to a `JoinSet<Payload>`.
pub type orkester_join_set_t = *mut c_void;

/// Create a new JoinSet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_create(
    system: *const orkester_async_t,
) -> orkester_join_set_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let js: JoinSet<Payload> = system.join_set();
    Box::into_raw(Box::new(js)) as orkester_join_set_t
}

/// Push a future into the JoinSet. Consumes the future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_push(
    js: orkester_join_set_t,
    future: orkester_future_t,
) {
    let js = unsafe { &mut *(js as *mut JoinSet<Payload>) };
    let future = unsafe { *Box::from_raw(future as *mut Future<Payload>) };
    js.push(future);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_len(js: orkester_join_set_t) -> usize {
    let js = unsafe { &*(js as *const JoinSet<Payload>) };
    js.len()
}

/// Wait for all futures in the JoinSet. Consumes the handle.
/// Returns the number of futures that resolved successfully.
/// `out_total` receives the total count. Failures are counted as
/// `*out_total - return_value`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_join_all(
    js: orkester_join_set_t,
    out_total: *mut usize,
) -> usize {
    let js = unsafe { *Box::from_raw(js as *mut JoinSet<Payload>) };
    let results = js.join_all();
    let total = results.len();
    let ok_count = results.iter().filter(|r| r.is_ok()).count();
    if !out_total.is_null() {
        unsafe { *out_total = total };
    }
    ok_count
}

/// Wait for the next future to complete. Returns true if a result was
/// obtained, false if the JoinSet is empty.
/// `out_ok` is set to true if the future resolved, false if it rejected.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_join_next(
    js: orkester_join_set_t,
    out_ok: *mut bool,
) -> bool {
    let js = unsafe { &mut *(js as *mut JoinSet<Payload>) };
    match js.join_next() {
        Some(result) => {
            if !out_ok.is_null() {
                unsafe { *out_ok = result.is_ok() };
            }
            true
        }
        None => false,
    }
}

/// Drop a JoinSet without waiting. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_drop(js: orkester_join_set_t) {
    if !js.is_null() {
        unsafe { drop(Box::from_raw(js as *mut JoinSet<Payload>)) };
    }
}

/// Opaque handle to a channel `Sender<*mut c_void>`.
pub type orkester_sender_t = *mut c_void;

/// Opaque handle to a channel `Receiver<*mut c_void>`.
pub type orkester_receiver_t = *mut c_void;

/// Create a bounded mpsc channel with the given capacity.
/// Writes sender and receiver handles to the output pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_channel_create(
    capacity: usize,
    out_sender: *mut orkester_sender_t,
    out_receiver: *mut orkester_receiver_t,
) {
    let (tx, rx) = channel::mpsc::<*mut c_void>(capacity);
    unsafe {
        *out_sender = Box::into_raw(Box::new(tx)) as orkester_sender_t;
        *out_receiver = Box::into_raw(Box::new(rx)) as orkester_receiver_t;
    }
}

/// Create a one-shot channel (capacity 1).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_channel_create_oneshot(
    out_sender: *mut orkester_sender_t,
    out_receiver: *mut orkester_receiver_t,
) {
    let (tx, rx) = channel::oneshot::<*mut c_void>();
    unsafe {
        *out_sender = Box::into_raw(Box::new(tx)) as orkester_sender_t;
        *out_receiver = Box::into_raw(Box::new(rx)) as orkester_receiver_t;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_clone(sender: orkester_sender_t) -> orkester_sender_t {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    Box::into_raw(Box::new(sender.clone())) as orkester_sender_t
}

/// Send a value through the channel (blocking if full).
/// Returns true on success, false if the receiver has been dropped.
/// On failure, `out_value` receives the unsent value back.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_send(
    sender: orkester_sender_t,
    value: *mut c_void,
    out_value: *mut *mut c_void,
) -> bool {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    match sender.send(value) {
        Ok(()) => true,
        Err(e) => {
            if !out_value.is_null() {
                unsafe { *out_value = e.0 };
            }
            false
        }
    }
}

/// Try to send without blocking.
/// On failure, `out_value` receives the unsent value back.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_try_send(
    sender: orkester_sender_t,
    value: *mut c_void,
    out_value: *mut *mut c_void,
) -> orkester_send_result_t {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    match sender.try_send(value) {
        Ok(()) => orkester_send_result_t::ORKESTER_SEND_OK,
        Err(e) => {
            let code = if e.is_full() {
                orkester_send_result_t::ORKESTER_SEND_FULL
            } else {
                orkester_send_result_t::ORKESTER_SEND_CLOSED
            };
            if !out_value.is_null() {
                unsafe { *out_value = e.into_inner() };
            }
            code
        }
    }
}

/// Send with a timeout in milliseconds.
/// On failure, `out_value` receives the value back.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_send_timeout(
    sender: orkester_sender_t,
    value: *mut c_void,
    timeout_ms: u64,
    out_value: *mut *mut c_void,
) -> orkester_send_result_t {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    match sender.send_timeout(value, std::time::Duration::from_millis(timeout_ms)) {
        Ok(()) => orkester_send_result_t::ORKESTER_SEND_OK,
        Err(e) => {
            let code = if e.is_full() {
                orkester_send_result_t::ORKESTER_SEND_FULL
            } else {
                orkester_send_result_t::ORKESTER_SEND_CLOSED
            };
            if !out_value.is_null() {
                unsafe { *out_value = e.into_inner() };
            }
            code
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_is_closed(sender: orkester_sender_t) -> bool {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    sender.is_closed()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_drop(sender: orkester_sender_t) {
    if !sender.is_null() {
        unsafe { drop(Box::from_raw(sender as *mut Sender<*mut c_void>)) };
    }
}

/// Receive a value (blocking). Returns true if a value was received.
/// `out_value` receives the pointer. Returns false when all senders are
/// dropped and the buffer is empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_recv(
    receiver: orkester_receiver_t,
    out_value: *mut *mut c_void,
) -> bool {
    let receiver = unsafe { &*(receiver as *const Receiver<*mut c_void>) };
    match receiver.recv() {
        Some(v) => {
            if !out_value.is_null() {
                unsafe { *out_value = v };
            }
            true
        }
        None => false,
    }
}

/// Non-blocking receive. Returns true if a value was available.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_try_recv(
    receiver: orkester_receiver_t,
    out_value: *mut *mut c_void,
) -> bool {
    let receiver = unsafe { &*(receiver as *const Receiver<*mut c_void>) };
    match receiver.try_recv() {
        Some(v) => {
            if !out_value.is_null() {
                unsafe { *out_value = v };
            }
            true
        }
        None => false,
    }
}

/// Receive with a timeout in milliseconds. Returns true if a value arrived
/// before the deadline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_recv_timeout(
    receiver: orkester_receiver_t,
    timeout_ms: u64,
    out_value: *mut *mut c_void,
) -> bool {
    let receiver = unsafe { &*(receiver as *const Receiver<*mut c_void>) };
    match receiver.recv_timeout(std::time::Duration::from_millis(timeout_ms)) {
        Some(v) => {
            if !out_value.is_null() {
                unsafe { *out_value = v };
            }
            true
        }
        None => false,
    }
}

/// Check if a receiver's channel is closed and empty.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_is_closed(receiver: orkester_receiver_t) -> bool {
    let receiver = unsafe { &*(receiver as *const Receiver<*mut c_void>) };
    receiver.is_closed()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_drop(receiver: orkester_receiver_t) {
    if !receiver.is_null() {
        unsafe { drop(Box::from_raw(receiver as *mut Receiver<*mut c_void>)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn create_test_system() -> *mut orkester_async_t {
        unsafe { orkester_async_create_default(2) }
    }

    #[test]
    fn promise_resolve_with_round_trips_value() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        // Resolve with a boxed i32
        let value = Box::into_raw(Box::new(42i32)) as *mut c_void;
        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }
        unsafe { orkester_promise_resolve_with(promise, value, Some(destroy_i32)) };

        // Wait and extract
        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert!(!out_value.is_null());
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, 42);

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn then_transforms_value() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        // Resolve with 10
        let value = Box::into_raw(Box::new(10i32)) as *mut c_void;
        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }
        unsafe { orkester_promise_resolve_with(promise, value, Some(destroy_i32)) };

        // Transform: multiply by 3
        unsafe extern "C" fn triple(_ctx: *mut c_void, val: *mut c_void) -> *mut c_void {
            let input = unsafe { Box::from_raw(val as *mut i32) };
            let output = *input * 3;
            Box::into_raw(Box::new(output)) as *mut c_void
        }
        let future = unsafe {
            orkester_future_map(
                future,
                orkester_context_t::ORKESTER_CONTEXT_BACKGROUND,
                triple,
                std::ptr::null_mut(),
                Some(destroy_i32),
            )
        };
        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, 30);

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn map_transforms_value_inline() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        let value = Box::into_raw(Box::new(7i32)) as *mut c_void;
        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }
        unsafe { orkester_promise_resolve_with(promise, value, Some(destroy_i32)) };

        unsafe extern "C" fn double(_ctx: *mut c_void, val: *mut c_void) -> *mut c_void {
            let input = unsafe { Box::from_raw(val as *mut i32) };
            Box::into_raw(Box::new(*input * 2)) as *mut c_void
        }
        let future = unsafe {
            orkester_future_map(
                future,
                orkester_context_t::ORKESTER_CONTEXT_IMMEDIATE,
                double,
                std::ptr::null_mut(),
                Some(destroy_i32),
            )
        };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, 14);

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn payload_destructor_is_called_on_drop() {
        static DESTROYED: AtomicUsize = AtomicUsize::new(0);

        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        let value = Box::into_raw(Box::new(1i32)) as *mut c_void;
        unsafe extern "C" fn counting_destroy(p: *mut c_void) {
            DESTROYED.fetch_add(1, Ordering::SeqCst);
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }
        unsafe { orkester_promise_resolve_with(promise, value, Some(counting_destroy)) };

        DESTROYED.store(0, Ordering::SeqCst);
        // Drop the future without waiting — destructor should fire
        unsafe { orkester_future_drop(future) };
        // Give the background thread time to resolve
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(DESTROYED.load(Ordering::SeqCst), 1);

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn shared_future_clones_payload() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        unsafe extern "C" fn clone_i32(p: *mut c_void) -> *mut c_void {
            let val = unsafe { *(p as *const i32) };
            Box::into_raw(Box::new(val)) as *mut c_void
        }
        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }

        let value = Box::into_raw(Box::new(55i32)) as *mut c_void;
        unsafe {
            orkester_promise_resolve_with(promise, value, Some(destroy_i32))
        };

        let shared = unsafe { orkester_future_share(future, Some(clone_i32)) };
        let shared2 = unsafe { orkester_shared_future_clone(shared) };

        // Both clones should produce the value
        let mut v1: *mut c_void = std::ptr::null_mut();
        let mut v2: *mut c_void = std::ptr::null_mut();
        let ok1 = unsafe {
            orkester_shared_future_block(
                shared,
                &mut v1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        let ok2 = unsafe {
            orkester_shared_future_block(
                shared2,
                &mut v2,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok1);
        assert!(ok2);

        let r1 = unsafe { Box::from_raw(v1 as *mut i32) };
        let r2 = unsafe { Box::from_raw(v2 as *mut i32) };
        assert_eq!(*r1, 55);
        assert_eq!(*r2, 55);

        unsafe { orkester_shared_future_drop(shared) };
        unsafe { orkester_shared_future_drop(shared2) };
        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn empty_payload_wait_returns_null() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        // Resolve with no payload
        unsafe { orkester_promise_resolve(promise) };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert!(out_value.is_null());

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn then_chain_carries_value_across_contexts() {
        let sys = create_test_system();
        let scope = unsafe { orkester_main_thread_scope_create(sys) };

        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }
        unsafe extern "C" fn add_one(_ctx: *mut c_void, val: *mut c_void) -> *mut c_void {
            let input = unsafe { Box::from_raw(val as *mut i32) };
            Box::into_raw(Box::new(*input + 1)) as *mut c_void
        }

        let value = Box::into_raw(Box::new(0i32)) as *mut c_void;
        unsafe { orkester_promise_resolve_with(promise, value, Some(destroy_i32)) };

        // Chain: worker +1, main +1, worker +1 = 3
        let future = unsafe {
            orkester_future_map(
                future,
                orkester_context_t::ORKESTER_CONTEXT_BACKGROUND,
                add_one,
                std::ptr::null_mut(),
                Some(destroy_i32),
            )
        };
        let future = unsafe {
            orkester_future_map(
                future,
                orkester_context_t::ORKESTER_CONTEXT_MAIN,
                add_one,
                std::ptr::null_mut(),
                Some(destroy_i32),
            )
        };
        let future = unsafe {
            orkester_future_map(
                future,
                orkester_context_t::ORKESTER_CONTEXT_BACKGROUND,
                add_one,
                std::ptr::null_mut(),
                Some(destroy_i32),
            )
        };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block_with_main(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, 3);

        unsafe { orkester_main_thread_scope_drop(scope) };
        unsafe { orkester_async_destroy(sys) };
    }

    // ─── Tests for new Phase-2 FFI functions ───────────────────────────

    #[test]
    fn then_async_flattens_inner_future() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }

        // Resolve outer with 5
        let value = Box::into_raw(Box::new(5i32)) as *mut c_void;
        unsafe { orkester_promise_resolve_with(promise, value, Some(destroy_i32)) };

        // Async transform: receives 5, creates a new resolved future with 5*10=50
        unsafe extern "C" fn async_multiply(
            ctx: *mut c_void,
            val: *mut c_void,
        ) -> orkester_future_t {
            let sys = ctx; // ctx holds the system pointer
            let input = unsafe { Box::from_raw(val as *mut i32) };
            let output = *input * 10;
            unsafe extern "C" fn destroy_i32(p: *mut c_void) {
                drop(unsafe { Box::from_raw(p as *mut i32) });
            }
            let output_ptr = Box::into_raw(Box::new(output)) as *mut c_void;
            unsafe {
                orkester_future_create_resolved_with(
                    sys as *const _,
                    output_ptr,
                    Some(destroy_i32),
                )
            }
        }

        // Use sys as the context so the callback can create futures
        let future = unsafe {
            orkester_future_then(
                future,
                orkester_context_t::ORKESTER_CONTEXT_BACKGROUND,
                async_multiply,
                sys as *mut c_void,
            )
        };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, 50);

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn then_async_null_return_resolves_empty() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }

        let value = Box::into_raw(Box::new(1i32)) as *mut c_void;
        unsafe { orkester_promise_resolve_with(promise, value, Some(destroy_i32)) };

        // Callback returns null → resolved with empty payload
        unsafe extern "C" fn return_null(_ctx: *mut c_void, val: *mut c_void) -> orkester_future_t {
            // Consume the input value
            drop(unsafe { Box::from_raw(val as *mut i32) });
            std::ptr::null_mut()
        }

        let future = unsafe {
            orkester_future_then(
                future,
                orkester_context_t::ORKESTER_CONTEXT_BACKGROUND,
                return_null,
                std::ptr::null_mut(),
            )
        };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert!(out_value.is_null()); // empty payload

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn reject_opaque_and_catch_with_value() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }

        // Reject with an opaque error (a boxed i32 error code)
        let error = Box::into_raw(Box::new(404i32)) as *mut c_void;
        unsafe { orkester_promise_reject_with(promise, error, Some(destroy_i32)) };

        // Catch: receive opaque error, return recovery value
        unsafe extern "C" fn recover(_ctx: *mut c_void, error_ptr: *mut c_void) -> *mut c_void {
            assert!(!error_ptr.is_null());
            let code = unsafe { *(error_ptr as *const i32) };
            assert_eq!(code, 404);
            // Return a recovery value
            Box::into_raw(Box::new(-1i32)) as *mut c_void
        }

        let future = unsafe {
            orkester_future_catch(
                future,
                orkester_context_t::ORKESTER_CONTEXT_BACKGROUND,
                recover,
                std::ptr::null_mut(),
                Some(destroy_i32),
            )
        };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, -1);

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn catch_with_value_passes_through_on_success() {
        let sys = create_test_system();
        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }

        // Resolve normally
        let value = Box::into_raw(Box::new(42i32)) as *mut c_void;
        unsafe { orkester_promise_resolve_with(promise, value, Some(destroy_i32)) };

        // Catch should be skipped; value passes through
        unsafe extern "C" fn should_not_be_called(
            _ctx: *mut c_void,
            _error: *mut c_void,
        ) -> *mut c_void {
            panic!("catch handler should not be called on success");
        }

        let future = unsafe {
            orkester_future_catch(
                future,
                orkester_context_t::ORKESTER_CONTEXT_BACKGROUND,
                should_not_be_called,
                std::ptr::null_mut(),
                Some(destroy_i32),
            )
        };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                future,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, 42);

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn all_with_values_preserves_payloads() {
        let sys = create_test_system();

        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }

        // Create 3 resolved futures with values 10, 20, 30
        let mut futures: Vec<orkester_future_t> = Vec::new();
        for val in [10i32, 20, 30] {
            let ptr = Box::into_raw(Box::new(val)) as *mut c_void;
            let f = unsafe {
                orkester_future_create_resolved_with(sys as *const _, ptr, Some(destroy_i32))
            };
            futures.push(f);
        }

        let combined = unsafe {
            orkester_future_all_with_values(sys as *const _, futures.as_mut_ptr(), futures.len())
        };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                combined,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert!(!out_value.is_null());

        // Extract values from the payload array
        let len = unsafe { orkester_payload_array_len(out_value) };
        assert_eq!(len, 3);

        let v0 = unsafe { orkester_payload_array_get(out_value, 0) };
        let v1 = unsafe { orkester_payload_array_get(out_value, 1) };
        let v2 = unsafe { orkester_payload_array_get(out_value, 2) };

        assert_eq!(unsafe { *(v0 as *const i32) }, 10);
        assert_eq!(unsafe { *(v1 as *const i32) }, 20);
        assert_eq!(unsafe { *(v2 as *const i32) }, 30);

        // Clean up extracted values
        unsafe { destroy_i32(v0) };
        unsafe { destroy_i32(v1) };
        unsafe { destroy_i32(v2) };

        // Drop the array itself (remaining payloads already extracted)
        unsafe { orkester_payload_array_drop(out_value) };

        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn all_with_values_empty_array() {
        let sys = create_test_system();

        let combined =
            unsafe { orkester_future_all_with_values(sys as *const _, std::ptr::null_mut(), 0) };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_future_block(
                combined,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        assert!(!out_value.is_null());

        let len = unsafe { orkester_payload_array_len(out_value) };
        assert_eq!(len, 0);

        unsafe { orkester_payload_array_drop(out_value) };
        unsafe { orkester_async_destroy(sys) };
    }

    #[test]
    fn shared_future_block_with_main_works() {
        let sys = create_test_system();
        let scope = unsafe { orkester_main_thread_scope_create(sys) };

        let mut promise: orkester_promise_t = std::ptr::null_mut();
        let mut future: orkester_future_t = std::ptr::null_mut();
        unsafe { orkester_promise_create(sys, &mut promise, &mut future) };

        unsafe extern "C" fn clone_i32(p: *mut c_void) -> *mut c_void {
            let val = unsafe { *(p as *const i32) };
            Box::into_raw(Box::new(val)) as *mut c_void
        }
        unsafe extern "C" fn destroy_i32(p: *mut c_void) {
            drop(unsafe { Box::from_raw(p as *mut i32) });
        }

        let value = Box::into_raw(Box::new(77i32)) as *mut c_void;
        unsafe {
            orkester_promise_resolve_with(promise, value, Some(destroy_i32))
        };

        let shared = unsafe { orkester_future_share(future, Some(clone_i32)) };

        let mut out_value: *mut c_void = std::ptr::null_mut();
        let ok = unsafe {
            orkester_shared_future_block_with_main(
                shared,
                &mut out_value,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert!(ok);
        let result = unsafe { Box::from_raw(out_value as *mut i32) };
        assert_eq!(*result, 77);

        unsafe { orkester_shared_future_drop(shared) };
        unsafe { orkester_main_thread_scope_drop(scope) };
        unsafe { orkester_async_destroy(sys) };
    }
}
