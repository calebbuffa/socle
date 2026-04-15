//! `tiles3d-selekt` — selekt adapter for 3D Tiles streaming and LOD selection.
//!
//! Provides the async loading pipeline ([`TilesetLoader`], [`TilesetLoaderFactory`]),
//! spatial hierarchy adapters ([`ExplicitTilesetHierarchy`],
//! [`ImplicitQuadtreeHierarchy`], [`ImplicitOctreeHierarchy`]),
//! external tileset resolution ([`ExternalTilesetResolver`]), and the
//! high-level [`TilesetBuilder`] that wires them together.

pub(crate) mod ellipsoid_content_loader;
mod ellipsoid_tileset_loader;
mod evaluator;
mod height_sampler;
pub(crate) mod hierarchy;
mod loader;
mod resolver;
mod skirt;
mod tileset;

pub use ellipsoid_tileset_loader::EllipsoidTilesetLoader;
pub use evaluator::{GEOMETRIC_ERROR_FAMILY, GeometricErrorEvaluator};
pub use height_sampler::{ApproximateHeightSampler, HeightSampler, SampleHeightResult};
pub use hierarchy::{ExplicitTilesetHierarchy, ImplicitOctreeHierarchy, ImplicitQuadtreeHierarchy};
pub use loader::{LoadResult, Tiles3dError, TilesetLoader, TilesetLoaderFactory};
pub use resolver::ExternalTilesetResolver;
pub use tiles3d_content::TileFormat;
pub use tileset::{ReadyTileset, TilesetBuilder, TilesetResult};
pub use skirt::SkirtMeshMetadata;
