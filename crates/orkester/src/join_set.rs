//! Detached tasks and join sets.

use std::sync::Arc;

use crate::context::Context;
use crate::error::AsyncError;
use crate::scheduler::Scheduler;
use crate::task::{Task, TaskInner};
use crate::task_cell::TaskCell;

/// A collection of tasks that can be joined together.
///
/// Push tasks into the set, then call [`join_all`](JoinSet::join_all) to
/// wait for all of them to complete and collect results, or
/// [`join_next`](JoinSet::join_next) to pop results one at a time.
pub struct JoinSet<T: Send + 'static> {
    #[allow(dead_code)]
    system: Scheduler,
    entries: Vec<Arc<TaskCell<T>>>,
}

impl<T: Send + 'static> JoinSet<T> {
    pub(crate) fn new(system: Scheduler) -> Self {
        Self {
            system,
            entries: Vec::new(),
        }
    }

    /// Add a task to the set.
    pub fn push(&mut self, task: Task<T>) {
        match task.inner {
            TaskInner::Pending(cell) => {
                self.entries.push(cell);
            }
            TaskInner::Ready(result) => {
                let cell = Arc::new(TaskCell::new());
                match result {
                    Some(Ok(v)) => cell.complete(Ok(v)),
                    Some(Err(e)) => cell.complete(Err(e)),
                    None => cell.complete(Err(AsyncError::msg("Task already consumed"))),
                }
                self.entries.push(cell);
            }
        }
    }

    /// Number of tasks in the set.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the set contains no tasks.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Block until all tasks complete. Returns results in insertion order.
    pub fn join_all(self) -> Vec<Result<T, AsyncError>> {
        self.entries
            .into_iter()
            .map(|cell| {
                cell.wait_until_ready();
                cell.take_result()
                    .unwrap_or_else(|| Err(AsyncError::msg("JoinSet entry consumed")))
            })
            .collect()
    }

    /// Block until the next task completes (in insertion order).
    /// Returns `None` when the set is exhausted.
    pub fn join_next(&mut self) -> Option<Result<T, AsyncError>> {
        if self.entries.is_empty() {
            return None;
        }
        let cell = self.entries.remove(0);
        cell.wait_until_ready();
        Some(
            cell.take_result()
                .unwrap_or_else(|| Err(AsyncError::msg("JoinSet entry consumed"))),
        )
    }
}

impl Scheduler {
    /// Create a new empty [`JoinSet`].
    pub fn join_set<T: Send + 'static>(&self) -> JoinSet<T> {
        JoinSet::new(self.clone())
    }

    /// Spawn a detached task in the given scheduling context.
    ///
    /// The task runs to completion (or panic) with no way to observe its
    /// result. Use [`Context::IMMEDIATE`] to run inline on the current
    /// thread.
    pub fn spawn_detached<F>(&self, context: Context, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        match self.inner.executor_for(context) {
            Some(executor) => executor.execute(Box::new(f)),
            None => f(), // Immediate — run inline
        }
    }
}
