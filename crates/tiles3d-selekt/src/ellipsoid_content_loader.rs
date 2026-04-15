//! Procedural content loader for ellipsoid globe tiles.
//!
//! Builds a [`moderu::GltfModel`] directly from the tile's lon/lat bounding
//! rectangle without any network I/O, then feeds it through the
//! [`ContentPipeline`].
use std::sync::Arc;

use glam::DVec3;
use moderu::GltfModelBuilder;
use terra::{Cartographic, Ellipsoid};

/// Resolution of the tessellated grid per tile side (matches cesium-native's 24).
const RESOLUTION: usize = 65;

/// Build a `GltfModel` for a lon/lat patch on the ellipsoid surface, including
/// skirt geometry to hide cracks between adjacent LOD tiles.
pub(crate) fn build_model(
    ellipsoid: &Ellipsoid,
    west: f64,
    south: f64,
    east: f64,
    north: f64,
    skirt_height: f64,
) -> moderu::GltfModel {
    let lon_step = (east - west) / (RESOLUTION - 1) as f64;
    // lat_step is negative: we iterate from north (y=0) toward south (y=max).
    let lat_step = (south - north) / (RESOLUTION - 1) as f64;

    // Tile-local origin = center of the tile in ECEF.
    // Vertices are stored relative to this origin to preserve f32 precision.
    let center_lon = (west + east) * 0.5;
    let center_lat = (south + north) * 0.5;
    let origin = ellipsoid.cartographic_to_ecef(Cartographic::new(center_lon, center_lat, 0.0));

    // -- Core grid ----------------------------------------------------------
    let total = RESOLUTION * RESOLUTION;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total);
    let mut texcoords: Vec<[f32; 2]> = Vec::with_capacity(total);
    let mut indices: Vec<u32> = Vec::with_capacity(6 * (RESOLUTION - 1) * (RESOLUTION - 1));

    for x in 0..RESOLUTION {
        let lon = west + lon_step * x as f64;
        let u = x as f32 / (RESOLUTION - 1) as f32;
        for y in 0..RESOLUTION {
            let lat = north + lat_step * y as f64;
            let v = y as f32 / (RESOLUTION - 1) as f32;
            let ecef: DVec3 = ellipsoid.cartographic_to_ecef(Cartographic::new(lon, lat, 0.0));
            let local = ecef - origin;
            positions.push([local.x as f32, local.y as f32, local.z as f32]);
            texcoords.push([u, v]);

            if x < RESOLUTION - 1 && y < RESOLUTION - 1 {
                let i = (x * RESOLUTION + y) as u32;
                // Two triangles per quad.
                indices.extend_from_slice(&[
                    i + RESOLUTION as u32,
                    i,
                    i + 1,
                    i + RESOLUTION as u32,
                    i + 1,
                    i + RESOLUTION as u32 + 1,
                ]);
            }
        }
    }

    let no_skirt_indices_count = indices.len() as u32;
    let no_skirt_vertices_count = positions.len() as u32;

    // -- Skirt geometry (cesium-native addSkirt pattern) ---------------------
    // For each of the 4 edges (west, south, east, north), duplicate the edge
    // vertices at negative height to hide cracks between LOD levels.
    //
    // We recompute positions from cartographic (f64) to avoid f32 round-trip
    // precision loss.
    if skirt_height > 0.0 {
        let r = RESOLUTION;

        // Each edge as a list of (x_idx, y_idx) pairs in the grid.
        // West  edge: x=0,     y=0..R
        let west_edge: Vec<(usize, usize)> = (0..r).map(|y| (0, y)).collect();
        // South edge: y=R-1,   x=0..R
        let south_edge: Vec<(usize, usize)> = (0..r).map(|x| (x, r - 1)).collect();
        // East  edge: x=R-1,   y=(R-1)..=0  (reversed for winding)
        let east_edge: Vec<(usize, usize)> = (0..r).rev().map(|y| (r - 1, y)).collect();
        // North edge: y=0,     x=(R-1)..=0  (reversed for winding)
        let north_edge: Vec<(usize, usize)> = (0..r).rev().map(|x| (x, 0)).collect();

        let edges: [&[(usize, usize)]; 4] = [&west_edge, &south_edge, &east_edge, &north_edge];

        for edge in &edges {
            let skirt_base = positions.len() as u32;
            for &(xi, yi) in *edge {
                let grid_idx = xi * r + yi;
                // Recompute the surface point in f64 from cartographic coords.
                let lon = west + lon_step * xi as f64;
                let lat = north + lat_step * yi as f64;
                let ecef =
                    ellipsoid.cartographic_to_ecef(Cartographic::new(lon, lat, -skirt_height));
                let local = ecef - origin;
                positions.push([local.x as f32, local.y as f32, local.z as f32]);
                texcoords.push(texcoords[grid_idx]);
            }
            // Stitch triangles between edge and skirt vertices
            let n = edge.len() as u32;
            for i in 0..(n - 1) {
                let edge_curr = (edge[i as usize].0 * r + edge[i as usize].1) as u32;
                let edge_next = (edge[(i + 1) as usize].0 * r + edge[(i + 1) as usize].1) as u32;
                let skirt_curr = skirt_base + i;
                let skirt_next = skirt_base + i + 1;
                indices.extend_from_slice(&[
                    edge_curr, edge_next, skirt_curr, skirt_curr, edge_next, skirt_next,
                ]);
            }
        }
    }

    log::debug!(
        "build_model: skirt_height={:.1} core_verts={} skirt_verts={} total_idx={}",
        skirt_height,
        no_skirt_vertices_count,
        positions.len() as u32 - no_skirt_vertices_count,
        indices.len(),
    );

    // -- Build glTF model ---------------------------------------------------
    let mut builder = GltfModelBuilder::new();
    let pos_acc = builder.push_positions(&positions);
    let tc_acc = builder.push_tex_coords(&texcoords);
    let idx_acc = builder.push_indices(&indices);
    let mat = builder.push_material(moderu::Material {
        pbr_metallic_roughness: Some(moderu::MaterialPbrMetallicRoughness {
            base_color_factor: vec![0.0, 0.35, 0.7, 1.0],
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            ..Default::default()
        }),
        double_sided: true, // ensure skirt triangles are visible
        ..Default::default()
    });
    let prim = builder
        .primitive()
        .attribute("POSITION", pos_acc)
        .attribute("TEXCOORD_0", tc_acc)
        .indices(idx_acc)
        .material(mat)
        .build();
    let mesh = builder.push_mesh(prim);

    // Embed skirt metadata so renderers can optionally skip skirt indices.
    if skirt_height > 0.0 {
        let skirt_meta = crate::skirt::SkirtMeshMetadata {
            no_skirt_indices_begin: 0,
            no_skirt_indices_count,
            no_skirt_vertices_begin: 0,
            no_skirt_vertices_count,
            mesh_center: [origin.x, origin.y, origin.z],
            skirt_west_height: skirt_height,
            skirt_south_height: skirt_height,
            skirt_east_height: skirt_height,
            skirt_north_height: skirt_height,
        };
        builder.model_mut().meshes[mesh.0].extras = Some(skirt_meta.to_extras());
    }

    // Don't embed the model→ECEF transform in the glTF node — the tile
    // hierarchy already provides it via RenderNode.world_transform.
    builder.node().mesh(mesh).build();
    builder.set_up_axis(2); // Z-up (ECEF)
    builder.finish()
}

pub(crate) fn total_byte_size() -> usize {
    // positions: RESOLUTION^2 * 3 f32s
    RESOLUTION * RESOLUTION * 3 * 4 + 6 * (RESOLUTION - 1) * (RESOLUTION - 1) * 4 // indices: u32
}
