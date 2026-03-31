//! Pure mutable frame-state for [`SelectionEngine`](crate::SelectionEngine).
//!
//! [`EngineState`] holds everything that changes frame-to-frame: node lifecycles,
//! resident content, view-group camera tracking, and reusable scratch buffers.
//! It contains no async handles, no trait-object plugins, and no thread contexts —
//! making it the natural input/output type for the pure `step()` function.

use std::collections::HashMap;
use std::time::Instant;

use glam::DVec3;

use crate::frame::FrameResult;
use crate::lod_threshold::LodThreshold;
use crate::node::{NodeId, NodeStateVec};
use crate::traversal::TraversalBuffers;
use crate::scheduler::WeightedFairScheduler;
use crate::view::{ViewGroupHandle, ViewState};


pub(crate) struct ResidentContent<C> {
    pub content: Option<C>,
    pub byte_size: usize,
}

/// Decoded content keyed by node ID, with running byte total.
pub(crate) struct ResidentStore<C> {
    pub map: HashMap<NodeId, ResidentContent<C>>,
    pub total_bytes: usize,
}

impl<C> ResidentStore<C> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            total_bytes: 0,
        }
    }

    pub fn insert(&mut self, node_id: NodeId, content: Option<C>, byte_size: usize) {
        if let Some(prev) = self.map.insert(node_id, ResidentContent { content, byte_size }) {
            self.total_bytes = self.total_bytes.saturating_sub(prev.byte_size);
        }
        self.total_bytes = self.total_bytes.saturating_add(byte_size);
    }

    pub fn remove(&mut self, node_id: NodeId) -> Option<ResidentContent<C>> {
        let removed = self.map.remove(&node_id)?;
        self.total_bytes = self.total_bytes.saturating_sub(removed.byte_size);
        Some(removed)
    }

    pub fn content(&self, node_id: NodeId) -> Option<&C> {
        self.map.get(&node_id).and_then(|r| r.content.as_ref())
    }

    pub fn content_mut(&mut self, node_id: NodeId) -> Option<&mut C> {
        self.map.get_mut(&node_id).and_then(|r| r.content.as_mut())
    }
}


pub(crate) struct ViewGroupSlot {
    pub generation: u32,
    pub weight: f64,
    pub active: bool,
    pub result: FrameResult,
    /// Instant of the last call to `update_view_group` for this slot.
    pub last_update: Option<Instant>,
    /// Accumulated seconds since the camera last moved (for foveated rendering).
    pub camera_stationary_seconds: f32,
    pub last_primary_position: DVec3,
    pub last_primary_direction: DVec3,
    pub last_view_count: usize,
    pub camera_velocity: DVec3,
}

impl ViewGroupSlot {
    fn new(generation: u32, weight: f64) -> Self {
        Self {
            generation,
            weight,
            active: true,
            result: FrameResult::default(),
            last_update: None,
            camera_stationary_seconds: 0.0,
            last_primary_position: DVec3::ZERO,
            last_primary_direction: DVec3::ZERO,
            last_view_count: 0,
            camera_velocity: DVec3::ZERO,
        }
    }
}

impl ViewGroupSlot {
    /// Update camera-tracking fields and return `(stationary_secs, velocity)`.
    pub fn tick_camera(&mut self, views: &[ViewState]) -> (f32, DVec3) {
        let now = Instant::now();
        let delta_time = self
            .last_update
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.0);

        let camera_moved = self.last_view_count != views.len()
            || (views[0].position - self.last_primary_position).length_squared() > 1e-10
            || (views[0].direction - self.last_primary_direction).length_squared() > 1e-10;

        let camera_stationary_seconds = if camera_moved {
            0.0f32
        } else {
            self.camera_stationary_seconds + delta_time
        };

        let camera_velocity = if camera_moved && self.last_view_count > 0 && delta_time > 1e-6 {
            (views[0].position - self.last_primary_position) / delta_time as f64
        } else if camera_moved {
            DVec3::ZERO
        } else {
            self.camera_velocity
        };

        self.last_update = Some(now);
        self.camera_stationary_seconds = camera_stationary_seconds;
        self.camera_velocity = camera_velocity;
        self.last_primary_position = views[0].position;
        self.last_primary_direction = views[0].direction;
        self.last_view_count = views.len();

        (camera_stationary_seconds, camera_velocity)
    }
}

pub(crate) struct ViewGroupTable {
    pub slots: Vec<ViewGroupSlot>,
    pub next_generation: u32,
}

impl ViewGroupTable {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            next_generation: 1,
        }
    }

    pub fn insert(&mut self, weight: f64) -> ViewGroupHandle {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1);

        if let Some((index, slot)) = self.slots.iter_mut().enumerate().find(|(_, s)| !s.active) {
            *slot = ViewGroupSlot::new(generation, weight);
            return ViewGroupHandle { index: index as u32, generation };
        }

        let index = self.slots.len() as u32;
        self.slots.push(ViewGroupSlot::new(generation, weight));
        ViewGroupHandle { index, generation }
    }

    pub fn remove(&mut self, handle: ViewGroupHandle) -> bool {
        if let Some(slot) = self.slots.get_mut(handle.index as usize) {
            if slot.active && slot.generation == handle.generation {
                slot.active = false;
                return true;
            }
        }
        false
    }

    pub fn get(&self, handle: ViewGroupHandle) -> Option<&ViewGroupSlot> {
        self.slots
            .get(handle.index as usize)
            .filter(|s| s.active && s.generation == handle.generation)
    }

    pub fn get_mut(&mut self, handle: ViewGroupHandle) -> Option<&mut ViewGroupSlot> {
        self.slots
            .get_mut(handle.index as usize)
            .filter(|s| s.active && s.generation == handle.generation)
    }
}


/// Pure mutable frame-state.
///
/// Contains everything that changes from frame to frame: node lifecycles,
/// resident content, view-group camera data, and reusable scratch allocations.
/// Has no async handles, no trait-objects, and no thread contexts.
pub(crate) struct EngineState<C: Send + 'static> {
    /// Per-node lifecycle + importance + retry state.
    pub node_states: NodeStateVec,
    /// Decoded content resident in memory.
    pub resident: ResidentStore<C>,
    /// Monotonically increasing frame counter.
    pub frame_index: u64,
    /// Budget-aware LOD multiplier with hysteresis.
    pub lod_threshold: LodThreshold,
    /// Per-view-group camera tracking and last frame results.
    pub view_groups: ViewGroupTable,
    /// Epoch for `NodeState::loaded_epoch_secs` timestamps.
    pub load_epoch: Instant,

    /// DFS traversal scratch — cleared each frame, capacity retained.
    pub traversal_buffers: TraversalBuffers,
    /// Reusable per-frame load scheduler (retains group allocations between frames).
    pub scheduler: WeightedFairScheduler,
}

impl<C: Send + 'static> EngineState<C> {
    pub fn new() -> Self {
        Self {
            node_states: NodeStateVec::new(),
            resident: ResidentStore::new(),
            frame_index: 0,
            lod_threshold: LodThreshold::new(1.0),
            view_groups: ViewGroupTable::new(),
            load_epoch: Instant::now(),
            traversal_buffers: TraversalBuffers::new(),
            scheduler: WeightedFairScheduler::new(),
        }
    }
}
