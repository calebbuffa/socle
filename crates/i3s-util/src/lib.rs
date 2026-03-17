//! Shared utilities for the i3s-native engine.
//!
//! Provides error types, math constants, and helper functions used
//! across all i3s crates.

pub mod error;
pub mod math;

pub use error::{I3sError, Result};
