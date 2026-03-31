use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::sync::Arc;

/// Classification code for async errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Unclassified error.
    Generic,
    /// The operation was cancelled via [`CancellationToken`](crate::CancellationToken).
    Cancelled,
    /// The operation exceeded a time limit.
    TimedOut,
    /// A [`Resolver`](crate::Resolver) was dropped without resolving.
    Dropped,
}

/// Error type used by async primitives in this crate.
#[derive(Clone)]
pub struct AsyncError {
    code: ErrorCode,
    inner: Arc<dyn Error + Send + Sync + 'static>,
}

impl AsyncError {
    /// Wrap a typed error.
    pub fn new<E>(error: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            code: ErrorCode::Generic,
            inner: Arc::new(error),
        }
    }

    /// Construct an error from a message.
    pub fn msg(message: impl Into<String>) -> Self {
        Self::new(StringError(message.into()))
    }

    /// Construct an error with a specific code and message.
    pub fn with_code(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            inner: Arc::new(StringError(message.into())),
        }
    }

    /// Returns the error classification code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// Try to downcast to a concrete error type.
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: Error + 'static,
    {
        self.inner.as_ref().downcast_ref::<E>()
    }

    /// Access the wrapped error object.
    pub fn inner(&self) -> &(dyn Error + Send + Sync + 'static) {
        self.inner.as_ref()
    }
}

impl Display for AsyncError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self.inner.as_ref(), f)
    }
}

impl Debug for AsyncError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AsyncError")
            .field(&self.inner.to_string())
            .finish()
    }
}

impl Error for AsyncError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.inner.as_ref())
    }
}

impl From<String> for AsyncError {
    fn from(value: String) -> Self {
        Self::msg(value)
    }
}

impl From<&str> for AsyncError {
    fn from(value: &str) -> Self {
        Self::msg(value)
    }
}

impl From<Box<dyn Error + Send + Sync + 'static>> for AsyncError {
    fn from(value: Box<dyn Error + Send + Sync + 'static>) -> Self {
        Self {
            code: ErrorCode::Generic,
            inner: Arc::from(value),
        }
    }
}

#[derive(Debug)]
struct StringError(String);

impl Display for StringError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for StringError {}
