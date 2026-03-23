/// A scheduling context identifying where a task should execute.
///
/// Built-in contexts:
/// - [`Context::BACKGROUND`] — background thread pool
/// - [`Context::MAIN`] — main/UI thread queue
/// - [`Context::IMMEDIATE`] — inline on current thread
///
/// Custom contexts can be registered via
/// [`AsyncSystem::register_context`](crate::AsyncSystem::register_context).
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct Context(pub(crate) u32);

impl Context {
    /// Background thread pool (via the configured [`Executor`](crate::Executor)).
    pub const BACKGROUND: Context = Context(0);

    /// Main-thread dispatch queue. Requires explicit pumping via
    /// [`AsyncSystem::flush_main`](crate::AsyncSystem::flush_main).
    pub const MAIN: Context = Context(1);

    /// Inline on the thread that completed the prior stage.
    /// No scheduling overhead, but blocks the completing thread.
    pub const IMMEDIATE: Context = Context(u32::MAX);
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Context::BACKGROUND => write!(f, "Context::BACKGROUND"),
            Context::MAIN => write!(f, "Context::MAIN"),
            Context::IMMEDIATE => write!(f, "Context::IMMEDIATE"),
            Context(id) => write!(f, "Context({id})"),
        }
    }
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Context::BACKGROUND => write!(f, "Background"),
            Context::MAIN => write!(f, "Main"),
            Context::IMMEDIATE => write!(f, "Immediate"),
            Context(id) => write!(f, "Custom({id})"),
        }
    }
}
