use crate::view::ViewState;
use zukei::bounds::SpatialBounds;

/// Whether children supplement or replace the parent during refinement.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefinementMode {
    /// Additive: render parent and children simultaneously.
    Add,
    /// Replacement: children replace the parent when fully loaded.
    /// Requires ancestor fallback while children are loading.
    Replace,
}

/// Format-agnostic LOD metric descriptor.
/// The `family` string identifies the metric type (e.g., `"geometric_error"`, `"max_screen_size"`).
/// `values` contains one or more metric values in family-defined units.
/// Adapters map format-specific metrics to this form; `LodEvaluator` interprets them.
#[derive(Clone, Debug, PartialEq)]
pub struct LodDescriptor {
    pub family: String,
    pub values: Vec<f64>,
}

/// Determines when a node should refine to its children.
///
/// Implementations map format-specific metrics (Cesium geometric error,
/// I3S profile screen-size, etc.) to a refinement decision.
pub trait LodEvaluator: Send + Sync + 'static {
    /// Return `true` if the node should refine (i.e., load and show children).
    ///
    /// **Continuity lock**: when `mode == RefinementMode::Replace`, the engine
    /// will not stop rendering the parent until all children are `Renderable`.
    /// This method only controls *whether* refinement is desired, not *when* the
    /// parent is kicked — the engine enforces the ancestor fallback invariant.
    fn should_refine(
        &self,
        descriptor: &LodDescriptor,
        view: &ViewState,
        bounds: &SpatialBounds,
        mode: RefinementMode,
    ) -> bool;
}

impl LodEvaluator for Box<dyn LodEvaluator> {
    fn should_refine(
        &self,
        descriptor: &LodDescriptor,
        view: &ViewState,
        bounds: &SpatialBounds,
        mode: RefinementMode,
    ) -> bool {
        (**self).should_refine(descriptor, view, bounds, mode)
    }
}
