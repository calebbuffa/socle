//! [`Tiles3dFormat`] — the [`Format`] adapter for 3D Tiles.
//!
//! Bundles a dynamic [`SpatialHierarchy`] (explicit or implicit, chosen at
//! runtime after parsing `tileset.json`), [`GeometricErrorEvaluator`],
//! [`ExternalTilesetResolver`], and [`TilesetLoader`] into one type so that
//! `SelectionEngine::<Tiles3dFormat<R>>` compiles with a single parameter.
//!
//! Using `Box<dyn SpatialHierarchy>` as the hierarchy type lets
//! [`TilesetLoaderFactory`] return either an [`ExplicitTilesetHierarchy`],
//! [`ImplicitQuadtreeHierarchy`], or [`ImplicitOctreeHierarchy`] depending
//! on whether the root tile carries an `implicitTiling` descriptor.

use std::marker::PhantomData;

use selekt::{Format, GeometricErrorEvaluator, SpatialHierarchy};
use egaku::PrepareRendererResources;

use crate::loader::TilesetLoader;
use crate::resolver::ExternalTilesetResolver;

/// The `Format` adapter for explicit (non-implicit) 3D Tiles datasets.
///
/// # Type parameters
///
/// * `R` — A [`PrepareRendererResources`] implementation.
///   Determines `Content`, `WorkerResult`, and the decode/GPU-upload behaviour.
///
/// # Usage
///
/// ```ignore
/// use tiles3d::format::Tiles3dFormat;
/// use tiles3d::loader::TilesetLoaderFactory;
/// use selekt::SelectionEngine;
///
/// // MyRenderer implements PrepareRendererResources.
/// let factory = TilesetLoaderFactory::<MyRenderer>::new(url, Arc::new(renderer));
/// let engine = SelectionEngine::<Tiles3dFormat<MyRenderer>>::from_factory(
///     externals, factory, resolver, policy, options,
/// );
/// ```
pub struct Tiles3dFormat<R>(PhantomData<fn() -> R>)
where
    R: PrepareRendererResources;

impl<R> Format for Tiles3dFormat<R>
where
    R: PrepareRendererResources + 'static,
    R::WorkerResult: Send + 'static,
    R::Content: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    type Content = R::Content;
    type Hierarchy = Box<dyn SpatialHierarchy>;
    type Lod = GeometricErrorEvaluator;
    type Resolver = ExternalTilesetResolver;
    type Loader = TilesetLoader<R>;
}
