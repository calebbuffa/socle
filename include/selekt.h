#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Structural classification of a node in the hierarchy.
 */
typedef enum NodeKind {
  /**
   * Has renderable content (mesh, point cloud, etc.).
   */
  NodeKind_Renderable,
  /**
   * Interior pass-through: no content, exists only to structure the hierarchy.
   */
  NodeKind_Empty,
  /**
   * Links to an external child hierarchy (triggers `HierarchyResolver`).
   */
  NodeKind_Reference,
  /**
   * Root of a composite multi-layer structure (e.g., I3S building sublayers).
   */
  NodeKind_CompositeRoot,
} NodeKind;

/**
 * Load scheduling tier. Determines which candidates are popped from the queue first.
 * Processing order: Urgent → Normal → Preload.
 */
typedef enum PriorityGroup {
  /**
   * Speculative: siblings of culled nodes, pre-loaded for smooth panning.
   */
  PriorityGroup_Preload = 0,
  /**
   * Normal: nodes required for current-frame LOD.
   */
  PriorityGroup_Normal = 1,
  /**
   * Urgent: nodes whose absence causes kicked ancestors (visible detail loss).
   */
  PriorityGroup_Urgent = 2,
} PriorityGroup;

/**
 * Whether children supplement or replace the parent during refinement.
 */
typedef enum RefinementMode {
  /**
   * Additive: render parent and children simultaneously.
   */
  RefinementMode_Add,
  /**
   * Replacement: children replace the parent when fully loaded.
   * Requires ancestor fallback while children are loading.
   */
  RefinementMode_Replace,
} RefinementMode;

/**
 * Opaque engine handle exposed to C. Wraps a type-erased `SelectionEngine`.
 */
typedef struct selekt_engine_t {
  uint8_t _private[0];
} selekt_engine_t;

/**
 * Opaque stable identifier for a node in the spatial hierarchy.
 */
typedef uint64_t NodeId;

/**
 * Result of `selekt_engine_update_view_group`.
 */
typedef struct selekt_view_update_result_t {
  /**
   * Pointer to array of selected node IDs.
   */
  const NodeId *selected_ptr;
  /**
   * Number of selected nodes.
   */
  uintptr_t selected_len;
  /**
   * Total nodes visited during traversal.
   */
  uintptr_t visited;
  /**
   * Total nodes culled by visibility.
   */
  uintptr_t culled;
  /**
   * Number of new load requests queued.
   */
  uintptr_t queued_requests;
  /**
   * Number of nodes in the worker thread load queue.
   */
  uintptr_t worker_thread_load_queue_length;
  /**
   * Number of nodes in the main thread load queue.
   */
  uintptr_t main_thread_load_queue_length;
  /**
   * Monotonically increasing frame counter.
   */
  uint64_t frame_number;
} selekt_view_update_result_t;

/**
 * Identifies a view group managed by the engine.
 */
typedef struct selekt_view_group_handle_t {
  uint32_t index;
  uint32_t generation;
} selekt_view_group_handle_t;

/**
 * C-ABI camera state for tile selection.
 */
typedef struct selekt_view_state_t {
  uint32_t viewport_width;
  uint32_t viewport_height;
  double position[3];
  double direction[3];
  double up[3];
  double fov_x;
  double fov_y;
  float lod_metric_multiplier;
} selekt_view_state_t;

/**
 * Result of `selekt_engine_load` or `selekt_engine_dispatch_main_thread_tasks`.
 */
typedef struct selekt_load_pass_result_t {
  uintptr_t started_requests;
  uintptr_t completed_main_thread_tasks;
  uintptr_t pending_worker_queue;
  uintptr_t pending_main_queue;
} selekt_load_pass_result_t;

/**
 * FFI-safe double-precision math types.
 */
typedef struct Vec2 {
  double x;
  double y;
} Vec2;

typedef struct Vec3 {
  double x;
  double y;
  double z;
} Vec3;

/**
 * Column-major 3×3 matrix.
 */
typedef struct Mat3 {
  struct Vec3 cols[3];
} Mat3;

/**
 * Spatial extent of a node.
 */
