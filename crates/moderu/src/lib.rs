//! glTF 2.0 data model with vendor extension support.
//!
//! All types are generated from the [glTF JSON Schema][spec] by `schema-gen`.
//! Every struct is fully self-contained — no base classes, no inheritance.
//!
//! [spec]: https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html

mod generated;

pub mod accessor;
pub mod buffer_data;
pub mod property_type;
pub mod sampler;
pub mod semantics;

pub use accessor::{AccessorType, ComponentType};
pub use buffer_data::{BufferData, ImageData};
pub use generated::*;
pub use property_type::{PropertyComponentType, PropertyType};
