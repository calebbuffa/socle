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

class LocalDirection(IntEnum):
    East = 0
    North = 1
    West = 2
    South = 3
    Up = 4
    Down = 5

class LocalHorizontalCoordinateSystem:
    """A local horizontal coordinate system (ENU or similar)."""

    def __init__(
        self,
        origin: npt.NDArray[np.float64],
        x_axis: LocalDirection,
        y_axis: LocalDirection,
        z_axis: LocalDirection,
        scale_to_meters: float = 1.0,
        ellipsoid: Ellipsoid | None = None,
    ) -> None: ...
    @staticmethod
    def from_ecef(
        origin: npt.NDArray[np.float64],
        ellipsoid: Ellipsoid | None = None,
    ) -> LocalHorizontalCoordinateSystem: ...
    @staticmethod
    def from_matrix(
        matrix: npt.NDArray[np.float64],
        ellipsoid: Ellipsoid | None = None,
    ) -> LocalHorizontalCoordinateSystem: ...
    @property
    def local_to_ecef_transform(self) -> npt.NDArray[np.float64]: ...
    @property
    def ecef_to_local_transform(self) -> npt.NDArray[np.float64]: ...
    def local_position_to_ecef(
        self, point: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...
    def ecef_position_to_local(
        self, point: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...
    def local_direction_to_ecef(
        self, direction: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...
    def ecef_direction_to_local(
        self, direction: npt.NDArray[np.float64]
    ) -> npt.NDArray[np.float64]: ...
    def compute_transformation_to_another_local(
        self, other: LocalHorizontalCoordinateSystem
    ) -> npt.NDArray[np.float64]: ...
    def __repr__(self) -> str: ...

class GlobeRectangle:
    """A geographic rectangle in radians."""

    def __init__(
        self, west: float, south: float, east: float, north: float
    ) -> None: ...
    @staticmethod
    def from_degrees(
        west: float, south: float, east: float, north: float
    ) -> GlobeRectangle: ...
    @property
    def west(self) -> float: ...
    @property
    def south(self) -> float: ...
    @property
    def east(self) -> float: ...
    @property
    def north(self) -> float: ...
    def width(self) -> float: ...
    def height(self) -> float: ...
    def center_longitude(self) -> float: ...
    def center_latitude(self) -> float: ...
    def contains(self, longitude: float, latitude: float) -> bool: ...
    def __repr__(self) -> str: ...

class BoundingRegion:
    """A geographic bounding region (rectangle + height range)."""

    def __init__(
        self,
        rectangle: GlobeRectangle,
        minimum_height: float,
        maximum_height: float,
    ) -> None: ...
    @property
    def rectangle(self) -> GlobeRectangle: ...
    @property
    def minimum_height(self) -> float: ...
    @property
    def maximum_height(self) -> float: ...
    def to_bounding_sphere(
        self, ellipsoid: Ellipsoid | None = None
    ) -> npt.NDArray[np.float64]: ...
    def contains(self, cartographic: Cartographic) -> bool: ...
    def __repr__(self) -> str: ...

def enu_frame(
    cartesian: npt.NDArray[np.float64], ellipsoid: Ellipsoid | None = None
) -> npt.NDArray[np.float64]: ...
def enu_matrix_at(
    cartesian: npt.NDArray[np.float64], ellipsoid: Ellipsoid | None = None
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