typedef enum SpatialBounds_Tag {
  /**
   * 2D rectangle (lon/lat or projected).
   */
  SpatialBounds_Rectangle,
  /**
   * Axis-aligned box in 3D.
   */
  SpatialBounds_AxisAlignedBox,
  /**
   * Bounding sphere.
   */
  SpatialBounds_Sphere,
  /**
   * Oriented bounding box.
   */
  SpatialBounds_OrientedBox,
} SpatialBounds_Tag;

typedef struct SpatialBounds_Rectangle_Body {
  struct Vec2 min;
  struct Vec2 max;
} SpatialBounds_Rectangle_Body;

typedef struct SpatialBounds_AxisAlignedBox_Body {
  struct Vec3 min;
  struct Vec3 max;
} SpatialBounds_AxisAlignedBox_Body;

typedef struct SpatialBounds_Sphere_Body {
  struct Vec3 center;
  double radius;
} SpatialBounds_Sphere_Body;

typedef struct SpatialBounds_OrientedBox_Body {
  struct Vec3 center;
  struct Mat3 half_axes;
} SpatialBounds_OrientedBox_Body;

typedef struct SpatialBounds {
  SpatialBounds_Tag tag;
  union {
    SpatialBounds_Rectangle_Body rectangle;
    SpatialBounds_AxisAlignedBox_Body axis_aligned_box;
    SpatialBounds_Sphere_Body sphere;
    SpatialBounds_OrientedBox_Body oriented_box;
  };
} SpatialBounds;

/**
 * C-ABI view of a [`selekt::lod::LodDescriptor`].
 *
 * Pointers are **borrowed** from the C side and must remain valid for the
 * duration of the callback invocation.
 */
typedef struct selekt_lod_descriptor_t {
  const uint8_t *family_ptr;
  uintptr_t family_len;
  const double *values_ptr;
  uintptr_t values_len;
} selekt_lod_descriptor_t;

/**
 * C-ABI vtable for [`selekt::SpatialHierarchy`].
 *
 * The `ctx` pointer is passed as the first argument to every callback.
 * All returned pointers (from `children`, `bounds`, etc.) must remain
 * valid at least until the next call to any method on the same hierarchy.
 */
typedef struct selekt_hierarchy_vtable_t {
  void *ctx;
  /**
   * Return the root node ID.
   */
  NodeId (*root)(void *ctx);
  /**
   * Return the parent of `node`. Write it to `*out_parent` and return `true`.
   * Return `false` if `node` is the root.
   */
  bool (*parent)(void *ctx, NodeId node, NodeId *out_parent);
  /**
   * Write the children of `node` to `*out_ptr` / `*out_len`.
   * The pointed-to array must stay valid until the next hierarchy call.
   */
  void (*children)(void *ctx, NodeId node, const NodeId **out_ptr, uintptr_t *out_len);
  /**
   * Return the structural classification of `node`.
   */
  enum NodeKind (*node_kind)(void *ctx, NodeId node);
  /**
   * Write the bounding volume of `node` into `*out`.
   */
  void (*bounds)(void *ctx, NodeId node, struct SpatialBounds *out);
  /**
   * Write the LOD descriptor of `node` into `*out`.
   * Pointers inside `selekt_lod_descriptor_t` must remain valid until the
   * next hierarchy call.
   */
  void (*lod_descriptor)(void *ctx, NodeId node, struct selekt_lod_descriptor_t *out);
  /**
   * Return the refinement mode of `node`.
   */
  enum RefinementMode (*refinement_mode)(void *ctx, NodeId node);
  /**
   * Write content bounds into `*out` and return `true`, or `false` if none.
   */
  bool (*content_bounds)(void *ctx, NodeId node, struct SpatialBounds *out);
  /**
   * Write content key (pointer + length) and return `true`, or `false` if none.
   * The string must remain valid until the next hierarchy call.
   */
  bool (*content_key)(void *ctx, NodeId node, const uint8_t **out_ptr, uintptr_t *out_len);
  /**
   * Apply a hierarchy patch. `inserted_ptr`/`inserted_len` describe
   * new NodeIds added under `parent`. Return `true` on success.
   */
  bool (*apply_patch)(void *ctx, NodeId parent, const NodeId *inserted_ptr, uintptr_t inserted_len);
  /**
   * Destroy the hierarchy context. Called when the engine is dropped.
   * May be null if no cleanup is needed.
   */
  void (*destroy)(void *ctx);
} selekt_hierarchy_vtable_t;

