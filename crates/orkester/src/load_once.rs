//! Deduplicating concurrent loader.
//!
//! [`LoadOnce`] ensures that multiple concurrent callers requesting the same
//! key trigger exactly one in-flight computation. All callers that arrive
//! while the first load is still running receive a clone of the same
//! [`Handle<V>`] and share the result when it completes.
//!
//! Once a key has resolved, the result is **not** retained — subsequent calls
//! for the same key trigger a new load. If you need persistent caching, wrap
//! the resolved value in your own data structure. This deliberate simplicity
//! avoids baking eviction policy, lifetime management, or `Arc` ownership
//! conventions into the library.
//!
//! # Example
//!
//! ```rust,ignore
//! use orkester::{LoadOnce, ThreadPool};
//!
//! let pool = ThreadPool::default();
//! let ctx  = pool.context();
//! let loader: LoadOnce<String, Vec<u8>> = LoadOnce::new();
//!
//! // Two concurrent callers for the same key — only one fetch runs.
//! let a = loader.get_or_load("config.json".to_owned(), |key| {
//!     ctx.run(|| fetch_bytes(key))
//! });
//! let b = loader.get_or_load("config.json".to_owned(), |key| {
//!     ctx.run(|| fetch_bytes(key))   // factory is NOT called; `a`'s handle is returned
//! });
//!
//! assert_eq!(a.block().unwrap(), b.block().unwrap());
//! ```

use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

use crate::task::Task;

/// Deduplicating concurrent loader.
///
/// Cloning is cheap — all clones share the same in-flight state.
///
/// See the [module docs](self) for a full example.
#[derive(Clone)]
pub struct LoadOnce<K, V>
where
    K: Eq + Hash + Send + 'static,
    V: Clone + Send + 'static,
{
    in_flight: Arc<Mutex<HashMap<K, crate::Handle<V>>>>,
}

impl<K, V> LoadOnce<K, V>
where
    K: Eq + Hash + Send + 'static,
    V: Clone + Send + 'static,
{
    /// Create a new, empty loader.
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl<K, V> LoadOnce<K, V>
where
    K: Eq + Hash + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    /// Return a task for `key`, starting a new load via `factory` only if no
    /// load for that key is currently in-flight.
    ///
    /// - If a load for `key` is already running, returns a new handle backed
    ///   by the **same** underlying task.
    /// - If no load is running, calls `factory(key)` with the lock **released**
    ///   so factories that call `get_or_load` recursively are safe.
    /// - When the task completes (successfully or not), the key is removed from
    ///   the in-flight set so subsequent calls start fresh.
    pub fn get_or_load<F>(&self, key: K, factory: F) -> crate::Handle<V>
    where
        F: FnOnce(&K) -> Task<V> + Send + 'static,
    {
        // Fast path: already in flight.
        {
            let guard = self.in_flight.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(handle) = guard.get(&key) {
                return handle.clone();
            }
        }

        // Call factory with the lock fully released so:
        //   (a) factories that call `get_or_load` recursively are safe, and
        //   (b) ready tasks whose `.map()` closure runs inline cannot deadlock.
        let load_task = factory(&key);

        // If the task is already resolved there is nothing to deduplicate —
        // no concurrent caller could join an already-finished load. Skip the
        // in-flight map and return a handle directly.
        if load_task.is_ready() {
            return load_task.share();
        }

        // Pending task: re-acquire to insert, after a double-check.
        let mut guard = self.in_flight.lock().unwrap_or_else(|p| p.into_inner());

        if let Some(handle) = guard.get(&key) {
            // Another thread raced us; discard load_task (Resolver drop auto-rejects
            // any chained continuations, but nobody holds a reference to ours).
            return handle.clone();
        }

        // Wire up auto-removal on completion. The lock is NOT held during
        // this call because `load_task` is Pending (its map closure will only
        // execute later, on a different call stack).
        let handle = {
            let in_flight = Arc::clone(&self.in_flight);
            let key2 = key.clone();
            load_task
                .map(move |v| {
                    in_flight
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .remove(&key2);
                    v
                })
                .share()
        };

        guard.insert(key, handle.clone());
        drop(guard);

        handle
    }

    /// Number of keys currently being loaded.
    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    /// Returns `true` if no loads are currently in-flight.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.in_flight_count() == 0
    }
}

impl<K, V> Default for LoadOnce<K, V>
where
    K: Eq + Hash + Send + 'static,
    V: Clone + Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> fmt::Debug for LoadOnce<K, V>
where
    K: Eq + Hash + Clone + Send + 'static,
    V: Clone + Send + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadOnce")
            .field("in_flight", &self.in_flight_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Task;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn single_load_for_concurrent_requests() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let loader: LoadOnce<&str, String> = LoadOnce::new();

        let cc = Arc::clone(&call_count);
        let (resolver, task) = crate::task::create_pair::<String>();

        // First call — starts the load.
        let h1 = loader.get_or_load("key", move |_| {
            cc.fetch_add(1, Ordering::SeqCst);
            task
        });

        // Second call while still in-flight — must reuse h1's future.
        let cc2 = Arc::clone(&call_count);
        let h2 = loader.get_or_load("key", move |_| {
            cc2.fetch_add(1, Ordering::SeqCst);
            Task::ready("should not be called".to_owned())
        });

        assert_eq!(call_count.load(Ordering::SeqCst), 1, "factory called twice");
        assert_eq!(loader.in_flight_count(), 1);

        resolver.resolve("hello".to_owned());

        assert_eq!(h1.block().unwrap(), "hello");
        assert_eq!(h2.block().unwrap(), "hello");

        // After completion the entry is removed.
        assert_eq!(loader.in_flight_count(), 0);
    }

    #[test]
    fn subsequent_call_starts_new_load() {
        let loader: LoadOnce<&str, i32> = LoadOnce::new();

        let (r1, t1) = crate::task::create_pair::<i32>();
        let h1 = loader.get_or_load("k", |_| t1);
        r1.resolve(1);
        assert_eq!(h1.block().unwrap(), 1);

        // Now the key is no longer in-flight — a new factory call must occur.
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        let h2 = loader.get_or_load("k", move |_| {
            cc.fetch_add(1, Ordering::SeqCst);
            Task::ready(2)
        });
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(h2.block().unwrap(), 2);
    }
}
