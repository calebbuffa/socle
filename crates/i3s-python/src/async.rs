//! Python bindings for i3s-async's `AsyncSystem`, `Future<T>`, and `Promise<T>`.
//!
//! Thin wrappers around the Rust types — all async machinery lives in the Rust crate.

use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use i3s_async::TaskProcessor;
use i3s_async::async_system::{AsyncSystem, Future as RustFuture};
use i3s_async::task_processor::ThreadPoolTaskProcessor;

/// Thread-pool task processor.
#[pyclass(name = "NativeTaskProcessor")]
pub struct PyNativeTaskProcessor {
    pub inner: Arc<dyn TaskProcessor>,
}

#[pymethods]
impl PyNativeTaskProcessor {
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

/// Async system: owns a worker thread pool and main-thread task queue.
#[pyclass(name = "AsyncSystem")]
pub struct PyAsyncSystem {
    pub inner: AsyncSystem,
}

#[pymethods]
impl PyAsyncSystem {
    #[new]
    fn new(task_processor: &PyNativeTaskProcessor) -> Self {
        Self {
            inner: AsyncSystem::new(task_processor.inner.clone()),
        }
    }

    /// Dispatch pending main-thread tasks. Call once per frame from the main thread.
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

/// Internal state for a Python future.
enum PyFutureState {
    /// Backed by a Rust Future<Py<PyAny>>.
    Pending(RustFuture<Py<PyAny>>),
    /// Already consumed.
    Consumed,
}

// SAFETY: PyFutureState is only accessed through Mutex (inside PyFuture).
// Py<PyAny> is Send when not attached to the GIL.
// RustFuture<Py<PyAny>> is Send+Sync by its own impl.
unsafe impl Send for PyFutureState {}
unsafe impl Sync for PyFutureState {}

/// A future that resolves to a Python object.
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
    /// Block until the result is available. Releases the GIL. Can only be called once.
    fn wait(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let rust_future = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| PyRuntimeError::new_err("future state lock poisoned"))?;
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
            Err(e) => Err(PyRuntimeError::new_err(e.to_string())),
        }
    }

    /// Block until resolved, pumping the main-thread task queue each iteration.
    fn wait_in_main_thread(
        &self,
        py: Python<'_>,
        async_system: &PyAsyncSystem,
    ) -> PyResult<Py<PyAny>> {
        let rust_future = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| PyRuntimeError::new_err("future state lock poisoned"))?;
            match std::mem::replace(&mut *state, PyFutureState::Consumed) {
                PyFutureState::Pending(f) => f,
                PyFutureState::Consumed => {
                    return Err(PyRuntimeError::new_err("Future already consumed"));
                }
            }
        };
        let sys = async_system.inner.clone();
        // Pump the main-thread queue while we wait. Release the GIL each
        // iteration so worker threads can complete their work.
        loop {
            if rust_future.is_ready() {
                break;
            }
            // Dispatch any queued main-thread callbacks (GIL held here)
            sys.dispatch_main_thread_tasks();
            if rust_future.is_ready() {
                break;
            }
            // Briefly release the GIL so workers can progress
            py.detach(|| std::thread::yield_now());
        }
        match rust_future.wait() {
            Ok(obj) => Ok(obj),
            Err(e) => Err(PyRuntimeError::new_err(e.to_string())),
        }
    }

    /// Check if the result is available without blocking.
    fn is_ready(&self) -> bool {
        let state = self.state.lock().expect("future state lock poisoned");
        match &*state {
            PyFutureState::Pending(f) => f.is_ready(),
            PyFutureState::Consumed => false,
        }
    }

    /// Enable ``await future`` in asyncio.
    fn __await__(slf: Py<Self>) -> PyFutureAwaitable {
        PyFutureAwaitable {
            future: slf,
            asyncio_future: None,
        }
    }

    fn __repr__(&self) -> String {
        let state = self.state.lock().expect("future state lock poisoned");
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

#[pyclass]
struct PyFutureAwaitable {
    future: Py<PyFuture>,
    /// asyncio.Future created on first poll; used to properly suspend the loop.
    asyncio_future: Option<Py<PyAny>>,
}

#[pymethods]
impl PyFutureAwaitable {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Option<Py<PyAny>>> {
        let future_ref = self.future.bind(py);
        let f: &PyFuture = &future_ref.borrow();

        let mut state = f
            .state
            .lock()
            .map_err(|_| PyRuntimeError::new_err("future state lock poisoned"))?;
        match &*state {
            PyFutureState::Pending(rust_future) => {
                if rust_future.is_ready() {
                    // Ready — consume and raise StopIteration with the value
                    match std::mem::replace(&mut *state, PyFutureState::Consumed) {
                        PyFutureState::Pending(fut) => {
                            drop(state);
                            match fut.wait() {
                                Ok(obj) => Err(pyo3::exceptions::PyStopIteration::new_err(obj)),
                                Err(e) => Err(PyRuntimeError::new_err(e.to_string())),
                            }
                        }
                        _ => unreachable!(),
                    }
                } else {
                    // Not ready — yield an asyncio.Future so the event loop
                    // truly suspends (not a busy-poll).
                    drop(state);
                    let asyncio_fut = match self.asyncio_future.take() {
                        Some(f) => f,
                        None => {
                            let asyncio = py.import("asyncio")?;
                            let loop_ = asyncio.call_method0("get_event_loop")?;
                            let fut = loop_.call_method0("create_future")?;
                            fut.unbind()
                        }
                    };
                    // Re-store for next poll
                    self.asyncio_future = Some(asyncio_fut.clone_ref(py));
                    Ok(Some(asyncio_fut))
                }
            }
            PyFutureState::Consumed => Err(PyRuntimeError::new_err("Future already consumed")),
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyNativeTaskProcessor>()?;
    m.add_class::<PyAsyncSystem>()?;
    m.add_class::<PyFuture>()?;
    Ok(())
}
