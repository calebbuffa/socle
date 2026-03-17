//! Python bindings for i3s-async's `AsyncSystem`, `Future<T>`, and `Promise<T>`.
//!
//! Mirrors cesium-native's `cesium.async_` module. These are thin wrappers
//! around the Rust types — all async machinery lives in the Rust crate.

use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use i3s_async::async_system::{
    AsyncSystem, Future as RustFuture,
};
use i3s_async::task_processor::ThreadPoolTaskProcessor;
use i3s_async::TaskProcessor;

// ============================================================================
// NativeTaskProcessor — default thread pool
// ============================================================================

/// A native thread-pool task processor.
///
/// Mirrors cesium-native's ``NativeTaskProcessor``.
#[pyclass(name = "NativeTaskProcessor")]
pub struct PyNativeTaskProcessor {
    pub inner: Arc<dyn TaskProcessor>,
}

#[pymethods]
impl PyNativeTaskProcessor {
    /// Create a thread pool with the given number of worker threads.
    ///
    /// If ``num_threads`` is 0, uses ``available_parallelism() - 1``.
    #[new]
    #[pyo3(signature = (num_threads = 0))]
    fn new(num_threads: usize) -> Self {
        let pool: Arc<dyn TaskProcessor> = if num_threads == 0 {
            Arc::new(ThreadPoolTaskProcessor::default_pool())
        } else {
            Arc::new(ThreadPoolTaskProcessor::new(num_threads))
        };
        Self { inner: pool }
    }

    fn __repr__(&self) -> String {
        "NativeTaskProcessor(...)".to_string()
    }
}

// ============================================================================
// PyAsyncSystem — wraps i3s_async::AsyncSystem
// ============================================================================

/// The async system: owns a worker thread pool and main-thread task queue.
///
/// Mirrors cesium-native's ``AsyncSystem``. Create one per application
/// and share it with all scene layers.
///
/// Example::
///
///     tp = NativeTaskProcessor(4)
///     async_system = AsyncSystem(tp)
#[pyclass(name = "AsyncSystem")]
pub struct PyAsyncSystem {
    pub inner: AsyncSystem,
}

#[pymethods]
impl PyAsyncSystem {
    /// Create an async system backed by the given task processor.
    ///
    /// Parameters
    /// ----------
    /// task_processor : NativeTaskProcessor
    ///     The thread pool that executes worker tasks.
    #[new]
    fn new(task_processor: &PyNativeTaskProcessor) -> Self {
        Self {
            inner: AsyncSystem::new(task_processor.inner.clone()),
        }
    }

    /// Dispatch all pending main-thread tasks.
    ///
    /// Call this once per frame from the main thread to process callbacks
    /// queued by worker threads. Returns the number of tasks dispatched.
    fn dispatch_main_thread_tasks(&self) -> usize {
        self.inner.dispatch_main_thread_tasks()
    }

    /// Whether there are pending main-thread tasks.
    fn has_pending_main_thread_tasks(&self) -> bool {
        self.inner.has_pending_main_thread_tasks()
    }

    fn __repr__(&self) -> String {
        "AsyncSystem(...)".to_string()
    }
}

// ============================================================================
// PyFuture — wraps i3s_async::Future<Py<PyAny>>
// ============================================================================

/// Internal state for a Python future.
enum PyFutureState {
    /// Backed by a Rust Future<Py<PyAny>>.
    Pending(RustFuture<Py<PyAny>>),
    /// Already consumed.
    Consumed,
}

// Safety: PyFutureState is only accessed through Mutex (inside PyFuture).
// Py<PyAny> is Send when not attached to the GIL.
// RustFuture<Py<PyAny>> is Send+Sync by its own impl.
unsafe impl Send for PyFutureState {}
unsafe impl Sync for PyFutureState {}

