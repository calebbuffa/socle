//! Integration tests for the `SelectionEngine` using mock types.

use std::sync::{Arc, Mutex};

use glam::DVec3;
use orkester::{Context, ThreadPool};
use selekt::*;
use zukei::SpatialBounds;

#[derive(Debug, Clone)]
struct MockContent {
    label: String,
}

struct MockHierarchy {
    nodes: Vec<MockNode>,
}

struct MockNode {
    #[allow(dead_code)]
    id: NodeId,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    kind: NodeKind,
    bounds: SpatialBounds,
    lod: LodDescriptor,
    refinement: RefinementMode,
    content_keys: Vec<ContentKey>,
}

impl MockHierarchy {
    fn two_level() -> Self {
        let root = MockNode {
            id: NodeId::from_index(0),
            parent: None,
            children: vec![NodeId::from_index(1), NodeId::from_index(2)],
            kind: NodeKind::Renderable,
            bounds: SpatialBounds::Sphere {
                center: DVec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                radius: 100.0,
            },
            lod: LodDescriptor {
                family: LodFamily::NONE,
                value: 100.0,
            },
            refinement: RefinementMode::Replace,
            content_keys: vec![ContentKey("root".into())],
        };
        let child0 = MockNode {
            id: NodeId::from_index(1),
            parent: Some(NodeId::from_index(0)),
            children: vec![],
            kind: NodeKind::Renderable,
            bounds: SpatialBounds::Sphere {
                center: DVec3 {
                    x: -50.0,
                    y: 0.0,
                    z: 0.0,
                },
                radius: 50.0,
            },
            lod: LodDescriptor {
                family: LodFamily::NONE,
                value: 10.0,
            },
            refinement: RefinementMode::Replace,
            content_keys: vec![ContentKey("child0".into())],
        };
        let child1 = MockNode {
            id: NodeId::from_index(2),
            parent: Some(NodeId::from_index(0)),
            children: vec![],
            kind: NodeKind::Renderable,
            bounds: SpatialBounds::Sphere {
                center: DVec3 {
                    x: 50.0,
                    y: 0.0,
                    z: 0.0,
                },
                radius: 50.0,
            },
            lod: LodDescriptor {
                family: LodFamily::NONE,
                value: 10.0,
            },
            refinement: RefinementMode::Replace,
            content_keys: vec![ContentKey("child1".into())],
        };
        Self {
            nodes: vec![root, child0, child1],
        }
    }

    fn node(&self, id: NodeId) -> &MockNode {
        &self.nodes[id.index()]
    }
}

impl SpatialHierarchy for MockHierarchy {
    fn root(&self) -> NodeId {
        NodeId::from_index(0)
    }
    fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.node(node).parent
    }
    fn children(&self, node: NodeId) -> &[NodeId] {
        &self.node(node).children
    }
    fn node_kind(&self, node: NodeId) -> NodeKind {
        self.node(node).kind
    }
    fn bounds(&self, node: NodeId) -> &SpatialBounds {
        &self.node(node).bounds
    }
    fn lod_descriptor(&self, node: NodeId) -> &LodDescriptor {
        &self.node(node).lod
    }
    fn refinement_mode(&self, node: NodeId) -> RefinementMode {
        self.node(node).refinement
    }
    fn content_keys(&self, node: NodeId) -> &[ContentKey] {
        &self.node(node).content_keys
    }
    fn expand(&mut self, _patch: HierarchyExpansion) -> Result<(), HierarchyExpansionError> {
        Ok(())
    }
}

struct AlwaysRefine;

impl LodEvaluator for AlwaysRefine {
    fn should_refine(
        &self,
        _descriptor: &LodDescriptor,
        _view: &ViewState,
        _multiplier: f32,
        _bounds: &SpatialBounds,
        _mode: RefinementMode,
    ) -> bool {
        true
    }
}

// Never-refine evaluator - keep root selected.
struct NeverRefine;

impl LodEvaluator for NeverRefine {
    fn should_refine(
        &self,
        _descriptor: &LodDescriptor,
        _view: &ViewState,
        _multiplier: f32,
        _bounds: &SpatialBounds,
        _mode: RefinementMode,
    ) -> bool {
        false
    }
}

struct MockLoader;

impl MockLoader {
    fn new() -> Self {
        Self
    }
}

impl ContentLoader<MockContent> for MockLoader {
    type Error = std::io::Error;

    fn load(
        &self,
        _bg: &Context,
        _main: &orkester::Context,
        node_id: NodeId,
        _key: &ContentKey,
        _cancel: orkester::CancellationToken,
    ) -> orkester::Task<Result<LoadResult<MockContent>, Self::Error>> {
        let content = MockContent {
            label: format!("node_{node_id}"),
        };
        orkester::resolved(Ok(LoadResult::Content {
            content: Some(content),
            byte_size: 1024,
        }))
    }
}

fn make_engine(bg_context: Context, lod: impl LodEvaluator + 'static) -> SelectionEngine<MockContent> {
    SelectionEngineBuilder::new(
        bg_context,
        MockHierarchy::two_level(),
        lod,
        MockLoader::new(),
    ).build()
}

fn test_view() -> ViewState {
    ViewState::perspective(
        DVec3 {
            x: 0.0,
            y: 0.0,
            z: 200.0,
        },
        DVec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
        DVec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        [800, 600],
        std::f64::consts::FRAC_PI_4,
        std::f64::consts::FRAC_PI_4 * 0.75,
    )
}

fn make_bg_context() -> (ThreadPool, Context) {
    let pool = ThreadPool::new(1);
    let ctx = pool.context();
    (pool, ctx)
}

// ===========================================================================
// Tests
// ===========================================================================

#[test]
fn engine_selects_root_when_no_refinement() {
    let (_pool, bg_context) = make_bg_context();
    let mut engine = make_engine(bg_context, NeverRefine);

    let handle = engine.add_view_group(1.0);
    let stats = engine.update_view_group(handle, &[test_view()]);

    // Root should be queued for load (not yet renderable).
    // No children should be queued because NeverRefine prevents descent.
    assert_eq!(stats.queued_requests, 1, "only root should be queued");
    assert_eq!(stats.nodes_visited, 1, "only root visited");
}

#[test]
fn engine_refines_and_loads_children() {
    let (_pool, bg_context) = make_bg_context();
    let mut engine = make_engine(bg_context, AlwaysRefine);

    let handle = engine.add_view_group(1.0);

    // Frame 1: traversal queues root + children
    let stats = engine.update_view_group(handle, &[test_view()]);
    assert!(stats.queued_requests >= 1, "should queue at least root");

    // Load pass: dispatches and completes (mock returns immediately)
    let load = engine.load();
    assert!(load.started_requests > 0, "should start requests");
}

#[test]
fn engine_builder_sets_options() {
    let (_pool, bg_context) = make_bg_context();
    let opts = SelectionOptions {
        loading: selekt::LoadingOptions {
            max_simultaneous_loads: 5,
            retry_limit: 10,
            ..Default::default()
        },
        ..Default::default()
    };
    let engine = SelectionEngineBuilder::new(
            bg_context,
            MockHierarchy::two_level(),
            NeverRefine,
            MockLoader::new(),
        )
        .with_options(opts)
        .build();

    assert_eq!(engine.options().loading.max_simultaneous_loads, 5);
    assert_eq!(engine.options().loading.retry_limit, 10);
}

#[test]
fn engine_view_group_lifecycle() {
    let (_pool, bg_context) = make_bg_context();
    let mut engine = make_engine(bg_context, NeverRefine);

    let handle = engine.add_view_group(1.0);
    assert!(engine.is_view_group_active(handle));

    assert!(engine.remove_view_group(handle));
    assert!(!engine.is_view_group_active(handle));

    // Updating a removed handle returns default (empty) stats.
    let stats = engine.update_view_group(handle, &[test_view()]);
    assert!(engine.view_group_result(handle).is_none());
    assert_eq!(stats.nodes_visited, 0);
}

