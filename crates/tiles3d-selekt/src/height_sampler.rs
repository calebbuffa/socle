//! Approximate height sampler based on bounding-volume ray traversal.
//!
//! [`ApproximateHeightSampler`] implements [`selekt::HeightSampler`] by
//! ray-casting straight down from above the ellipsoid through the tile
//! hierarchy's bounding volumes.  It is fast because it uses only OBBs /
//! spheres already in memory — no tile content is accessed and no network I/O
//! occurs.  Results are approximate: the returned height is the surface of the
//! deepest bounding volume intersected, not actual mesh geometry.
//!
//! # Algorithm
//!
//! For each query position (lon, lat):
//! 1. Convert to ECEF and project onto the ellipsoid surface.
//! 2. Orient a ray from `surface + max_radius * up` pointing in the `-up`
//!    direction.
//! 3. Perform a depth-first traversal of the spatial hierarchy, pruning
//!    branches whose bounding volume is not intersected by the ray.
//! 4. At leaf nodes record the maximum parametric `t` (deepest intersection =
//!    closest to the actual surface from above).
//! 5. Back-project the hit point to `Cartographic` height.

use std::collections::VecDeque;
use std::sync::Arc;

use glam::DVec3;
use orkester::Task;
use selekt::{NodeId, SpatialHierarchy};
use terra::{Cartographic, Ellipsoid};

/// Optional height sampling for terrain queries.
///
/// Implemented by format loaders that can answer point-in-terrain queries.
pub trait HeightSampler: Send + Sync {
    /// Asynchronously sample terrain heights at the given positions.
    fn sample_heights(&self, positions: Vec<Cartographic>) -> Task<SampleHeightResult>;
}

/// Result of a [`HeightSampler::sample_heights`] call.
#[derive(Debug, Default)]
pub struct SampleHeightResult {
    /// Positions with heights filled in (in radians / metres).
    pub positions: Vec<Cartographic>,
    /// One entry per position — `true` if the height was successfully sampled.
    pub sample_success: Vec<bool>,
    /// Non-fatal warnings (e.g. node not loaded at that location yet).
    pub warnings: Vec<String>,
}

/// A snapshot of the spatial hierarchy suitable for ray-traversal.
///
/// Cloned out of the live engine so the sampler can be `Send + Sync` without
/// holding a lock.
struct HierarchySnapshot {
    /// `(root_id, bounds, children)` flattened by NodeId index.
    ///
    /// We walk it DFS, so children are stored as `Vec<NodeId>` per node.
    nodes: Vec<(zukei::SpatialBounds, Vec<NodeId>)>,
    root: NodeId,
}

impl HierarchySnapshot {
    fn from_hierarchy(h: &dyn SpatialHierarchy) -> Self {
        let root = h.root();
        // BFS to collect all reachable nodes.
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        queue.push_back(root);
        // Capacity guess — many hierarchies are in the thousands.
        let mut nodes: Vec<Option<(zukei::SpatialBounds, Vec<NodeId>)>> = Vec::new();

        while let Some(id) = queue.pop_front() {
            let idx = id.index();
            if nodes.len() <= idx {
                nodes.resize_with(idx + 1, || None);
            }
            if nodes[idx].is_none() {
                let bounds = h.bounds(id).clone();
                let children: Vec<NodeId> = h.children(id).to_vec();
                for &child in &children {
                    queue.push_back(child);
                }
                nodes[idx] = Some((bounds, children));
            }
        }

        // Replace None slots (unreachable indices) with dummy empty entries.
        let nodes = nodes
            .into_iter()
            .map(|opt| {
                opt.unwrap_or_else(|| {
                    (
                        zukei::SpatialBounds::Sphere {
                            center: DVec3::ZERO,
                            radius: 0.0,
                        },
                        Vec::new(),
                    )
                })
            })
            .collect();

        Self { nodes, root }
    }

    /// DFS traversal with branch pruning: only descends into nodes whose
    /// bounding volume is intersected by the ray.  Returns the maximum
    /// parametric `t` among all leaf hits.
    fn ray_cast_pruned(&self, origin: DVec3, direction: DVec3) -> Option<f64> {
        let mut best_t: Option<f64> = None;
        let mut stack = vec![self.root];

        while let Some(id) = stack.pop() {
            let idx = id.index();
            let Some((bounds, children)) = self.nodes.get(idx) else {
                continue;
            };

            let Some(t) = bounds.ray_intersect(origin, direction) else {
                continue; // prune — ray misses this branch
            };

            if t < 0.0 {
                continue; // behind the ray origin
            }

            if children.is_empty() {
                best_t = Some(match best_t {
                    Some(prev) => prev.max(t),
                    None => t,
                });
            } else {
                for &child in children {
                    stack.push(child);
                }
            }
        }

        best_t
    }
}

