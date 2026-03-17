//! Memory-budgeted LRU cache for loaded node content.
//!
//! Tracks per-node content size and evicts least-recently-used nodes when
//! the total memory budget is exceeded.
//!
//! Uses a `BTreeMap<u64, u32>` as an access-order index for O(log n)
//! eviction instead of scanning all entries.

use std::collections::{BTreeMap, HashMap};

use crate::content::NodeContent;

/// Memory-budgeted cache for loaded node content.
///
/// Nodes are inserted with their decoded content and tracked by byte size.
/// When the total exceeds the budget, the least-recently-accessed nodes
/// are evicted first.
pub struct NodeCache {
    entries: HashMap<u32, CacheEntry>,
    /// Maps `last_access` → `node_id` for O(log n) LRU eviction.
    lru_order: BTreeMap<u64, u32>,
    /// Total byte size of all cached content.
    total_bytes: usize,
    /// Maximum allowed byte size.
    budget: usize,
    /// Monotonically increasing access counter for LRU ordering.
    access_counter: u64,
}

struct CacheEntry {
    content: NodeContent,
    last_access: u64,
}

impl NodeCache {
    /// Create a cache with the given memory budget in bytes.
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: BTreeMap::new(),
            total_bytes: 0,
            budget,
            access_counter: 0,
        }
    }

    /// Insert node content into the cache.
    ///
    /// If the node is already cached, replaces its content. If the total
    /// budget is exceeded after insertion, returns the IDs of evicted nodes.
    pub fn insert(&mut self, node_id: u32, content: NodeContent) -> Vec<u32> {
        self.access_counter += 1;
        let access = self.access_counter;

        // Remove existing entry if present
        if let Some(old) = self.entries.remove(&node_id) {
            self.total_bytes -= old.content.byte_size;
            self.lru_order.remove(&old.last_access);
        }

        self.total_bytes += content.byte_size;
        self.lru_order.insert(access, node_id);
        self.entries.insert(
            node_id,
            CacheEntry {
                content,
                last_access: access,
            },
        );

        self.evict_if_needed()
    }

    /// Get a reference to cached node content.
    ///
    /// Updates the LRU access time.
    pub fn get(&mut self, node_id: u32) -> Option<&NodeContent> {
        self.access_counter += 1;
        let new_access = self.access_counter;
        let entry = self.entries.get_mut(&node_id)?;
        // Update LRU order
        self.lru_order.remove(&entry.last_access);
        entry.last_access = new_access;
        self.lru_order.insert(new_access, node_id);
        Some(&entry.content)
    }

    /// Check if a node is in the cache without updating access time.
    pub fn contains(&self, node_id: u32) -> bool {
        self.entries.contains_key(&node_id)
    }

    /// Remove a specific node from the cache.
    pub fn remove(&mut self, node_id: u32) -> Option<NodeContent> {
        self.entries.remove(&node_id).map(|e| {
            self.total_bytes -= e.content.byte_size;
            self.lru_order.remove(&e.last_access);
            e.content
        })
    }

    /// Current total byte size of cached content.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Number of cached nodes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Evict LRU nodes until total_bytes <= budget. Returns evicted node IDs.
    fn evict_if_needed(&mut self) -> Vec<u32> {
        let mut evicted = Vec::new();

        while self.total_bytes > self.budget && !self.lru_order.is_empty() {
            // Pop the entry with the smallest access counter (LRU)
            let (&access, &lru_id) = self.lru_order.iter().next().unwrap();
            self.lru_order.remove(&access);

            if let Some(entry) = self.entries.remove(&lru_id) {
                self.total_bytes -= entry.content.byte_size;
                evicted.push(lru_id);
            }
        }

        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i3s_reader::geometry::GeometryData;

    fn make_content(byte_size: usize) -> NodeContent {
        NodeContent {
            geometry: GeometryData::default(),
            texture_data: Vec::new(),
            attributes: Vec::new(),
            byte_size,
        }
    }

    #[test]
    fn insert_and_get() {
        let mut cache = NodeCache::new(1024);
        cache.insert(1, make_content(100));
        assert!(cache.contains(1));
        assert_eq!(cache.total_bytes(), 100);
        assert!(cache.get(1).is_some());
    }

    #[test]
    fn eviction_when_over_budget() {
        let mut cache = NodeCache::new(200);
        cache.insert(1, make_content(100));
        cache.insert(2, make_content(100));
        // At budget exactly — no eviction
        assert_eq!(cache.len(), 2);

        // This pushes over budget (300 > 200)
        let evicted = cache.insert(3, make_content(100));
        // At least one node evicted
        assert!(!evicted.is_empty());
        assert!(cache.total_bytes() <= 200);
    }

    #[test]
    fn lru_evicts_oldest() {
        let mut cache = NodeCache::new(250);
        cache.insert(1, make_content(100));
        cache.insert(2, make_content(100));

        // Access node 1 to make it most recent
        cache.get(1);

        // Insert node 3 — pushes over budget, should evict node 2 (LRU)
        let evicted = cache.insert(3, make_content(100));
        assert!(evicted.contains(&2));
        assert!(!evicted.contains(&1));
        assert!(cache.contains(1));
        assert!(cache.contains(3));
    }

    #[test]
    fn remove_frees_space() {
        let mut cache = NodeCache::new(1024);
        cache.insert(1, make_content(500));
        assert_eq!(cache.total_bytes(), 500);

        cache.remove(1);
        assert_eq!(cache.total_bytes(), 0);
        assert!(!cache.contains(1));
    }

    #[test]
    fn replace_updates_size() {
        let mut cache = NodeCache::new(1024);
        cache.insert(1, make_content(100));
        assert_eq!(cache.total_bytes(), 100);

        cache.insert(1, make_content(200));
        assert_eq!(cache.total_bytes(), 200);
        assert_eq!(cache.len(), 1);
    }
}
