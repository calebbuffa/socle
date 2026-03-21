//! C-ABI FFI for selekt's tile selection engine.
//!
//! All types use `selekt_*_t` snake_case naming with `_t` suffixes.
//! All functions use a `selekt_` prefix.
//! All callbacks use `void(*)(void*)`.
//!
//! **No format-specific concepts** (3D Tiles, I3S, glTF) live here.
//! Those belong in consumer crates that depend on both `selekt` and `selekt-ffi`.

// C-style snake_case naming is intentional for FFI types.
#![allow(non_camel_case_types)]

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use selekt::{NodeId, ViewState};

pub mod vtable;
pub mod wrappers;

pub use vtable::*;
pub use wrappers::{
    FfiContent, FfiContentLoader, FfiHierarchy, FfiHierarchyResolver, FfiLodEvaluator,
};


/// Opaque engine handle exposed to C. Wraps a type-erased `SelectionEngine`.
#[repr(C)]
pub struct selekt_engine_t {
    _private: [u8; 0],
}

/// Identifies a view group managed by the engine.
#[repr(C)]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct selekt_view_group_handle_t {
    pub index: u32,
    pub generation: u32,
}


/// C-ABI camera state for tile selection.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct selekt_view_state_t {
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub position: [f64; 3],
    pub direction: [f64; 3],
    pub up: [f64; 3],
    pub fov_x: f64,
    pub fov_y: f64,
    pub lod_metric_multiplier: f32,
}

impl From<&selekt_view_state_t> for ViewState {
    fn from(ffi: &selekt_view_state_t) -> Self {
        use zukei::math::Vec3;

        Self {
            viewport_px: [ffi.viewport_width, ffi.viewport_height],
            position: Vec3 {
                x: ffi.position[0],
                y: ffi.position[1],
                z: ffi.position[2],
            },
            direction: Vec3 {
                x: ffi.direction[0],
                y: ffi.direction[1],
                z: ffi.direction[2],
            },
            up: Vec3 {
                x: ffi.up[0],
                y: ffi.up[1],
                z: ffi.up[2],
            },
            fov_x: ffi.fov_x,
            fov_y: ffi.fov_y,
            lod_metric_multiplier: ffi.lod_metric_multiplier,
        }
    }
}


/// Result of `selekt_engine_update_view_group`.
#[repr(C)]
pub struct selekt_view_update_result_t {
    /// Pointer to array of selected node IDs.
    pub selected_ptr: *const NodeId,
    /// Number of selected nodes.
    pub selected_len: usize,
    /// Total nodes visited during traversal.
    pub visited: usize,
    /// Total nodes culled by visibility.
    pub culled: usize,
    /// Number of new load requests queued.
    pub queued_requests: usize,
    /// Number of nodes in the worker thread load queue.
    pub worker_thread_load_queue_length: usize,
    /// Number of nodes in the main thread load queue.
    pub main_thread_load_queue_length: usize,
    /// Monotonically increasing frame counter.
    pub frame_number: u64,
}

/// Result of `selekt_engine_load` or `selekt_engine_dispatch_main_thread_tasks`.
#[repr(C)]
pub struct selekt_load_pass_result_t {
    pub started_requests: usize,
    pub completed_main_thread_tasks: usize,
    pub pending_worker_queue: usize,
    pub pending_main_queue: usize,
}


impl From<selekt::ViewGroupHandle> for selekt_view_group_handle_t {
    fn from(h: selekt::ViewGroupHandle) -> Self {
        Self {
            index: h.index,
            generation: h.generation,
        }
    }
}

impl From<selekt_view_group_handle_t> for selekt::ViewGroupHandle {
    fn from(h: selekt_view_group_handle_t) -> Self {
        Self {
            index: h.index,
            generation: h.generation,
        }
    }
}


/// Type-erased selection engine interface for FFI.
///
/// Any `SelectionEngine<C,H,B,X,L,S,M>` implements this via a blanket impl,
/// erasing all generic parameters so the C API sees a single opaque handle.
pub trait DynSelectionEngine: Send {
    fn add_view_group(&mut self, weight: f64) -> selekt::ViewGroupHandle;
    fn remove_view_group(&mut self, handle: selekt::ViewGroupHandle) -> bool;
    fn update_view_group(
        &mut self,
        handle: selekt::ViewGroupHandle,
        views: &[ViewState],
    ) -> selekt::ViewUpdateResult;
    fn load(&mut self) -> selekt::LoadPassResult;
    fn dispatch_main_thread_tasks(&mut self) -> selekt::LoadPassResult;
    fn is_root_available(&self) -> bool;
    fn compute_load_progress(&self) -> f32;
    fn number_of_tiles_loaded(&self) -> usize;
    fn total_data_bytes(&self) -> usize;

