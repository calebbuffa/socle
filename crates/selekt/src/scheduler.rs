use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

use crate::load::{LoadCandidate, PriorityGroup};

/// Pluggable scheduling contract.
///
/// The engine pushes candidates after traversal and drains them during the load
/// pass, bounded by `LoadPassBudget::max_new_requests`.
pub trait LoadScheduler: Send + Sync + 'static {
    fn push(&mut self, candidate: LoadCandidate);
    fn pop(&mut self) -> Option<LoadCandidate>;
    fn tick(&mut self, frame_index: u64);
    fn clear(&mut self);
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Clone, Debug)]
struct Candidate(LoadCandidate);

impl Candidate {
    fn priority_group(&self) -> PriorityGroup {
        self.0.priority.group
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        let a = &self.0.priority;
        let b = &other.0.priority;

        a.group
            .cmp(&b.group)
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| other.0.node_id.cmp(&self.0.node_id))
    }
}

#[derive(Debug, Default)]
struct GroupQueue {
    weight: u16,
    virtual_time: f64,
    heap: BinaryHeap<Candidate>,
}

/// Weighted fair scheduler with one logical queue per view group.
///
/// Ordering rules:
/// - Highest `PriorityGroup` wins globally.
/// - Within the same tier, each view group exposes its best local candidate.
/// - Between groups at the same tier, weighted fair queueing is approximated via
///   per-group virtual time: each pop adds `1 / weight` to the selected group.
pub struct WeightedFairScheduler {
    groups: BTreeMap<u64, GroupQueue>,
    frame_index: u64,
}

impl WeightedFairScheduler {
    pub fn new() -> Self {
        Self {
            groups: BTreeMap::new(),
            frame_index: 0,
        }
    }

    fn normalize_virtual_time(&mut self) {
        let min = self
            .groups
            .values()
            .map(|group| group.virtual_time)
            .min_by(|a, b| a.total_cmp(b));

        if let Some(min_value) = min {
            if min_value > 0.0 {
                for group in self.groups.values_mut() {
                    group.virtual_time -= min_value;
                }
            }
        }
    }
}

impl Default for WeightedFairScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadScheduler for WeightedFairScheduler {
    fn push(&mut self, candidate: LoadCandidate) {
        let entry = self.groups.entry(candidate.view_group).or_default();
        entry.weight = candidate.priority.view_group_weight.max(1);
        entry.heap.push(Candidate(candidate));
    }

    fn pop(&mut self) -> Option<LoadCandidate> {
        let highest_group = self
            .groups
            .values()
            .filter_map(|group| {
                group
                    .heap
                    .peek()
                    .map(|candidate| candidate.priority_group())
            })
            .max()?;

        let mut best_group_id = None;
        let mut best_virtual_time = 0.0;
        let mut best_score = 0i64;

        for (&group_id, group) in &self.groups {
            let Some(head) = group.heap.peek() else {
                continue;
            };
            if head.priority_group() != highest_group {
                continue;
            }

            let replace = match best_group_id {
                None => true,
                Some(current_group_id) => {
                    let vt_cmp = group.virtual_time.total_cmp(&best_virtual_time);
                    if vt_cmp == Ordering::Less {
                        true
                    } else if vt_cmp == Ordering::Equal {
                        if head.0.priority.score < best_score {
                            true
                        } else if head.0.priority.score == best_score {
                            group_id < current_group_id
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
            };

            if replace {
                best_group_id = Some(group_id);
                best_virtual_time = group.virtual_time;
                best_score = head.0.priority.score;
            }
        }

        let group_id = best_group_id?;
        let group = self.groups.get_mut(&group_id)?;
        let candidate = group.heap.pop()?.0;
        group.virtual_time += 1.0 / f64::from(group.weight.max(1));
        Some(candidate)
    }

    fn tick(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.normalize_virtual_time();
    }

    fn clear(&mut self) {
        for group in self.groups.values_mut() {
            group.heap.clear();
        }
    }

    fn len(&self) -> usize {
        self.groups.values().map(|group| group.heap.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::{LoadScheduler, WeightedFairScheduler};
    use crate::load::{ContentKey, LoadCandidate, LoadPriority, PriorityGroup};

    fn candidate(
        node_id: u64,
        view_group: u64,
        weight: u16,
        group: PriorityGroup,
        score: i64,
    ) -> LoadCandidate {
        LoadCandidate {
            view_group,
            node_id,
            key: ContentKey(format!("node-{node_id}")),
            priority: LoadPriority {
                group,
                score,
                view_group_weight: weight,
            },
        }
    }

    #[test]
    fn urgent_candidates_beat_normal_candidates() {
        let mut scheduler = WeightedFairScheduler::new();
        scheduler.push(candidate(1, 10, 1, PriorityGroup::Normal, 0));
        scheduler.push(candidate(2, 20, 1, PriorityGroup::Urgent, 100));

        let popped = scheduler.pop().expect("expected candidate");
        assert_eq!(popped.node_id, 2);
        assert_eq!(popped.priority.group, PriorityGroup::Urgent);
    }

    #[test]
    fn weighted_groups_share_service_without_starvation() {
        let mut scheduler = WeightedFairScheduler::new();

        for node_id in 1..=6 {
            scheduler.push(candidate(node_id, 100, 2, PriorityGroup::Normal, 0));
        }
        for node_id in 101..=106 {
            scheduler.push(candidate(node_id, 200, 1, PriorityGroup::Normal, 0));
        }

        let popped_groups: Vec<u64> = (0..6)
            .map(|_| scheduler.pop().expect("expected candidate").view_group)
            .collect();

        let heavy_count = popped_groups.iter().filter(|&&id| id == 100).count();
        let light_count = popped_groups.iter().filter(|&&id| id == 200).count();

        assert_eq!(heavy_count, 4);
        assert_eq!(light_count, 2);
        assert!(popped_groups[..3].contains(&200));
    }
}
