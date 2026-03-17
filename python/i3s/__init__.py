"""i3s — Python bindings for the i3s-native engine.

The compiled Rust extension lives at ``i3s._native``.  Each submodule
(``geometry``, ``geospatial``, ``async_``, ``selection``) re-exports
the native types and can add pure-Python classes on top::

    from i3s.geometry import BoundingSphere
    from i3s.selection import SceneLayer
"""

__all__ = ["async_", "geometry", "geospatial", "selection"]