    fn get_max_simultaneous_tile_loads(&self) -> usize;
    fn set_max_simultaneous_tile_loads(&mut self, val: usize);
    fn get_max_cached_bytes(&self) -> usize;
    fn set_max_cached_bytes(&mut self, val: usize);
    fn get_enable_frustum_culling(&self) -> bool;
    fn set_enable_frustum_culling(&mut self, val: bool);
    fn get_enable_occlusion_culling(&self) -> bool;
    fn set_enable_occlusion_culling(&mut self, val: bool);
    fn get_prevent_holes(&self) -> bool;
    fn set_prevent_holes(&mut self, val: bool);
    fn get_loading_descendant_limit(&self) -> usize;
    fn set_loading_descendant_limit(&mut self, val: usize);
}


struct EngineWrapper {
    engine: Box<dyn DynSelectionEngine>,
    last_view_result: Option<selekt::ViewUpdateResult>,
}


/// Create an engine handle from a boxed `DynSelectionEngine`.
///
/// # Safety
/// `engine` must be a `*mut Box<dyn DynSelectionEngine>` obtained by
/// `Box::into_raw(Box::new(boxed_engine))`.
/// The returned handle must be freed with `selekt_engine_drop`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_new(engine: *mut c_void) -> *mut selekt_engine_t {
    let boxed_engine = unsafe { *Box::from_raw(engine as *mut Box<dyn DynSelectionEngine>) };
    let wrapper = Box::new(EngineWrapper {
        engine: boxed_engine,
        last_view_result: None,
    });
    Box::into_raw(wrapper) as *mut selekt_engine_t
}

/// Destroy an engine handle.
///
/// # Safety
/// `engine` must be a valid handle from `selekt_engine_new`.
/// Must not be used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_drop(engine: *mut selekt_engine_t) {
    if !engine.is_null() {
        let _ = unsafe { Box::from_raw(engine as *mut EngineWrapper) };
    }
}


/// Update a view group with the given camera states.
///
/// The returned `selected_ptr` is valid until the next call to this function
/// on the same engine, or until the engine is dropped.
///
/// # Safety
/// `engine` must be a valid handle from `selekt_engine_new`.
/// `views_ptr` must point to `views_len` valid `selekt_view_state_t` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_update_view_group(
    engine: *mut selekt_engine_t,
    handle: selekt_view_group_handle_t,
    views_ptr: *const selekt_view_state_t,
    views_len: usize,
) -> selekt_view_update_result_t {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    let ffi_views = unsafe { std::slice::from_raw_parts(views_ptr, views_len) };
    let views: Vec<ViewState> = ffi_views.iter().map(ViewState::from).collect();

    let result = wrapper.engine.update_view_group(handle.into(), &views);
    wrapper.last_view_result = Some(result);
    let cached = wrapper.last_view_result.as_ref().unwrap();

    selekt_view_update_result_t {
        selected_ptr: cached.selected.as_ptr(),
        selected_len: cached.selected.len(),
        visited: cached.visited,
        culled: cached.culled,
        queued_requests: cached.queued_requests,
        worker_thread_load_queue_length: cached.worker_thread_load_queue_length,
        main_thread_load_queue_length: cached.main_thread_load_queue_length,
        frame_number: cached.frame_number,
    }
}

/// Run a load pass — drain the scheduler, issue requests, process completions.
///
/// # Safety
/// `engine` must be a valid handle from `selekt_engine_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_load(
    engine: *mut selekt_engine_t,
) -> selekt_load_pass_result_t {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    let result = wrapper.engine.load();
    selekt_load_pass_result_t {
        started_requests: result.started_requests,
        completed_main_thread_tasks: result.completed_main_thread_tasks,
        pending_worker_queue: result.pending_worker_queue,
        pending_main_queue: result.pending_main_queue,
    }
}