/// A future that resolves to a Python object.
///
/// Wraps ``i3s_async::Future<T>``. Created by async operations like
/// ``SceneLayer.open_async()``.
///
/// Mirrors cesium-native's ``Future[T]``.
///
/// Supports:
/// - ``future.wait()`` — blocking wait (releases GIL)
/// - ``future.is_ready()`` — non-blocking poll
/// - ``await future`` — asyncio integration
#[pyclass(name = "Future")]
pub struct PyFuture {
    state: std::sync::Mutex<PyFutureState>,
}

impl PyFuture {
    /// Create a PyFuture wrapping a Rust Future<Py<PyAny>>.
    pub fn from_rust(future: RustFuture<Py<PyAny>>) -> Self {
        Self {
            state: std::sync::Mutex::new(PyFutureState::Pending(future)),
        }
    }
}

#[pymethods]
impl PyFuture {
    /// Block until the result is available and return it.
    ///
    /// Releases the GIL while waiting so worker threads can make progress.
    /// Can only be called once — subsequent calls raise RuntimeError.
    fn wait(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let rust_future = {
            let mut state = self.state.lock().unwrap();
            match std::mem::replace(&mut *state, PyFutureState::Consumed) {
                PyFutureState::Pending(f) => f,
                PyFutureState::Consumed => {
                    return Err(PyRuntimeError::new_err("Future already consumed"));
                }
            }
        };
        // Release the GIL while blocking on the Rust future
        let result = py.detach(|| rust_future.wait());
        match result {
            Ok(obj) => Ok(obj),
            Err(e) => Err(PyRuntimeError::new_err(e)),
        }
    }

    /// Block in the main thread — dispatches main-thread tasks while waiting.
    ///
    /// Use this when the future's resolution depends on main-thread callbacks.
    fn wait_in_main_thread(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        // For now, same as wait(). When we have proper main-thread dispatch
        // integration, this will pump the main-thread queue while waiting.
        self.wait(py)
    }

    /// Check if the result is available without blocking.
    fn is_ready(&self) -> bool {
        let state = self.state.lock().unwrap();
        match &*state {
            PyFutureState::Pending(f) => f.is_ready(),
            PyFutureState::Consumed => false,
        }
    }

    /// Enable ``await future`` in asyncio.
    fn __await__(slf: Py<Self>) -> PyFutureAwaitable {
        PyFutureAwaitable { future: slf }
    }

    fn __repr__(&self) -> String {
        let state = self.state.lock().unwrap();
        match &*state {
            PyFutureState::Pending(f) => {
                if f.is_ready() {
                    "Future(ready)".to_string()
                } else {
                    "Future(pending)".to_string()
                }
            }
            PyFutureState::Consumed => "Future(consumed)".to_string(),
        }
    }
}

// ============================================================================
// PyFutureAwaitable — makes Future work with `await`
// ============================================================================

/// Iterator wrapper for Python's ``await`` protocol.
#[pyclass]
struct PyFutureAwaitable {
    future: Py<PyFuture>,
}

#[pymethods]
impl PyFutureAwaitable {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let future_ref = self.future.bind(py);
        let f: &PyFuture = &future_ref.borrow();

        let mut state = f.state.lock().unwrap();
        match &*state {
            PyFutureState::Pending(rust_future) => {
                if rust_future.is_ready() {
                    // Ready — consume and raise StopIteration with the value
                    match std::mem::replace(&mut *state, PyFutureState::Consumed) {
                        PyFutureState::Pending(fut) => {
                            drop(state);
                            match fut.wait() {
                                Ok(obj) => {
                                    Err(pyo3::exceptions::PyStopIteration::new_err(obj))
                                }
                                Err(e) => Err(PyRuntimeError::new_err(e)),
                            }
                        }
                        _ => unreachable!(),
                    }
                } else {
                    // Not ready — yield None to asyncio event loop
                    Ok(Some(py.None().into()))
                }
            }
            PyFutureState::Consumed => {
                Err(PyRuntimeError::new_err("Future already consumed"))
            }
        }
    }
}

// ============================================================================
// Module registration
// ============================================================================

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyNativeTaskProcessor>()?;
    m.add_class::<PyAsyncSystem>()?;
    m.add_class::<PyFuture>()?;
    Ok(())
}
