//! Test harness: mock trait implementations for end-to-end FFI testing.
//!
//! Provides a simple two-level hierarchy and instant-resolve content loader
//! so C++ tests can exercise the full selekt pipeline through the FFI.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use orkester::AsyncSystem;
use selekt::{
    ContentKey, ContentLoader, HierarchyPatch, HierarchyPatchError, HierarchyReference,
    HierarchyResolver, LoadPriority, LoadedContent, LodDescriptor, LodEvaluator, NodeId, NodeKind,
    Payload, RefinementMode, RequestId, ResidencyPolicy, SelectionEngine, SelectionOptions,
    SpatialHierarchy, ViewState, VisibilityPolicy, WeightedFairScheduler,
};
use zukei::bounds::SpatialBounds;
use zukei::math::Vec3;


/// Minimal test content — just carries the node ID it was loaded for.
#[derive(Clone, Debug)]
pub struct TestContent {
    pub node_id: NodeId,
}


/// Simple fixed hierarchy: root (0) with children [1, 2].
pub struct TestHierarchy {
    children_of_root: Vec<NodeId>,
    bounds: SpatialBounds,
    lod: LodDescriptor,
    content_keys: [ContentKey; 3],
}

impl TestHierarchy {
    fn new() -> Self {
        Self {
            children_of_root: vec![1, 2],
            bounds: SpatialBounds::Sphere {
                center: Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                radius: 1000.0,
            },
            lod: LodDescriptor {
                family: "geometric_error".to_string(),
                values: vec![100.0],
            },
            content_keys: [
                ContentKey("root".to_string()),
                ContentKey("node_1".to_string()),
                ContentKey("node_2".to_string()),
            ],
        }
    }
}

impl SpatialHierarchy for TestHierarchy {
    fn root(&self) -> NodeId {
        0
    }

    fn parent(&self, node: NodeId) -> Option<NodeId> {
        match node {
            0 => None,
            1 | 2 => Some(0),
            _ => None,
        }
    }

    fn children(&self, node: NodeId) -> &[NodeId] {
        match node {
            0 => &self.children_of_root,
            _ => &[],
        }
    }

    fn node_kind(&self, node: NodeId) -> NodeKind {
        match node {
            0 => NodeKind::Renderable,
            1 | 2 => NodeKind::Renderable,
            _ => NodeKind::Empty,
        }
    }

    fn bounds(&self, _node: NodeId) -> &SpatialBounds {
        &self.bounds
    }

    fn lod_descriptor(&self, _node: NodeId) -> &LodDescriptor {
        &self.lod
    }

    fn refinement_mode(&self, _node: NodeId) -> RefinementMode {
        RefinementMode::Replace
    }

    fn content_key(&self, node: NodeId) -> Option<&ContentKey> {
        match node {
            0 => Some(&self.content_keys[0]),
            1 => Some(&self.content_keys[1]),
            2 => Some(&self.content_keys[2]),
            _ => None,
        }
    }

    fn apply_patch(&mut self, _patch: HierarchyPatch) -> Result<(), HierarchyPatchError> {
        Err(HierarchyPatchError {
            message: "test hierarchy does not support patches".to_string(),
        })
    }
}


/// Configurable LOD evaluator for testing.
pub struct TestLodEvaluator {
    always_refine: bool,
}

impl LodEvaluator for TestLodEvaluator {
    fn should_refine(
        &self,
        _descriptor: &LodDescriptor,
        _view: &ViewState,
        _bounds: &SpatialBounds,
        _mode: RefinementMode,
    ) -> bool {
        self.always_refine
    }
}


/// No-op resolver — test hierarchy has no external references.
pub struct TestHierarchyResolver;

impl HierarchyResolver for TestHierarchyResolver {
    type Error = TestError;

    fn resolve_reference(
        &self,
        async_system: &AsyncSystem,
        _reference: HierarchyReference,
    ) -> orkester::Future<Result<Option<HierarchyPatch>, Self::Error>> {
        async_system.resolved(Ok(None))
    }
}


/// Instantly-resolving content loader. Every request returns `Payload::Renderable`
/// with a `TestContent` containing the node ID.
pub struct TestContentLoader {
    next_request_id: AtomicU64,
}

impl TestContentLoader {
    fn new() -> Self {
        Self {
            next_request_id: AtomicU64::new(1),
        }
    }
}

impl ContentLoader<TestContent> for TestContentLoader {
    type Error = TestError;

    fn request(
        &self,
        async_system: &AsyncSystem,
        node_id: NodeId,
        _key: &ContentKey,
        _priority: LoadPriority,
    ) -> (
        RequestId,
        orkester::Future<Result<LoadedContent<TestContent>, Self::Error>>,
    ) {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let content = LoadedContent {
            payload: Payload::Renderable(TestContent { node_id }),
            byte_size: 1024,
        };
        let future = async_system.resolved(Ok(content));
        (request_id, future)
    }

    fn cancel(&self, _request_id: RequestId) -> bool {
        false
    }
}


/// Always-visible, no-eviction policy for testing.
pub struct TestPolicy;

impl VisibilityPolicy for TestPolicy {
    fn is_visible(&self, _node_id: NodeId, _bounds: &SpatialBounds, _view: &ViewState) -> bool {
        true
    }
}

impl ResidencyPolicy for TestPolicy {
    fn select_evictions(
        &self,
        _resident_nodes: &[(NodeId, usize)],
        _memory_budget_bytes: usize,
        _out: &mut Vec<NodeId>,
    ) {
        // no eviction
    }
}


#[derive(Clone, Debug)]
pub struct TestError(String);

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TestError {}


pub fn create_test_engine(
    always_refine: bool,
) -> SelectionEngine<
    TestContent,
    TestHierarchy,
    TestLodEvaluator,
    TestHierarchyResolver,
    TestContentLoader,
    WeightedFairScheduler,
    TestPolicy,
> {
    let async_system = AsyncSystem::with_threads(2);

    SelectionEngine::new(
        async_system,
        TestHierarchy::new(),
        TestLodEvaluator { always_refine },
        TestHierarchyResolver,
        TestContentLoader::new(),
        Arc::new(Mutex::new(WeightedFairScheduler::new())),
        TestPolicy,
        SelectionOptions::default(),
    )
}
