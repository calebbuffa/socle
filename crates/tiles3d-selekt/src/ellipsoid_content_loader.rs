//! Procedural content loader for ellipsoid globe tiles.
//!
//! Mirrors `CesiumGeospatial::EllipsoidTilesetLoader::loadTileContent` —
//! builds a [`moderu::GltfModel`] directly from the tile's lon/lat bounding
//! rectangle without any network I/O, then feeds it through the standard
//! [`PrepareRendererResources`] two-phase pipeline (worker decode →
//! main-thread GPU upload).

use std::sync::Arc;

use egaku::PrepareRendererResources;
use glam::{DMat4, DVec3};
use moderu::GltfModelBuilder;
use orkester::{CancellationToken, Context, Task};
use selekt::{ContentKey, ContentLoader, NodeContent, NodeId};
use terra::{Cartographic, Ellipsoid};

use crate::loader::Tiles3dError;

/// Resolution of the tessellated grid per tile side (matches cesium-native's 24).
const RESOLUTION: usize = 24;

/// Procedurally generates a `GltfModel` patch mesh for each ellipsoid tile.
///
/// The content key must be `"west,south,east,north"` in radians, as encoded by
/// [`EllipsoidTilesetLoader`](crate::EllipsoidTilesetLoader).
pub struct EllipsoidContentLoader<R: PrepareRendererResources> {
    ellipsoid: Ellipsoid,
    preparer: Arc<R>,
}

impl<R: PrepareRendererResources> EllipsoidContentLoader<R> {
    pub fn new(ellipsoid: Ellipsoid, preparer: Arc<R>) -> Self {
        Self { ellipsoid, preparer }
    }
}

/// Build a `GltfModel` for a lon/lat patch on the ellipsoid surface.
///
/// Vertices are tile-local: ECEF position minus the NW-corner ECEF origin,
/// matching cesium-native's `inverseTransform * ecef_vertex` pattern.
fn build_model(ellipsoid: &Ellipsoid, west: f64, south: f64, east: f64, north: f64) -> moderu::GltfModel {
    let lon_step = (east - west) / (RESOLUTION - 1) as f64;
    // lat_step is negative: we iterate from north (y=0) toward south (y=max).
    let lat_step = (south - north) / (RESOLUTION - 1) as f64;

    // Tile-local origin = center of the tile in ECEF.
    // Vertices are stored relative to this origin to preserve f32 precision.
    let center_lon = (west + east) * 0.5;
    let center_lat = (south + north) * 0.5;
    let origin = ellipsoid.cartographic_to_ecef(Cartographic::new(center_lon, center_lat, 0.0));

    let total = RESOLUTION * RESOLUTION;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total);
    let mut indices: Vec<u32> = Vec::with_capacity(6 * (RESOLUTION - 1) * (RESOLUTION - 1));

    for x in 0..RESOLUTION {
        let lon = west + lon_step * x as f64;
        for y in 0..RESOLUTION {
            let lat = north + lat_step * y as f64;
            let ecef: DVec3 = ellipsoid.cartographic_to_ecef(Cartographic::new(lon, lat, 0.0));
            let local = ecef - origin;
            positions.push([local.x as f32, local.y as f32, local.z as f32]);

            if x < RESOLUTION - 1 && y < RESOLUTION - 1 {
                let i = (x * RESOLUTION + y) as u32;
                // Two triangles per quad.
                indices.extend_from_slice(&[i + RESOLUTION as u32, i, i + 1,
                                             i + RESOLUTION as u32, i + 1, i + RESOLUTION as u32 + 1]);
            }
        }
    }

    let mut builder = GltfModelBuilder::new();
    let pos_acc = builder.push_positions(&positions);
    let idx_acc = builder.push_indices(&indices);
    let mat = builder.push_default_material([0.0, 0.35, 0.7, 1.0]);
    let prim = builder
        .primitive()
        .attribute("POSITION", pos_acc)
        .indices(idx_acc)
        .material(mat)
        .build();
    let mesh = builder.push_mesh(prim);
    builder.push_node(mesh);
    builder.set_up_axis(2); // Z-up (ECEF)
    builder.finish()
}

impl<R> ContentLoader<R::Content> for EllipsoidContentLoader<R>
where
    R: PrepareRendererResources + 'static,
    R::WorkerResult: Send + 'static,
    R::Content: Send + 'static,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    type Error = Tiles3dError;

    fn load(
        &self,
        bg_context: &Context,
        main_context: &Context,
        _node_id: NodeId,
        key: &ContentKey,
        _parent_world_transform: glam::DMat4,
        _cancel: CancellationToken,
    ) -> Task<Result<NodeContent<R::Content>, Self::Error>> {
        // Parse "west,south,east,north" from the content key.
        let parts: Vec<f64> = key.0.split(',')
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() != 4 {
            return orkester::resolved(Ok(NodeContent::empty()));
        }
        let (west, south, east, north) = (parts[0], parts[1], parts[2], parts[3]);
        let byte_size = total_byte_size();

        let ellipsoid = self.ellipsoid.clone();
        let preparer = Arc::clone(&self.preparer);
        let main_context = main_context.clone();

        // Worker thread: tessellate and run prepare_in_load_thread.
        bg_context.run(move || -> Task<Result<NodeContent<R::Content>, Tiles3dError>> {
            let model = build_model(&ellipsoid, west, south, east, north);
            let worker_result = match preparer.prepare_in_load_thread(model) {
                Ok(r) => r,
                Err(e) => return orkester::resolved(Err(Tiles3dError::Decode(Box::new(e)))),
            };
            // Main thread: run prepare_in_main_thread.
            main_context.run(move || {
                let content = preparer.prepare_in_main_thread(worker_result);
                Ok(NodeContent::renderable(content, byte_size))
            })
        })
    }
}

fn total_byte_size() -> usize {
    // positions: RESOLUTION^2 * 3 f32s
    RESOLUTION * RESOLUTION * 3 * 4
        + 6 * (RESOLUTION - 1) * (RESOLUTION - 1) * 4 // indices: u32
}