#[test]
fn engine_load_progress_starts_at_100() {
    let (_pool, bg_context) = make_bg_context();
    let engine = make_engine(bg_context, NeverRefine);

    // No nodes tracked yet = 100%
    assert_eq!(engine.compute_load_progress(), 100.0);
}

#[test]
fn engine_content_none_before_load() {
    let (_pool, bg_context) = make_bg_context();
    let engine = make_engine(bg_context, NeverRefine);

    assert!(engine.content(NodeId::from_index(0)).is_none());
    assert!(engine.content(NodeId::from_index(999)).is_none());
}

#[test]
fn engine_on_load_error_callback() {
    let (_pool, bg_context) = make_bg_context();
    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let errors_clone = Arc::clone(&errors);

    let mut engine = SelectionEngineBuilder::new(
        bg_context,
        MockHierarchy::two_level(),
        NeverRefine,
        MockLoader::new(),
    )
    .on_error(move |details| {
        errors_clone.lock().unwrap().push(details.message.clone());
    })
    .build();

    // The callback is set - we can verify it's installed even without triggering errors.
    let handle = engine.add_view_group(1.0);
    engine.update_view_group(handle, &[test_view()]);
    engine.load();

    // No errors expected with MockLoader (always succeeds).
    assert!(errors.lock().unwrap().is_empty());
}

#[test]
fn engine_loads_content_to_renderable() {
    let (_pool, bg_context) = make_bg_context();
    let mut engine = make_engine(bg_context, NeverRefine);

    let handle = engine.add_view_group(1.0);

    // Frame 1: queue root
    engine.update_view_group(handle, &[test_view()]);
    // Load: dispatch and immediately complete (mock loader)
    engine.load();

    // Frame 2: root should be renderable now and selected
    engine.update_view_group(handle, &[test_view()]);
    let selected = &engine.view_group_result(handle).unwrap().nodes_to_render;
    let root = NodeId::from_index(0);
    assert!(
        selected.contains(&root),
        "root should be selected after loading"
    );
    assert!(
        engine.content(root).is_some(),
        "root should have content after loading"
    );
    assert_eq!(engine.content(root).unwrap().label, "node_0");
}

#[test]
fn projection_accessors() {
    let view = ViewState::perspective(
        DVec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        DVec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
        DVec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        [800, 600],
        1.0,
        0.75,
    );
    assert_eq!(view.fov_x(), Some(1.0));
    assert_eq!(view.fov_y(), Some(0.75));

    let ortho_view = ViewState::orthographic(
        DVec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        DVec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
        DVec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        },
        [800, 600],
        100.0,
        75.0,
    );
    assert_eq!(ortho_view.fov_x(), None);
    assert_eq!(ortho_view.fov_y(), None);
}

#[test]
fn engine_frame_index_increments() {
    let (_pool, bg_context) = make_bg_context();
    let mut engine = make_engine(bg_context, NeverRefine);

    assert_eq!(engine.frame_index(), 0);

    let handle = engine.add_view_group(1.0);
    engine.update_view_group(handle, &[test_view()]);
    assert_eq!(engine.frame_index(), 1);

    engine.update_view_group(handle, &[test_view()]);
    assert_eq!(engine.frame_index(), 2);
}

#[test]
fn per_view_selected_accessor() {
    let (_pool, bg_context) = make_bg_context();
    let mut engine = make_engine(bg_context, NeverRefine);

    let handle = engine.add_view_group(1.0);

    // Load root first
    engine.update_view_group(handle, &[test_view()]);
    engine.load();

    // Now root should be selected
    engine.update_view_group(handle, &[test_view()]);
    let selected = &engine.view_group_result(handle).unwrap().nodes_to_render;
    let root = NodeId::from_index(0);
    assert!(selected.contains(&root), "root should be in selection");
}
