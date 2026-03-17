//! Auto-generated I3S type definitions.
//!
//! This crate provides pure data types for the I3S (Indexed 3D Scene Layer)
//! specification, organized by feature domain.

pub mod building;
pub mod core;
pub mod display;
pub mod feature;
pub mod geometry;
pub mod material;
pub mod node;
pub mod pointcloud;
pub mod spatial;

// Re-export key types for convenience
pub use core::SceneLayerInfo;
pub use core::SceneLayerType as LayerType;
