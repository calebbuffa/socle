"""i3s.geospatial — ellipsoid, cartographic, CRS transforms.

Re-exports compiled types from ``i3s._native.geospatial``.
Adds ``CrsTransform`` protocol and ``ProjTransform`` (pyproj-backed).
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

from i3s import _native as _native  # type: ignore[attr-defined]

if TYPE_CHECKING:
    import numpy as np
    import numpy.typing as npt

_mod = _native.geospatial

Cartographic = _mod.Cartographic
Ellipsoid = _mod.Ellipsoid
SceneCoordinateSystem = _mod.SceneCoordinateSystem
TransverseMercatorParams = _mod.TransverseMercatorParams
WkidTransform = _mod.WkidTransform
transverse_mercator_project = _mod.transverse_mercator_project
transverse_mercator_unproject = _mod.transverse_mercator_unproject
utm_params = _mod.utm_params
web_mercator_project = _mod.web_mercator_project
web_mercator_unproject = _mod.web_mercator_unproject


@runtime_checkable
class CrsTransform(Protocol):
    """Protocol for CRS-to-ECEF coordinate transforms.

    Any object that implements ``to_ecef`` can be passed to
    ``SceneLayer()`` as the ``crs_transform`` argument.  The Rust
    backend calls this method during node loading to position
    bounding volumes in ECEF.

    Built-in implementations:

    - ``WkidTransform`` — pure-Rust, zero-copy, GIL-released.
      Fastest path for common EPSG codes (3857, 4269, UTM, …).
    - ``ProjTransform`` — pyproj-backed, supports *any* CRS.
      Acquires the GIL once per node load (~1-8 positions), which
      is negligible vs. network + decode cost.

    You can also implement your own::

        class MyCrs:
            def to_ecef(self, positions: NDArray) -> NDArray:
                ...  # (N, 3) float64 in, (N, 3) float64 out
    """

    def to_ecef(
        self, positions: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...


class ProjTransform:
    """CRS-to-ECEF transform backed by `pyproj <https://pyproj4.github.io/>`_.

    This class conforms to the CRS transform protocol expected by the
    Rust backend (a ``to_ecef(positions)`` method that accepts and returns
    ``(N, 3) float64`` arrays).  It supports *any* CRS that pyproj can
    handle — EPSG codes, WKT strings, PROJ strings, etc.

    Parameters
    ----------
    source_crs : int | str
        Source CRS.  An EPSG code (``int``) or any CRS definition that
        ``pyproj.CRS`` accepts (WKT, PROJ string, authority string, …).

    Examples
    --------
    >>> from i3s.geospatial import ProjTransform
    >>> from i3s.selection import SceneLayer, SceneLayerExternals
    >>> xform = ProjTransform(2230)          # NAD83 / California zone 6 (ftUS)
    >>> layer = SceneLayer.from_url_with_transform(externals, url, xform)

    >>> xform = ProjTransform("EPSG:32617")  # UTM zone 17N via string
    """

    def __init__(self, source_crs: int | str) -> None:
        import pyproj

        if isinstance(source_crs, int):
            src = pyproj.CRS.from_epsg(source_crs)
        else:
            src = pyproj.CRS(source_crs)
        ecef = pyproj.CRS.from_epsg(4978)  # WGS 84 geocentric (ECEF)
        self._transformer = pyproj.Transformer.from_crs(src, ecef, always_xy=True)
        self._source_crs = src

    def to_ecef(self, positions: npt.NDArray) -> npt.NDArray:
        """Transform ``(N, 3)`` positions from source CRS to ECEF.

        Parameters
        ----------
        positions : ndarray, shape (N, 3)
            Source coordinates ``[x, y, z]`` in the source CRS units.

        Returns
        -------
        ndarray, shape (N, 3)
            ECEF coordinates ``[x, y, z]`` in meters.
        """
        import numpy as np

        positions = np.asarray(positions, dtype=np.float64)
        x, y, z = self._transformer.transform(
            positions[:, 0], positions[:, 1], positions[:, 2]
        )
        return np.column_stack((x, y, z))

    def __repr__(self) -> str:
        return f"ProjTransform({self._source_crs.to_epsg() or self._source_crs.name!r})"


__all__ = [
    "Cartographic",
    "CrsTransform",
    "Ellipsoid",
    "ProjTransform",
    "SceneCoordinateSystem",
    "TransverseMercatorParams",
    "WkidTransform",
    "transverse_mercator_project",
    "transverse_mercator_unproject",
    "utm_params",
    "web_mercator_project",
    "web_mercator_unproject",
]