/**
 * C-ABI vtable for [`selekt::LodEvaluator`].
 */
typedef struct selekt_lod_evaluator_vtable_t {
  void *ctx;
  /**
   * Return `true` if the node should refine to its children.
   */
  bool (*should_refine)(void *ctx,
                        const struct selekt_lod_descriptor_t *descriptor,
                        const struct selekt_view_state_t *view,
                        const struct SpatialBounds *bounds,
                        enum RefinementMode mode);
  void (*destroy)(void *ctx);
} selekt_lod_evaluator_vtable_t;

/**
 * Opaque identifier for an in-flight load request.
 */
typedef uint64_t RequestId;

/**
 * Opaque handle for delivering loaded content back to the engine.
 *
 * Created by the engine when calling the content loader's `request` callback.
 * The C side must eventually call one of:
 * - `selekt_load_delivery_resolve` — deliver content successfully
 * - `selekt_load_delivery_reject` — signal a load failure
 * - `selekt_load_delivery_drop` — abandon (equivalent to reject)
 */
typedef struct selekt_load_delivery_t {
  uint8_t _private[0];
} selekt_load_delivery_t;

/**
 * C-ABI load priority.
 */
typedef struct selekt_load_priority_t {
  enum PriorityGroup group;
  int64_t score;
  uint16_t view_group_weight;
} selekt_load_priority_t;

/**
 * C-ABI vtable for [`selekt::ContentLoader`].
 *
 * `request` receives a `selekt_load_delivery_t*` that the C side must
 * eventually resolve, reject, or drop. The delivery handle carries the
 * Rust promise that feeds the engine's load pipeline.
 *
 * The content type `C` is erased to `*mut c_void`. The C side owns the
 * content and must provide a `destroy_content` callback so Rust can
 * clean up evicted content.
 */
typedef struct selekt_content_loader_vtable_t {
  void *ctx;
  /**
   * Start an asynchronous content load.
   *
   * - `delivery`: opaque handle — must be resolved via `selekt_load_delivery_*`.
   * - `node_id`: which node needs content.
   * - `key_ptr`/`key_len`: the content key (URI or path), borrowed.
   * - `priority`: load priority hint.
   *
   * Returns a `RequestId` that can be passed to `cancel`.
   */
  RequestId (*request)(void *ctx,
                       struct selekt_load_delivery_t *delivery,
                       NodeId node_id,
                       const uint8_t *key_ptr,
                       uintptr_t key_len,
                       struct selekt_load_priority_t priority);
  /**
   * Cancel a previously-issued request. Return `true` if it was cancelled.
   */
  bool (*cancel)(void *ctx, RequestId request_id);
  /**
   * Destroy the loader context. Called when the engine is dropped.
   */
  void (*destroy)(void *ctx);
} selekt_content_loader_vtable_t;

/**
 * Opaque handle for delivering a resolved hierarchy patch.
 *
 * Created by the engine when calling the hierarchy resolver's `resolve` callback.
 * The C side must eventually call one of:
 * - `selekt_hierarchy_delivery_resolve` — deliver the patch
 * - `selekt_hierarchy_delivery_reject` — signal failure
 * - `selekt_hierarchy_delivery_drop` — abandon
 */
typedef struct selekt_hierarchy_delivery_t {
  uint8_t _private[0];
} selekt_hierarchy_delivery_t;

/**
 * C-ABI vtable for [`selekt::HierarchyResolver`].
 *
 * `resolve` receives a `selekt_hierarchy_delivery_t*` that the C side must
 * eventually resolve with a patch, reject, or drop.
 */