/// Finalize main-thread tasks without issuing new loads.
///
/// # Safety
/// `engine` must be a valid handle from `selekt_engine_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_dispatch_main_thread_tasks(
    engine: *mut selekt_engine_t,
) -> selekt_load_pass_result_t {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    let result = wrapper.engine.dispatch_main_thread_tasks();
    selekt_load_pass_result_t {
        started_requests: result.started_requests,
        completed_main_thread_tasks: result.completed_main_thread_tasks,
        pending_worker_queue: result.pending_worker_queue,
        pending_main_queue: result.pending_main_queue,
    }
}


/// Add a view group with the given scheduling weight.
///
/// # Safety
/// `engine` must be a valid handle from `selekt_engine_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_add_view_group(
    engine: *mut selekt_engine_t,
    weight: f64,
) -> selekt_view_group_handle_t {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.add_view_group(weight).into()
}

/// Remove a view group by handle. Returns `true` if it existed.
///
/// # Safety
/// `engine` must be a valid handle from `selekt_engine_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_remove_view_group(
    engine: *mut selekt_engine_t,
    handle: selekt_view_group_handle_t,
) -> bool {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.remove_view_group(handle.into())
}


/// Whether the hierarchy root is available for traversal.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_is_root_available(engine: *const selekt_engine_t) -> bool {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.is_root_available()
}

/// Load progress as a percentage in [0.0, 100.0].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_compute_load_progress(
    engine: *const selekt_engine_t,
) -> f32 {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.compute_load_progress()
}

/// Number of nodes with content currently loaded.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_number_of_tiles_loaded(
    engine: *const selekt_engine_t,
) -> usize {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.number_of_tiles_loaded()
}

/// Total bytes of currently-resident content.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_total_data_bytes(engine: *const selekt_engine_t) -> usize {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.total_data_bytes()
}


