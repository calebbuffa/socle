"""Type stubs for ``i3s.geometry`` — bounding volumes, intersection tests, transforms."""

from __future__ import annotations

from enum import IntEnum

import numpy as np
import numpy.typing as npt


class CullingResult(IntEnum):
    Inside = 0
    Outside = 1
    Intersecting = 2


class Plane:
    """An infinite plane defined by a unit normal and distance from origin."""

    def __init__(
        self, normal: npt.ArrayLike, distance: float
    ) -> None: ...

    @staticmethod
    def from_point_normal(
        point: npt.ArrayLike, normal: npt.ArrayLike
    ) -> Plane: ...

    @staticmethod
    def from_coefficients(a: float, b: float, c: float, d: float) -> Plane: ...

    @staticmethod
    def origin_xy() -> Plane: ...
    @staticmethod
    def origin_yz() -> Plane: ...
    @staticmethod
    def origin_zx() -> Plane: ...

    @property
    def normal(self) -> npt.NDArray[np.float64]: ...
    @property
    def distance(self) -> float: ...

    def signed_distance(self, point: npt.ArrayLike) -> float: ...

    def project_point(
        self, point: npt.ArrayLike
    ) -> npt.NDArray[np.float64]: ...

    def __repr__(self) -> str: ...


class BoundingSphere:
    """A sphere defined by center and radius."""

    def __init__(self, center: npt.ArrayLike, radius: float) -> None: ...

    @property
    def center(self) -> npt.NDArray[np.float64]: ...
    @property
    def radius(self) -> float: ...

    def contains(self, point: npt.ArrayLike) -> bool: ...

    def distance_squared_to(self, point: npt.ArrayLike) -> float: ...

    def intersect_plane(self, plane: Plane) -> CullingResult: ...

    def transform(
        self, transformation: list[list[float]]
    ) -> BoundingSphere: ...

    def __repr__(self) -> str: ...


class OrientedBoundingBox:
    """An oriented bounding box (OBB)."""

    def __init__(
        self,
        center: npt.ArrayLike,
        half_size: npt.ArrayLike,
        quaternion: npt.ArrayLike,
    ) -> None: ...

    @staticmethod
    def from_i3s(
        center: list[float],
        half_size: list[float],
        quaternion: list[float],
    ) -> OrientedBoundingBox: ...

    @staticmethod
    def from_axis_aligned(
        aabb_min: npt.ArrayLike, aabb_max: npt.ArrayLike
    ) -> OrientedBoundingBox: ...

    @staticmethod
    def from_sphere(sphere: BoundingSphere) -> OrientedBoundingBox: ...

    @property
    def center(self) -> npt.NDArray[np.float64]: ...
    @property
    def half_size(self) -> npt.NDArray[np.float64]: ...
    @property
    def quaternion(self) -> npt.NDArray[np.float64]: ...

    def corners(self) -> npt.NDArray[np.float64]: ...
    def rotation_matrix(self) -> npt.NDArray[np.float64]: ...

    def contains(self, point: npt.ArrayLike) -> bool: ...

    def distance_squared_to(self, point: npt.ArrayLike) -> float: ...

    def intersect_plane(self, plane: Plane) -> CullingResult: ...

    def to_aabb(
        self,
    ) -> tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]]: ...

    def to_bounding_sphere(self) -> BoundingSphere: ...
    def inverse_half_axes(self) -> npt.NDArray[np.float64]: ...
    def lengths(self) -> npt.NDArray[np.float64]: ...

    def transform(
        self, transformation: list[list[float]]
    ) -> OrientedBoundingBox: ...

    def projected_area(
        self,
        camera_position: npt.ArrayLike,
        viewport_height: float,
        fov_y: float,
    ) -> float: ...

    def __repr__(self) -> str: ...


class Ray:
    """A ray with origin and direction."""

    def __init__(
        self, origin: npt.ArrayLike, direction: npt.ArrayLike
    ) -> None: ...

    @property
    def origin(self) -> npt.NDArray[np.float64]: ...
    @property
    def direction(self) -> npt.NDArray[np.float64]: ...

    def at(self, t: float) -> npt.NDArray[np.float64]: ...

    def transform(self, transformation: list[list[float]]) -> Ray: ...
    def negate(self) -> Ray: ...

    def __repr__(self) -> str: ...


class Rectangle:
    """A 2-D axis-aligned rectangle."""

    def __init__(
        self,
        minimum_x: float,
        minimum_y: float,
        maximum_x: float,
        maximum_y: float,
    ) -> None: ...

    @property
    def minimum_x(self) -> float: ...
    @property
    def minimum_y(self) -> float: ...
    @property
    def maximum_x(self) -> float: ...
    @property
    def maximum_y(self) -> float: ...
    @property
    def width(self) -> float: ...
    @property
    def height(self) -> float: ...

    def center(self) -> tuple[float, float]: ...
    def contains(self, x: float, y: float) -> bool: ...
    def overlaps(self, other: Rectangle) -> bool: ...
    def fully_contains(self, other: Rectangle) -> bool: ...
    def signed_distance(self, x: float, y: float) -> float: ...
    def intersection(self, other: Rectangle) -> Rectangle | None: ...
    def union(self, other: Rectangle) -> Rectangle: ...

    def __repr__(self) -> str: ...


class Axis(IntEnum):
    X = 0
    Y = 1
    Z = 2



def ray_plane(ray: Ray, plane: Plane) -> float | None: ...
def ray_sphere(ray: Ray, sphere: BoundingSphere) -> float | None: ...
def ray_aabb(
    ray: Ray, aabb_min: npt.ArrayLike, aabb_max: npt.ArrayLike
) -> float | None: ...
def ray_obb(ray: Ray, obb: OrientedBoundingBox) -> float | None: ...
def ray_triangle(
    ray: Ray,
    v0: npt.ArrayLike,
    v1: npt.ArrayLike,
    v2: npt.ArrayLike,
) -> float | None: ...
def ray_ellipsoid(
    ray: Ray, radii: npt.ArrayLike
) -> tuple[float, float] | None: ...
def point_in_triangle_2d(
    px: float,
    py: float,
    ax: float,
    ay: float,
    bx: float,
    by: float,
    cx: float,
    cy: float,
) -> bool: ...
def point_in_triangle_3d(
    point: npt.ArrayLike,
    a: npt.ArrayLike,
    b: npt.ArrayLike,
    c: npt.ArrayLike,
) -> npt.NDArray[np.float64] | None: ...


def create_trs_matrix(
    translation: npt.ArrayLike,
    rotation: npt.ArrayLike,
    scale: npt.ArrayLike,
) -> npt.NDArray[np.float64]: ...
def get_up_axis_transform(
    from_: Axis, to: Axis
) -> npt.NDArray[np.float64]: ...
def create_view_matrix(
    position: npt.ArrayLike,
    direction: npt.ArrayLike,
    up: npt.ArrayLike,
) -> npt.NDArray[np.float64]: ...
def create_perspective_fov(
    fov_x: float, fov_y: float, z_near: float, z_far: float
) -> npt.NDArray[np.float64]: ...
def create_orthographic(
    left: float,
    right: float,
    bottom: float,
    top: float,
    z_near: float,
    z_far: float,
) -> npt.NDArray[np.float64]: ...
