"""Type stubs for ``i3s.async_`` — async system, futures, task processor."""

from __future__ import annotations

from collections.abc import Generator
from typing import Any, Generic, TypeVar

_T = TypeVar("_T")

class NativeTaskProcessor:
    """Thread-pool task processor."""

    def __init__(self, num_threads: int = 0) -> None: ...
    def __repr__(self) -> str: ...

class AsyncSystem:
    """Async system: owns a worker thread pool and main-thread task queue."""

    def __init__(self, task_processor: NativeTaskProcessor) -> None: ...
    def dispatch_main_thread_tasks(self) -> int: ...
    def has_pending_main_thread_tasks(self) -> bool: ...
    def __repr__(self) -> str: ...

class Future(Generic[_T]):
    """A future that resolves to a value of type *T*.

    Supports ``await future`` in asyncio code.

    .. note::
        ``wait()`` and ``wait_in_main_thread()`` are **single-shot**: each
        call consumes the future.  Calling either method a second time raises
        ``RuntimeError``.
    """

    def __class_getitem__(cls, item: _T) -> type[Future[_T]]: ...
    def wait(self) -> _T:
        """Block until the result is available and return it.

        Releases the GIL while waiting.  Can only be called once.
        """
        ...
    def wait_in_main_thread(self, async_system: AsyncSystem) -> _T:
        """Block while pumping the main-thread task queue until resolved.

        Use this when the future's completion depends on main-thread
        callbacks (e.g. ``prepare_in_main_thread``).  Can only be called
        once.
        """
        ...
    def is_ready(self) -> bool: ...
    def __await__(self) -> Generator[Any, None, _T]: ...
    def __repr__(self) -> str: ...
