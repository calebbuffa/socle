use crate::view::ViewState;
use zukei::SpatialBounds;

// ── LodFamily ────────────────────────────────────────────────────────────────

/// An opaque, const-compatible token that uniquely identifies an LOD metric
/// family (e.g. geometric-error, max-screen-threshold).
///
/// Construct a unique family with a private sentinel function:
///
/// ```rust
/// use selekt::LodFamily;
/// fn _my_family_token() {}
/// pub const MY_FAMILY: LodFamily = LodFamily::from_token(_my_family_token);
/// ```
///
/// Compare with `==` to verify that a [`LodDescriptor`] belongs to the
/// expected evaluator before performing metric arithmetic.
#[derive(Clone, Copy)]
pub struct LodFamily(fn());

impl LodFamily {
    /// A sentinel meaning "no family / unset".  Evaluators must reject this.
    pub const NONE: Self = {
        fn _none() {}
        LodFamily(_none)
    };

    /// Build a `LodFamily` from a private zero-sized sentinel function.
    /// Each distinct `sentinel` function produces a distinct `LodFamily`.
    pub const fn from_token(sentinel: fn()) -> Self {
        LodFamily(sentinel)
    }
}

impl PartialEq for LodFamily {
    fn eq(&self, other: &Self) -> bool {
        self.0 as usize == other.0 as usize
    }
}

impl Eq for LodFamily {}

impl std::fmt::Debug for LodFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LodFamily({:#x})", self.0 as usize)
    }
}

// ── LodDescriptor / LodEvaluator ────────────────────────────────────────────

/// Whether children supplement or replace the parent during refinement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefinementMode {
    /// Additive: render parent and children simultaneously.
    Add,
    /// Replacement: children replace the parent when fully loaded.
    /// Requires ancestor fallback while children are loading.
    Replace,
}

/// Format-agnostic LOD metric descriptor.
///
/// The `family` token identifies the metric type (e.g., the family constant
/// exported by a particular [`LodEvaluator`] implementation).  `value`
/// contains the metric value in family-defined units.
///
/// Adapters map format-specific metrics to this form; `LodEvaluator`
/// interprets them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LodDescriptor {
    /// Metric family.  Evaluators check this before interpreting `value`; they
    /// return `false` immediately when the family does not match their own.
    pub family: LodFamily,
    /// Metric value in family-defined units (e.g., geometric error in metres).
    pub value: f64,
}

/// Determines when a node should refine to its children.
///
/// Implementations map format-specific metrics (Cesium geometric error,
/// I3S profile screen-size, etc.) to a refinement decision.
pub trait LodEvaluator: Send + Sync + 'static {
    /// Return `true` if the node should refine (i.e., load and show children).
    ///
    /// `multiplier` is the final LOD metric multiplier computed by the engine
    /// (combines `view.lod_metric_multiplier` with fog, progressive, foveated,
    /// and dynamic-detail adjustments). The evaluator should treat its threshold
    /// as if it were divided by `multiplier`.
    ///
    /// **Continuity lock**: when `mode == RefinementMode::Replace`, the engine
    /// will not stop rendering the parent until all children are `Renderable`.
    /// This method only controls *whether* refinement is desired, not *when* the
    /// parent is kicked — the engine enforces the ancestor fallback invariant.
    fn should_refine(
        &self,
        descriptor: &LodDescriptor,
        view: &ViewState,
        multiplier: f32,
        bounds: &SpatialBounds,
        mode: RefinementMode,
    ) -> bool;
}

impl LodEvaluator for Box<dyn LodEvaluator> {
    fn should_refine(
        &self,
        descriptor: &LodDescriptor,
        view: &ViewState,
        multiplier: f32,
        bounds: &SpatialBounds,
        mode: RefinementMode,
    ) -> bool {
        (**self).should_refine(descriptor, view, multiplier, bounds, mode)
    }
}
