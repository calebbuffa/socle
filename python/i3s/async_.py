"""i3s.async_ — async system, futures, task processor.

Re-exports compiled types from ``i3s._native.async_``.
"""

from i3s import _native as _native  # type: ignore[attr-defined]

_mod = getattr(_native, "async_")

NativeTaskProcessor = _mod.NativeTaskProcessor
AsyncSystem = _mod.AsyncSystem
Future = _mod.Future

__all__ = [
    "AsyncSystem",
    "Future",
    "NativeTaskProcessor",
]
