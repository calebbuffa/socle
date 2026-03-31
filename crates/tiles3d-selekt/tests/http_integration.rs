//! Integration tests that load real 3D Tiles tilesets over HTTP.
//!
//! These tests require network access and are therefore marked `#[ignore]`.
//! Run them with:
//!
//! ```text
//! cargo test -p tiles3d-selekt -- --ignored
//! ```

use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use egaku::PrepareRendererResources;
use glam::DVec3;
use moderu::GltfModel;
use orkester::{Context, ThreadPool, WorkQueue};
use orkester_io::{AssetAccessor, HttpAccessor};
use selekt::{SelectionEngine, SelectionEngineBuilder, ViewState};
use tiles3d_selekt::TilesetLoaderFactory;

#[derive(Clone)]
struct CountingPreparer {
    count: Arc<AtomicUsize>,
}

impl CountingPreparer {
    fn new() -> Self {
        Self {
            count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn decoded(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }
}

impl PrepareRendererResources for CountingPreparer {
    type WorkerResult = usize;
    type Content = usize;
    type Error = Infallible;

    fn prepare_in_load_thread(&self, _model: GltfModel) -> Result<usize, Infallible> {
        Ok(1)
    }

    fn prepare_in_main_thread(&self, mesh_count: usize) -> usize {
        self.count.fetch_add(1, Ordering::SeqCst);
        mesh_count
    }
}

/// Camera at 20 000 km above the North Pole — every Earth tile has a tiny
/// geometric-error SSE, so the root tile(s) are always selected for rendering
/// without requiring further refinement.
fn far_view() -> ViewState {
    ViewState::perspective(
        DVec3::new(0.0, 0.0, 20_000_000.0),
        DVec3::new(0.0, 0.0, -1.0),
        DVec3::new(1.0, 0.0, 0.0),
        [1280, 720],
        std::f64::consts::FRAC_PI_3,
        std::f64::consts::FRAC_PI_4,
    )
}

fn make_bg_context() -> (ThreadPool, Context) {
    let pool = ThreadPool::new(4);
    let ctx = pool.context();
    (pool, ctx)
}

/// Drive the engine until at least one piece of content has been decoded, or
/// the deadline passes (test fails).
fn pump_until_content(
    engine: &mut SelectionEngine<usize>,
    handle: selekt::ViewGroupHandle,
    view: &ViewState,
    preparer: &CountingPreparer,
    main_queue: &mut WorkQueue,
) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        engine.update_view_group(handle, &[view.clone()]);
        engine.load();
        main_queue.pump_timed(Duration::from_millis(16));

        if preparer.decoded() > 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for tile content"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore]
fn trakai_castle_loads_content() {
    const URL: &str = "https://tiles.arcgis.com/tiles/P3ePLMYs2RVChkJx/arcgis/rest/services/Trakai_Island_Castle/3DTilesServer/tileset.json";

    let (_pool, bg_context) = make_bg_context();
    let preparer = CountingPreparer::new();
    let accessor: Arc<dyn AssetAccessor> = Arc::new(HttpAccessor::new(bg_context.clone()));

    let mut main_queue = WorkQueue::new();
    let main_context = main_queue.context();

    let factory = TilesetLoaderFactory::new(URL, Arc::new(preparer.clone()));
    let (config, _attribution) = factory
        .create(bg_context, &accessor)
        .block()
        .expect("task failed")
        .expect("failed to load tileset.json");
    let config = config.with_main_context(main_context);

    let mut engine = config.build();
    let view = far_view();
    let handle = engine.add_view_group(1.0);
    pump_until_content(&mut engine, handle, &view, &preparer, &mut main_queue);

    assert!(preparer.decoded() > 0, "no tile content was decoded");
}
