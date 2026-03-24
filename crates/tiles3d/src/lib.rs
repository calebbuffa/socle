//! 3D Tiles data model with extension support.
//!
//! All types are generated from the [3D Tiles JSON Schema][spec] by `schema-gen`.
//! Every struct is fully self-contained — no base classes, no inheritance.
//!
//! [spec]: https://github.com/CesiumGS/3d-tiles/tree/main/specification

mod generated;

pub use generated::*;
