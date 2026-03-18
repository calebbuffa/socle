//! I3S LOD selection algorithm — maxScreenThreshold-based node traversal.
//!
//! I3S uses **node-switching**: when a node's projected screen size exceeds
//! its `lodThreshold`, the node is replaced by its children. Parent and children
//! are **never shown simultaneously**.
//!
//! The `lodThreshold` **increases** toward leaf nodes (higher-detail nodes tolerate
//! larger screen projections). This is inverted from 3D Tiles' screen-space error
//! where error *decreases* toward leaves.
//!
//! ## Algorithm
//!
//! ```text
//! traverse(node, camera):
//!   if frustum_cull(node.obb) == Outside: return
//!   screen_size = project(node.obb, camera)
//!   if screen_size <= node.lodThreshold:
//!     RENDER node  // detail is sufficient
//!   else if node has children:
//!     for child in children: traverse(child, camera)
//!   else:
//!     RENDER node  // leaf — max detail available
//! ```
//!
//! Enhancements:
//!
//! - **Near-to-far child ordering**: children are visited closest to camera first
//!   so the closest geometry gets highest load priority.
//! - **Loading descendant limit**: if too many descendants of an ancestor are in
//!   loading state, the descendants are "kicked" and the ancestor renders instead.
//! - **Load priority groups**: Urgent (rendering fallback), Normal (selected),
//!   Preload (ancestors/siblings).
//! - **Traversal statistics**: tiles visited, culled, kicked, max depth.

use glam::DVec3;

use i3s_geometry::culling::CullingResult;
use i3s_geometry::obb::OrientedBoundingBox;

use crate::node_state::{NodeLoadState, NodeState};
use crate::options::SelectionOptions;
use crate::update_result::{LoadPriority, LoadRequest, TraversalStats, ViewUpdateResult};
use crate::view_state::ViewState;

/// Minimum projected size (pixels) below which fog-culled nodes are rejected.
const FOG_CULL_THRESHOLD: f64 = 1.0;

/// The LOD metric type, matching `nodePageDefinition.lodSelectionMetricType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodMetric {
    /// `maxScreenThreshold`: compare projected bounding-sphere **diameter**
    /// in pixels against `lodThreshold`.
    MaxScreenThreshold,
    /// `maxScreenThresholdSQ`: compare projected bounding-volume **area**
    /// in pixels² against `lodThreshold`.
    MaxScreenThresholdSQ,
    /// `density-threshold` (PointCloud): compare projected bounding-volume
    /// **area** in pixels² against `lodThreshold` representing effective 2D area.
    DensityThreshold,
}

/// Item on the traversal stack.
struct StackEntry {
    node_id: u32,
    depth: u32,
}