#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_get_max_simultaneous_tile_loads(
    engine: *const selekt_engine_t,
) -> usize {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.get_max_simultaneous_tile_loads()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_set_max_simultaneous_tile_loads(
    engine: *mut selekt_engine_t,
    val: usize,
) {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.set_max_simultaneous_tile_loads(val);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_get_max_cached_bytes(
    engine: *const selekt_engine_t,
) -> usize {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.get_max_cached_bytes()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_set_max_cached_bytes(
    engine: *mut selekt_engine_t,
    val: usize,
) {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.set_max_cached_bytes(val);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_get_enable_frustum_culling(
    engine: *const selekt_engine_t,
) -> bool {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.get_enable_frustum_culling()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_set_enable_frustum_culling(
    engine: *mut selekt_engine_t,
    val: bool,
) {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.set_enable_frustum_culling(val);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_get_enable_occlusion_culling(
    engine: *const selekt_engine_t,
) -> bool {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.get_enable_occlusion_culling()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_set_enable_occlusion_culling(
    engine: *mut selekt_engine_t,
    val: bool,
) {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.set_enable_occlusion_culling(val);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_get_prevent_holes(engine: *const selekt_engine_t) -> bool {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.get_prevent_holes()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_set_prevent_holes(engine: *mut selekt_engine_t, val: bool) {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.set_prevent_holes(val);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_get_loading_descendant_limit(
    engine: *const selekt_engine_t,
) -> usize {
    let wrapper = unsafe { &*(engine as *const EngineWrapper) };
    wrapper.engine.get_loading_descendant_limit()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_set_loading_descendant_limit(
    engine: *mut selekt_engine_t,
    val: usize,
) {
    let wrapper = unsafe { &mut *(engine as *mut EngineWrapper) };
    wrapper.engine.set_loading_descendant_limit(val);
}


impl<C, H, B, X, L, S, M> DynSelectionEngine for selekt::SelectionEngine<C, H, B, X, L, S, M>
where
    C: Send + 'static,
    H: selekt::SpatialHierarchy,
    B: selekt::LodEvaluator,
    X: selekt::HierarchyResolver,
    L: selekt::ContentLoader<C>,
    S: selekt::LoadScheduler,
    M: selekt::Policy,
{
    fn add_view_group(&mut self, weight: f64) -> selekt::ViewGroupHandle {
        self.add_view_group(selekt::ViewGroupOptions { weight })
    }
    fn remove_view_group(&mut self, handle: selekt::ViewGroupHandle) -> bool {
        self.remove_view_group(handle)
    }
    fn update_view_group(
        &mut self,
        handle: selekt::ViewGroupHandle,
        views: &[ViewState],
    ) -> selekt::ViewUpdateResult {
        self.update_view_group(handle, views)
    }
    fn load(&mut self) -> selekt::LoadPassResult {
        self.load()
    }
    fn dispatch_main_thread_tasks(&mut self) -> selekt::LoadPassResult {
        self.dispatch_main_thread_tasks()
    }
    fn is_root_available(&self) -> bool {
        self.is_root_available()
    }
    fn compute_load_progress(&self) -> f32 {
        self.compute_load_progress()
    }
    fn number_of_tiles_loaded(&self) -> usize {
        self.number_of_tiles_loaded()
    }
    fn total_data_bytes(&self) -> usize {
        self.total_data_bytes()
    }
    fn get_max_simultaneous_tile_loads(&self) -> usize {
        self.options().max_simultaneous_tile_loads
    }
    fn set_max_simultaneous_tile_loads(&mut self, val: usize) {
        self.options_mut().max_simultaneous_tile_loads = val;
    }
    fn get_max_cached_bytes(&self) -> usize {
        self.options().max_cached_bytes
    }
    fn set_max_cached_bytes(&mut self, val: usize) {
        self.options_mut().max_cached_bytes = val;
    }
    fn get_enable_frustum_culling(&self) -> bool {
        self.options().enable_frustum_culling
    }
    fn set_enable_frustum_culling(&mut self, val: bool) {
        self.options_mut().enable_frustum_culling = val;
    }
    fn get_enable_occlusion_culling(&self) -> bool {
        self.options().enable_occlusion_culling
    }
    fn set_enable_occlusion_culling(&mut self, val: bool) {
        self.options_mut().enable_occlusion_culling = val;
    }
    fn get_prevent_holes(&self) -> bool {
        self.options().prevent_holes
    }
    fn set_prevent_holes(&mut self, val: bool) {
        self.options_mut().prevent_holes = val;
    }
    fn get_loading_descendant_limit(&self) -> usize {
        self.options().loading_descendant_limit
    }
    fn set_loading_descendant_limit(&mut self, val: usize) {
        self.options_mut().loading_descendant_limit = val;
    }
}


mod test_harness;

/// Create a test engine with a simple two-level hierarchy for FFI testing.
///
/// - Root node (ID=0) with children [1, 2]
/// - `always_refine=true`: root always refines to children (Replace mode)
/// - `always_refine=false`: root is never refined (root-only selection)
///
/// Returns an opaque engine handle ready for `selekt_engine_*` calls.
///
/// # Safety
/// The returned handle must be freed with `selekt_engine_drop`.
#[unsafe(no_mangle)]
pub extern "C" fn selekt_test_create_engine(always_refine: bool) -> *mut selekt_engine_t {
    let engine = test_harness::create_test_engine(always_refine);
    let boxed: Box<dyn DynSelectionEngine> = Box::new(engine);
    let wrapper = Box::new(EngineWrapper {
        engine: boxed,
        last_view_result: None,
    });
    Box::into_raw(wrapper) as *mut selekt_engine_t
}


/// Create an engine from C-provided vtables.
///
/// The engine takes ownership of all vtable contexts and will call their
/// `destroy` callbacks when dropped.
///
/// `async_system` must be an `orkester_async_t*` handle; the engine clones it
/// internally (the caller retains ownership of the original handle).
///
/// `content_destroy` is called when the engine evicts content — it receives
/// the `content_ptr` that was passed to `selekt_load_delivery_resolve`.
/// Pass null if content does not need cleanup.
///
/// `options` may be null to use defaults.
///
/// # Safety
/// - `async_system` must be a valid `orkester_async_t*` (i.e., a `Box<AsyncSystem>`).
/// - All vtable function pointers must be valid for the lifetime of the engine.
/// - The returned handle must be freed with `selekt_engine_drop`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_engine_create(
    async_system: *const c_void,
    hierarchy: selekt_hierarchy_vtable_t,
    lod_evaluator: selekt_lod_evaluator_vtable_t,
    content_loader: selekt_content_loader_vtable_t,
    hierarchy_resolver: selekt_hierarchy_resolver_vtable_t,
    content_destroy: selekt_content_destroy_fn_t,
    options: *const selekt::SelectionOptions,
) -> *mut selekt_engine_t {
    // Recover AsyncSystem from the opaque orkester_async_t* handle.
    let system = unsafe { &*(async_system as *const orkester::AsyncSystem) };
    let system = system.clone();

    let opts = if options.is_null() {
        selekt::SelectionOptions::default()
    } else {
        unsafe { &*options }.clone()
    };

    let ffi_hierarchy = FfiHierarchy::new(hierarchy);
    let ffi_lod = FfiLodEvaluator::new(lod_evaluator);
    let ffi_loader = FfiContentLoader::new(content_loader, content_destroy, system.clone());
    let ffi_resolver = FfiHierarchyResolver::new(hierarchy_resolver, system.clone());

    let scheduler = Arc::new(Mutex::new(selekt::WeightedFairScheduler::new()));

    let engine = selekt::SelectionEngine::new(
        system,
        ffi_hierarchy,
        ffi_lod,
        ffi_resolver,
        ffi_loader,
        scheduler,
        selekt::DefaultPolicy,
        opts,
    );

    let boxed: Box<dyn DynSelectionEngine> = Box::new(engine);
    let wrapper = Box::new(EngineWrapper {
        engine: boxed,
        last_view_result: None,
    });
    Box::into_raw(wrapper) as *mut selekt_engine_t
}


/// Resolve a content load delivery with renderable content.
///
/// - `content_ptr`: opaque handle to the loaded content (owned by C side).
///   The engine will call the `content_destroy` callback (from `selekt_engine_create`)
///   when evicting this content.
/// - `byte_size`: size of the content in bytes (for memory budget tracking).
///
/// Consumes the delivery handle.
///
/// # Safety
/// `delivery` must be a valid handle received from a content loader `request` callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_load_delivery_resolve(
    delivery: *mut selekt_load_delivery_t,
    content_ptr: *mut c_void,
    byte_size: usize,
) {
    let d = unsafe { Box::from_raw(delivery as *mut wrappers::LoadDelivery) };
    let content = wrappers::FfiContent {
        ptr: content_ptr,
        destroy: None, // Engine manages destroy via the content_destroy from create
    };
    let loaded = selekt::LoadedContent {
        payload: selekt::Payload::Renderable(content),
        byte_size,
    };
    *d.slot.lock().unwrap() = Some(Ok(loaded));
    d.promise.resolve(());
}

/// Resolve a content load delivery as an external hierarchy reference.
///
/// Consumes the delivery handle.
///
/// # Safety
/// `delivery` must be a valid handle received from a content loader `request` callback.
/// `key_ptr`/`key_len` must be valid for reading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_load_delivery_resolve_reference(
    delivery: *mut selekt_load_delivery_t,
    key_ptr: *const u8,
    key_len: usize,
    source_node: NodeId,
    has_transform: bool,
    transform: *const [f64; 16],
) {
    let d = unsafe { Box::from_raw(delivery as *mut wrappers::LoadDelivery) };
    let key_bytes = unsafe { std::slice::from_raw_parts(key_ptr, key_len) };
    let key = selekt::ContentKey(String::from_utf8_lossy(key_bytes).into_owned());

    let xform = if has_transform && !transform.is_null() {
        let t = unsafe { &*transform };
        Some(zukei::math::Mat4 {
            cols: [
                zukei::math::Vec4 {
                    x: t[0],
                    y: t[1],
                    z: t[2],
                    w: t[3],
                },
                zukei::math::Vec4 {
                    x: t[4],
                    y: t[5],
                    z: t[6],
                    w: t[7],
                },
                zukei::math::Vec4 {
                    x: t[8],
                    y: t[9],
                    z: t[10],
                    w: t[11],
                },
                zukei::math::Vec4 {
                    x: t[12],
                    y: t[13],
                    z: t[14],
                    w: t[15],
                },
            ],
        })
    } else {
        None
    };

    let reference = selekt::HierarchyReference {
        key,
        source: source_node,
        transform: xform,
    };
    let loaded = selekt::LoadedContent {
        payload: selekt::Payload::Reference(reference),
        byte_size: 0,
    };
    *d.slot.lock().unwrap() = Some(Ok(loaded));
    d.promise.resolve(());
}

/// Resolve a content load delivery as empty (node has no content).
///
/// Consumes the delivery handle.
///
/// # Safety
/// `delivery` must be a valid handle from a content loader `request` callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_load_delivery_resolve_empty(delivery: *mut selekt_load_delivery_t) {
    let d = unsafe { Box::from_raw(delivery as *mut wrappers::LoadDelivery) };
    let loaded = selekt::LoadedContent {
        payload: selekt::Payload::Empty,
        byte_size: 0,
    };
    *d.slot.lock().unwrap() = Some(Ok(loaded));
    d.promise.resolve(());
}

/// Reject a content load delivery with an error message.
///
/// Consumes the delivery handle.
///
/// # Safety
/// `delivery` must be a valid handle from a content loader `request` callback.
/// `error_ptr`/`error_len` must be valid for reading.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_load_delivery_reject(
    delivery: *mut selekt_load_delivery_t,
    error_ptr: *const u8,
    error_len: usize,
) {
    let d = unsafe { Box::from_raw(delivery as *mut wrappers::LoadDelivery) };
    let msg = if error_ptr.is_null() || error_len == 0 {
        "unknown error".to_string()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(error_ptr, error_len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    *d.slot.lock().unwrap() = Some(Err(msg));
    d.promise.resolve(());
}

/// Drop a content load delivery without resolving.
/// Equivalent to rejecting with "delivery dropped".
///
/// # Safety
/// `delivery` must be a valid handle from a content loader `request` callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_load_delivery_drop(delivery: *mut selekt_load_delivery_t) {
    if !delivery.is_null() {
        let d = unsafe { Box::from_raw(delivery as *mut wrappers::LoadDelivery) };
        *d.slot.lock().unwrap() = Some(Err("delivery dropped".into()));
        d.promise.resolve(());
    }
}


/// Resolve a hierarchy delivery with a patch.
///
/// - `parent`: the node the patch is anchored to.
/// - `inserted_ptr`/`inserted_len`: new NodeIds inserted beneath `parent`.
///
/// Pass `inserted_len = 0` to indicate the reference resolved to nothing.
///
/// Consumes the delivery handle.
///
/// # Safety
/// `delivery` must be a valid handle from a hierarchy resolver `resolve` callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_hierarchy_delivery_resolve(
    delivery: *mut selekt_hierarchy_delivery_t,
    parent: NodeId,
    inserted_ptr: *const NodeId,
    inserted_len: usize,
) {
    let d = unsafe { Box::from_raw(delivery as *mut wrappers::HierarchyDelivery) };
    if inserted_len == 0 {
        *d.slot.lock().unwrap() = Some(Ok(None));
    } else {
        let nodes = unsafe { std::slice::from_raw_parts(inserted_ptr, inserted_len) }.to_vec();
        let patch = selekt::HierarchyPatch {
            parent,
            inserted_nodes: nodes,
        };
        *d.slot.lock().unwrap() = Some(Ok(Some(patch)));
    }
    d.promise.resolve(());
}

/// Reject a hierarchy delivery with an error.
///
/// Consumes the delivery handle.
///
/// # Safety
/// `delivery` must be a valid handle from a hierarchy resolver `resolve` callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_hierarchy_delivery_reject(
    delivery: *mut selekt_hierarchy_delivery_t,
    error_ptr: *const u8,
    error_len: usize,
) {
    let d = unsafe { Box::from_raw(delivery as *mut wrappers::HierarchyDelivery) };
    let msg = if error_ptr.is_null() || error_len == 0 {
        "unknown error".to_string()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(error_ptr, error_len) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    *d.slot.lock().unwrap() = Some(Err(msg));
    d.promise.resolve(());
}

/// Drop a hierarchy delivery without resolving.
///
/// # Safety
/// `delivery` must be a valid handle from a hierarchy resolver `resolve` callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn selekt_hierarchy_delivery_drop(
    delivery: *mut selekt_hierarchy_delivery_t,
) {
    if !delivery.is_null() {
        let d = unsafe { Box::from_raw(delivery as *mut wrappers::HierarchyDelivery) };
        *d.slot.lock().unwrap() = Some(Err("delivery dropped".into()));
        d.promise.resolve(());
    }
}
