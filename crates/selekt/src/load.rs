//! Content key and load priority types.
//!
//! These are the only load-related types selekt needs — it emits load
//! requests but does not perform loading itself.

/// Stable content address for a node (URI, key, or other format-defined identifier).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentKey(pub String);

/// Load scheduling tier. Processing order: Urgent → Normal → Preload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityGroup {
    /// Speculative: siblings of culled nodes, pre-loaded for smooth panning.
    Preload = 0,
    /// Normal: nodes required for current-frame LOD.
    Normal = 1,
    /// Urgent: nodes whose absence causes kicked ancestors (visible detail loss).
    Urgent = 2,
}

/// Full load priority for a candidate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadPriority {
    /// Scheduling tier.
    pub group: PriorityGroup,
    /// Within-group score: lower value = higher priority.
    pub score: i64,
    /// View-group weight for cross-group fairness.
    pub view_group_weight: u16,
}