typedef struct selekt_hierarchy_resolver_vtable_t {
  void *ctx;
  /**
   * Start resolving an external hierarchy reference.
   *
   * - `delivery`: opaque handle — must be resolved via `selekt_hierarchy_delivery_*`.
   * - `key_ptr`/`key_len`: the content key of the external hierarchy, borrowed.
   * - `source_node`: the node that contains the reference.
   * - `has_transform`/`transform`: optional 4×4 column-major transform matrix.
   */
  void (*resolve)(void *ctx,
                  struct selekt_hierarchy_delivery_t *delivery,
                  const uint8_t *key_ptr,
                  uintptr_t key_len,
                  NodeId source_node,
                  bool has_transform,
                  const double (*transform)[16]);
  /**
   * Destroy the resolver context.
   */
  void (*destroy)(void *ctx);
} selekt_hierarchy_resolver_vtable_t;

/**
 * Callback to destroy C-side content when evicted by the engine.
 */
typedef void (*selekt_content_destroy_fn_t)(void *content);

/**
 * Core engine options. Format-agnostic; no format-specific flags belong here.
 */
typedef struct SelectionOptions {
  /**
   * Maximum number of children simultaneously in the `Loading` state before the
   * traversal stops descending further. Prevents unbounded in-flight load explosion.
   * See continuity lock.
   */
  uintptr_t loading_descendant_limit;
  /**
   * If `true`, the selection must not produce holes — a parent node is always
   * selected as a fallback if replacement children are not yet `Renderable`.
   * If `false`, mixed parent/child visibility is permitted (more aggressive refinement).
   * Hole prevention: culled Replace-refined tiles are force-queued at `Normal`
   * priority to prevent LOD seams during camera movement.
   */
  bool prevent_holes;
  /**
   * If `true`, ancestors of rendered nodes are pre-loaded at `Preload`
   * priority. This improves the zoom-out experience by ensuring parent
   * tiles are ready before they are needed.
   */
  bool preload_ancestors;
  /**
   * If `true`, siblings of rendered nodes are pre-loaded at `Preload`
   * priority. This improves panning by loading tiles adjacent to the
   * current view so they are ready when the camera moves.
   */
  bool preload_siblings;
  /**
   * Maximum number of load retry attempts before a node transitions to `Failed`.
   */
  uint8_t retry_limit;
  /**
   * Frames to wait before re-queuing a `RetryScheduled` node.
   */
  uint32_t retry_backoff_frames;
  /**
   * Maximum new load requests to dispatch per load pass.
   */
  uintptr_t max_simultaneous_tile_loads;
  /**
   * Maximum main-thread finalization tasks to run per frame.
   */
  uintptr_t max_main_thread_tasks;
  /**
   * Memory ceiling in bytes for resident content. Eviction is triggered when
   * exceeded.
   */
  uintptr_t max_cached_bytes;
  /**
   * Whether frustum culling is enabled. When `false`, all nodes are treated
   * as visible (useful for debugging).
   */
  bool enable_frustum_culling;
  /**
   * Whether occlusion-driven refinement deferral is enabled.
   * Requires an [`OcclusionTester`] to be wired into the engine.
   */
  bool enable_occlusion_culling;
} SelectionOptions;







#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Create an engine handle from a boxed `DynSelectionEngine`.
 *
 * # Safety
 * `engine` must be a `*mut Box<dyn DynSelectionEngine>` obtained by
 * `Box::into_raw(Box::new(boxed_engine))`.
 * The returned handle must be freed with `selekt_engine_drop`.
 */
struct selekt_engine_t *selekt_engine_new(void *engine);

/**
 * Destroy an engine handle.
 *
 * # Safety
 * `engine` must be a valid handle from `selekt_engine_new`.
 * Must not be used after this call.
 */
void selekt_engine_drop(struct selekt_engine_t *engine);

/**
 * Update a view group with the given camera states.
 *
 * The returned `selected_ptr` is valid until the next call to this function
 * on the same engine, or until the engine is dropped.
 *
 * # Safety
 * `engine` must be a valid handle from `selekt_engine_new`.
 * `views_ptr` must point to `views_len` valid `selekt_view_state_t` values.
 */
struct selekt_view_update_result_t selekt_engine_update_view_group(struct selekt_engine_t *engine,
                                                                   struct selekt_view_group_handle_t handle,
                                                                   const struct selekt_view_state_t *views_ptr,
                                                                   uintptr_t views_len);

