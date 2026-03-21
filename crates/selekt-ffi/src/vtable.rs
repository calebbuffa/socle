//! C-ABI vtable structs for each selekt trait.
//!
//! Each vtable carries an opaque `ctx` pointer and function pointers
//! matching the trait's methods. The `destroy` callback is called
//! when the engine is dropped.

use std::ffi::c_void;

use selekt::{LoadPriority, NodeId, NodeKind, PriorityGroup, RefinementMode, RequestId};
use zukei::bounds::SpatialBounds;

use crate::selekt_view_state_t;


/// C-ABI view of a [`selekt::lod::LodDescriptor`].
///
/// Pointers are **borrowed** from the C side and must remain valid for the
/// duration of the callback invocation.
#[repr(C)]
pub struct selekt_lod_descriptor_t {
    pub family_ptr: *const u8,
    pub family_len: usize,
    pub values_ptr: *const f64,
    pub values_len: usize,
}


/// C-ABI load priority.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct selekt_load_priority_t {
    pub group: PriorityGroup,
    pub score: i64,
    pub view_group_weight: u16,
}

impl From<LoadPriority> for selekt_load_priority_t {
    fn from(p: LoadPriority) -> Self {
        Self {
            group: p.group,
            score: p.score,
            view_group_weight: p.view_group_weight,
        }
    }
}


/// Opaque handle for delivering loaded content back to the engine.
///
/// Created by the engine when calling the content loader's `request` callback.
/// The C side must eventually call one of:
/// - `selekt_load_delivery_resolve` — deliver content successfully
/// - `selekt_load_delivery_reject` — signal a load failure
/// - `selekt_load_delivery_drop` — abandon (equivalent to reject)
#[repr(C)]
pub struct selekt_load_delivery_t {
    _private: [u8; 0],
}

/// Opaque handle for delivering a resolved hierarchy patch.
///
/// Created by the engine when calling the hierarchy resolver's `resolve` callback.
/// The C side must eventually call one of:
/// - `selekt_hierarchy_delivery_resolve` — deliver the patch
/// - `selekt_hierarchy_delivery_reject` — signal failure
/// - `selekt_hierarchy_delivery_drop` — abandon
#[repr(C)]
pub struct selekt_hierarchy_delivery_t {
    _private: [u8; 0],
}


/// C-ABI vtable for [`selekt::SpatialHierarchy`].
///
/// The `ctx` pointer is passed as the first argument to every callback.
/// All returned pointers (from `children`, `bounds`, etc.) must remain
/// valid at least until the next call to any method on the same hierarchy.
#[repr(C)]
pub struct selekt_hierarchy_vtable_t {
    pub ctx: *mut c_void,

    /// Return the root node ID.
    pub root: unsafe extern "C" fn(ctx: *mut c_void) -> NodeId,

    /// Return the parent of `node`. Write it to `*out_parent` and return `true`.
    /// Return `false` if `node` is the root.
    pub parent:
        unsafe extern "C" fn(ctx: *mut c_void, node: NodeId, out_parent: *mut NodeId) -> bool,

    /// Write the children of `node` to `*out_ptr` / `*out_len`.
    /// The pointed-to array must stay valid until the next hierarchy call.
    pub children: unsafe extern "C" fn(
        ctx: *mut c_void,
        node: NodeId,
        out_ptr: *mut *const NodeId,
        out_len: *mut usize,
    ),

    /// Return the structural classification of `node`.
    pub node_kind: unsafe extern "C" fn(ctx: *mut c_void, node: NodeId) -> NodeKind,

    /// Write the bounding volume of `node` into `*out`.
    pub bounds: unsafe extern "C" fn(ctx: *mut c_void, node: NodeId, out: *mut SpatialBounds),

    /// Write the LOD descriptor of `node` into `*out`.
    /// Pointers inside `selekt_lod_descriptor_t` must remain valid until the
    /// next hierarchy call.
    pub lod_descriptor:
        unsafe extern "C" fn(ctx: *mut c_void, node: NodeId, out: *mut selekt_lod_descriptor_t),

    /// Return the refinement mode of `node`.
    pub refinement_mode: unsafe extern "C" fn(ctx: *mut c_void, node: NodeId) -> RefinementMode,

    /// Write content bounds into `*out` and return `true`, or `false` if none.
    pub content_bounds:
        unsafe extern "C" fn(ctx: *mut c_void, node: NodeId, out: *mut SpatialBounds) -> bool,

    /// Write content key (pointer + length) and return `true`, or `false` if none.
    /// The string must remain valid until the next hierarchy call.
    pub content_key: unsafe extern "C" fn(
        ctx: *mut c_void,
        node: NodeId,
        out_ptr: *mut *const u8,
        out_len: *mut usize,
    ) -> bool,

