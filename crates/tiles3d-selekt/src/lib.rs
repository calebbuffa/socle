//! `tiles3d-selekt` — selekt adapter for 3D Tiles streaming and LOD selection.
//!
//! Provides the async loading pipeline ([`TilesetLoader`], [`TilesetLoaderFactory`]),
//! spatial hierarchy adapters ([`ExplicitTilesetHierarchy`],
//! [`ImplicitQuadtreeHierarchy`], [`ImplicitOctreeHierarchy`]),
//! external tileset resolution ([`ExternalTilesetResolver`]), and the
//! high-level [`Tiles3dFormat`] adapter that wires them all together for
//! `selekt::SelectionEngine`.

mod ellipsoid_tileset_loader;
mod evaluator;
mod format;
mod hierarchy;
mod height_sampler;
mod loader;
mod resolver;
mod tileset;

pub use ellipsoid_tileset_loader::EllipsoidTilesetLoader;
pub use evaluator::{GEOMETRIC_ERROR_FAMILY, GeometricErrorEvaluator};
pub use hierarchy::{ExplicitTilesetHierarchy, ImplicitOctreeHierarchy, ImplicitQuadtreeHierarchy};
pub use loader::{
    TileContentDecoder, Tiles3dError, TilesetLoader, TilesetLoaderFactory,
};
pub use height_sampler::{ApproximateHeightSampler, HeightSampler, SampleHeightResult};
pub use resolver::ExternalTilesetResolver;
pub use tiles3d_content::TileFormat;
pub use tileset::{Tileset, TilesetBuilder};
