//! Concurrent node content loader with priority-based scheduling.
//!
//! Manages bounded worker-thread requests for node geometry, textures, and
//! attributes. Dispatches work via the user-provided [`TaskProcessor`] and
//! collects results through a channel — no async runtime required.
//!
//! Follows cesium-native's loading pipeline:
//! 1. **Worker thread**: fetch bytes via [`AssetAccessor`] + [`ResourceUriResolver`] + decode
//! 2. **Worker thread**: [`PrepareRendererResources::prepare_in_load_thread`]
//! 3. **Main thread**: [`PrepareRendererResources::prepare_in_main_thread`]

use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use i3s_async::{
    AssetAccessor, ResourceUriResolver, TaskProcessor, TextureRequestFormat, block_on,
};
use i3s_geospatial::crs::CrsTransform;
use i3s_reader::attribute::{AttributeValueType, parse_attribute_buffer};
use i3s_reader::geometry::{GeometryLayout, parse_geometry_buffer};
use i3s_util::Result;

use crate::content::NodeContent;
use crate::prepare::{PrepareRendererResources, RendererResources};
use crate::update_result::LoadPriority;

/// Describes a single attribute to load for each node.
#[derive(Debug, Clone)]
pub struct AttributeInfo {
    /// The attribute's storage key (used as the ID for `provider.attribute()`).
    pub attribute_id: u32,
    /// The value type for decoding.
    pub value_type: AttributeValueType,
}

/// A request to load node content, ordered by priority group then screen size.
#[derive(Debug, Clone)]
struct LoadRequest {
    node_id: u32,
    /// Priority group: Urgent > Normal > Preload.
    priority_group: LoadPriority,
    /// Larger projected screen size = higher priority within a group.
    screen_size: f64,
}

impl PartialEq for LoadRequest {
    fn eq(&self, other: &Self) -> bool {
        self.node_id == other.node_id
    }
}

impl Eq for LoadRequest {}

impl PartialOrd for LoadRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LoadRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority_group
            .cmp(&other.priority_group)
            .then_with(|| {
                self.screen_size
                    .partial_cmp(&other.screen_size)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

/// Result of a completed load-thread phase.
pub struct LoadResult {
    /// The node that was loaded.
    pub node_id: u32,
    /// The decoded content, or an error.
    pub result: Result<NodeContent>,
    /// Renderer resources from `prepare_in_load_thread` (if any).
    pub load_thread_resources: Option<RendererResources>,
}

/// Tracks an in-flight worker task with a cancellation flag.
struct InFlightTask {
    node_id: u32,
    cancelled: Arc<AtomicBool>,
}

/// Manages concurrent node content fetches with a bounded work queue.
///
/// Each frame, the selection algorithm produces load requests. The loader
/// queues them by priority (projected screen size), then dispatches up to
/// `max_simultaneous_loads` tasks through the [`TaskProcessor`].
///
/// Results flow back via an internal channel. Call:
/// 1. [`request`](Self::request) — enqueue nodes for loading
/// 2. [`dispatch`](Self::dispatch) — submit tasks to the TaskProcessor
/// 3. [`collect_completed`](Self::collect_completed) — harvest finished results (sync)
pub struct NodeContentLoader<A: AssetAccessor + 'static> {
    accessor: Arc<A>,
    resolver: Arc<dyn ResourceUriResolver>,
    task_processor: Arc<dyn TaskProcessor>,
    prepare_renderer: Arc<dyn PrepareRendererResources>,
    crs_transform: Option<Arc<dyn CrsTransform>>,
    max_simultaneous: usize,
    pending: BinaryHeap<LoadRequest>,
    in_flight: Vec<InFlightTask>,
    /// Channel for completed load results.
    result_sender: mpsc::Sender<LoadResult>,
    result_receiver: Mutex<mpsc::Receiver<LoadResult>>,
    layout: GeometryLayout,
    attribute_infos: Arc<Vec<AttributeInfo>>,
}

impl<A: AssetAccessor + 'static> NodeContentLoader<A> {
    /// Create a new loader.
    pub fn new(
        accessor: Arc<A>,
        resolver: Arc<dyn ResourceUriResolver>,
        task_processor: Arc<dyn TaskProcessor>,
        prepare_renderer: Arc<dyn PrepareRendererResources>,
        crs_transform: Option<Arc<dyn CrsTransform>>,
        max_simultaneous: usize,
        layout: GeometryLayout,
        attribute_infos: Vec<AttributeInfo>,
    ) -> Self {
        let (result_sender, result_receiver) = mpsc::channel();
        Self {
            accessor,
            resolver,
            task_processor,
            prepare_renderer,
            crs_transform,
            max_simultaneous,
            pending: BinaryHeap::new(),
            in_flight: Vec::new(),
            result_sender,
            result_receiver: Mutex::new(result_receiver),
            layout,
            attribute_infos: Arc::new(attribute_infos),
        }
    }

    /// Enqueue a node for loading with priority group and projected screen size.
    pub fn request(&mut self, node_id: u32, priority_group: LoadPriority, screen_size: f64) {
        self.pending.push(LoadRequest {
            node_id,
            priority_group,
            screen_size,
        });
    }

    /// Dispatch up to `max_simultaneous - in_flight` tasks to the TaskProcessor.
    ///
    /// Call this once per frame after enqueueing requests.
    pub fn dispatch(&mut self) {
        while self.in_flight.len() < self.max_simultaneous {
            let req = match self.pending.pop() {
                Some(r) => r,
                None => break,
            };

            let cancelled = Arc::new(AtomicBool::new(false));
            let cancel_flag = Arc::clone(&cancelled);
            let accessor = Arc::clone(&self.accessor);
            let resolver = Arc::clone(&self.resolver);
            let layout = self.layout.clone();
            let attr_infos = Arc::clone(&self.attribute_infos);
            let prepare_renderer = Arc::clone(&self.prepare_renderer);
            let crs_xform = self.crs_transform.clone();
            let sender = self.result_sender.clone();
            let node_id = req.node_id;

            self.task_processor.start_task(Box::new(move || {
                // Check cancellation before starting work
                if cancel_flag.load(Ordering::Relaxed) {
                    return;
                }

                // Phase 1: Fetch + decode (worker thread)
                let result =
                    load_node_content(&*accessor, &*resolver, node_id, &layout, &attr_infos);

                if cancel_flag.load(Ordering::Relaxed) {
                    return;
                }

                // Phase 2: prepare_in_load_thread (worker thread)
                let load_thread_resources = match &result {
                    Ok(content) => prepare_renderer.prepare_in_load_thread(
                        node_id,
                        content,
                        crs_xform.as_ref(),
                    ),
                    Err(_) => None,
                };

                // Send result to main thread for phase 3
                let _ = sender.send(LoadResult {
                    node_id,
                    result,
                    load_thread_resources,
                });
            }));

            self.in_flight.push(InFlightTask { node_id, cancelled });
        }
    }

    /// Collect completed loads without blocking (sync).
    ///
    /// Drains the result channel. Call this on the main thread each frame.
    pub fn collect_completed(&mut self) -> Vec<LoadResult> {
        let mut completed = Vec::new();
        let rx = self.result_receiver.lock().unwrap();
        while let Ok(result) = rx.try_recv() {
            // Remove from in_flight
            self.in_flight.retain(|t| t.node_id != result.node_id);
            completed.push(result);
        }
        drop(rx);
        completed
    }

    /// Cancel all in-flight tasks and clear the pending queue.
    ///
    /// Sets the cancel flag on all in-flight tasks. Worker threads will
    /// check this flag and skip work. The tasks may still complete but
    /// their results will be discarded next frame.
    ///
    /// Returns the node IDs of cancelled in-flight tasks.
    pub fn cancel_all(&mut self) -> Vec<u32> {
        self.pending.clear();
        let cancelled: Vec<u32> = self.in_flight.iter().map(|t| t.node_id).collect();
        for task in &self.in_flight {
            task.cancelled.store(true, Ordering::Relaxed);
        }
        self.in_flight.clear();
        // Drain any results that arrived for now-cancelled tasks
        let rx = self.result_receiver.lock().unwrap();
        while rx.try_recv().is_ok() {}
        drop(rx);
        cancelled
    }

    /// Cancel in-flight tasks for specific nodes.
    pub fn cancel_nodes(&mut self, node_ids: &[u32]) {
        self.in_flight.retain(|task| {
            if node_ids.contains(&task.node_id) {
                task.cancelled.store(true, Ordering::Relaxed);
                false
            } else {
                true
            }
        });
    }

    /// Number of in-flight tasks.
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    /// Number of pending (not yet dispatched) requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Clear all pending requests (e.g., on camera jump).
    pub fn clear_pending(&mut self) {
        self.pending.clear();
    }
}

/// Fetch and decode all content for a node (runs on a worker thread).
///
/// Uses [`block_on`](i3s_async::block_on) to drive the accessor's async methods.
fn load_node_content<A: AssetAccessor>(
    accessor: &A,
    resolver: &dyn ResourceUriResolver,
    node_id: u32,
    layout: &GeometryLayout,
    attribute_infos: &[AttributeInfo],
) -> Result<NodeContent> {
    // Fetch geometry (geometry ID 0 is the standard single geometry)
    let geo_uri = resolver.geometry_uri(node_id, 0);
    let geo_bytes = block_on(accessor.get(&geo_uri))?.into_data()?;
    let geometry = parse_geometry_buffer(&geo_bytes, layout)?;

    // Fetch texture (best-effort; node may not have textures)
    let tex_uri = resolver.texture_uri(node_id, 0, TextureRequestFormat::Jpeg);
    let texture_data = match block_on(accessor.get(&tex_uri)).and_then(|r| r.into_data()) {
        Ok(bytes) => bytes,
        Err(_) => Vec::new(),
    };

    // Fetch attributes (best-effort per attribute)
    let mut attributes = Vec::with_capacity(attribute_infos.len());
    for info in attribute_infos {
        let attr_uri = resolver.attribute_uri(node_id, info.attribute_id);
        match block_on(accessor.get(&attr_uri)).and_then(|r| r.into_data()) {
            Ok(bytes) => match parse_attribute_buffer(&bytes, info.value_type) {
                Ok(data) => attributes.push(data),
                Err(_) => {} // skip malformed attribute
            },
            Err(_) => {} // skip unavailable attribute
        }
    }

    let byte_size = NodeContent::estimate_byte_size(&geometry, &texture_data, &attributes);

    Ok(NodeContent {
        geometry,
        texture_data,
        attributes,
        byte_size,
    })
}
