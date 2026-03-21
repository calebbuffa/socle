//! C-ABI FFI for orkester scheduling primitives.
//!
//! This crate provides the thin C boundary for orkester's async runtime.
//! All callbacks use the simple `void(*)(void*)` signature.
//! No orchestration protocol — typed value handling stays in the consumer language.

// C-style snake_case naming is intentional for FFI types.
#![allow(non_camel_case_types)]

use orkester::channel::{self, Receiver, Sender};
use orkester::{
    AsyncSystem, CancellationToken, Context, ErrorCode, Future, JoinSet, MainThreadScope,
    Semaphore, TaskProcessor, ThreadPool, ThreadPoolTaskProcessor,
};
use std::ffi::{c_char, c_void};
use std::sync::Arc;


/// Opaque async runtime handle exposed to C. Wraps `orkester::AsyncSystem`.
///
/// cbindgen sees this as an opaque struct so it emits a forward declaration.
#[repr(C)]
pub struct orkester_async_t {
    _opaque: u8,
}

/// Opaque handle to a `Future<()>`.
pub type orkester_future_t = *mut c_void;

/// Opaque handle to a `SharedFuture<()>` (cloneable).
pub type orkester_shared_future_t = *mut c_void;

/// Opaque handle to a `Promise<()>` (completion trigger).
pub type orkester_promise_t = *mut c_void;

/// Opaque handle to a `MainThreadScope`.
pub type orkester_main_thread_scope_t = *mut c_void;

/// Opaque handle to a `ThreadPool`.
pub type orkester_thread_pool_t = *mut c_void;

/// Callback for continuations — `void(*)(void*)`.
pub type orkester_callback_fn_t = unsafe extern "C" fn(*mut c_void);

/// Scheduling context enum exposed to C.
#[repr(C)]
pub enum orkester_context_t {
    ORKESTER_WORKER = 0,
    ORKESTER_MAIN = 1,
    ORKESTER_IMMEDIATE = 2,
}

impl orkester_context_t {
    fn to_context(self) -> Context {
        match self {
            orkester_context_t::ORKESTER_WORKER => Context::Worker,
            orkester_context_t::ORKESTER_MAIN => Context::Main,
            orkester_context_t::ORKESTER_IMMEDIATE => Context::Immediate,
        }
    }
}


/// Dispatch function type for scheduling work on a background thread.
///
/// The host calls `work(work_data)` on a background thread.
/// Every language can implement this trivially — it's just `void(*)(void*)`.
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
    let processor = Arc::new(FfiTaskProcessor {
        dispatch,
        ctx: SendCtx(ctx),
        destroy,
    });
    let system = AsyncSystem::new(processor);
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


