//! C-ABI FFI for orkester scheduling primitives.
//!
//! Task-handle–centric API. Values cross the FFI boundary as `*mut c_void`
//! with an associated destructor. Rust owns all scheduling and lifecycle.

#![allow(non_camel_case_types)]

use orkester::channel::{self, Receiver, Sender};
use orkester::{
    CancellationToken, Context, Executor, JoinSet, MainThreadScope, Scheduler, Semaphore, Task,
    ThreadPool,
};
use std::ffi::{c_char, c_void};

// ─── Opaque handle types ────────────────────────────────────────────────────

/// Opaque async runtime handle. Wraps `orkester::Scheduler`.
#[repr(C)]
pub struct orkester_t {
    _opaque: u8,
}

/// Opaque handle to a `Task<Payload>`.
pub type orkester_task_t = *mut c_void;

/// Opaque handle to a `Resolver<Payload>`.
pub type orkester_resolver_t = *mut c_void;

/// Opaque handle to a `MainThreadScope`.
pub type orkester_main_scope_t = *mut c_void;

/// Opaque handle to a `ThreadPool`.
pub type orkester_thread_pool_t = *mut c_void;

/// Callback for fire-and-forget operations — `void(*)(void*)`.
pub type orkester_callback_fn_t = unsafe extern "C" fn(*mut c_void);

/// Value destructor callback — `void(*)(void*)`.
pub type orkester_destroy_fn_t = unsafe extern "C" fn(*mut c_void);

/// Async-transform callback for continuations.
/// Receives `(ctx, input_value)`, returns an `orkester_task_t`.
/// The callback takes ownership of `input_value`.
/// Returning null is treated as resolving with an empty payload.
pub type orkester_then_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, value: *mut c_void) -> orkester_task_t;

/// Error-catch callback that receives an opaque error pointer
/// and returns an `orkester_task_t`.
/// The callback does NOT take ownership of `error`.
pub type orkester_catch_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, error: *mut c_void) -> orkester_task_t;

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
    ORKESTER_SEND_OK = 0,
    ORKESTER_SEND_FULL = 1,
    ORKESTER_SEND_CLOSED = 2,
}

/// Dispatch function type for scheduling work on a background thread.
pub type orkester_dispatch_fn_t =
    unsafe extern "C" fn(ctx: *mut c_void, work: orkester_callback_fn_t, work_data: *mut c_void);

/// Returned by `orkester_resolver_create`.
#[repr(C)]
pub struct orkester_resolver_pair_t {
    pub resolver: orkester_resolver_t,
    pub task: orkester_task_t,
}

// ─── Internal helpers ───────────────────────────────────────────────────────

struct FfiExecutor {
    dispatch: orkester_dispatch_fn_t,
    ctx: SendCtx,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Send for FfiExecutor {}
unsafe impl Sync for FfiExecutor {}

impl Executor for FfiExecutor {
    fn execute(&self, task: Box<dyn FnOnce() + Send + 'static>) {
        let work_data = Box::into_raw(Box::new(task)) as *mut c_void;

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

#[derive(Clone, Copy)]
struct SendCtx(*mut c_void);
unsafe impl Send for SendCtx {}
unsafe impl Sync for SendCtx {}

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

/// Wrapper for async-transform callbacks to satisfy `Send`.
struct SendThenAsync {
    func: orkester_then_fn_t,
    ctx: SendCtx,
}
unsafe impl Send for SendThenAsync {}

/// Wrapper for catch callbacks to satisfy `Send`.
struct SendCatchAsync {
    func: orkester_catch_fn_t,
    ctx: SendCtx,
}
unsafe impl Send for SendCatchAsync {}

/// An opaque error object stored inside `AsyncError`.
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

/// A value payload carried by tasks across the FFI boundary.
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

    fn new(value: *mut c_void, destroy_fn: Option<unsafe extern "C" fn(*mut c_void)>) -> Self {
        Payload {
            value,
            clone_fn: None,
            destroy_fn,
        }
    }

    fn with_clone(
        mut self,
        clone_fn: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    ) -> Self {
        self.clone_fn = clone_fn;
        self
    }

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

/// A collected array of payload values returned by `orkester_join_all_values`.
struct ValueArray {
    payloads: Vec<Payload>,
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

unsafe fn write_ffi_error_with_opaque(
    error: orkester::AsyncError,
    out_value: *mut *mut c_void,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) {
    // Check if this is an opaque error — pass the pointer back
    if let Some(opaque) = error.downcast_ref::<OpaqueError>() {
        if !out_value.is_null() {
            unsafe { *out_value = opaque.ptr };
        }
        // Don't write string error when we have opaque
        return;
    }
    if !out_value.is_null() {
        unsafe { *out_value = std::ptr::null_mut() };
    }
    unsafe { write_ffi_error(error, out_error_ptr, out_error_len) };
}

// ─── Scheduler (orkester_t) ─────────────────────────────────────────────────

/// Create a `Scheduler` from a dispatch function.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_create(
    dispatch: orkester_dispatch_fn_t,
    ctx: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) -> *mut orkester_t {
    let executor = FfiExecutor {
        dispatch,
        ctx: SendCtx(ctx),
        destroy,
    };
    let system = Scheduler::new(executor);
    Box::into_raw(Box::new(system)) as *mut orkester_t
}

/// Create a `Scheduler` with a built-in thread pool.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_create_default(num_threads: usize) -> *mut orkester_t {
    let system = Scheduler::with_threads(num_threads);
    Box::into_raw(Box::new(system)) as *mut orkester_t
}

/// Destroy a `Scheduler`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_destroy(ptr: *mut orkester_t) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr as *mut Scheduler)) };
    }
}

/// Clone a `Scheduler` (cheap Arc clone).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_clone(ptr: *const orkester_t) -> *mut orkester_t {
    let system = unsafe { &*(ptr as *const Scheduler) };
    Box::into_raw(Box::new(system.clone())) as *mut orkester_t
}

// ─── Resolver ───────────────────────────────────────────────────────────────

/// Create a resolver/task pair.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolver_create(
    system: *const orkester_t,
) -> orkester_resolver_pair_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let (resolver, task) = system.resolver::<Payload>();
    orkester_resolver_pair_t {
        resolver: Box::into_raw(Box::new(resolver)) as orkester_resolver_t,
        task: Box::into_raw(Box::new(task)) as orkester_task_t,
    }
}

/// Resolve with a value payload. Consumes the resolver handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolver_resolve_with(
    resolver: orkester_resolver_t,
    value: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) {
    if !resolver.is_null() {
        let resolver = unsafe { Box::from_raw(resolver as *mut orkester::Resolver<Payload>) };
        resolver.resolve(Payload::new(value, destroy));
    }
}

/// Resolve with no payload (void). Consumes the resolver handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolver_resolve_empty(resolver: orkester_resolver_t) {
    if !resolver.is_null() {
        let resolver = unsafe { Box::from_raw(resolver as *mut orkester::Resolver<Payload>) };
        resolver.resolve(Payload::empty());
    }
}

/// Reject with an opaque error object. Consumes the resolver handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolver_reject_opaque(
    resolver: orkester_resolver_t,
    error: *mut c_void,
    destroy: Option<unsafe extern "C" fn(*mut c_void)>,
) {
    if resolver.is_null() {
        return;
    }
    let resolver = unsafe { Box::from_raw(resolver as *mut orkester::Resolver<Payload>) };
    resolver.reject(orkester::AsyncError::new(OpaqueError {
        ptr: error,
        destroy,
    }));
}

/// Reject with a string message. Consumes the resolver handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolver_reject(
    resolver: orkester_resolver_t,
    message: *const c_char,
    message_len: usize,
) {
    if resolver.is_null() {
        return;
    }
    let resolver = unsafe { Box::from_raw(resolver as *mut orkester::Resolver<Payload>) };
    let msg = if message.is_null() {
        "FFI resolver rejected".to_string()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(message as *const u8, message_len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    resolver.reject(orkester::AsyncError::msg(msg));
}

/// Drop a resolver without resolving (auto-rejects the paired task).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolver_drop(resolver: orkester_resolver_t) {
    if !resolver.is_null() {
        unsafe { drop(Box::from_raw(resolver as *mut orkester::Resolver<Payload>)) };
    }
}

// ─── Task ───────────────────────────────────────────────────────────────────

/// Check if a task is ready.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_is_ready(task: orkester_task_t) -> bool {
    if task.is_null() {
        return false;
    }
    let task = unsafe { &*(task as *const Task<Payload>) };
    task.is_ready()
}