/**
 * Run a load pass — drain the scheduler, issue requests, process completions.
 *
 * # Safety
 * `engine` must be a valid handle from `selekt_engine_new`.
 */
struct selekt_load_pass_result_t selekt_engine_load(struct selekt_engine_t *engine);

/**
 * Finalize main-thread tasks without issuing new loads.
 *
 * # Safety
 * `engine` must be a valid handle from `selekt_engine_new`.
 */
struct selekt_load_pass_result_t selekt_engine_dispatch_main_thread_tasks(struct selekt_engine_t *engine);

/**
 * Add a view group with the given scheduling weight.
 *
 * # Safety
 * `engine` must be a valid handle from `selekt_engine_new`.
 */
struct selekt_view_group_handle_t selekt_engine_add_view_group(struct selekt_engine_t *engine,
                                                               double weight);

/**
 * Remove a view group by handle. Returns `true` if it existed.
 *
 * # Safety
 * `engine` must be a valid handle from `selekt_engine_new`.
 */
bool selekt_engine_remove_view_group(struct selekt_engine_t *engine,
                                     struct selekt_view_group_handle_t handle);

/**
 * Whether the hierarchy root is available for traversal.
 */
bool selekt_engine_is_root_available(const struct selekt_engine_t *engine);

/**
 * Load progress as a percentage in [0.0, 100.0].
 */
float selekt_engine_compute_load_progress(const struct selekt_engine_t *engine);

/**
 * Number of nodes with content currently loaded.
 */
uintptr_t selekt_engine_number_of_tiles_loaded(const struct selekt_engine_t *engine);

/**
 * Total bytes of currently-resident content.
 */
uintptr_t selekt_engine_total_data_bytes(const struct selekt_engine_t *engine);

uintptr_t selekt_engine_get_max_simultaneous_tile_loads(const struct selekt_engine_t *engine);

void selekt_engine_set_max_simultaneous_tile_loads(struct selekt_engine_t *engine, uintptr_t val);

uintptr_t selekt_engine_get_max_cached_bytes(const struct selekt_engine_t *engine);

void selekt_engine_set_max_cached_bytes(struct selekt_engine_t *engine, uintptr_t val);

bool selekt_engine_get_enable_frustum_culling(const struct selekt_engine_t *engine);

void selekt_engine_set_enable_frustum_culling(struct selekt_engine_t *engine, bool val);

bool selekt_engine_get_enable_occlusion_culling(const struct selekt_engine_t *engine);

void selekt_engine_set_enable_occlusion_culling(struct selekt_engine_t *engine, bool val);

bool selekt_engine_get_prevent_holes(const struct selekt_engine_t *engine);

void selekt_engine_set_prevent_holes(struct selekt_engine_t *engine, bool val);

uintptr_t selekt_engine_get_loading_descendant_limit(const struct selekt_engine_t *engine);

void selekt_engine_set_loading_descendant_limit(struct selekt_engine_t *engine, uintptr_t val);

/**
 * Create a test engine with a simple two-level hierarchy for FFI testing.
 *
 * - Root node (ID=0) with children [1, 2]
 * - `always_refine=true`: root always refines to children (Replace mode)
 * - `always_refine=false`: root is never refined (root-only selection)
 *
 * Returns an opaque engine handle ready for `selekt_engine_*` calls.
 *
 * # Safety
 * The returned handle must be freed with `selekt_engine_drop`.
 */
struct selekt_engine_t *selekt_test_create_engine(bool always_refine);

/**
 * Create an engine from C-provided vtables.
 *
 * The engine takes ownership of all vtable contexts and will call their
 * `destroy` callbacks when dropped.
 *
 * `async_system` must be an `orkester_async_t*` handle; the engine clones it
 * internally (the caller retains ownership of the original handle).
 *
 * `content_destroy` is called when the engine evicts content — it receives
 * the `content_ptr` that was passed to `selekt_load_delivery_resolve`.
 * Pass null if content does not need cleanup.
 *
 * `options` may be null to use defaults.
 *
 * # Safety
 * - `async_system` must be a valid `orkester_async_t*` (i.e., a `Box<AsyncSystem>`).
 * - All vtable function pointers must be valid for the lifetime of the engine.
 * - The returned handle must be freed with `selekt_engine_drop`.
 */
