//! Selection configuration options.

/// Options controlling scene layer traversal and LOD selection behavior.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionOptions {
    /// Maximum number of simultaneous node content loads. Default: 20
    pub max_simultaneous_loads: usize,
    /// Total memory budget for loaded node content (bytes). Default: 512 MB
    pub maximum_cached_bytes: usize,

    /// Preload ancestor nodes so zoom-out is fast. Default: true
    pub preload_ancestors: bool,
    /// Preload sibling nodes so panning is smooth. Default: true
    pub preload_siblings: bool,

    /// Maximum loading descendants before "kicking" them and rendering the
    /// ancestor as a fallback. Prevents the camera from staring at low-LOD
    /// tiles while dozens of descendants trickle in. Default: 20
    pub loading_descendant_limit: u32,

    /// Never allow holes: keep rendering a parent until *all* its children
    /// are loaded, even if the camera is very close. Default: false
    pub forbid_holes: bool,

    /// Enable frustum culling (reject off-screen nodes). Default: true
    pub enable_frustum_culling: bool,

    /// Enable fog culling (reject far-away nodes in atmosphere). Default: true
    pub enable_fog_culling: bool,

    /// Multiplier applied to each node's `lodThreshold` before comparison.
    /// Values > 1.0 reduce quality (fewer refinements), < 1.0 increase quality.
    /// Default: 1.0
    pub lod_threshold_multiplier: f64,
}

impl Default for SelectionOptions {
    fn default() -> Self {
        Self {
            max_simultaneous_loads: 20,
            maximum_cached_bytes: 512 * 1024 * 1024,
            preload_ancestors: true,
            preload_siblings: true,
            loading_descendant_limit: 20,
            forbid_holes: false,
            enable_frustum_culling: true,
            enable_fog_culling: true,
            lod_threshold_multiplier: 1.0,
        }
    }
}