/// Block until a task completes. Consumes the handle.
/// On success: `out_value` receives the payload value, returns true.
/// On failure: `out_value` receives opaque error ptr (if any),
///             `out_error_ptr`/`out_error_len` receive string error, returns false.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_wait(
    task: orkester_task_t,
    out_value: *mut *mut c_void,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    if task.is_null() {
        return false;
    }
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    match task.block() {
        Ok(mut payload) => {
            if !out_value.is_null() {
                unsafe { *out_value = payload.take_value() };
            }
            true
        }
        Err(e) => {
            unsafe { write_ffi_error_with_opaque(e, out_value, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Block dispatching main-thread tasks while waiting. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_wait_main(
    task: orkester_task_t,
    out_value: *mut *mut c_void,
    out_error_ptr: *mut *const c_char,
    out_error_len: *mut usize,
) -> bool {
    if task.is_null() {
        return false;
    }
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    match task.block_with_main() {
        Ok(mut payload) => {
            if !out_value.is_null() {
                unsafe { *out_value = payload.take_value() };
            }
            true
        }
        Err(e) => {
            unsafe { write_ffi_error_with_opaque(e, out_value, out_error_ptr, out_error_len) };
            false
        }
    }
}

/// Attach an async-transform continuation. The callback receives the input
/// value and returns an `orkester_task_t`. Consumes the input task handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_chain(
    task: orkester_task_t,
    context: orkester_context_t,
    transform: orkester_then_fn_t,
    ctx: *mut c_void,
) -> orkester_task_t {
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    let system = task.system();
    let tx = SendThenAsync {
        func: transform,
        ctx: SendCtx(ctx),
    };
    let next = task.then::<Payload, _, Task<Payload>>(context.to_context(), move |mut payload| {
        let tx = &tx;
        let input_value = payload.take_value();
        let inner_handle = unsafe { (tx.func)(tx.ctx.0, input_value) };
        if inner_handle.is_null() {
            return system.resolved(Payload::empty());
        }
        unsafe { *Box::from_raw(inner_handle as *mut Task<Payload>) }
    });
    Box::into_raw(Box::new(next)) as orkester_task_t
}

/// Attach an async-transform continuation in a thread pool.
/// Consumes the input task handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_chain_in_pool(
    task: orkester_task_t,
    pool: orkester_thread_pool_t,
    transform: orkester_then_fn_t,
    ctx: *mut c_void,
) -> orkester_task_t {
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    let system = task.system();
    let pool = unsafe { &*(pool as *const ThreadPool) };
    let tx = SendThenAsync {
        func: transform,
        ctx: SendCtx(ctx),
    };
    let next = task.then_in_pool::<Payload, _, Task<Payload>>(pool, move |mut payload| {
        let tx = &tx;
        let input_value = payload.take_value();
        let inner_handle = unsafe { (tx.func)(tx.ctx.0, input_value) };
        if inner_handle.is_null() {
            return system.resolved(Payload::empty());
        }
        unsafe { *Box::from_raw(inner_handle as *mut Task<Payload>) }
    });
    Box::into_raw(Box::new(next)) as orkester_task_t
}

/// Attach an error-catch handler. The callback receives an opaque error
/// pointer and returns an `orkester_task_t`. Consumes the input task handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_catch(
    task: orkester_task_t,
    context: orkester_context_t,
    callback: orkester_catch_fn_t,
    ctx: *mut c_void,
) -> orkester_task_t {
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    let cb = SendCatchAsync {
        func: callback,
        ctx: SendCtx(ctx),
    };
    let next = task.catch::<_, Task<Payload>>(context.to_context(), move |error| {
        let cb = &cb;
        let opaque_ptr = error
            .downcast_ref::<OpaqueError>()
            .map(|e| e.ptr)
            .unwrap_or(std::ptr::null_mut());
        let inner_handle = unsafe { (cb.func)(cb.ctx.0, opaque_ptr) };
        if inner_handle.is_null() {
            // Treat null return as re-raising the error — but we can't get
            // it back since catch consumed it. Return empty resolved.
            // The C++ side always returns a valid task from catch.
            panic!("catch callback returned null");
        }
        unsafe { *Box::from_raw(inner_handle as *mut Task<Payload>) }
    });
    Box::into_raw(Box::new(next)) as orkester_task_t
}

/// Convert a task to a shared task. Consumes the task handle.
/// `clone_fn` is used to clone the payload for shared access.
/// Pass null if sharing is only used for fan-out without accessing the value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_share(
    task: orkester_task_t,
    clone_fn: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
) -> orkester_task_t {
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    // Attach clone_fn to the payload before sharing
    let task = if clone_fn.is_some() {
        task.map(move |payload| payload.with_clone(clone_fn))
    } else {
        task
    };
    let shared = task.share();
    Box::into_raw(Box::new(shared)) as orkester_task_t
}

