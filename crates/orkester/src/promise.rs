use crate::error::{AsyncError, ErrorCode};
use crate::state::SharedState;
use std::sync::Arc;

/// A one-shot producer for resolving or rejecting a paired [`crate::Future`].
pub struct Promise<T: Send + 'static> {
    state: Option<Arc<SharedState<T>>>,
}

impl<T: Send + 'static> Promise<T> {
    pub(crate) fn new(state: Arc<SharedState<T>>) -> Self {
        Self { state: Some(state) }
    }

    pub fn resolve(mut self, value: T) {
        if let Some(state) = self.state.take() {
            state.resolve(value);
        }
    }

    pub fn reject(mut self, error: impl Into<AsyncError>) {
        if let Some(state) = self.state.take() {
            state.reject(error.into());
        }
    }
}

impl<T: Send + 'static> Drop for Promise<T> {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            state.reject(AsyncError::with_code(
                ErrorCode::Dropped,
                "Promise dropped without resolving",
            ));
        }
    }
}
