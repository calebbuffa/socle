/// Scheduling context for continuations and task dispatch.
///
/// Determines which thread/pool a continuation or task runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Context {
    /// Background thread pool (via the configured [`TaskProcessor`](crate::TaskProcessor)).
    Worker,
    /// Main-thread dispatch queue. Requires explicit pumping via
    /// [`AsyncSystem::dispatch_main_thread_tasks`](crate::AsyncSystem::dispatch_main_thread_tasks)
    /// or runs inline if called from within
    /// [`AsyncSystem::enter_main_thread`](crate::AsyncSystem::enter_main_thread).
    Main,
    /// Inline on the thread that completed the prior stage.
    /// No scheduling overhead, but blocks the completing thread.
    Immediate,
}
