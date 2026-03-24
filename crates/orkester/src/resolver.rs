//! One-shot producer for resolving or rejecting a paired [`Task`](crate::Task).

use std::sync::Arc;

use crate::error::{AsyncError, ErrorCode};
use crate::task_cell::TaskCell;

/// A one-shot producer that completes a paired [`Task`](crate::Task).
///
/// Resolving or rejecting consumes the `Resolver`. If dropped without
/// resolving, the paired task is automatically rejected with
/// [`ErrorCode::Dropped`].
pub struct Resolver<T: Send + 'static> {
    cell: Option<Arc<TaskCell<T>>>,
}

impl<T: Send + 'static> Resolver<T> {
    pub(crate) fn new(cell: Arc<TaskCell<T>>) -> Self {
        Self { cell: Some(cell) }
    }

    /// Resolve the paired task with a value.
    pub fn resolve(mut self, value: T) {
        if let Some(cell) = self.cell.take() {
            cell.complete(Ok(value));
        }
    }

    /// Reject the paired task with an error.
    pub fn reject(mut self, error: impl Into<AsyncError>) {
        if let Some(cell) = self.cell.take() {
            cell.complete(Err(error.into()));
        }
    }
}

impl<T: Send + 'static> Drop for Resolver<T> {
    fn drop(&mut self) {
        if let Some(cell) = self.cell.take() {
            cell.complete(Err(AsyncError::with_code(
                ErrorCode::Dropped,
                "Resolver dropped without resolving",
            )));
        }
    }
}