    /// Apply a hierarchy patch. `inserted_ptr`/`inserted_len` describe
    /// new NodeIds added under `parent`. Return `true` on success.
    pub apply_patch: unsafe extern "C" fn(
        ctx: *mut c_void,
        parent: NodeId,
        inserted_ptr: *const NodeId,
        inserted_len: usize,
    ) -> bool,

    /// Destroy the hierarchy context. Called when the engine is dropped.
    /// May be null if no cleanup is needed.
    pub destroy: Option<unsafe extern "C" fn(ctx: *mut c_void)>,
}

unsafe impl Send for selekt_hierarchy_vtable_t {}
unsafe impl Sync for selekt_hierarchy_vtable_t {}


/// C-ABI vtable for [`selekt::LodEvaluator`].
#[repr(C)]
pub struct selekt_lod_evaluator_vtable_t {
    pub ctx: *mut c_void,

    /// Return `true` if the node should refine to its children.
    pub should_refine: unsafe extern "C" fn(
        ctx: *mut c_void,
        descriptor: *const selekt_lod_descriptor_t,
        view: *const selekt_view_state_t,
        bounds: *const SpatialBounds,
        mode: RefinementMode,
    ) -> bool,

    pub destroy: Option<unsafe extern "C" fn(ctx: *mut c_void)>,
}

unsafe impl Send for selekt_lod_evaluator_vtable_t {}
unsafe impl Sync for selekt_lod_evaluator_vtable_t {}


/// C-ABI vtable for [`selekt::ContentLoader`].
///
/// `request` receives a `selekt_load_delivery_t*` that the C side must
/// eventually resolve, reject, or drop. The delivery handle carries the
/// Rust promise that feeds the engine's load pipeline.
///
/// The content type `C` is erased to `*mut c_void`. The C side owns the
/// content and must provide a `destroy_content` callback so Rust can
/// clean up evicted content.
#[repr(C)]
pub struct selekt_content_loader_vtable_t {
    pub ctx: *mut c_void,

    /// Start an asynchronous content load.
    ///
    /// - `delivery`: opaque handle — must be resolved via `selekt_load_delivery_*`.
    /// - `node_id`: which node needs content.
    /// - `key_ptr`/`key_len`: the content key (URI or path), borrowed.
    /// - `priority`: load priority hint.
    ///
    /// Returns a `RequestId` that can be passed to `cancel`.
    pub request: unsafe extern "C" fn(
        ctx: *mut c_void,
        delivery: *mut selekt_load_delivery_t,
        node_id: NodeId,
        key_ptr: *const u8,
        key_len: usize,
        priority: selekt_load_priority_t,
    ) -> RequestId,

    /// Cancel a previously-issued request. Return `true` if it was cancelled.
    pub cancel: unsafe extern "C" fn(ctx: *mut c_void, request_id: RequestId) -> bool,

    /// Destroy the loader context. Called when the engine is dropped.
    pub destroy: Option<unsafe extern "C" fn(ctx: *mut c_void)>,
}

unsafe impl Send for selekt_content_loader_vtable_t {}
unsafe impl Sync for selekt_content_loader_vtable_t {}


/// C-ABI vtable for [`selekt::HierarchyResolver`].
///
/// `resolve` receives a `selekt_hierarchy_delivery_t*` that the C side must
/// eventually resolve with a patch, reject, or drop.
#[repr(C)]
pub struct selekt_hierarchy_resolver_vtable_t {
    pub ctx: *mut c_void,

    /// Start resolving an external hierarchy reference.
    ///
    /// - `delivery`: opaque handle — must be resolved via `selekt_hierarchy_delivery_*`.
    /// - `key_ptr`/`key_len`: the content key of the external hierarchy, borrowed.
    /// - `source_node`: the node that contains the reference.
    /// - `has_transform`/`transform`: optional 4×4 column-major transform matrix.
    pub resolve: unsafe extern "C" fn(
        ctx: *mut c_void,
        delivery: *mut selekt_hierarchy_delivery_t,
        key_ptr: *const u8,
        key_len: usize,
        source_node: NodeId,
        has_transform: bool,
        transform: *const [f64; 16],
    ),

    /// Destroy the resolver context.
    pub destroy: Option<unsafe extern "C" fn(ctx: *mut c_void)>,
}

unsafe impl Send for selekt_hierarchy_resolver_vtable_t {}
unsafe impl Sync for selekt_hierarchy_resolver_vtable_t {}


/// Callback to destroy C-side content when evicted by the engine.
pub type selekt_content_destroy_fn_t = Option<unsafe extern "C" fn(content: *mut c_void)>;
