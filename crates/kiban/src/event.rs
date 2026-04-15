//! Unified event stream for the kiban runtime.

use kasane::OverlayEvent;
use selekt::NodeId;

/// A lifecycle event emitted by the kiban runtime each frame.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Event {
    /// A node's content has been evicted from the cache.
    ContentEvicted { node_id: NodeId },
    /// An overlay engine lifecycle event (e.g. overlay attached/detached).
    Overlay(OverlayEvent),
}

impl From<OverlayEvent> for Event {
    fn from(e: OverlayEvent) -> Self {
        Event::Overlay(e)
    }
}