struct FfiTaskProcessor {
    dispatch: orkester_dispatch_fn_t,
    ctx: SendCtx,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Send for FfiTaskProcessor {}
unsafe impl Sync for FfiTaskProcessor {}

impl TaskProcessor for FfiTaskProcessor {
    fn start_task(&self, task: Box<dyn FnOnce() + Send + 'static>) {
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

impl Drop for FfiTaskProcessor {
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
    let (promise, future) = system.create_promise::<()>();
    unsafe {
        *out_promise = Box::into_raw(Box::new(promise)) as orkester_promise_t;
        *out_future = Box::into_raw(Box::new(future)) as orkester_future_t;
    }
}

/// Signal completion (resolve the promise). Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_resolve(promise: orkester_promise_t) {
    if !promise.is_null() {
        let promise = unsafe { Box::from_raw(promise as *mut orkester::Promise<()>) };
        promise.resolve(());
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
    let promise = unsafe { Box::from_raw(promise as *mut orkester::Promise<()>) };
    let msg = if message.is_null() {
        "FFI promise rejected".to_string()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(message as *const u8, message_len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    promise.reject(orkester::AsyncError::msg(msg));
}

/// Drop a promise without resolving.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_drop(promise: orkester_promise_t) {
    if !promise.is_null() {
        unsafe { drop(Box::from_raw(promise as *mut orkester::Promise<()>)) };
    }
}


/// Check if a future has completed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_is_ready(future: orkester_future_t) -> bool {
    let future = unsafe { &*(future as *const Future<()>) };
    future.is_ready()
}

/// Block until a future completes. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_wait(
    future: orkester_future_t,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    if future.is_null() {
        return false;
    }
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    match future.wait() {
        Ok(()) => true,
        Err(e) => {
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Block in main thread, dispatching main-thread tasks while waiting. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_wait_in_main(
    future: orkester_future_t,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    if future.is_null() {
        return false;
    }
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    match future.wait_in_main_thread() {
        Ok(()) => true,
        Err(e) => {
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Attach a continuation in the given scheduling context. Consumes the input handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_then(
    future: orkester_future_t,
    context: orkester_context_t,
    callback: orkester_callback_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(ctx),
    };
    let next = future.then(context.to_context(), move |()| {
        unsafe { cb.call() };
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach a continuation on a worker thread. Consumes the input handle.
/// Compatibility wrapper — prefer `orkester_future_then` with `ORKESTER_WORKER`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_then_in_worker(
    future: orkester_future_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe {
        orkester_future_then(
            future,
            orkester_context_t::ORKESTER_WORKER,
            callback,
            context,
        )
    }
}

/// Attach a continuation on the main thread. Consumes the input handle.
/// Compatibility wrapper — prefer `orkester_future_then` with `ORKESTER_MAIN`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_then_in_main(
    future: orkester_future_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe { orkester_future_then(future, orkester_context_t::ORKESTER_MAIN, callback, context) }
}

/// Attach a continuation that runs immediately. Consumes the input handle.
/// Compatibility wrapper — prefer `orkester_future_then` with `ORKESTER_IMMEDIATE`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_then_immediately(
    future: orkester_future_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe {
        orkester_future_then(
            future,
            orkester_context_t::ORKESTER_IMMEDIATE,
            callback,
            context,
        )
    }
}

/// Convert a future to a shared future. Consumes the future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_share(
    future: orkester_future_t,
) -> orkester_shared_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    let shared = future.share();
    Box::into_raw(Box::new(shared)) as orkester_shared_future_t
}

/// Drop a future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_drop(future: orkester_future_t) {
    if !future.is_null() {
        unsafe { drop(Box::from_raw(future as *mut Future<()>)) };
    }
}


/// Clone a shared future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_clone(
    shared: orkester_shared_future_t,
) -> orkester_shared_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<()>) };
    Box::into_raw(Box::new(shared.clone())) as orkester_shared_future_t
}

/// Check if a shared future has completed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_is_ready(shared: orkester_shared_future_t) -> bool {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<()>) };
    shared.is_ready()
}

/// Block until a shared future completes. Does NOT consume the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_wait(
    shared: orkester_shared_future_t,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<()>) };
    match shared.wait() {
        Ok(()) => true,
        Err(e) => {
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Attach a continuation on a shared future in the given scheduling context.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_then(
    shared: orkester_shared_future_t,
    context: orkester_context_t,
    callback: orkester_callback_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<()>) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(ctx),
    };
    let next = shared.then(context.to_context(), move |()| {
        unsafe { cb.call() };
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach a continuation on a shared future (worker thread).
/// Compatibility wrapper — prefer `orkester_shared_future_then` with `ORKESTER_WORKER`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_then_in_worker(
    shared: orkester_shared_future_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe {
        orkester_shared_future_then(
            shared,
            orkester_context_t::ORKESTER_WORKER,
            callback,
            context,
        )
    }
}

/// Attach a continuation on a shared future (main thread).
/// Compatibility wrapper — prefer `orkester_shared_future_then` with `ORKESTER_MAIN`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_then_in_main(
    shared: orkester_shared_future_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe {
        orkester_shared_future_then(shared, orkester_context_t::ORKESTER_MAIN, callback, context)
    }
}

/// Attach a continuation on a shared future (immediately).
/// Compatibility wrapper — prefer `orkester_shared_future_then` with `ORKESTER_IMMEDIATE`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_then_immediately(
    shared: orkester_shared_future_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe {
        orkester_shared_future_then(
            shared,
            orkester_context_t::ORKESTER_IMMEDIATE,
            callback,
            context,
        )
    }
}

/// Drop a shared future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_drop(shared: orkester_shared_future_t) {
    if !shared.is_null() {
        unsafe { drop(Box::from_raw(shared as *mut orkester::SharedFuture<()>)) };
    }
}

/// Convert a shared future into a unique future. Consumes the shared handle.
///
/// Creates a `Future<()>` that completes when the shared future completes.
/// The shared handle is consumed (dropped); other clones remain valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_into_unique(
    shared: orkester_shared_future_t,
) -> orkester_future_t {
    let shared = unsafe { Box::from_raw(shared as *mut orkester::SharedFuture<()>) };
    let unique = shared.then_immediately(|()| {});
    Box::into_raw(Box::new(unique)) as orkester_future_t
}


/// Create an already-resolved future.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_create_resolved(
    system: *const orkester_async_t,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let future = system.create_resolved_future(());
    Box::into_raw(Box::new(future)) as orkester_future_t
}

/// Schedule a callback in the given context. Returns a future.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_run(
    system: *const orkester_async_t,
    context: orkester_context_t,
    callback: orkester_callback_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(ctx),
    };
    let future = system.run(context.to_context(), move || {
        unsafe { cb.call() };
    });
    Box::into_raw(Box::new(future)) as orkester_future_t
}

/// Schedule a callback on a worker thread. Returns a future.
/// Compatibility wrapper — prefer `orkester_async_run` with `ORKESTER_WORKER`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_run_in_worker(
    system: *const orkester_async_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe {
        orkester_async_run(
            system,
            orkester_context_t::ORKESTER_WORKER,
            callback,
            context,
        )
    }
}

/// Schedule a callback on the main thread. Returns a future.
/// Compatibility wrapper — prefer `orkester_async_run` with `ORKESTER_MAIN`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_run_in_main(
    system: *const orkester_async_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    unsafe { orkester_async_run(system, orkester_context_t::ORKESTER_MAIN, callback, context) }
}

/// Dispatch all queued main-thread tasks. Returns how many were dispatched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_dispatch(system: *const orkester_async_t) -> usize {
    let system = unsafe { &*(system as *const AsyncSystem) };
    system.dispatch_main_thread_tasks()
}

/// Dispatch a single main-thread task. Returns true if one was dispatched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_dispatch_one(system: *const orkester_async_t) -> bool {
    let system = unsafe { &*(system as *const AsyncSystem) };
    system.dispatch_one_main_thread_task()
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
    let scope = system.enter_main_thread();
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

/// Drop a thread pool handle. Consumes the handle.
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

/// Schedule a callback in a thread pool. Returns a future.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_run_in_pool(
    system: *const orkester_async_t,
    pool: orkester_thread_pool_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let pool = unsafe { &*(pool as *const ThreadPool) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(context),
    };
    let future = system.run_in_pool(pool, move || {
        unsafe { cb.call() };
    });
    Box::into_raw(Box::new(future)) as orkester_future_t
}

/// Attach a continuation in a thread pool. Consumes the input handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_then_in_pool(
    future: orkester_future_t,
    pool: orkester_thread_pool_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    let pool = unsafe { &*(pool as *const ThreadPool) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(context),
    };
    let next = future.then_in_pool(pool, move |()| {
        unsafe { cb.call() };
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach a continuation on a shared future in a thread pool.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_then_in_pool(
    shared: orkester_shared_future_t,
    pool: orkester_thread_pool_t,
    callback: orkester_callback_fn_t,
    context: *mut c_void,
) -> orkester_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<()>) };
    let pool = unsafe { &*(pool as *const ThreadPool) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(context),
    };
    let next = shared.then_in_pool(pool, move |()| {
        unsafe { cb.call() };
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}


/// Create an `orkester_async_t` with a built-in thread pool task processor.
/// No vtable needed — orkester manages its own threads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_async_create_default(
    num_threads: usize,
) -> *mut orkester_async_t {
    let processor = Arc::new(ThreadPoolTaskProcessor::new(num_threads));
    let system = AsyncSystem::new(processor);
    Box::into_raw(Box::new(system)) as *mut orkester_async_t
}

/// Wait for all futures to complete. Consumes all input handles.
/// Returns a single future that resolves when every input has resolved.
/// If any input future rejects, the output future rejects with the first error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_all(
    system: *const orkester_async_t,
    futures: *mut orkester_future_t,
    count: usize,
) -> orkester_future_t {
    let system_ref = unsafe { &*(system as *const AsyncSystem) };

    if count == 0 {
        let resolved = system_ref.create_resolved_future(());
        return Box::into_raw(Box::new(resolved)) as orkester_future_t;
    }

    let mut futs: Vec<Future<()>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *futures.add(i) };
        futs.push(unsafe { *Box::from_raw(handle as *mut Future<()>) });
    }

    let combined = system_ref.all(futs);
    // all() returns Future<Vec<()>>; map to Future<()>
    let signal = combined.then(Context::Immediate, |_| ());
    Box::into_raw(Box::new(signal)) as orkester_future_t
}