/// Clone a shared task handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_clone(task: orkester_task_t) -> orkester_task_t {
    let shared = unsafe { &*(task as *const orkester::SharedTask<Payload>) };
    // Create a new unique task from the shared task via then(IMMEDIATE)
    let unique = shared.map(|p| p);
    Box::into_raw(Box::new(unique)) as orkester_task_t
}

/// Drop a task handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_drop(task: orkester_task_t) {
    if !task.is_null() {
        unsafe { drop(Box::from_raw(task as *mut Task<Payload>)) };
    }
}

// ─── Resolved / Convenience ────────────────────────────────────────────────

/// Create an already-resolved task with no payload.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolved(system: *const orkester_t) -> orkester_task_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let task = system.resolved(Payload::empty());
    Box::into_raw(Box::new(task)) as orkester_task_t
}

/// Create an already-resolved task carrying a value.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_resolved_with_destroy(
    system: *const orkester_t,
    value: *mut c_void,
    destroy_fn: Option<unsafe extern "C" fn(*mut c_void)>,
) -> orkester_task_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let task = system.resolved(Payload::new(value, destroy_fn));
    Box::into_raw(Box::new(task)) as orkester_task_t
}

// ─── Join All ──────────────────────────────────────────────────────────────

/// Wait for all tasks to complete (void — payloads dropped).
/// Consumes all input handles. Returns a single task.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_all(
    system: *const orkester_t,
    tasks: *mut orkester_task_t,
    count: usize,
) -> orkester_task_t {
    let system_ref = unsafe { &*(system as *const Scheduler) };

    if count == 0 {
        let resolved = system_ref.resolved(Payload::empty());
        return Box::into_raw(Box::new(resolved)) as orkester_task_t;
    }

    let mut task_vec: Vec<Task<Payload>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *tasks.add(i) };
        task_vec.push(unsafe { *Box::from_raw(handle as *mut Task<Payload>) });
    }

    let combined = system_ref.join_all(task_vec);
    let signal = combined.map(|_| Payload::empty());
    Box::into_raw(Box::new(signal)) as orkester_task_t
}

/// Wait for all tasks to complete, preserving per-element payloads.
/// Consumes all input handles. Returns a task whose payload is a `ValueArray`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_all_values(
    system: *const orkester_t,
    tasks: *mut orkester_task_t,
    count: usize,
) -> orkester_task_t {
    let system_ref = unsafe { &*(system as *const Scheduler) };

    if count == 0 {
        let arr = ValueArray {
            payloads: Vec::new(),
        };
        let resolved = system_ref.resolved(Payload::new(
            Box::into_raw(Box::new(arr)) as *mut c_void,
            Some(value_array_destroy),
        ));
        return Box::into_raw(Box::new(resolved)) as orkester_task_t;
    }

    let mut task_vec: Vec<Task<Payload>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *tasks.add(i) };
        task_vec.push(unsafe { *Box::from_raw(handle as *mut Task<Payload>) });
    }

    let combined = system_ref.join_all(task_vec);
    let result = combined.map(|payloads| {
        let arr = ValueArray { payloads };
        Payload::new(
            Box::into_raw(Box::new(arr)) as *mut c_void,
            Some(value_array_destroy),
        )
    });
    Box::into_raw(Box::new(result)) as orkester_task_t
}

unsafe extern "C" fn value_array_destroy(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr as *mut ValueArray)) };
    }
}

/// Get the number of elements in a value array.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_value_array_len(array_payload: *const c_void) -> usize {
    if array_payload.is_null() {
        return 0;
    }
    let arr = unsafe { &*(array_payload as *const ValueArray) };
    arr.payloads.len()
}

/// Extract the value pointer at `index` from a value array.
/// Transfers ownership to the caller. Each index should only be extracted once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_value_array_get(
    array_payload: *mut c_void,
    index: usize,
) -> *mut c_void {
    if array_payload.is_null() {
        return std::ptr::null_mut();
    }
    let arr = unsafe { &mut *(array_payload as *mut ValueArray) };
    if index >= arr.payloads.len() {
        return std::ptr::null_mut();
    }
    arr.payloads[index].take_value()
}

/// Destroy a value array and all remaining un-extracted values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_value_array_drop(array_payload: *mut c_void) {
    if !array_payload.is_null() {
        unsafe { drop(Box::from_raw(array_payload as *mut ValueArray)) };
    }
}

