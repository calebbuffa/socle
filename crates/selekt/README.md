# selekt

Format-agnostic tile selection engine for spatial data.

*selekt is for deciding what to render from hierarchical spatial datasets.*

## Overview

selekt is a runtime-agnostic selection and load-orchestration core for tiled spatial formats.
It handles LOD traversal, content loading, residency management, and async load scheduling —
with no dependency on any specific tile format (3D Tiles, I3S, etc).

**Core types:**

- **`SelectionEngine<C>`** — the main engine, generic over content type `C`
- **`SpatialHierarchy`** — trait for providing the spatial node tree
- **`ContentLoader<C>`** — trait for async content fetching and decoding
- **`LodEvaluator`** — trait for LOD metric evaluation
- **`ViewState`** — camera/view parameters for selection
- **`ViewUpdateResult`** — per-frame selection output (selected nodes, load requests)

## Design Principles

- **Format-agnostic**: no I3S, 3D Tiles, or glTF concepts in core
- **Content-generic**: `SelectionEngine<C>` stores loaded content of any type `C`
- **No GPU coupling**: selekt decides *what* to render, not *how* — GPU preparation lives in `belag`
- **Deterministic**: synchronous traversal and load orchestration with explicit main-thread pumping
- **Async-internal**: uses `orkester` futures internally, but the public API is synchronous

## Implementor Guide

### What to implement

Adapting selekt to a new spatial format requires three traits:

#### 1. `SpatialHierarchy`

Provides the node tree: bounds, LOD descriptors, children, content keys, and refinement rules.

```rust
struct MyHierarchy { /* your data */ }

impl SpatialHierarchy for MyHierarchy {
    fn root(&self) -> NodeId { NodeId::from_raw(0) }
    fn parent(&self, node: NodeId) -> Option<NodeId> { /* ... */ }
    fn children(&self, node: NodeId) -> &[NodeId] { /* ... */ }
    fn node_kind(&self, node: NodeId) -> NodeKind { /* ... */ }
    fn bounds(&self, node: NodeId) -> &SpatialBounds { /* ... */ }
    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor { /* ... */ }
    fn refinement_mode(&self, node: NodeId) -> RefinementMode { RefinementMode::Replace }
    fn content_keys(&self, node: NodeId) -> &[ContentKey] { /* ... */ }
    fn expand(&mut self, patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError> {
        Ok(()) // no-op if your hierarchy is fully pre-loaded
    }
}
```

#### 2. `ContentLoader<C>`

Fetches and decodes a single node's content asynchronously.
The engine issues one `load()` call per node and holds the `CancellationToken` —
call `token.cancel()` from the engine side is handled automatically on eviction;
your loader just honors it via `.with_cancellation(&cancel)` on I/O futures.

```rust
struct MyLoader { accessor: Arc<dyn AssetAccessor> }

impl ContentLoader<MyContent> for MyLoader {
    type Error = MyError;

    fn load(
        &self,
        bg: &Context,
        _main: &Context,
        _node: NodeId,
        key: &ContentKey,
        cancel: CancellationToken,
    ) -> Task<Result<LoadResult<MyContent>, MyError>> {
        let accessor = self.accessor.clone();
        let url = key.0.clone();
        bg.run(move || {
            let bytes = accessor.get(&url).with_cancellation(&cancel).block()??;
            let content = MyContent::decode(&bytes)?;
            Ok(LoadResult::Content { content: Some(content), byte_size: bytes.len() })
        })
    }
}
```

#### 3. `LodEvaluator`

Decides whether a node should be refined into its children based on the current view.
For screen-space error (3D Tiles style):

```rust
struct GeometricErrorEvaluator { threshold_pixels: f64 }

impl LodEvaluator for GeometricErrorEvaluator {
    fn should_refine(
        &self, desc: &LodDescriptor, view: &ViewState,
        multiplier: f64, bounds: &SpatialBounds, mode: RefinementMode,
    ) -> bool {
        // compute SSE and compare to threshold_pixels * multiplier
    }
}
```

### Sync construction (hierarchy known up-front)

```rust
let mut main_queue = orkester::WorkQueue::new();
let pool = orkester::ThreadPool::new(4);
let bg = pool.context();

let engine = SelectionEngineBuilder::new(hierarchy, lod, loader)
    .with_main_context(main_queue.context())
    .build();

let view_group = engine.add_view_group(1.0);

// Per frame:
engine.update_view_group(view_group, &[camera]);
engine.load();
main_queue.pump_timed(Duration::from_millis(4)); // finalize GPU uploads

for node in &engine.view_group_result(view_group).nodes_to_render {
    if let Some(content) = engine.content(*node) {
        render(content);
    }
}
```

### Async construction (hierarchy loaded from network)

```rust
let engine_task = bg.run(move || {
    let hierarchy = MyHierarchy::fetch_from_network(&url)?;
    Ok::<_, MyError>(
        SelectionEngineBuilder::new(hierarchy, lod, loader)
            .with_main_context(main_queue.context())
            .build()
    )
});

// Poll each frame until ready; returns empty results in the meantime:
match engine_task.poll() {
    Some(Ok(engine)) => { /* engine ready */ }
    Some(Err(e))     => { /* loading failed */ }
    None             => { /* still loading */ }
}
```

### Optional extensions

| Extension | Trait | Purpose |
|---|---|---|
| Frustum culling | `VisibilityPolicy` | Default: `FrustumVisibilityPolicy` |
| Memory eviction | `ResidencyPolicy` | Default: `LruResidencyPolicy` |
| External refs | `HierarchyResolver` | For formats with child-tileset references |
| Node exclusion | `NodeExcluder` | Runtime per-node hide/show |
| Occlusion | `OcclusionTester` | Feed GPU occlusion query results back in |

### Provided defaults

| Item | Default |
|---|---|
| Visibility policy | `FrustumVisibilityPolicy` (perspective + orthographic frustum) |
| Residency policy | `LruResidencyPolicy` (evict least-recently-rendered first) |
| Load scheduler | `WeightedFairScheduler` (fair across multiple view groups) |
| Hierarchy resolver | No-op (panic on `LoadResult::Reference` if not set) |

## License

Apache-2.0
