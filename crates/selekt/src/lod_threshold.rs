/// Budget-aware LOD multiplier with hysteresis.
///
/// Wraps a base LOD multiplier (from user config) and adjusts it dynamically
/// each frame based on resident-cache pressure:
///
/// - **Over budget**: multiplier is raised (coarser LOD) immediately, by
///   [`coarsen_rate`](LodThreshold::coarsen_rate) per frame, up to
///   [`max_multiplier_factor`](LodThreshold::max_multiplier_factor)× the base.
/// - **Under budget**: multiplier is lowered (finer LOD) slowly — only after
///   [`refine_stable_frames`](LodThreshold::refine_stable_frames) consecutive
///   under-budget frames, by [`refine_rate`](LodThreshold::refine_rate) per
///   frame, down to the base.
///
/// The asymmetry prevents thrashing: memory pressure is relieved quickly while
/// quality is reclaimed conservatively once the budget is stable.
///
/// Use [`LodThreshold::multiplier`] as the `lod_multiplier` argument to
/// [`traverse`](crate::traversal::traverse).
pub struct LodThreshold {
    /// User-configured base multiplier. The dynamic `current` value never goes
    /// below this floor.
    pub base: f64,
    /// Effective multiplier for the current frame. Feed this into traversal.
    pub current: f64,
    /// Consecutive frames the cache has remained under budget.
    frames_under_budget: u32,

    /// Number of consecutive under-budget frames required before quality begins
    /// recovering. Default: `60`.
    pub refine_stable_frames: u32,
    /// Per-frame multiplicative coarsening factor applied when over budget.
    /// Values > 1.0 increase the multiplier (coarser LOD). Default: `1.05` (5%).
    pub coarsen_rate: f64,
    /// Per-frame multiplicative refinement factor applied when under budget for
    /// long enough. Values < 1.0 decrease the multiplier (finer LOD). Default:
    /// `0.995` (0.5%).
    pub refine_rate: f64,
    /// The multiplier is capped at `base × max_multiplier_factor`. Default: `4.0`.
    pub max_multiplier_factor: f64,
}

impl LodThreshold {
    /// Create a threshold starting at `base` with default hysteresis parameters.
    pub fn new(base: f64) -> Self {
        Self {
            base,
            current: base,
            frames_under_budget: 0,
            refine_stable_frames: 60,
            coarsen_rate: 1.05,
            refine_rate: 0.995,
            max_multiplier_factor: 4.0,
        }
    }

    /// Call once per frame after eviction, returns the effective multiplier for
    /// this frame which should be passed to [`traverse`](crate::traversal::traverse).
    ///
    /// - `resident_bytes`: total bytes currently resident in the cache.
    /// - `budget_bytes`: maximum allowed resident bytes (`SelectionOptions::max_cached_bytes`).
    pub fn adjust(&mut self, resident_bytes: usize, budget_bytes: usize) -> f64 {
        if resident_bytes > budget_bytes {
            self.frames_under_budget = 0;
            self.current =
                (self.current * self.coarsen_rate).min(self.base * self.max_multiplier_factor);
        } else {
            self.frames_under_budget += 1;
            if self.frames_under_budget >= self.refine_stable_frames {
                self.current = (self.current * self.refine_rate).max(self.base);
            }
        }
        self.current
    }

    /// Reset to base multiplier (e.g. after a scene change).
    pub fn reset(&mut self) {
        self.current = self.base;
        self.frames_under_budget = 0;
    }
}

impl Default for LodThreshold {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarsens_when_over_budget() {
        let mut t = LodThreshold::new(1.0);
        let m = t.adjust(200, 100);
        assert!(m > 1.0, "should coarsen when over budget");
        assert!((m - t.coarsen_rate).abs() < 1e-10);
    }

    #[test]
    fn does_not_refine_immediately_under_budget() {
        let mut t = LodThreshold::new(1.0);
        t.current = 2.0;
        // Under budget but not enough stable frames.
        for _ in 0..(t.refine_stable_frames - 1) {
            t.adjust(50, 100);
        }
        assert_eq!(
            t.current, 2.0,
            "should not refine before stable frame threshold"
        );
    }

    #[test]
    fn refines_after_stable_frames() {
        let mut t = LodThreshold::new(1.0);
        t.current = 2.0;
        let refine_stable_frames = t.refine_stable_frames;
        for _ in 0..refine_stable_frames {
            t.adjust(50, 100);
        }
        assert!(t.current < 2.0, "should start refining after stable frames");
    }

    #[test]
    fn never_exceeds_max_factor() {
        let mut t = LodThreshold::new(1.0);
        for _ in 0..1000 {
            t.adjust(200, 100);
        }
        assert!(t.current <= t.max_multiplier_factor);
    }

    #[test]
    fn never_goes_below_base() {
        let mut t = LodThreshold::new(1.0);
        t.current = 1.001;
        for _ in 0..10_000 {
            t.adjust(50, 100);
        }
        assert!(t.current >= 1.0);
    }

    #[test]
    fn reset_restores_base() {
        let mut t = LodThreshold::new(1.0);
        t.adjust(200, 100);
        t.reset();
        assert_eq!(t.current, 1.0);
        assert_eq!(t.frames_under_budget, 0);
    }
}