/// Run I3S LOD selection on the node tree.
///
/// Traverses nodes starting from `root_id` using the I3S node-switching model.
/// A node is refined (replaced by children) when its projected screen size
/// exceeds its `lodThreshold`.
///
/// Accepts multiple [`ViewState`]s for multi-view scenarios (VR, shadow maps).
/// A node is visible if *any* view's frustum includes it, and the screen size
/// used for LOD is the maximum across all views.
///
/// # Arguments
/// - `nodes` — mutable slice of all node states (indexed by node ID)
/// - `children_of` — closure returning the child node IDs for a given node
/// - `obb_of` — closure returning the OBB for a given node
/// - `lod_threshold_of` — closure returning the node's `lodThreshold`
/// - `root_id` — starting node for traversal
/// - `views` — camera/viewport states (frustum derived from each)
/// - `metric` — LOD metric type for this scene layer
/// - `options` — selection tuning parameters
/// - `frame` — current frame number
/// - `nodes_per_page` — nodes per page (for detecting missing pages)
/// - `loaded_pages` — closure returning whether a page is loaded
pub fn select_nodes<F, G, H, P>(
    nodes: &mut [NodeState],
    children_of: F,
    obb_of: G,
    lod_threshold_of: H,
    root_id: u32,
    views: &[ViewState],
    metric: LodMetric,
    options: &SelectionOptions,
    frame: u64,
    nodes_per_page: usize,
    page_loaded: P,
) -> ViewUpdateResult
where
    F: Fn(u32, &mut Vec<u32>),
    G: Fn(u32) -> Option<OrientedBoundingBox>,
    H: Fn(u32) -> f64,
    P: Fn(u32) -> bool,
{
    // Derive culling volumes from view states
    let culling_volumes: Vec<_> = views.iter().map(|v| v.culling_volume()).collect();
    // Use the first view for fog culling camera height/position
    let primary_view = &views[0];
    let mut result = ViewUpdateResult::default();
    let mut stats = TraversalStats::default();
    let mut stack = vec![StackEntry {
        node_id: root_id,
        depth: 0,
    }];
    let mut children = Vec::new();
    let mut child_dists: Vec<(u32, f64)> = Vec::new();

    while let Some(entry) = stack.pop() {
        let node_id = entry.node_id;
        let depth = entry.depth;
        let idx = node_id as usize;

        // If the node's page isn't loaded yet, record the missing page
        if nodes_per_page > 0 {
            let page_id = (idx / nodes_per_page) as u32;
            if !page_loaded(page_id) {
                if !result.pages_needed.contains(&page_id) {
                    result.pages_needed.push(page_id);
                }
                continue;
            }
        }

        if idx >= nodes.len() {
            continue;
        }

        stats.tiles_visited += 1;
        if depth > stats.max_depth_visited {
            stats.max_depth_visited = depth;
        }

        nodes[idx].last_visited_frame = frame;

        // Get OBB for frustum culling and screen projection
        let obb = match obb_of(node_id) {
            Some(obb) => obb,
            None => {
                // No OBB means no geometry (e.g., root node in 3DObject/IntegratedMesh).
                // Always descend to children.
                children.clear();
                children_of(node_id, &mut children);
                for &child_id in &children {
                    stack.push(StackEntry {
                        node_id: child_id,
                        depth: depth + 1,
                    });
                }
                continue;
            }
        };

        // Frustum cull: visible if any view's frustum includes the OBB
        if options.enable_frustum_culling {
            let visible_in_any = culling_volumes
                .iter()
                .any(|cv| cv.visibility_obb(&obb) != CullingResult::Outside);
            if !visible_in_any {
                nodes[idx].visible = false;
                nodes[idx].selected = false;
                stats.tiles_culled += 1;
                continue;
            }
        }

        // Fog cull: reject nodes whose screen contribution is negligible due to distance.
        // Fog density scales with distance/view_height.
        if options.enable_fog_culling {
            let distance = primary_view.position.distance(obb.center);
            let radius = obb.half_size.length();
            // Camera height above origin — a proxy for altitude in ECEF scenes.
            let camera_height = primary_view.position.length().max(1.0);
            // Fog attenuation: objects much farther than 2× camera height are strongly attenuated
            let fog_ratio = distance / (camera_height * 2.0);
            if fog_ratio > 1.0 {
                // Attenuate screen size by fog
                let attenuation = 1.0 / (fog_ratio * fog_ratio);
                let attenuated_diameter =
                    primary_view.projected_screen_diameter(obb.center, radius) * attenuation;
                if attenuated_diameter < FOG_CULL_THRESHOLD {
                    nodes[idx].visible = false;
                    nodes[idx].selected = false;
                    stats.tiles_culled += 1;
                    continue;
                }
            }
        }

        nodes[idx].visible = true;

        // Compute projected screen size — max across all views
        let radius = obb.half_size.length();
        let screen_size = views
            .iter()
            .map(|v| match metric {
                LodMetric::MaxScreenThreshold => v.projected_screen_diameter(obb.center, radius),
                LodMetric::MaxScreenThresholdSQ | LodMetric::DensityThreshold => {
                    v.projected_screen_area(obb.center, radius)
                }
            })
            .fold(0.0_f64, f64::max);
        nodes[idx].projected_screen_size = screen_size;

        let lod_threshold = lod_threshold_of(node_id) * options.lod_threshold_multiplier;
        children.clear();
        children_of(node_id, &mut children);

        // I3S decision logic:
        // - screen_size <= lodThreshold → node detail is sufficient → RENDER
        // - screen_size > lodThreshold → node is too coarse → REFINE to children
        // - lodThreshold == 0 or no children → render as-is
        let should_refine =
            lod_threshold > 0.0 && screen_size > lod_threshold && !children.is_empty();

        if should_refine {
            // Check if all children are loaded
            let all_children_loaded = children.iter().all(|&cid| {
                let ci = cid as usize;
                ci < nodes.len() && nodes[ci].load_state == NodeLoadState::Loaded
            });

            // Check if some children are loaded (for forbid_holes=false)
            let some_children_loaded = !all_children_loaded
                && children.iter().any(|&cid| {
                    let ci = cid as usize;
                    ci < nodes.len() && nodes[ci].load_state == NodeLoadState::Loaded
                });

            if all_children_loaded {
                // Node-switching: replace this node with its children.
                // Push children nearest-to-camera last so they pop first (DFS near-to-far).
                nodes[idx].selected = false;
                push_children_near_to_far(
                    &mut stack,
                    &children,
                    &obb_of,
                    primary_view.position,
                    depth,
                    &mut child_dists,
                );
            } else if !options.forbid_holes && some_children_loaded {
                // Allow partial refinement: show loaded children, request the rest.
                // Don't render parent — accept holes for faster visual updates.
                nodes[idx].selected = false;
                for &child_id in &children {
                    let ci = child_id as usize;
                    if ci < nodes.len() && nodes[ci].load_state == NodeLoadState::Loaded {
                        stack.push(StackEntry {
                            node_id: child_id,
                            depth: depth + 1,
                        });
                    } else {
                        if nodes_per_page > 0 {
                            let child_page = (ci / nodes_per_page) as u32;
                            if !page_loaded(child_page) {
                                if !result.pages_needed.contains(&child_page) {
                                    result.pages_needed.push(child_page);
                                }
                                continue;
                            }
                        }
                        if ci < nodes.len() && nodes[ci].load_state == NodeLoadState::Unloaded {
                            result.load_requests.push(LoadRequest {
                                node_id: child_id,
                                priority: LoadPriority::Urgent,
                                screen_size,
                            });
                        }
                    }
                }
            } else {
                // Count loading descendants for this node
                let loading_count = children
                    .iter()
                    .filter(|&&cid| {
                        let ci = cid as usize;
                        ci < nodes.len() && nodes[ci].load_state == NodeLoadState::Loading
                    })
                    .count() as u32;

                if loading_count > options.loading_descendant_limit {
                    // Too many loading descendants — kick them, render parent
                    stats.tiles_kicked += loading_count;
                    render_node(nodes, idx, node_id, screen_size, &mut result);
                } else {
                    // Children not loaded yet — render this node as fallback
                    // and request children to load (Urgent because user sees stale data)
                    render_node(nodes, idx, node_id, screen_size, &mut result);
                    for &child_id in &children {
                        let ci = child_id as usize;
                        // If the child's page isn't loaded, request it
                        if nodes_per_page > 0 {
                            let child_page = (ci / nodes_per_page) as u32;
                            if !page_loaded(child_page) {
                                if !result.pages_needed.contains(&child_page) {
                                    result.pages_needed.push(child_page);
                                }
                                continue;
                            }
                        }
                        if ci < nodes.len() && nodes[ci].load_state == NodeLoadState::Unloaded {
                            result.load_requests.push(LoadRequest {
                                node_id: child_id,
                                priority: LoadPriority::Urgent,
                                screen_size,
                            });
                        }
                    }

                    // Preload siblings: if we're rendering this parent, request siblings
                    // of the children (i.e., other children) that might become visible
                    // during panning. This is already handled above since all children
                    // are requested — siblings of a child are its parent's other children.
                }
            }
        } else {
            // Detail is sufficient (or leaf node) — render this node
            render_node(nodes, idx, node_id, screen_size, &mut result);

            // Preload ancestors: request this node's children at reduced priority
            // so zoom-in transitions are smoother.
            if options.preload_ancestors && !children.is_empty() {
                for &child_id in &children {
                    let ci = child_id as usize;
                    if nodes_per_page > 0 {
                        let child_page = (ci / nodes_per_page) as u32;
                        if !page_loaded(child_page) {
                            if !result.pages_needed.contains(&child_page) {
                                result.pages_needed.push(child_page);
                            }
                            continue;
                        }
                    }
                    if ci < nodes.len() && nodes[ci].load_state == NodeLoadState::Unloaded {
                        result.load_requests.push(LoadRequest {
                            node_id: child_id,
                            priority: LoadPriority::Preload,
                            screen_size: screen_size * 0.5, // lower within group
                        });
                    }
                }
            }
        }
    }

    // Find nodes to unload: loaded but not visited recently
    for node in nodes.iter() {
        if node.load_state == NodeLoadState::Loaded && node.last_visited_frame + 2 < frame {
            result.nodes_to_unload.push(node.node_id);
        }
    }

    // Sort load requests: highest priority group first, then by screen size within group
    result.load_requests.sort_by(|a, b| {
        b.priority.cmp(&a.priority).then(
            b.screen_size
                .partial_cmp(&a.screen_size)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });

    result.stats = stats;
    result
}

/// Push children onto the stack in near-to-far order relative to the camera.
///
/// Children closest to the camera are pushed last so they are popped first
/// in the DFS traversal, giving them higher effective priority.
fn push_children_near_to_far<G>(
    stack: &mut Vec<StackEntry>,
    children: &[u32],
    obb_of: &G,
    camera_pos: DVec3,
    parent_depth: u32,
    buf: &mut Vec<(u32, f64)>,
) where
    G: Fn(u32) -> Option<OrientedBoundingBox>,
{
    if children.len() <= 1 {
        for &child_id in children {
            stack.push(StackEntry {
                node_id: child_id,
                depth: parent_depth + 1,
            });
        }
        return;
    }

    // Compute distance from camera to each child's OBB center
    buf.clear();
    buf.extend(children.iter().map(|&cid| {
        let dist = obb_of(cid)
            .map(|obb| camera_pos.distance_squared(obb.center))
            .unwrap_or(f64::MAX);
        (cid, dist)
    }));

    // Sort farthest first so closest ends up on top of the stack
    buf.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    for &(child_id, _) in buf.iter() {
        stack.push(StackEntry {
            node_id: child_id,
            depth: parent_depth + 1,
        });
    }
}

/// Mark a node for rendering if loaded, or request loading if unloaded.
fn render_node(
    nodes: &mut [NodeState],
    idx: usize,
    node_id: u32,
    screen_size: f64,
    result: &mut ViewUpdateResult,
) {
    nodes[idx].selected = true;
    match nodes[idx].load_state {
        NodeLoadState::Loaded => {
            result.nodes_to_render.push(node_id);
        }
        NodeLoadState::Unloaded => {
            result.load_requests.push(LoadRequest {
                node_id,
                priority: LoadPriority::Normal,
                screen_size,
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use glam::DVec3;

    use i3s_geometry::obb::OrientedBoundingBox;

    use super::*;

    fn make_obb(center: DVec3, half_size: f64) -> OrientedBoundingBox {
        OrientedBoundingBox::from_i3s(
            center.to_array(),
            [half_size, half_size, half_size],
            [0.0, 0.0, 0.0, 1.0],
        )
    }

    fn default_options() -> SelectionOptions {
        SelectionOptions::default()
    }

    fn far_camera() -> ViewState {
        ViewState::new(
            DVec3::new(0.0, 0.0, 1000.0),
            DVec3::NEG_Z,
            DVec3::Y,
            1920,
            1080,
            std::f64::consts::FRAC_PI_3,
        )
    }

    fn close_camera() -> ViewState {
        ViewState::new(
            DVec3::new(0.0, 0.0, 0.01),
            DVec3::NEG_Z,
            DVec3::Y,
            1920,
            1080,
            std::f64::consts::FRAC_PI_3,
        )
    }

    /// Helper to check if a node_id appears in load_requests
    fn has_load_request(result: &ViewUpdateResult, node_id: u32) -> bool {
        result.load_requests.iter().any(|r| r.node_id == node_id)
    }

    #[test]
    fn far_camera_renders_root_without_refining() {
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;
        nodes[1].load_state = NodeLoadState::Loaded;
        nodes[2].load_state = NodeLoadState::Loaded;

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 1.0)),
            |id| if id == 0 { 500.0 } else { 2000.0 },
            0,
            &[far_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert_eq!(result.nodes_to_render, vec![0]);
        assert!(!result.nodes_to_render.contains(&1));
        assert!(!result.nodes_to_render.contains(&2));
    }

    #[test]
    fn close_camera_refines_to_children() {
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;
        nodes[1].load_state = NodeLoadState::Loaded;
        nodes[2].load_state = NodeLoadState::Loaded;

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |id| if id == 0 { 50.0 } else { 200000.0 },
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert!(!result.nodes_to_render.contains(&0));
        assert!(result.nodes_to_render.contains(&1));
        assert!(result.nodes_to_render.contains(&2));
    }

    #[test]
    fn unloaded_root_is_requested() {
        let mut nodes = vec![NodeState::new(0)];

        let result = select_nodes(
            &mut nodes,
            |_, _: &mut Vec<u32>| {},
            |_| Some(make_obb(DVec3::ZERO, 1.0)),
            |_| 500.0,
            0,
            &[far_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert!(result.nodes_to_render.is_empty());
        assert!(has_load_request(&result, 0));
    }

    #[test]
    fn frustum_culled_node_not_selected() {
        let mut nodes = vec![NodeState::new(0)];
        nodes[0].load_state = NodeLoadState::Loaded;

        // Node far to the side, well outside the ~46° half-horizontal-FOV
        let result = select_nodes(
            &mut nodes,
            |_, _: &mut Vec<u32>| {},
            |_| Some(make_obb(DVec3::new(10000.0, 0.0, 0.0), 1.0)),
            |_| 500.0,
            0,
            &[far_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert!(result.nodes_to_render.is_empty());
        assert_eq!(result.stats.tiles_culled, 1);
    }

    #[test]
    fn unloaded_children_cause_fallback_to_parent() {
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |id| if id == 0 { 50.0 } else { 200000.0 },
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert!(result.nodes_to_render.contains(&0));
        assert!(has_load_request(&result, 1));
        assert!(has_load_request(&result, 2));
        // Children should be Urgent since we're rendering parent as fallback
        for req in &result.load_requests {
            if req.node_id == 1 || req.node_id == 2 {
                assert_eq!(req.priority, LoadPriority::Urgent);
            }
        }
    }

    #[test]
    fn root_with_no_obb_descends_to_children() {
        let mut nodes = vec![NodeState::new(0), NodeState::new(1)];
        nodes[1].load_state = NodeLoadState::Loaded;

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1]);
                }
            },
            |id| {
                if id == 0 {
                    None
                } else {
                    Some(make_obb(DVec3::ZERO, 1.0))
                }
            },
            |_| 500.0,
            0,
            &[far_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert!(!result.nodes_to_render.contains(&0));
        assert!(result.nodes_to_render.contains(&1));
    }

    #[test]
    fn leaf_node_always_rendered() {
        let mut nodes = vec![NodeState::new(0)];
        nodes[0].load_state = NodeLoadState::Loaded;

        let result = select_nodes(
            &mut nodes,
            |_, _: &mut Vec<u32>| {},
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |_| 1.0,
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert!(result.nodes_to_render.contains(&0));
    }

    #[test]
    fn lod_threshold_multiplier_reduces_refinement() {
        // Use far camera where projected size is small (~1.87px for 1m radius at 1000m).
        // With threshold 1.0 and multiplier 1.0: 1.87 > 1.0 → refines
        // With threshold 1.0 and multiplier 10.0: 1.87 < 10.0 → renders root
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;
        nodes[1].load_state = NodeLoadState::Loaded;
        nodes[2].load_state = NodeLoadState::Loaded;

        let mut opts = default_options();
        opts.preload_ancestors = false; // simplify test
        let result1 = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 1.0)),
            |_| 1.0, // low threshold — screen_size 1.87 exceeds it
            0,
            &[far_camera()],
            LodMetric::MaxScreenThreshold,
            &opts,
            1,
            64,
            |_| true,
        );
        // Close enough that 1.87 > 1.0 → refined to children
        assert!(result1.nodes_to_render.contains(&1));

        // With multiplier 10.0, effective threshold = 10.0, screen_size=1.87 < 10 → render root
        opts.lod_threshold_multiplier = 10.0;
        let result2 = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 1.0)),
            |_| 1.0,
            0,
            &[far_camera()],
            LodMetric::MaxScreenThreshold,
            &opts,
            2,
            64,
            |_| true,
        );
        assert!(result2.nodes_to_render.contains(&0)); // NOT refined
    }

    #[test]
    fn traversal_stats_recorded() {
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;
        nodes[1].load_state = NodeLoadState::Loaded;
        nodes[2].load_state = NodeLoadState::Loaded;

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |id| if id == 0 { 50.0 } else { 200000.0 },
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        assert!(result.stats.tiles_visited >= 3);
        assert_eq!(result.stats.max_depth_visited, 1);
    }

    #[test]
    fn load_requests_sorted_by_priority() {
        let mut nodes = vec![
            NodeState::new(0),
            NodeState::new(1),
            NodeState::new(2),
            NodeState::new(3),
            NodeState::new(4),
        ];
        nodes[0].load_state = NodeLoadState::Loaded;
        // nodes 1-4 stay Unloaded

        // Node 0 wants to refine to [1,2] → Urgent requests for 1,2
        // Node 0 also has children [3,4] in preload → Preload requests for 3,4
        // Actually let's set up a simple scenario: root refines, children unloaded
        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| match id {
                0 => out.extend_from_slice(&[1, 2]),
                _ => out.extend_from_slice(&[3, 4]),
            },
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |id| if id == 0 { 50.0 } else { 200000.0 },
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,
            |_| true,
        );

        // Urgent requests should come first
        if result.load_requests.len() >= 2 {
            let first_priority = result.load_requests[0].priority;
            for req in &result.load_requests[1..] {
                assert!(req.priority <= first_priority || first_priority == req.priority);
            }
        }
    }

    #[test]
    fn preload_children_when_rendering() {
        // When a node is rendered (detail sufficient), its children should be
        // preloaded if preload_ancestors is enabled
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;
        // children unloaded

        let opts = SelectionOptions {
            preload_ancestors: true,
            ..default_options()
        };

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 1.0)),
            |_| 500.0, // threshold is high → render root (detail sufficient)
            0,
            &[far_camera()],
            LodMetric::MaxScreenThreshold,
            &opts,
            1,
            64,
            |_| true,
        );

        // Root is rendered (detail sufficient)
        assert!(result.nodes_to_render.contains(&0));
        // Children are preloaded
        assert!(has_load_request(&result, 1));
        assert!(has_load_request(&result, 2));
        // Preload priority
        for req in &result.load_requests {
            if req.node_id == 1 || req.node_id == 2 {
                assert_eq!(req.priority, LoadPriority::Preload);
            }
        }
    }

    #[test]
    fn missing_page_recorded_in_pages_needed() {
        // Root is on page 0 (loaded), children 64+ are on page 1 (not loaded)
        let mut nodes = vec![NodeState::new(0)];
        nodes[0].load_state = NodeLoadState::Loaded;

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[64]);
                }
            }, // child 64 is page 1
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |id| if id == 0 { 50.0 } else { 200000.0 },
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &default_options(),
            1,
            64,                     // nodes_per_page
            |page_id| page_id == 0, // only page 0 loaded
        );

        // Page 1 should be requested
        assert!(result.pages_needed.contains(&1));
        // Root should still render as fallback since children can't be traversed
        assert!(result.nodes_to_render.contains(&0));
    }

    #[test]
    fn forbid_holes_false_shows_partial_children() {
        // Node 0 refines to [1, 2]. Node 1 is loaded, node 2 is not.
        // With forbid_holes=false (default): node 1 renders, node 2 is requested, parent NOT rendered.
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;
        nodes[1].load_state = NodeLoadState::Loaded;
        // node 2 stays Unloaded

        let opts = SelectionOptions {
            forbid_holes: false,
            ..default_options()
        };

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |id| if id == 0 { 50.0 } else { 200000.0 },
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &opts,
            1,
            64,
            |_| true,
        );

        // Child 1 should render (loaded), parent should NOT render
        assert!(result.nodes_to_render.contains(&1));
        assert!(!result.nodes_to_render.contains(&0));
        // Child 2 should be requested as Urgent
        assert!(has_load_request(&result, 2));
    }

    #[test]
    fn forbid_holes_true_shows_parent_fallback() {
        // Same setup but with forbid_holes=true: parent renders, children loaded individually don't.
        let mut nodes = vec![NodeState::new(0), NodeState::new(1), NodeState::new(2)];
        nodes[0].load_state = NodeLoadState::Loaded;
        nodes[1].load_state = NodeLoadState::Loaded;
        // node 2 stays Unloaded

        let opts = SelectionOptions {
            forbid_holes: true,
            ..default_options()
        };

        let result = select_nodes(
            &mut nodes,
            |id, out: &mut Vec<u32>| {
                if id == 0 {
                    out.extend_from_slice(&[1, 2]);
                }
            },
            |_| Some(make_obb(DVec3::ZERO, 10.0)),
            |id| if id == 0 { 50.0 } else { 200000.0 },
            0,
            &[close_camera()],
            LodMetric::MaxScreenThreshold,
            &opts,
            1,
            64,
            |_| true,
        );

        // Parent should render as fallback
        assert!(result.nodes_to_render.contains(&0));
        // Child 1 should NOT render individually
        assert!(!result.nodes_to_render.contains(&1));
    }

    #[test]
    fn fog_culling_rejects_distant_tiny_node() {
        // Camera at height 100, node at distance ~10,000 with 1m radius.
        // fog_ratio = 10000 / (100 * 2) = 50, attenuation = 1/2500.
        // Screen diameter ≈ 1870 * (1/10000) = 0.187, * 1/2500 ≈ negligible → culled.
        let mut nodes = vec![NodeState::new(0)];
        nodes[0].load_state = NodeLoadState::Loaded;

        let view = ViewState::new(
            DVec3::new(0.0, 0.0, 100.0),
            DVec3::NEG_Z,
            DVec3::Y,
            1920,
            1080,
            std::f64::consts::FRAC_PI_3,
        );

        let opts = SelectionOptions {
            enable_fog_culling: true,
            enable_frustum_culling: false, // disable so frustum doesn't catch it first
            ..default_options()
        };

        let result = select_nodes(
            &mut nodes,
            |_, _: &mut Vec<u32>| {},
            |_| Some(make_obb(DVec3::new(10000.0, 0.0, 0.0), 1.0)),
            |_| 500.0,
            0,
            &[view],
            LodMetric::MaxScreenThreshold,
            &opts,
            1,
            64,
            |_| true,
        );

        assert!(result.nodes_to_render.is_empty());
        assert_eq!(result.stats.tiles_culled, 1);
    }
}
