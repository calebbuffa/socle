//! Detached tasks and join sets.

use std::sync::Arc;

use crate::context::Context;
use crate::error::AsyncError;
use crate::future::Future;
use crate::state::SharedState;
use crate::system::AsyncSystem;

/// A collection of futures that can be joined together.
///
/// Push futures into the set, then call [`join_all`](JoinSet::join_all) to
/// wait for all of them to complete and collect results, or
/// [`join_next`](JoinSet::join_next) to pop results one at a time.
pub struct JoinSet<T: Send + 'static> {
    #[allow(dead_code)]
    system: AsyncSystem,
    entries: Vec<Arc<SharedState<T>>>,
}

impl<T: Send + 'static> JoinSet<T> {
    pub(crate) fn new(system: AsyncSystem) -> Self {
        Self {
            system,
            entries: Vec::new(),
        }
    }

    /// Add a future to the set.
    pub fn push(&mut self, mut future: Future<T>) {
        if let Some(state) = future.state.take() {
            self.entries.push(state);
        }
    }

    /// Number of futures in the set.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the set contains no futures.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Block until all futures complete. Returns results in insertion order.
    pub fn join_all(self) -> Vec<Result<T, AsyncError>> {
        self.entries
            .into_iter()
            .map(|state| {
                state.wait_until_ready();
                state
                    .take_result()
                    .unwrap_or_else(|| Err(AsyncError::msg("JoinSet entry consumed")))
            })
            .collect()
    }

    /// Block until the next future completes (in insertion order).
    /// Returns `None` when the set is exhausted.
    pub fn join_next(&mut self) -> Option<Result<T, AsyncError>> {
        if self.entries.is_empty() {
            return None;
        }
        let state = self.entries.remove(0);
        state.wait_until_ready();
        Some(
            state
                .take_result()
                .unwrap_or_else(|| Err(AsyncError::msg("JoinSet entry consumed"))),
        )
    }
}

impl AsyncSystem {
    /// Create a new empty [`JoinSet`].
    pub fn join_set<T: Send + 'static>(&self) -> JoinSet<T> {
        JoinSet::new(self.clone())
    }

    /// Spawn a detached task in the given scheduling context.
    ///
    /// The task runs to completion (or panic) with no way to observe its
    /// result. Use [`Context::Immediate`] to run inline on the current
    /// thread.
    pub fn spawn<F>(&self, context: Context, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        match self.inner.scheduler_for(context) {
            Some(scheduler) => scheduler.schedule(Box::new(f)),
            None => f(), // Immediate — run inline
        }
    }
}
