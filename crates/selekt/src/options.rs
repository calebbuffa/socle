/// Core engine options. Format-agnostic; no format-specific flags belong here.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct SelectionOptions {
    /// Maximum number of children simultaneously in the `Loading` state before the
    /// traversal stops descending further. Prevents unbounded in-flight load explosion.
    /// See continuity lock.
    pub loading_descendant_limit: usize,

    /// If `true`, the selection must not produce holes — a parent node is always
    /// selected as a fallback if replacement children are not yet `Renderable`.
    /// If `false`, mixed parent/child visibility is permitted (more aggressive refinement).
    /// Hole prevention: culled Replace-refined tiles are force-queued at `Normal`
    /// priority to prevent LOD seams during camera movement.
    pub prevent_holes: bool,

    /// If `true`, ancestors of rendered nodes are pre-loaded at `Preload`
    /// priority. This improves the zoom-out experience by ensuring parent
    /// tiles are ready before they are needed.
    pub preload_ancestors: bool,

    /// If `true`, siblings of rendered nodes are pre-loaded at `Preload`
    /// priority. This improves panning by loading tiles adjacent to the
    /// current view so they are ready when the camera moves.
    pub preload_siblings: bool,

    /// Maximum number of load retry attempts before a node transitions to `Failed`.
    pub retry_limit: u8,

    /// Frames to wait before re-queuing a `RetryScheduled` node.
    pub retry_backoff_frames: u32,

    /// Maximum new load requests to dispatch per load pass.
    pub max_simultaneous_tile_loads: usize,

    /// Maximum main-thread finalization tasks to run per frame.
    pub max_main_thread_tasks: usize,

    /// Memory ceiling in bytes for resident content. Eviction is triggered when
    /// exceeded.
    pub max_cached_bytes: usize,

    /// Whether frustum culling is enabled. When `false`, all nodes are treated
    /// as visible (useful for debugging).
    pub enable_frustum_culling: bool,

    /// Whether occlusion-driven refinement deferral is enabled.
    /// Requires an [`OcclusionTester`] to be wired into the engine.
    pub enable_occlusion_culling: bool,
}

impl Default for SelectionOptions {
    fn default() -> Self {
        Self {
            loading_descendant_limit: 20,
            prevent_holes: true,
            preload_ancestors: true,
            preload_siblings: true,
            retry_limit: 3,
            retry_backoff_frames: 8,
            max_simultaneous_tile_loads: 20,
            max_main_thread_tasks: 128,
            max_cached_bytes: 512 * 1024 * 1024,
            enable_frustum_culling: true,
            enable_occlusion_culling: false,
        }
    }
}

/// Error returned by `SelectionEngine` workflow methods.
#[derive(Clone, Debug)]
pub struct SelectionError {
    pub message: String,
}

impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SelectionError {}