// SAFETY: the snapshot contains only cloned data (SpatialBounds: Clone +
// Send, NodeId: Copy).
unsafe impl Send for HierarchySnapshot {}
unsafe impl Sync for HierarchySnapshot {}

// ---------------------------------------------------------------------------
// Public sampler
// ---------------------------------------------------------------------------

/// Approximate height sampler that uses bounding-volume ray traversal.
///
/// Create from a [`SelectionEngine`](selekt::SelectionEngine) (or any
/// [`SpatialHierarchy`]) and use as a [`HeightSampler`].
///
/// # Accuracy
///
/// Heights are the parametric surface of the leaf bounding volume closest to
/// the ellipsoid surface directly below each query point.  Accuracy depends on
/// how tightly the loaded hierarchy's OBBs wrap the actual geometry.  For
/// typical 3D Tiles terrain datasets this is within a few metres for
/// well-loaded tilesets.
///
/// # Example
///
/// ```ignore
/// let sampler = ApproximateHeightSampler::from_hierarchy(
///     tileset.hierarchy().unwrap(),
///     Ellipsoid::wgs84(),
/// );
/// let task = sampler.sample_heights(positions);
/// let result = task.block()?;
/// ```
pub struct ApproximateHeightSampler {
    snapshot: Arc<HierarchySnapshot>,
    ellipsoid: Ellipsoid,
}

impl ApproximateHeightSampler {
    /// Build from any [`SpatialHierarchy`] reference.
    ///
    /// Snapshots the entire hierarchy immediately (BFS walk, allocates one
    /// `SpatialBounds` clone per node). The sampler is then independent of
    /// the engine's lifetime.
    pub fn from_hierarchy(
        hierarchy: &dyn SpatialHierarchy,
        ellipsoid: Ellipsoid,
    ) -> Self {
        Self {
            snapshot: Arc::new(HierarchySnapshot::from_hierarchy(hierarchy)),
            ellipsoid,
        }
    }
}

impl HeightSampler for ApproximateHeightSampler {
    fn sample_heights(&self, positions: Vec<Cartographic>) -> Task<SampleHeightResult> {
        let snapshot = Arc::clone(&self.snapshot);
        let ellipsoid = self.ellipsoid.clone();
        let max_radius = ellipsoid.maximum_radius();

        // The computation is entirely in-memory (no I/O), so we resolve inline.
        let n = positions.len();
        let mut out_positions = Vec::with_capacity(n);
        let mut sample_success = Vec::with_capacity(n);
        let mut warnings = Vec::new();

        for pos in &positions {
            let ecef = ellipsoid.cartographic_to_ecef(*pos);
            let Some(surface) = ellipsoid.scale_to_geodetic_surface(ecef) else {
                out_positions.push(*pos);
                sample_success.push(false);
                warnings.push(format!(
                    "scale_to_geodetic_surface returned None for {:?}",
                    pos
                ));
                continue;
            };

            let up = ellipsoid.geodetic_surface_normal(surface);
            let origin = surface + max_radius * up;
            let direction = -up;

            match snapshot.ray_cast_pruned(origin, direction) {
                Some(t) => {
                    let hit_ecef = origin + t * direction;
                    let height = match ellipsoid.ecef_to_cartographic(hit_ecef) {
                        Some(c) => c.height,
                        None => {
                            // Fall back to distance from ellipsoid surface.
                            (hit_ecef - surface).length()
                        }
                    };
                    out_positions.push(Cartographic::new(pos.longitude, pos.latitude, height));
                    sample_success.push(true);
                }
                None => {
                    // No bounding volume was hit directly below — return
                    // the original position with height unchanged.
                    out_positions.push(*pos);
                    sample_success.push(false);
                    warnings.push(format!(
                        "No bounding volume intersected at lon={:.4}° lat={:.4}°",
                        pos.longitude.to_degrees(),
                        pos.latitude.to_degrees(),
                    ));
                }
            }
        }

        let result = SampleHeightResult {
            positions: out_positions,
            sample_success,
            warnings,
        };
        orkester::resolved(result)
    }
}
