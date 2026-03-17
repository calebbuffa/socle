"""i3s.geometry — bounding volumes, intersection tests, transforms.

Re-exports compiled types from ``i3s._native.geometry``.
"""

from i3s import _native  # type: ignore[attr-defined]

_mod = _native.geometry

Axis = _mod.Axis
BoundingSphere = _mod.BoundingSphere
CullingResult = _mod.CullingResult
OrientedBoundingBox = _mod.OrientedBoundingBox
Plane = _mod.Plane
Ray = _mod.Ray
Rectangle = _mod.Rectangle
create_orthographic = _mod.create_orthographic
create_perspective_fov = _mod.create_perspective_fov
create_trs_matrix = _mod.create_trs_matrix
create_view_matrix = _mod.create_view_matrix
get_up_axis_transform = _mod.get_up_axis_transform
point_in_triangle_2d = _mod.point_in_triangle_2d
point_in_triangle_3d = _mod.point_in_triangle_3d
ray_aabb = _mod.ray_aabb
ray_ellipsoid = _mod.ray_ellipsoid
ray_obb = _mod.ray_obb
ray_plane = _mod.ray_plane
ray_sphere = _mod.ray_sphere
ray_triangle = _mod.ray_triangle

__all__ = [
    "Axis",
    "BoundingSphere",
    "CullingResult",
    "OrientedBoundingBox",
    "Plane",
    "Ray",
    "Rectangle",
    "create_orthographic",
    "create_perspective_fov",
    "create_trs_matrix",
    "create_view_matrix",
    "get_up_axis_transform",
    "point_in_triangle_2d",
    "point_in_triangle_3d",
    "ray_aabb",
    "ray_ellipsoid",
    "ray_obb",
    "ray_plane",
    "ray_sphere",
    "ray_triangle",
]