/// Reject a promise with an integer error code and message. Consumes the handle.
/// The code is prepended to the message as "[code] message".
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_promise_reject_with_code(
    promise: orkester_promise_t,
    code: i32,
    message: *const c_char,
    message_len: usize,
) {
    if promise.is_null() {
        return;
    }
    let promise = unsafe { Box::from_raw(promise as *mut orkester::Promise<()>) };
    let text = if message.is_null() {
        String::new()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(message as *const u8, message_len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    let msg = format!("[{}] {}", code, text);
    promise.reject(orkester::AsyncError::msg(msg));
}


/// Structured error code exposed to C.
#[repr(C)]
pub enum orkester_error_code_t {
    ORKESTER_ERROR_GENERIC = 0,
    ORKESTER_ERROR_CANCELLED = 1,
    ORKESTER_ERROR_TIMED_OUT = 2,
    ORKESTER_ERROR_DROPPED = 3,
}

impl orkester_error_code_t {
    fn from_code(code: ErrorCode) -> Self {
        match code {
            ErrorCode::Generic => orkester_error_code_t::ORKESTER_ERROR_GENERIC,
            ErrorCode::Cancelled => orkester_error_code_t::ORKESTER_ERROR_CANCELLED,
            ErrorCode::TimedOut => orkester_error_code_t::ORKESTER_ERROR_TIMED_OUT,
            ErrorCode::Dropped => orkester_error_code_t::ORKESTER_ERROR_DROPPED,
        }
    }
}

/// Wait for a future to complete and retrieve the error code if it failed.
/// Returns true on success, false on error.
/// On error, writes the error code, message pointer, and message length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_wait_with_code(
    future: orkester_future_t,
    out_code: *mut orkester_error_code_t,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    if future.is_null() {
        return false;
    }
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    match future.wait() {
        Ok(()) => true,
        Err(e) => {
            if !out_code.is_null() {
                unsafe { *out_code = orkester_error_code_t::from_code(e.code()) };
            }
            unsafe { write_ffi_error(e, out_error_ptr, out_error_len) };
            false
        }
    }
}


/// Opaque handle to a `CancellationToken`.
pub type orkester_cancel_token_t = *mut c_void;

/// Create a new cancellation token.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_cancel_token_create() -> orkester_cancel_token_t {
    Box::into_raw(Box::new(CancellationToken::new())) as orkester_cancel_token_t
}

/// Clone a cancellation token handle.
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

/// Check if a cancellation token has been signalled.
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

/// Drop a cancellation token handle.
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
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    let token = unsafe { &*(token as *const CancellationToken) };
    let result = future.with_cancellation(token);
    Box::into_raw(Box::new(result)) as orkester_future_t
}


