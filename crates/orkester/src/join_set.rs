//! Detached tasks and join sets.

use std::sync::{Arc, Condvar, Mutex};

use crate::error::AsyncError;
use crate::task::{Task, TaskInner};
use crate::task_cell::TaskCell;

/// A collection of tasks that can be joined together.
pub struct JoinSet<T: Send + 'static> {
    entries: Vec<Arc<TaskCell<T>>>,
}

impl<T: Send + 'static> JoinSet<T> {
    pub fn new() -> Self {
        Self {
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
    pub fn block_all(self) -> Vec<Result<T, AsyncError>> {
        self.entries
            .into_iter()
            .map(|cell| {
                cell.wait_until_ready();
                cell.take_result()
                    .unwrap_or_else(|| Err(AsyncError::msg("JoinSet: missing result")))
            })
            .collect()
    }

    /// Block until the next task completes, and return its result.
    /// Returns `None` when the set is empty.
    pub fn join_next(&mut self) -> Option<Result<T, AsyncError>> {
        if self.entries.is_empty() {
            return None;
        }

        // Shared condvar that any completing cell will notify.
        let pair = Arc::new((Mutex::new(()), Condvar::new()));

        for cell in &self.entries {
            let pair = Arc::clone(&pair);
            TaskCell::on_ready(cell, move || {
                pair.1.notify_all();
            });
        }

        loop {
            for i in 0..self.entries.len() {
                if self.entries[i].is_ready() {
                    let cell = self.entries.swap_remove(i);
                    return Some(
                        cell.take_result()
                            .unwrap_or_else(|| Err(AsyncError::msg("JoinSet: missing result"))),
                    );
                }
            }
            let (lock, condvar) = &*pair;
            let guard = lock.lock().expect("join_next condvar lock");
            drop(condvar.wait(guard).expect("join_next condvar wait"));
        }
    }
}

impl<T: Send + 'static> Default for JoinSet<T> {
    fn default() -> Self {
        Self::new()
    }
}
