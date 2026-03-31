//! `kiban` — foundation utilities for socle.
//!
//! Zero-dependency crate providing cross-cutting utilities used by all layers
//! of the socle crate graph:
//!
//! - [`resolve_url`] — resolve a relative URI or file path against a base
//! - [`file_extension`] — extract the file extension from a URL or path,
//!   stripping query strings

mod uri;

pub use uri::{file_extension, resolve_url};
