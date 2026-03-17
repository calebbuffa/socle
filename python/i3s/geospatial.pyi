"""Type stubs for ``i3s.geospatial`` — ellipsoid, cartographic, CRS transforms."""

from __future__ import annotations

from enum import IntEnum
from typing import Protocol, overload, runtime_checkable

import numpy as np
import numpy.typing as npt

class Cartographic:
    """A geographic position in radians + height."""

    longitude: float
    latitude: float
    height: float

    def __init__(self, longitude: float, latitude: float, height: float) -> None: ...
    @overload
    @staticmethod
    def from_degrees(
        longitude: float, latitude: float, height: float = 0.0
    ) -> Cartographic: ...
    @overload
    @staticmethod
    def from_degrees(
        positions: npt.NDArray[np.float64],
    ) -> npt.NDArray[np.float64]: ...
    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...

class Ellipsoid:
    """A reference ellipsoid (e.g. WGS84)."""

    def __init__(self, x: float, y: float, z: float) -> None: ...
    @staticmethod
    def wgs84() -> Ellipsoid: ...
    @staticmethod
    def unit_sphere() -> Ellipsoid: ...
    @property
    def radii(self) -> npt.NDArray[np.float64]: ...
    @property
    def semi_major_axis(self) -> float: ...
    @property
    def semi_minor_axis(self) -> float: ...
    @property
    def maximum_radius(self) -> float: ...
    @property
    def minimum_radius(self) -> float: ...
    @overload
    def geodetic_surface_normal(
        self, position: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...
    @overload
    def geodetic_surface_normal(
        self, position: Cartographic
    ) -> npt.NDArray[np.float64]: ...
    @overload
    def cartographic_to_cartesian(
        self, cartographic: Cartographic
    ) -> npt.NDArray[np.float64]: ...
    @overload
    def cartographic_to_cartesian(
        self, cartographic: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...
    def cartesian_to_cartographic(
        self, cartesian: npt.NDArray[np.float64]
    ) -> Cartographic | None: ...
    def scale_to_geodetic_surface(
        self, cartesian: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64] | None: ...
    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...

class WkidTransform:
    """CRS-to-ECEF coordinate transform for projected/geographic CRSs."""

    @staticmethod
    def from_wkid(wkid: int) -> WkidTransform | None: ...
    @staticmethod
    def from_wkid_with_ellipsoid(
        wkid: int, ellipsoid: Ellipsoid
    ) -> WkidTransform | None: ...
    def to_ecef(self, position: npt.NDArray[np.float64]) -> npt.NDArray[np.float64]: ...
    def __repr__(self) -> str: ...

@runtime_checkable
class CrsTransform(Protocol):
    """Protocol for CRS-to-ECEF coordinate transforms."""

    def to_ecef(
        self, positions: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...

class SceneCoordinateSystem(IntEnum):
    Global = 0
    Local = 1

class TransverseMercatorParams:
    """Transverse Mercator projection parameters."""

    @property
    def central_meridian(self) -> float: ...
    @property
    def scale_factor(self) -> float: ...
    @property
    def false_easting(self) -> float: ...
    @property
    def false_northing(self) -> float: ...

@overload
def web_mercator_project(
    position: npt.NDArray[np.float64],
) -> npt.NDArray[np.float64]: ...
@overload
def web_mercator_project(
    position: Cartographic,
) -> npt.NDArray[np.float64]: ...
@overload
def web_mercator_unproject(
    position: npt.NDArray[np.float64],
) -> npt.NDArray[np.float64]: ...
@overload
def web_mercator_unproject(
    position: Cartographic,
) -> npt.NDArray[np.float64]: ...
def utm_params(zone: int, north: bool) -> TransverseMercatorParams: ...
@overload
def transverse_mercator_project(
    position: npt.NDArray[np.float64],
    params: TransverseMercatorParams,
) -> npt.NDArray[np.float64]: ...
@overload
def transverse_mercator_project(
    position: Cartographic,
    params: TransverseMercatorParams,
) -> npt.NDArray[np.float64]: ...
@overload
def transverse_mercator_unproject(
    position: npt.NDArray[np.float64],
    params: TransverseMercatorParams,
) -> npt.NDArray[np.float64]: ...
@overload
def transverse_mercator_unproject(
    position: Cartographic,
    params: TransverseMercatorParams,
) -> npt.NDArray[np.float64]: ...

class ProjTransform:
    """CRS-to-ECEF transform backed by pyproj.

    Implements the CRS transform protocol (``to_ecef``) so it can be
    passed to ``SceneLayer.from_url_with_transform()``.
    """

    def __init__(self, source_crs: int | str) -> None: ...
    def to_ecef(
        self, positions: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...
    def __repr__(self) -> str: ...
