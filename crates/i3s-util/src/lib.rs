//! Shared utilities for the i3s-native engine.
//!
//! Provides error types, math constants, and helper functions used
//! across all i3s crates.

pub mod error;
pub mod math;
pub mod uri;

pub use error::{I3SError, Result};
pub use math::{
    equals_epsilon, equals_epsilon_abs, from_snorm, mod_val, negative_pi_to_pi, sign_not_zero,
    to_degrees, to_radians, to_snorm, zero_to_two_pi,
};