// ─── Dispatch ──────────────────────────────────────────────────────────────

/// Dispatch all queued main-thread tasks. Returns how many were dispatched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_dispatch(system: *const orkester_t) -> usize {
    let system = unsafe { &*(system as *const Scheduler) };
    system.flush_main()
}

/// Dispatch a single main-thread task. Returns true if one was dispatched.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_dispatch_one(system: *const orkester_t) -> bool {
    let system = unsafe { &*(system as *const Scheduler) };
    system.flush_main_one()
}

// ─── Main Thread Scope ─────────────────────────────────────────────────────

/// Enter main-thread scope.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_main_scope_create(
    system: *const orkester_t,
) -> orkester_main_scope_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let scope = system.main_scope();
    Box::into_raw(Box::new(scope)) as orkester_main_scope_t
}

/// Leave main-thread scope. Consumes the handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_main_scope_drop(scope: orkester_main_scope_t) {
    if !scope.is_null() {
        unsafe { drop(Box::from_raw(scope as *mut MainThreadScope)) };
    }
}

// ─── String ────────────────────────────────────────────────────────────────

/// Free a string previously returned by orkester FFI error functions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_string_drop(ptr: *const c_char, len: usize) {
    if !ptr.is_null() {
        let _ = unsafe {
            Box::from_raw(std::slice::from_raw_parts_mut(ptr as *mut u8, len) as *mut [u8])
        };
    }
}

// ─── Thread Pool ───────────────────────────────────────────────────────────

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

// ─── Cancellation ──────────────────────────────────────────────────────────

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

/// Attach a cancellation token to a task. Consumes the task handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_task_cancel(
    task: orkester_task_t,
    token: orkester_cancel_token_t,
) -> orkester_task_t {
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    let token = unsafe { &*(token as *const CancellationToken) };
    let result = task.with_cancellation(token);
    Box::into_raw(Box::new(result)) as orkester_task_t
}

// ─── Delay / Timeout / Race ────────────────────────────────────────────────

/// Create a task that completes after `millis` milliseconds.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_delay(system: *const orkester_t, millis: u64) -> orkester_task_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let task = system
        .delay(std::time::Duration::from_millis(millis))
        .map(|()| Payload::empty());
    Box::into_raw(Box::new(task)) as orkester_task_t
}

/// Wrap a task with a timeout. Consumes the input task handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_timeout(
    system: *const orkester_t,
    task: orkester_task_t,
    millis: u64,
) -> orkester_task_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    let result = orkester::timeout(system, task, std::time::Duration::from_millis(millis));
    Box::into_raw(Box::new(result)) as orkester_task_t
}

/// Race multiple tasks. Consumes all input handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_race(
    system: *const orkester_t,
    tasks: *mut orkester_task_t,
    count: usize,
) -> orkester_task_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let mut task_vec: Vec<Task<Payload>> = Vec::with_capacity(count);
    for i in 0..count {
        let handle = unsafe { *tasks.add(i) };
        if !handle.is_null() {
            task_vec.push(unsafe { *Box::from_raw(handle as *mut Task<Payload>) });
        }
    }
    let result = orkester::race(system, task_vec);
    Box::into_raw(Box::new(result)) as orkester_task_t
}

// ─── Spawn ─────────────────────────────────────────────────────────────────

/// Spawn a detached task. Fire-and-forget.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_spawn(
    system: *const orkester_t,
    context: orkester_context_t,
    callback: orkester_callback_fn_t,
    ctx: *mut c_void,
) {
    let system = unsafe { &*(system as *const Scheduler) };
    let cb = SendCallback {
        func: callback,
        context: SendCtx(ctx),
    };
    system.spawn_detached(context.to_context(), move || {
        unsafe { cb.call() };
    });
}

// ─── Semaphore ─────────────────────────────────────────────────────────────

pub type orkester_semaphore_t = *mut c_void;
pub type orkester_semaphore_permit_t = *mut c_void;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_create(
    system: *const orkester_t,
    permits: usize,
) -> orkester_semaphore_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let sem = Semaphore::new(system, permits.max(1));
    Box::into_raw(Box::new(sem)) as orkester_semaphore_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_semaphore_acquire(
    sem: orkester_semaphore_t,
) -> orkester_semaphore_permit_t {
    let sem = unsafe { &*(sem as *const Semaphore) };
    let permit = sem.acquire();
    Box::into_raw(Box::new(permit)) as orkester_semaphore_permit_t
}

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