struct selekt_engine_t *selekt_engine_create(const void *async_system,
                                             struct selekt_hierarchy_vtable_t hierarchy,
                                             struct selekt_lod_evaluator_vtable_t lod_evaluator,
                                             struct selekt_content_loader_vtable_t content_loader,
                                             struct selekt_hierarchy_resolver_vtable_t hierarchy_resolver,
                                             selekt_content_destroy_fn_t content_destroy,
                                             const struct SelectionOptions *options);

/**
 * Resolve a content load delivery with renderable content.
 *
 * - `content_ptr`: opaque handle to the loaded content (owned by C side).
 *   The engine will call the `content_destroy` callback (from `selekt_engine_create`)
 *   when evicting this content.
 * - `byte_size`: size of the content in bytes (for memory budget tracking).
 *
 * Consumes the delivery handle.
 *
 * # Safety
 * `delivery` must be a valid handle received from a content loader `request` callback.
 */
void selekt_load_delivery_resolve(struct selekt_load_delivery_t *delivery,
                                  void *content_ptr,
                                  uintptr_t byte_size);

/**
 * Resolve a content load delivery as an external hierarchy reference.
 *
 * Consumes the delivery handle.
 *
 * # Safety
 * `delivery` must be a valid handle received from a content loader `request` callback.
 * `key_ptr`/`key_len` must be valid for reading.
 */
void selekt_load_delivery_resolve_reference(struct selekt_load_delivery_t *delivery,
                                            const uint8_t *key_ptr,
                                            uintptr_t key_len,
                                            NodeId source_node,
                                            bool has_transform,
                                            const double (*transform)[16]);

/**
 * Resolve a content load delivery as empty (node has no content).
 *
 * Consumes the delivery handle.
 *
 * # Safety
 * `delivery` must be a valid handle from a content loader `request` callback.
 */
void selekt_load_delivery_resolve_empty(struct selekt_load_delivery_t *delivery);

/**
 * Reject a content load delivery with an error message.
 *
 * Consumes the delivery handle.
 *
 * # Safety
 * `delivery` must be a valid handle from a content loader `request` callback.
 * `error_ptr`/`error_len` must be valid for reading.
 */
void selekt_load_delivery_reject(struct selekt_load_delivery_t *delivery,
                                 const uint8_t *error_ptr,
                                 uintptr_t error_len);

/**
 * Drop a content load delivery without resolving.
 * Equivalent to rejecting with "delivery dropped".
 *
 * # Safety
 * `delivery` must be a valid handle from a content loader `request` callback.
 */
void selekt_load_delivery_drop(struct selekt_load_delivery_t *delivery);

/**
 * Resolve a hierarchy delivery with a patch.
 *
 * - `parent`: the node the patch is anchored to.
 * - `inserted_ptr`/`inserted_len`: new NodeIds inserted beneath `parent`.
 *
 * Pass `inserted_len = 0` to indicate the reference resolved to nothing.
 *
 * Consumes the delivery handle.
 *
 * # Safety
 * `delivery` must be a valid handle from a hierarchy resolver `resolve` callback.
 */
void selekt_hierarchy_delivery_resolve(struct selekt_hierarchy_delivery_t *delivery,
                                       NodeId parent,
                                       const NodeId *inserted_ptr,
                                       uintptr_t inserted_len);

/**
 * Reject a hierarchy delivery with an error.
 *
 * Consumes the delivery handle.
 *
 * # Safety
 * `delivery` must be a valid handle from a hierarchy resolver `resolve` callback.
 */
void selekt_hierarchy_delivery_reject(struct selekt_hierarchy_delivery_t *delivery,
                                      const uint8_t *error_ptr,
                                      uintptr_t error_len);

/**
 * Drop a hierarchy delivery without resolving.
 *
 * # Safety
 * `delivery` must be a valid handle from a hierarchy resolver `resolve` callback.
 */
void selekt_hierarchy_delivery_drop(struct selekt_hierarchy_delivery_t *delivery);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus
