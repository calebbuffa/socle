//! Rust wrapper types that implement selekt traits via FFI vtables.
//!
//! Each wrapper holds a vtable and implements the corresponding trait
//! by calling through the function pointers.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use orkester::{AsyncSystem, Future as OrkFuture};
use selekt::{
    ContentKey, ContentLoader, HierarchyPatch, HierarchyPatchError, HierarchyReference,
    HierarchyResolver, LoadedContent, LodDescriptor, LodEvaluator, NodeId, NodeKind,
    RefinementMode, RequestId, SpatialHierarchy, ViewState,
};
use zukei::bounds::SpatialBounds;
use zukei::math::Vec3;

use crate::selekt_view_state_t;
use crate::vtable::*;


/// Implements [`SpatialHierarchy`] by calling through a C vtable.
///
/// Caches results from callbacks so that returned `&T` references remain valid.
pub struct FfiHierarchy {
    vtable: selekt_hierarchy_vtable_t,
    // Caches for returning references — methods take &self but trait returns &T.
    bounds_cache: Mutex<HashMap<NodeId, SpatialBounds>>,
    lod_cache: Mutex<HashMap<NodeId, LodDescriptor>>,
    children_cache: Mutex<HashMap<NodeId, Vec<NodeId>>>,
    content_key_cache: Mutex<HashMap<NodeId, ContentKey>>,
    content_bounds_cache: Mutex<HashMap<NodeId, SpatialBounds>>,
}

impl FfiHierarchy {
    pub fn new(vtable: selekt_hierarchy_vtable_t) -> Self {
        Self {
            vtable,
            bounds_cache: Mutex::new(HashMap::new()),
            lod_cache: Mutex::new(HashMap::new()),
            children_cache: Mutex::new(HashMap::new()),
            content_key_cache: Mutex::new(HashMap::new()),
            content_bounds_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Invalidate caches for nodes affected by a patch.
    fn invalidate_for_patch(&self, parent: NodeId) {
        self.children_cache.lock().unwrap().remove(&parent);
    }
}

impl Drop for FfiHierarchy {
    fn drop(&mut self) {
        if let Some(destroy) = self.vtable.destroy {
            unsafe { destroy(self.vtable.ctx) };
        }
    }
}

// SAFETY: The vtable ctx is required to be thread-safe by the C consumer.
unsafe impl Send for FfiHierarchy {}
unsafe impl Sync for FfiHierarchy {}

impl SpatialHierarchy for FfiHierarchy {
    fn root(&self) -> NodeId {
        unsafe { (self.vtable.root)(self.vtable.ctx) }
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        let mut out: NodeId = 0;
        let has = unsafe { (self.vtable.parent)(self.vtable.ctx, node, &mut out) };
        if has { Some(out) } else { None }
    }

    fn children(&self, node: NodeId) -> &[NodeId] {
        let mut cache = self.children_cache.lock().unwrap();
        if !cache.contains_key(&node) {
            let mut ptr: *const NodeId = std::ptr::null();
            let mut len: usize = 0;
            unsafe {
                (self.vtable.children)(self.vtable.ctx, node, &mut ptr, &mut len);
            }
            let children = if ptr.is_null() || len == 0 {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec()
            };
            cache.insert(node, children);
        }
        // SAFETY: We hold the Mutex, and the data is in the cache.
        // The returned reference is valid as long as the HashMap entry exists.
        // The caller (the traversal) uses this within a single frame.
        let entry = cache.get(&node).unwrap();
        unsafe { std::slice::from_raw_parts(entry.as_ptr(), entry.len()) }
    }

    fn node_kind(&self, node: NodeId) -> NodeKind {
        unsafe { (self.vtable.node_kind)(self.vtable.ctx, node) }
    }

    fn bounds(&self, node: NodeId) -> &SpatialBounds {
        let mut cache = self.bounds_cache.lock().unwrap();
        if !cache.contains_key(&node) {
            let mut out = SpatialBounds::Sphere {
                center: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                radius: 0.0,
            };
            unsafe { (self.vtable.bounds)(self.vtable.ctx, node, &mut out) };
            cache.insert(node, out);
        }
        let ptr = cache.get(&node).unwrap() as *const SpatialBounds;
        unsafe { &*ptr }
    }

    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor {
        let mut cache = self.lod_cache.lock().unwrap();
        if !cache.contains_key(&node) {
            let mut desc = selekt_lod_descriptor_t {
                family_ptr: std::ptr::null(),
                family_len: 0,
                values_ptr: std::ptr::null(),
                values_len: 0,
            };
            unsafe {
                (self.vtable.lod_descriptor)(self.vtable.ctx, node, &mut desc);
            }
            let family = if desc.family_ptr.is_null() || desc.family_len == 0 {
                String::new()
            } else {
                let bytes = unsafe { std::slice::from_raw_parts(desc.family_ptr, desc.family_len) };
                String::from_utf8_lossy(bytes).into_owned()
            };
            let values = if desc.values_ptr.is_null() || desc.values_len == 0 {
                Vec::new()
            } else {
                unsafe { std::slice::from_raw_parts(desc.values_ptr, desc.values_len) }.to_vec()
            };
            cache.insert(node, LodDescriptor { family, values });
        }
        let ptr = cache.get(&node).unwrap() as *const LodDescriptor;
        unsafe { &*ptr }
    }

    fn refinement_mode(&self, node: NodeId) -> RefinementMode {
        unsafe { (self.vtable.refinement_mode)(self.vtable.ctx, node) }
    }

    fn content_bounds(&self, node: NodeId) -> Option<&SpatialBounds> {
        let mut cache = self.content_bounds_cache.lock().unwrap();
        if !cache.contains_key(&node) {
            let mut out = SpatialBounds::Sphere {
                center: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                radius: 0.0,
            };
            let has = unsafe { (self.vtable.content_bounds)(self.vtable.ctx, node, &mut out) };
            if has {
                cache.insert(node, out);
            } else {
                return None;
            }
        }
        cache.get(&node).map(|b| {
            let ptr = b as *const SpatialBounds;
            unsafe { &*ptr }
        })
    }

    fn content_key(&self, node: NodeId) -> Option<&ContentKey> {
        let mut cache = self.content_key_cache.lock().unwrap();
        if !cache.contains_key(&node) {
            let mut ptr: *const u8 = std::ptr::null();
            let mut len: usize = 0;
            let has =
                unsafe { (self.vtable.content_key)(self.vtable.ctx, node, &mut ptr, &mut len) };
            if has && !ptr.is_null() && len > 0 {
                let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
                let key = String::from_utf8_lossy(bytes).into_owned();
                cache.insert(node, ContentKey(key));
            } else {
                return None;
            }
        }
        cache.get(&node).map(|k| {
            let ptr = k as *const ContentKey;
            unsafe { &*ptr }
        })
    }

    fn apply_patch(&mut self, patch: HierarchyPatch) -> Result<(), HierarchyPatchError> {
        self.invalidate_for_patch(patch.parent);
        // Invalidate caches for all new nodes too.
        for &node in &patch.inserted_nodes {
            self.bounds_cache.get_mut().unwrap().remove(&node);
            self.lod_cache.get_mut().unwrap().remove(&node);
            self.children_cache.get_mut().unwrap().remove(&node);
            self.content_key_cache.get_mut().unwrap().remove(&node);
            self.content_bounds_cache.get_mut().unwrap().remove(&node);
        }
        let ok = unsafe {
            (self.vtable.apply_patch)(
                self.vtable.ctx,
                patch.parent,
                patch.inserted_nodes.as_ptr(),
                patch.inserted_nodes.len(),
            )
        };
        if ok {
            Ok(())
        } else {
            Err(HierarchyPatchError {
                message: "FFI apply_patch returned false".into(),
            })
        }
    }
}


/// Implements [`LodEvaluator`] by calling through a C vtable.
pub struct FfiLodEvaluator {
    vtable: selekt_lod_evaluator_vtable_t,
}

impl FfiLodEvaluator {
    pub fn new(vtable: selekt_lod_evaluator_vtable_t) -> Self {
        Self { vtable }
    }
}

impl Drop for FfiLodEvaluator {
    fn drop(&mut self) {
        if let Some(destroy) = self.vtable.destroy {
            unsafe { destroy(self.vtable.ctx) };
        }
    }
}

unsafe impl Send for FfiLodEvaluator {}
unsafe impl Sync for FfiLodEvaluator {}

impl LodEvaluator for FfiLodEvaluator {
    fn should_refine(
        &self,
        descriptor: &LodDescriptor,
        view: &ViewState,
        bounds: &SpatialBounds,
        mode: RefinementMode,
    ) -> bool {
        let ffi_desc = selekt_lod_descriptor_t {
            family_ptr: descriptor.family.as_ptr(),
            family_len: descriptor.family.len(),
            values_ptr: descriptor.values.as_ptr(),
            values_len: descriptor.values.len(),
        };
        let ffi_view = selekt_view_state_t {
            viewport_width: view.viewport_px[0],
            viewport_height: view.viewport_px[1],
            position: [view.position.x, view.position.y, view.position.z],
            direction: [view.direction.x, view.direction.y, view.direction.z],
            up: [view.up.x, view.up.y, view.up.z],
            fov_x: view.fov_x,
            fov_y: view.fov_y,
            lod_metric_multiplier: view.lod_metric_multiplier,
        };
        unsafe { (self.vtable.should_refine)(self.vtable.ctx, &ffi_desc, &ffi_view, bounds, mode) }
    }
}


/// Content wrapper that calls a C destroy callback on drop.
pub struct FfiContent {
    pub ptr: *mut c_void,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

unsafe impl Send for FfiContent {}

impl Drop for FfiContent {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy {
            if !self.ptr.is_null() {
                unsafe { destroy(self.ptr) };
            }
        }
    }
}

/// Internal state backing a `selekt_load_delivery_t` handle.
pub(crate) struct LoadDelivery {
    pub slot: Arc<Mutex<Option<Result<LoadedContent<FfiContent>, String>>>>,
    pub promise: orkester::Promise<()>,
}


/// Implements [`ContentLoader<FfiContent>`] by calling through a C vtable.
pub struct FfiContentLoader {
    vtable: selekt_content_loader_vtable_t,
    content_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    async_system: AsyncSystem,
}

impl FfiContentLoader {
    pub fn new(
        vtable: selekt_content_loader_vtable_t,
        content_destroy: Option<unsafe extern "C" fn(*mut c_void)>,
        async_system: AsyncSystem,
    ) -> Self {
        Self {
            vtable,
            content_destroy,
            async_system,
        }
    }
}

impl Drop for FfiContentLoader {
    fn drop(&mut self) {
        if let Some(destroy) = self.vtable.destroy {
            unsafe { destroy(self.vtable.ctx) };
        }
    }
}

unsafe impl Send for FfiContentLoader {}
unsafe impl Sync for FfiContentLoader {}

/// Error type for FFI content loading.
#[derive(Debug)]
pub struct FfiLoadError(pub String);

impl std::fmt::Display for FfiLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FfiLoadError {}

impl ContentLoader<FfiContent> for FfiContentLoader {
    type Error = FfiLoadError;

    fn request(
        &self,
        _async_system: &AsyncSystem,
        node_id: NodeId,
        key: &ContentKey,
        priority: selekt::LoadPriority,
    ) -> (
        RequestId,
        OrkFuture<Result<LoadedContent<FfiContent>, Self::Error>>,
    ) {
        // Create a typed promise/future pair for the delivery signal.
        let (signal_promise, signal_future) = self.async_system.promise();

        // Slot for the C side to deposit the loaded content.
        let slot: Arc<Mutex<Option<Result<LoadedContent<FfiContent>, String>>>> =
            Arc::new(Mutex::new(None));

        let delivery = Box::new(LoadDelivery {
            slot: slot.clone(),
            promise: signal_promise,
        });
        let delivery_ptr = Box::into_raw(delivery) as *mut selekt_load_delivery_t;

        let ffi_priority = selekt_load_priority_t::from(priority);

        let request_id = unsafe {
            (self.vtable.request)(
                self.vtable.ctx,
                delivery_ptr,
                node_id,
                key.0.as_ptr(),
                key.0.len(),
                ffi_priority,
            )
        };

        // When the signal fires, read the slot.
        let content_destroy = self.content_destroy;
        let _ = content_destroy; // will be captured in resolve path
        let result_future = signal_future.map(move |_| {
            let value = slot
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Err("FFI load delivery was never resolved".into()));
            value.map_err(FfiLoadError)
        });

        (request_id, result_future)
    }

    fn cancel(&self, request_id: RequestId) -> bool {
        unsafe { (self.vtable.cancel)(self.vtable.ctx, request_id) }
    }
}


/// Internal state backing a `selekt_hierarchy_delivery_t` handle.
pub(crate) struct HierarchyDelivery {
    pub slot: Arc<Mutex<Option<Result<Option<HierarchyPatch>, String>>>>,
    pub promise: orkester::Promise<()>,
}


/// Implements [`HierarchyResolver`] by calling through a C vtable.
pub struct FfiHierarchyResolver {
    vtable: selekt_hierarchy_resolver_vtable_t,
    async_system: AsyncSystem,
}

impl FfiHierarchyResolver {
    pub fn new(vtable: selekt_hierarchy_resolver_vtable_t, async_system: AsyncSystem) -> Self {
        Self {
            vtable,
            async_system,
        }
    }
}

impl Drop for FfiHierarchyResolver {
    fn drop(&mut self) {
        if let Some(destroy) = self.vtable.destroy {
            unsafe { destroy(self.vtable.ctx) };
        }
    }
}

unsafe impl Send for FfiHierarchyResolver {}
unsafe impl Sync for FfiHierarchyResolver {}

impl HierarchyResolver for FfiHierarchyResolver {
    type Error = FfiLoadError;

    fn resolve_reference(
        &self,
        _async_system: &AsyncSystem,
        reference: HierarchyReference,
    ) -> OrkFuture<Result<Option<HierarchyPatch>, Self::Error>> {
        let (signal_promise, signal_future) = self.async_system.promise();

        let slot: Arc<Mutex<Option<Result<Option<HierarchyPatch>, String>>>> =
            Arc::new(Mutex::new(None));

        let delivery = Box::new(HierarchyDelivery {
            slot: slot.clone(),
            promise: signal_promise,
        });
        let delivery_ptr = Box::into_raw(delivery) as *mut selekt_hierarchy_delivery_t;

        // Convert transform to f64 array if present.
        let has_transform = reference.transform.is_some();
        let transform_data: [f64; 16] = reference
            .transform
            .map(|m| {
                // Column-major: cols[0].x, cols[0].y, ...
                let c = m.cols;
                [
                    c[0].x, c[0].y, c[0].z, c[0].w, c[1].x, c[1].y, c[1].z, c[1].w, c[2].x, c[2].y,
                    c[2].z, c[2].w, c[3].x, c[3].y, c[3].z, c[3].w,
                ]
            })
            .unwrap_or([0.0; 16]);

        unsafe {
            (self.vtable.resolve)(
                self.vtable.ctx,
                delivery_ptr,
                reference.key.0.as_ptr(),
                reference.key.0.len(),
                reference.source,
                has_transform,
                &transform_data,
            );
        }

        signal_future.map(move |_| {
            let value = slot
                .lock()
                .unwrap()
                .take()
                .unwrap_or_else(|| Err("FFI hierarchy delivery was never resolved".into()));
            value.map_err(FfiLoadError)
        })
    }
}