// ─── Retry ─────────────────────────────────────────────────────────────────

pub type orkester_retry_fn_t = unsafe extern "C" fn(ctx: *mut c_void) -> orkester_task_t;

struct SendRetry {
    func: orkester_retry_fn_t,
    context: SendCtx,
}
unsafe impl Send for SendRetry {}
unsafe impl Sync for SendRetry {}

/// Retry an operation up to `max_attempts` times with exponential back-off.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_retry(
    system: *const orkester_t,
    max_attempts: u32,
    factory: orkester_retry_fn_t,
    ctx: *mut c_void,
) -> orkester_task_t {
    let system_ref = unsafe { &*(system as *const Scheduler) };
    let cb = SendRetry {
        func: factory,
        context: SendCtx(ctx),
    };
    let system_clone = system_ref.clone();
    let result = orkester::retry(system_ref, max_attempts, Default::default(), move || {
        let cb = &cb;
        let handle = unsafe { (cb.func)(cb.context.0) };
        if handle.is_null() {
            return system_clone.resolved(Err(orkester::AsyncError::msg(
                "retry factory returned null",
            )));
        }
        let task = unsafe { *Box::from_raw(handle as *mut Task<Payload>) };
        task.map(|payload| Ok(payload))
    });
    Box::into_raw(Box::new(result)) as orkester_task_t
}

// ─── JoinSet ───────────────────────────────────────────────────────────────

pub type orkester_join_set_t = *mut c_void;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_create(
    system: *const orkester_t,
) -> orkester_join_set_t {
    let system = unsafe { &*(system as *const Scheduler) };
    let js: JoinSet<Payload> = system.join_set();
    Box::into_raw(Box::new(js)) as orkester_join_set_t
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_push(js: orkester_join_set_t, task: orkester_task_t) {
    let js = unsafe { &mut *(js as *mut JoinSet<Payload>) };
    let task = unsafe { *Box::from_raw(task as *mut Task<Payload>) };
    js.push(task);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_len(js: orkester_join_set_t) -> usize {
    let js = unsafe { &*(js as *const JoinSet<Payload>) };
    js.len()
}

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_join_set_drop(js: orkester_join_set_t) {
    if !js.is_null() {
        unsafe { drop(Box::from_raw(js as *mut JoinSet<Payload>)) };
    }
}

// ─── Channel ───────────────────────────────────────────────────────────────

pub type orkester_sender_t = *mut c_void;
pub type orkester_receiver_t = *mut c_void;

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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_send(
    sender: orkester_sender_t,
    value: *mut c_void,
    out_value: *mut *mut c_void,
) -> orkester_send_result_t {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    match sender.send(value) {
        Ok(()) => orkester_send_result_t::ORKESTER_SEND_OK,
        Err(e) => {
            if !out_value.is_null() {
                unsafe { *out_value = e.0 };
            }
            orkester_send_result_t::ORKESTER_SEND_CLOSED
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_try_send(
    sender: orkester_sender_t,
    value: *mut c_void,
    out_value: *mut *mut c_void,
) -> orkester_send_result_t {
    let sender = unsafe { &*(sender as *const Sender<*mut c_void>) };
    match sender.try_send(value) {
        Ok(()) => orkester_send_result_t::ORKESTER_SEND_OK,
        Err(orkester::TrySendError::Full(v)) => {
            if !out_value.is_null() {
                unsafe { *out_value = v };
            }
            orkester_send_result_t::ORKESTER_SEND_FULL
        }
        Err(orkester::TrySendError::Closed(v)) => {
            if !out_value.is_null() {
                unsafe { *out_value = v };
            }
            orkester_send_result_t::ORKESTER_SEND_CLOSED
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_sender_drop(sender: orkester_sender_t) {
    if !sender.is_null() {
        unsafe { drop(Box::from_raw(sender as *mut Sender<*mut c_void>)) };
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_recv(
    receiver: orkester_receiver_t,
    out_value: *mut *mut c_void,
) -> bool {
    let receiver = unsafe { &*(receiver as *const Receiver<*mut c_void>) };
    match receiver.recv() {
        Some(value) => {
            if !out_value.is_null() {
                unsafe { *out_value = value };
            }
            true
        }
        None => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn orkester_receiver_drop(receiver: orkester_receiver_t) {
    if !receiver.is_null() {
        unsafe { drop(Box::from_raw(receiver as *mut Receiver<*mut c_void>)) };
    }
}
