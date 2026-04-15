//! Content preparation pipeline for tile format loaders.
//!
//! [`ContentPipeline`] is a single-closure pipeline that transforms a
//! [`GltfModel`] into renderer-ready content. The pipeline captures its
//! own [`orkester::Context`]s and decides internally what runs where:
//!
//! ```text
//! pipeline.run(model)   → Task<Result<C, PipelineError>>
//!   ├─ bg.run(|| decode(model))        ← user-chosen context
//!   └─ main.run(|| upload(cpu_data))   ← user-chosen context
//! ```
//!
//! Loaders only need one background context for the asset fetch; the
//! pipeline handles all scheduling of decode/upload internally.

use moderu::GltfModel;
use orkester::Task;

/// Error type returned by a content pipeline.
pub type PipelineError = Box<dyn std::error::Error + Send + Sync>;

/// Content preparation pipeline.
///
/// A single opaque function: [`GltfModel`] → [`Task<Result<C, PipelineError>>`].
/// The pipeline captures its own contexts and decides scheduling internally.
///
/// # Type parameter
///
/// * `C` — Final renderer-ready content (e.g. `Vec<GpuTile>`, `usize` for tests)
pub struct ContentPipeline<C: Send + 'static> {
    run: Box<dyn Fn(GltfModel) -> Task<Result<C, PipelineError>> + Send + Sync>,
    free: Option<Box<dyn Fn(C) + Send + Sync>>,
}

impl<C: Send + 'static> ContentPipeline<C> {
    /// Create a pipeline from a closure that transforms a model into content.
    ///
    /// The closure should capture any [`orkester::Context`]s it needs and
    /// schedule work on them internally.
    pub fn new(
        run: impl Fn(GltfModel) -> Task<Result<C, PipelineError>> + Send + Sync + 'static,
    ) -> Self {
        Self {
            run: Box::new(run),
            free: None,
        }
    }

    /// Set an optional callback to release content when evicted.
    pub fn on_free(mut self, f: impl Fn(C) + Send + Sync + 'static) -> Self {
        self.free = Some(Box::new(f));
        self
    }

    /// Run the pipeline on a model. Returns a task that resolves to content.
    pub fn run(&self, model: GltfModel) -> Task<Result<C, PipelineError>> {
        (self.run)(model)
    }

    /// Release content (GPU cleanup). No-op if no `on_free` was set.
    pub fn free_content(&self, content: C) {
        if let Some(f) = &self.free {
            f(content);
        }
    }
}
