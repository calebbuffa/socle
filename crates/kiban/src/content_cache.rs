//! An in-memory budget-managed cache for resolved tile content.
//!
//! [`ContentCache`] collapses what used to be three separate fields
//! (`content`, `content_bytes`, `total_bytes`) into a single structure
//! whose eviction logic is self-contained: call [`ContentCache::evict`]
//! and it will discard the least-important non-pinned entries until the
//! cache is within budget, returning the IDs it removed.

use std::collections::{HashMap, HashSet};

use selekt::NodeId;

struct Entry<C> {
    content: C,
    byte_size: usize,
}

/// An in-memory cache of rendered tile content with an eviction budget.
pub struct ContentCache<C> {
    entries: HashMap<NodeId, Entry<C>>,
    total_bytes: usize,
    max_bytes: usize,
}

impl<C> ContentCache<C> {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            total_bytes: 0,
            max_bytes,
        }
    }

    pub fn insert(&mut self, id: NodeId, content: C, byte_size: usize) {
        if let Some(old) = self.entries.insert(id, Entry { content, byte_size }) {
            self.total_bytes = self.total_bytes.saturating_sub(old.byte_size);
        }
        self.total_bytes += byte_size;
    }

    /// Remove an entry. Returns the content and byte size, if present.
    pub fn remove(&mut self, id: NodeId) -> Option<(C, usize)> {
        let entry = self.entries.remove(&id)?;
        self.total_bytes = self.total_bytes.saturating_sub(entry.byte_size);
        Some((entry.content, entry.byte_size))
    }

    pub fn get(&self, id: NodeId) -> Option<&C> {
        self.entries.get(&id).map(|e| &e.content)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut C> {
        self.entries.get_mut(&id).map(|e| &mut e.content)
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.entries.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn is_over_budget(&self) -> bool {
        self.total_bytes > self.max_bytes
    }

    /// Update the max budget. Does not immediately evict.
    pub fn set_max_bytes(&mut self, max_bytes: usize) {
        self.max_bytes = max_bytes;
    }

    /// Evict least-important non-pinned entries until within budget.
    ///
    /// - `pinned`: IDs that must not be evicted (e.g. the current render set).
    /// - `importance`: returns a score for each candidate; lower = evict first.
    ///
    /// Returns the IDs that were evicted.
    pub fn evict(
        &mut self,
        pinned: &HashSet<NodeId>,
        importance: impl Fn(NodeId) -> f32,
    ) -> Vec<NodeId> {
        if !self.is_over_budget() {
            return Vec::new();
        }

        let mut candidates: Vec<(NodeId, f32)> = self
            .entries
            .keys()
            .copied()
            .filter(|id| !pinned.contains(id))
            .map(|id| (id, importance(id)))
            .collect();

        candidates
            .sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut evicted = Vec::new();
        for (id, _) in candidates {
            if !self.is_over_budget() {
                break;
            }
            self.remove(id);
            evicted.push(id);
        }
        evicted
    }
}