/// Create a future that completes after `millis` milliseconds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_delay(
    system: *const orkester_async_t,
    millis: u64,
) -> orkester_future_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let future = orkester::delay(system, std::time::Duration::from_millis(millis));
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
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
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
    let mut futs: Vec<Future<()>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *futures.add(i) };
        if !handle.is_null() {
            futs.push(unsafe { *Box::from_raw(handle as *mut Future<()>) });
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
    system.spawn(context.to_context(), move || {
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

/// Return the number of available permits.
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

/// Drop a semaphore handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_drop(sem: orkester_semaphore_t) {
    if !sem.is_null() {
        unsafe { drop(Box::from_raw(sem as *mut Semaphore)) };
    }
}


/// Attach an error-recovery handler to a future. If the future rejects,
/// the callback is invoked in the given scheduling context. The returned
/// future resolves with `()` either way.
///
/// The callback receives a pointer to the error message and its byte length.
/// The pointer is valid only for the duration of the callback.
///
/// Consumes the input future handle.
pub type orkester_catch_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, error_ptr: *const c_char, error_len: usize);

struct SendCatch {
    func: orkester_catch_fn_t,
    context: SendCtx,
}
unsafe impl Send for SendCatch {}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_future_catch(
    future: orkester_future_t,
    context: orkester_context_t,
    callback: orkester_catch_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    let cb = SendCatch {
        func: callback,
        context: SendCtx(ctx),
    };
    let next = future.catch(context.to_context(), move |error| {
        let cb = &cb; // force whole-struct capture
        let msg = error.to_string();
        unsafe { (cb.func)(cb.context.0, msg.as_ptr() as *const c_char, msg.len()) };
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
}

/// Attach an error-recovery handler on a shared future. Does NOT consume
/// the shared future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_shared_future_catch(
    shared: orkester_shared_future_t,
    context: orkester_context_t,
    callback: orkester_catch_fn_t,
    ctx: *mut c_void,
) -> orkester_future_t {
    let shared = unsafe { &*(shared as *const orkester::SharedFuture<()>) };
    let cb = SendCatch {
        func: callback,
        context: SendCtx(ctx),
    };
    let next = shared.catch(context.to_context(), move |error| {
        let cb = &cb; // force whole-struct capture
        let msg = error.to_string();
        unsafe { (cb.func)(cb.context.0, msg.as_ptr() as *const c_char, msg.len()) };
    });
    Box::into_raw(Box::new(next)) as orkester_future_t
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
    let result = orkester::retry(system_ref, max_attempts, move || {
        let cb = &cb; // force whole-struct capture
        let handle = unsafe { (cb.func)(cb.context.0) };
        if handle.is_null() {
            return system_clone.create_resolved_future(Err(orkester::AsyncError::msg(
                "retry factory returned null",
            )));
        }
        let future = unsafe { *Box::from_raw(handle as *mut Future<()>) };
        future.then(Context::Immediate, |()| Ok(()))
    });
    Box::into_raw(Box::new(result)) as orkester_future_t
}


/// Opaque handle to a `JoinSet<()>`.
pub type orkester_join_set_t = *mut c_void;

/// Create a new JoinSet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_create(
    system: *const orkester_async_t,
) -> orkester_join_set_t {
    let system = unsafe { &*(system as *const AsyncSystem) };
    let js: JoinSet<()> = system.join_set();
    Box::into_raw(Box::new(js)) as orkester_join_set_t
}

/// Push a future into the JoinSet. Consumes the future handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_push(
    js: orkester_join_set_t,
    future: orkester_future_t,
) {
    let js = unsafe { &mut *(js as *mut JoinSet<()>) };
    let future = unsafe { *Box::from_raw(future as *mut Future<()>) };
    js.push(future);
}

/// Return the number of futures in the JoinSet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_len(js: orkester_join_set_t) -> usize {
    let js = unsafe { &*(js as *const JoinSet<()>) };
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
    let js = unsafe { *Box::from_raw(js as *mut JoinSet<()>) };
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
    let js = unsafe { &mut *(js as *mut JoinSet<()>) };
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
        unsafe { drop(Box::from_raw(js as *mut JoinSet<()>)) };
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

/// Clone a sender handle.
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

/// Try to send without blocking. Returns 0 on success, 1 if full, 2 if closed.
/// On failure, `out_value` receives the unsent value back.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_try_send(
    sender: orkester_sender_t,
    value: *mut c_void,
    out_value: *mut *mut c_void,
) -> u32 {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    match sender.try_send(value) {
        Ok(()) => 0,
        Err(e) => {
            let code = if e.is_full() { 1 } else { 2 };
            if !out_value.is_null() {
                unsafe { *out_value = e.into_inner() };
            }
            code
        }
    }
}

/// Send with a timeout in milliseconds. Returns 0 on success, 1 if timed
/// out (full), 2 if closed. On failure, `out_value` receives the value back.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_send_timeout(
    sender: orkester_sender_t,
    value: *mut c_void,
    timeout_ms: u64,
    out_value: *mut *mut c_void,
) -> u32 {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    match sender.send_timeout(value, std::time::Duration::from_millis(timeout_ms)) {
        Ok(()) => 0,
        Err(e) => {
            let code = if e.is_full() { 1 } else { 2 };
            if !out_value.is_null() {
                unsafe { *out_value = e.into_inner() };
            }
            code
        }
    }
}

/// Check if the receiver has been dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_is_closed(sender: orkester_sender_t) -> bool {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    sender.is_closed()
}

/// Drop a sender handle.
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

/// Drop a receiver handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_drop(receiver: orkester_receiver_t) {
    if !receiver.is_null() {
        unsafe { drop(Box::from_raw(receiver as *mut Receiver<*mut c_void>)) };
    }
}
