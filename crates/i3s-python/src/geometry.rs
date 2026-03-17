//! Python bindings for `i3s-geometry`: OBB, BoundingSphere, Plane, Ray,
//! Rectangle, Transforms, and intersection tests.
//!
//! Mirrors cesium-native's `GeometryBindings.cpp`. Batch methods accept
//! `(N, 3)` numpy arrays and release the GIL.

use glam::{DMat4, DVec2, DVec3};
use numpy::ndarray::{Array1, Array2};
use numpy::{IntoPyArray, PyArray1, PyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;

use i3s_geometry::aabb::AxisAlignedBoundingBox;
use i3s_geometry::culling::CullingResult;
use i3s_geometry::intersection;
use i3s_geometry::obb::OrientedBoundingBox;
use i3s_geometry::plane::Plane;
use i3s_geometry::ray::Ray;
use i3s_geometry::rectangle::Rectangle;
use i3s_geometry::sphere::BoundingSphere;
use i3s_geometry::transforms::{Axis, Transforms};

use crate::numpy_conv;


#[pyclass(name = "CullingResult", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PyCullingResult {
    Inside = 0,
    Outside = 1,
    Intersecting = 2,
}

impl From<CullingResult> for PyCullingResult {
    fn from(cr: CullingResult) -> Self {
        match cr {
            CullingResult::Inside => PyCullingResult::Inside,
            CullingResult::Outside => PyCullingResult::Outside,
            CullingResult::Intersecting => PyCullingResult::Intersecting,
        }
    }
}


#[pyclass(name = "Plane", skip_from_py_object)]
#[derive(Clone)]
pub struct PyPlane {
    pub inner: Plane,
}

#[pymethods]
impl PyPlane {
    /// Create from a unit normal and distance from origin.
    #[new]
    fn new(normal: &Bound<'_, PyAny>, distance: f64) -> PyResult<Self> {
        let n = numpy_conv::to_dvec3(normal)?;
        Ok(Self {
            inner: Plane::new(n, distance),
        })
    }

    /// Create from a point on the plane and its normal.
    #[staticmethod]
    fn from_point_normal(point: &Bound<'_, PyAny>, normal: &Bound<'_, PyAny>) -> PyResult<Self> {
        let p = numpy_conv::to_dvec3(point)?;
        let n = numpy_conv::to_dvec3(normal)?;
        Ok(Self {
            inner: Plane::from_point_normal(p, n),
        })
    }

    /// Create from equation coefficients ax + by + cz + d = 0.
    #[staticmethod]
    fn from_coefficients(a: f64, b: f64, c: f64, d: f64) -> Self {
        Self {
            inner: Plane::from_coefficients(a, b, c, d),
        }
    }

    /// Normal as (3,) numpy array.
    #[getter]
    fn normal<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.normal)
    }

    #[getter]
    fn distance(&self) -> f64 {
        self.inner.distance
    }

    /// Signed distance from a point. (3,) or (N,3).
    fn signed_distance<'py>(
        &self,
        py: Python<'py>,
        point: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if numpy_conv::is_points_array(point, 3) {
            let plane = self.inner;
            let result = numpy_conv::batch_scalar(py, point, 3, move |inp| {
                plane.signed_distance(DVec3::new(inp[0], inp[1], inp[2]))
            })?;
            return Ok(result.into_any().unbind());
        }
        let p = numpy_conv::to_dvec3(point)?;
        Ok(self
            .inner
            .signed_distance(p)
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }

    /// Project a point onto this plane. Returns closest point.
    fn project_point<'py>(
        &self,
        py: Python<'py>,
        point: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let p = numpy_conv::to_dvec3(point)?;
        Ok(numpy_conv::dvec3_to_numpy(py, self.inner.project_point(p)))
    }

    /// XY plane through origin (normal = +Z).
    #[staticmethod]
    fn origin_xy() -> Self {
        Self {
            inner: Plane::ORIGIN_XY_PLANE,
        }
    }

    /// YZ plane through origin (normal = +X).
    #[staticmethod]
    fn origin_yz() -> Self {
        Self {
            inner: Plane::ORIGIN_YZ_PLANE,
        }
    }

    /// ZX plane through origin (normal = +Y).
    #[staticmethod]
    fn origin_zx() -> Self {
        Self {
            inner: Plane::ORIGIN_ZX_PLANE,
        }
    }

    fn __repr__(&self) -> String {
        let n = self.inner.normal;
        format!(
            "Plane(normal=[{}, {}, {}], distance={})",
            n.x, n.y, n.z, self.inner.distance
        )
    }
}


#[pyclass(name = "BoundingSphere", skip_from_py_object)]
#[derive(Clone)]
pub struct PyBoundingSphere {
    pub inner: BoundingSphere,
}

#[pymethods]
impl PyBoundingSphere {
    #[new]
    fn new(center: &Bound<'_, PyAny>, radius: f64) -> PyResult<Self> {
        let c = numpy_conv::to_dvec3(center)?;
        Ok(Self {
            inner: BoundingSphere::new(c, radius),
        })
    }

    #[getter]
    fn center<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.center)
    }

    #[getter]
    fn radius(&self) -> f64 {
        self.inner.radius
    }

    /// Test point containment. (3,) -> bool or (N,3) -> (N,) bool.
    fn contains<'py>(&self, py: Python<'py>, point: &Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        if numpy_conv::is_points_array(point, 3) {
            let sphere = self.inner;
            let result = numpy_conv::batch_predicate(py, point, 3, move |inp| {
                sphere.contains(DVec3::new(inp[0], inp[1], inp[2]))
            })?;
            return Ok(result.into_any().unbind());
        }
        let p = numpy_conv::to_dvec3(point)?;
        Ok(self
            .inner
            .contains(p)
            .into_pyobject(py)?
            .to_owned()
            .into_any()
            .unbind())
    }

    /// Squared distance to point. (3,) -> f64 or (N,3) -> (N,) f64.
    fn distance_squared_to<'py>(
        &self,
        py: Python<'py>,
        point: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if numpy_conv::is_points_array(point, 3) {
            let sphere = self.inner;
            let result = numpy_conv::batch_scalar(py, point, 3, move |inp| {
                sphere.distance_squared_to(DVec3::new(inp[0], inp[1], inp[2]))
            })?;
            return Ok(result.into_any().unbind());
        }
        let p = numpy_conv::to_dvec3(point)?;
        Ok(self
            .inner
            .distance_squared_to(p)
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }

    /// Test against a plane.
    fn intersect_plane(&self, plane: &PyPlane) -> PyCullingResult {
        self.inner.intersect_plane(&plane.inner).into()
    }

    /// Transform by a 4x4 matrix. Returns new BoundingSphere.
    ///
    /// Accepts a ``(4, 4)`` float64 numpy array (e.g. ``np.eye(4)``) or a
    /// nested Python list ``[[...], ...]``.
    fn transform(&self, transformation: &Bound<'_, PyAny>) -> PyResult<PyBoundingSphere> {
        let mat = mat4_from_any(transformation)?;
        Ok(PyBoundingSphere {
            inner: self.inner.transform(&mat),
        })
    }

    fn __repr__(&self) -> String {
        let c = self.inner.center;
        format!(
            "BoundingSphere(center=[{}, {}, {}], radius={})",
            c.x, c.y, c.z, self.inner.radius
        )
    }
}


#[pyclass(name = "OrientedBoundingBox", skip_from_py_object)]
#[derive(Clone)]
pub struct PyOrientedBoundingBox {
    pub inner: OrientedBoundingBox,
}

#[pymethods]
impl PyOrientedBoundingBox {
    #[new]
    fn new(
        center: &Bound<'_, PyAny>,
        half_size: &Bound<'_, PyAny>,
        quaternion: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let c = numpy_conv::to_dvec3(center)?;
        let hs = numpy_conv::to_dvec3(half_size)?;
        let q = numpy_conv::to_dquat(quaternion)?;
        Ok(Self {
            inner: OrientedBoundingBox {
                center: c,
                half_size: hs,
                quaternion: q,
            },
        })
    }

    /// Construct from I3S-format arrays (center[3], half_size[3], quaternion[4]).
    #[staticmethod]
    fn from_i3s(center: [f64; 3], half_size: [f64; 3], quaternion: [f64; 4]) -> Self {
        Self {
            inner: OrientedBoundingBox::from_i3s(center, half_size, quaternion),
        }
    }

    /// Create from an axis-aligned bounding box (min[3], max[3]).
    #[staticmethod]
    fn from_axis_aligned(
        aabb_min: &Bound<'_, PyAny>,
        aabb_max: &Bound<'_, PyAny>,
    ) -> PyResult<Self> {
        let mn = numpy_conv::to_dvec3(aabb_min)?;
        let mx = numpy_conv::to_dvec3(aabb_max)?;
        let aabb = AxisAlignedBoundingBox::new(mn, mx);
        Ok(Self {
            inner: OrientedBoundingBox::from_axis_aligned(&aabb),
        })
    }

    /// Create from a bounding sphere.
    #[staticmethod]
    fn from_sphere(sphere: &PyBoundingSphere) -> Self {
        Self {
            inner: OrientedBoundingBox::from_sphere(&sphere.inner),
        }
    }

    #[getter]
    fn center<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.center)
    }

    #[getter]
    fn half_size<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.half_size)
    }

    #[getter]
    fn quaternion<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dquat_to_numpy(py, self.inner.quaternion)
    }

    /// 8 corner vertices as (8, 3) numpy array.
    fn corners<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f64>> {
        let c = self.inner.corners();
        let mut arr = Array2::<f64>::zeros((8, 3));
        for (i, v) in c.iter().enumerate() {
            arr[[i, 0]] = v.x;
            arr[[i, 1]] = v.y;
            arr[[i, 2]] = v.z;
        }
        arr.into_pyarray(py)
    }

    /// Rotation matrix as (3,3) numpy array.
    fn rotation_matrix<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f64>> {
        numpy_conv::dmat3_to_numpy(py, self.inner.rotation_matrix())
    }

    /// Test point containment. (3,) -> bool or (N,3) -> (N,) bool.
    fn contains<'py>(&self, py: Python<'py>, point: &Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        if numpy_conv::is_points_array(point, 3) {
            let obb = self.inner;
            let result = numpy_conv::batch_predicate(py, point, 3, move |inp| {
                obb.contains(DVec3::new(inp[0], inp[1], inp[2]))
            })?;
            return Ok(result.into_any().unbind());
        }
        let p = numpy_conv::to_dvec3(point)?;
        Ok(self
            .inner
            .contains(p)
            .into_pyobject(py)?
            .to_owned()
            .into_any()
            .unbind())
    }

    /// Squared distance to a point. (3,) -> f64 or (N,3) -> (N,) f64.
    fn distance_squared_to<'py>(
        &self,
        py: Python<'py>,
        point: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if numpy_conv::is_points_array(point, 3) {
            let obb = self.inner;
            let result = numpy_conv::batch_scalar(py, point, 3, move |inp| {
                obb.distance_squared_to(DVec3::new(inp[0], inp[1], inp[2]))
            })?;
            return Ok(result.into_any().unbind());
        }
        let p = numpy_conv::to_dvec3(point)?;
        Ok(self
            .inner
            .distance_squared_to(p)
            .into_pyobject(py)?
            .into_any()
            .unbind())
    }

    /// Test against a plane.
    fn intersect_plane(&self, plane: &PyPlane) -> PyCullingResult {
        self.inner.intersect_plane(&plane.inner).into()
    }

    /// To an axis-aligned bounding box. Returns (min[3], max[3]) as a tuple.
    fn to_aabb<'py>(
        &self,
        py: Python<'py>,
    ) -> (Bound<'py, PyArray1<f64>>, Bound<'py, PyArray1<f64>>) {
        let aabb = self.inner.to_aabb();
        (
            numpy_conv::dvec3_to_numpy(py, aabb.min),
            numpy_conv::dvec3_to_numpy(py, aabb.max),
        )
    }

    /// To a bounding sphere.
    fn to_bounding_sphere(&self) -> PyBoundingSphere {
        PyBoundingSphere {
            inner: self.inner.to_bounding_sphere(),
        }
    }

    /// Inverse half-axes matrix as (3,3) numpy array.
    fn inverse_half_axes<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f64>> {
        numpy_conv::dmat3_to_numpy(py, self.inner.inverse_half_axes())
    }

    /// Lengths (full extents) along each local axis as (3,) numpy array.
    fn lengths<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.lengths())
    }

    /// Transform by a 4x4 matrix. Returns new OrientedBoundingBox.
    ///
    /// Accepts a ``(4, 4)`` float64 numpy array (e.g. ``np.eye(4)``) or a
    /// nested Python list ``[[...], ...]``.
    fn transform(&self, transformation: &Bound<'_, PyAny>) -> PyResult<PyOrientedBoundingBox> {
        let mat = mat4_from_any(transformation)?;
        Ok(PyOrientedBoundingBox {
            inner: self.inner.transform(&mat),
        })
    }

    /// Projected screen area in pixels for LOD evaluation.
    fn projected_area(
        &self,
        camera_position: &Bound<'_, PyAny>,
        viewport_height: f64,
        fov_y: f64,
    ) -> PyResult<f64> {
        let cam = numpy_conv::to_dvec3(camera_position)?;
        Ok(self.inner.projected_area(cam, viewport_height, fov_y))
    }

    fn __repr__(&self) -> String {
        let c = self.inner.center;
        let hs = self.inner.half_size;
        format!(
            "OrientedBoundingBox(center=[{}, {}, {}], half_size=[{}, {}, {}])",
            c.x, c.y, c.z, hs.x, hs.y, hs.z
        )
    }
}


#[pyclass(name = "Ray", skip_from_py_object)]
#[derive(Clone)]
pub struct PyRay {
    pub inner: Ray,
}

#[pymethods]
impl PyRay {
    #[new]
    fn new(origin: &Bound<'_, PyAny>, direction: &Bound<'_, PyAny>) -> PyResult<Self> {
        let o = numpy_conv::to_dvec3(origin)?;
        let d = numpy_conv::to_dvec3(direction)?;
        Ok(Self {
            inner: Ray::new(o, d),
        })
    }

    #[getter]
    fn origin<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.origin)
    }

    #[getter]
    fn direction<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.direction)
    }

    /// Get a point at parameter t along the ray.
    fn at<'py>(&self, py: Python<'py>, t: f64) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.at(t))
    }

    /// Transform by a 4x4 matrix. Returns new Ray.
    ///
    /// Accepts a ``(4, 4)`` float64 numpy array (e.g. ``np.eye(4)``) or a
    /// nested Python list ``[[...], ...]``.
    fn transform(&self, transformation: &Bound<'_, PyAny>) -> PyResult<PyRay> {
        let mat = mat4_from_any(transformation)?;
        Ok(PyRay {
            inner: self.inner.transform(&mat),
        })
    }

    /// Return a ray with negated direction.
    fn negate(&self) -> PyRay {
        PyRay {
            inner: self.inner.negate(),
        }
    }

    fn __repr__(&self) -> String {
        let o = self.inner.origin;
        let d = self.inner.direction;
        format!(
            "Ray(origin=[{}, {}, {}], direction=[{}, {}, {}])",
            o.x, o.y, o.z, d.x, d.y, d.z
        )
    }
}


pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCullingResult>()?;
    m.add_class::<PyPlane>()?;
    m.add_class::<PyBoundingSphere>()?;
    m.add_class::<PyOrientedBoundingBox>()?;
    m.add_class::<PyRay>()?;
    m.add_class::<PyRectangle>()?;
    m.add_class::<PyAxis>()?;
    m.add_function(wrap_pyfunction!(py_ray_plane, m)?)?;
    m.add_function(wrap_pyfunction!(py_ray_sphere, m)?)?;
    m.add_function(wrap_pyfunction!(py_ray_aabb, m)?)?;
    m.add_function(wrap_pyfunction!(py_ray_obb, m)?)?;
    m.add_function(wrap_pyfunction!(py_ray_triangle, m)?)?;
    m.add_function(wrap_pyfunction!(py_ray_ellipsoid, m)?)?;
    m.add_function(wrap_pyfunction!(py_point_in_triangle_2d, m)?)?;
    m.add_function(wrap_pyfunction!(py_point_in_triangle_3d, m)?)?;
    m.add_function(wrap_pyfunction!(py_create_trs_matrix, m)?)?;
    m.add_function(wrap_pyfunction!(py_get_up_axis_transform, m)?)?;
    m.add_function(wrap_pyfunction!(py_create_view_matrix, m)?)?;
    m.add_function(wrap_pyfunction!(py_create_perspective_fov, m)?)?;
    m.add_function(wrap_pyfunction!(py_create_orthographic, m)?)?;
    Ok(())
}


fn mat4_from_nested(rows: &[Vec<f64>]) -> PyResult<DMat4> {
    use pyo3::exceptions::PyValueError;
    if rows.len() != 4 || rows.iter().any(|r| r.len() != 4) {
        return Err(PyValueError::new_err(
            "Expected 4x4 matrix as list of lists",
        ));
    }
    // Input is row-major, DMat4 is column-major
    Ok(DMat4::from_cols_array(&[
        rows[0][0], rows[1][0], rows[2][0], rows[3][0], rows[0][1], rows[1][1], rows[2][1],
        rows[3][1], rows[0][2], rows[1][2], rows[2][2], rows[3][2], rows[0][3], rows[1][3],
        rows[2][3], rows[3][3],
    ]))
}

/// Extract a 4x4 matrix from either a (4,4) float64 numpy array or a nested
/// Python list-of-lists.  `np.eye(4)`, `np.array(...)`, and `[[...], ...]`
/// all work.
fn mat4_from_any(obj: &Bound<'_, PyAny>) -> PyResult<DMat4> {
    use pyo3::exceptions::PyValueError;
    // Fast path: numpy (4, 4) float64
    if let Ok(arr) = obj.extract::<numpy::PyReadonlyArray2<f64>>() {
        let s = arr.shape();
        if s[0] != 4 || s[1] != 4 {
            return Err(PyValueError::new_err("Expected (4, 4) matrix"));
        }
        let v = arr.as_array();
        // Row-major → column-major for DMat4
        return Ok(DMat4::from_cols_array(&[
            v[[0, 0]], v[[1, 0]], v[[2, 0]], v[[3, 0]],
            v[[0, 1]], v[[1, 1]], v[[2, 1]], v[[3, 1]],
            v[[0, 2]], v[[1, 2]], v[[2, 2]], v[[3, 2]],
            v[[0, 3]], v[[1, 3]], v[[2, 3]], v[[3, 3]],
        ]));
    }
    // Fallback: nested list-of-lists
    let nested: Vec<Vec<f64>> = obj.extract()?;
    mat4_from_nested(&nested)
}

fn mat4_to_nested<'py>(py: Python<'py>, m: &DMat4) -> Bound<'py, PyArray2<f64>> {
    let mut arr = Array2::<f64>::zeros((4, 4));
    for col in 0..4 {
        for row in 0..4 {
            arr[[row, col]] = m.col(col)[row];
        }
    }
    arr.into_pyarray(py)
}


#[pyclass(name = "Rectangle", skip_from_py_object)]
#[derive(Clone)]
pub struct PyRectangle {
    pub inner: Rectangle,
}

#[pymethods]
impl PyRectangle {
    #[new]
    fn new(minimum_x: f64, minimum_y: f64, maximum_x: f64, maximum_y: f64) -> Self {
        Self {
            inner: Rectangle::new(minimum_x, minimum_y, maximum_x, maximum_y),
        }
    }

    #[getter]
    fn minimum_x(&self) -> f64 {
        self.inner.minimum_x
    }
    #[getter]
    fn minimum_y(&self) -> f64 {
        self.inner.minimum_y
    }
    #[getter]
    fn maximum_x(&self) -> f64 {
        self.inner.maximum_x
    }
    #[getter]
    fn maximum_y(&self) -> f64 {
        self.inner.maximum_y
    }

    #[getter]
    fn width(&self) -> f64 {
        self.inner.width()
    }
    #[getter]
    fn height(&self) -> f64 {
        self.inner.height()
    }

    fn center(&self) -> (f64, f64) {
        let c = self.inner.center();
        (c.x, c.y)
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        self.inner.contains(DVec2::new(x, y))
    }

    fn overlaps(&self, other: &PyRectangle) -> bool {
        self.inner.overlaps(&other.inner)
    }

    fn fully_contains(&self, other: &PyRectangle) -> bool {
        self.inner.fully_contains(&other.inner)
    }

    fn signed_distance(&self, x: f64, y: f64) -> f64 {
        self.inner.signed_distance(DVec2::new(x, y))
    }

    fn intersection(&self, other: &PyRectangle) -> Option<PyRectangle> {
        self.inner
            .intersection(&other.inner)
            .map(|r| PyRectangle { inner: r })
    }

    fn union(&self, other: &PyRectangle) -> PyRectangle {
        PyRectangle {
            inner: self.inner.union(&other.inner),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Rectangle({}, {}, {}, {})",
            self.inner.minimum_x, self.inner.minimum_y, self.inner.maximum_x, self.inner.maximum_y
        )
    }
}


#[pyclass(name = "Axis", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PyAxis {
    X = 0,
    Y = 1,
    Z = 2,
}

impl From<PyAxis> for Axis {
    fn from(a: PyAxis) -> Self {
        match a {
            PyAxis::X => Axis::X,
            PyAxis::Y => Axis::Y,
            PyAxis::Z => Axis::Z,
        }
    }
}


/// Intersect a ray with a plane. Returns parametric t, or None.
#[pyfunction]
#[pyo3(name = "ray_plane")]
fn py_ray_plane(ray: &PyRay, plane: &PyPlane) -> Option<f64> {
    intersection::ray_plane(&ray.inner, &plane.inner)
}

/// Intersect a ray with a bounding sphere. Returns parametric t, or None.
#[pyfunction]
#[pyo3(name = "ray_sphere")]
fn py_ray_sphere(ray: &PyRay, sphere: &PyBoundingSphere) -> Option<f64> {
    intersection::ray_sphere(&ray.inner, &sphere.inner)
}

/// Intersect a ray with an AABB (min, max). Returns parametric t, or None.
#[pyfunction]
#[pyo3(name = "ray_aabb")]
fn py_ray_aabb(
    ray: &PyRay,
    aabb_min: &Bound<'_, PyAny>,
    aabb_max: &Bound<'_, PyAny>,
) -> PyResult<Option<f64>> {
    let mn = numpy_conv::to_dvec3(aabb_min)?;
    let mx = numpy_conv::to_dvec3(aabb_max)?;
    let aabb = AxisAlignedBoundingBox::new(mn, mx);
    Ok(intersection::ray_aabb(&ray.inner, &aabb))
}

/// Intersect a ray with an OBB. Returns parametric t, or None.
#[pyfunction]
#[pyo3(name = "ray_obb")]
fn py_ray_obb(ray: &PyRay, obb: &PyOrientedBoundingBox) -> Option<f64> {
    intersection::ray_obb(&ray.inner, &obb.inner)
}

/// Intersect a ray with a triangle (v0, v1, v2). Returns parametric t, or None.
#[pyfunction]
#[pyo3(name = "ray_triangle")]
fn py_ray_triangle(
    ray: &PyRay,
    v0: &Bound<'_, PyAny>,
    v1: &Bound<'_, PyAny>,
    v2: &Bound<'_, PyAny>,
) -> PyResult<Option<f64>> {
    let a = numpy_conv::to_dvec3(v0)?;
    let b = numpy_conv::to_dvec3(v1)?;
    let c = numpy_conv::to_dvec3(v2)?;
    Ok(intersection::ray_triangle(&ray.inner, a, b, c))
}

/// Intersect a ray with an ellipsoid. Returns (t_near, t_far) or None.
#[pyfunction]
#[pyo3(name = "ray_ellipsoid")]
fn py_ray_ellipsoid(ray: &PyRay, radii: &Bound<'_, PyAny>) -> PyResult<Option<(f64, f64)>> {
    let r = numpy_conv::to_dvec3(radii)?;
    Ok(intersection::ray_ellipsoid(&ray.inner, r).map(|v| (v.x, v.y)))
}

/// Test if a 2D point is inside a triangle.
#[pyfunction]
#[pyo3(name = "point_in_triangle_2d")]
fn py_point_in_triangle_2d(
    px: f64,
    py_: f64,
    ax: f64,
    ay: f64,
    bx: f64,
    by: f64,
    cx: f64,
    cy: f64,
) -> bool {
    intersection::point_in_triangle_2d(
        DVec2::new(px, py_),
        DVec2::new(ax, ay),
        DVec2::new(bx, by),
        DVec2::new(cx, cy),
    )
}

/// Test if a 3D point is inside a triangle. Returns barycentric coords or None.
#[pyfunction]
#[pyo3(name = "point_in_triangle_3d")]
fn py_point_in_triangle_3d<'py>(
    py: Python<'py>,
    point: &Bound<'py, PyAny>,
    a: &Bound<'py, PyAny>,
    b: &Bound<'py, PyAny>,
    c: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyArray1<f64>>>> {
    let p = numpy_conv::to_dvec3(point)?;
    let va = numpy_conv::to_dvec3(a)?;
    let vb = numpy_conv::to_dvec3(b)?;
    let vc = numpy_conv::to_dvec3(c)?;
    Ok(intersection::point_in_triangle_3d(p, va, vb, vc)
        .map(|bary| Array1::from_vec(vec![bary.x, bary.y, bary.z]).into_pyarray(py)))
}


/// Create a translation-rotation-scale matrix.
#[pyfunction]
#[pyo3(name = "create_trs_matrix")]
fn py_create_trs_matrix<'py>(
    py: Python<'py>,
    translation: &Bound<'py, PyAny>,
    rotation: &Bound<'py, PyAny>,
    scale: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let t = numpy_conv::to_dvec3(translation)?;
    let r = numpy_conv::to_dquat(rotation)?;
    let s = numpy_conv::to_dvec3(scale)?;
    let m = Transforms::create_translation_rotation_scale(t, r, s);
    Ok(mat4_to_nested(py, &m))
}

/// Get the up-axis transform matrix.
#[pyfunction]
#[pyo3(name = "get_up_axis_transform")]
fn py_get_up_axis_transform<'py>(
    py: Python<'py>,
    from: &PyAxis,
    to: &PyAxis,
) -> Bound<'py, PyArray2<f64>> {
    let m = Transforms::get_up_axis_transform(from.clone().into(), to.clone().into());
    mat4_to_nested(py, m)
}

/// Create a view matrix from camera pose.
#[pyfunction]
#[pyo3(name = "create_view_matrix")]
fn py_create_view_matrix<'py>(
    py: Python<'py>,
    position: &Bound<'py, PyAny>,
    direction: &Bound<'py, PyAny>,
    up: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyArray2<f64>>> {
    let p = numpy_conv::to_dvec3(position)?;
    let d = numpy_conv::to_dvec3(direction)?;
    let u = numpy_conv::to_dvec3(up)?;
    let m = Transforms::create_view_matrix(p, d, u);
    Ok(mat4_to_nested(py, &m))
}

/// Create a perspective projection matrix (fov_x, fov_y, z_near, z_far).
#[pyfunction]
#[pyo3(name = "create_perspective_fov")]
fn py_create_perspective_fov<'py>(
    py: Python<'py>,
    fov_x: f64,
    fov_y: f64,
    z_near: f64,
    z_far: f64,
) -> Bound<'py, PyArray2<f64>> {
    let m = Transforms::create_perspective_fov(fov_x, fov_y, z_near, z_far);
    mat4_to_nested(py, &m)
}

/// Create an orthographic projection matrix.
#[pyfunction]
#[pyo3(name = "create_orthographic")]
fn py_create_orthographic<'py>(
    py: Python<'py>,
    left: f64,
    right: f64,
    bottom: f64,
    top: f64,
    z_near: f64,
    z_far: f64,
) -> Bound<'py, PyArray2<f64>> {
    let m = Transforms::create_orthographic(left, right, bottom, top, z_near, z_far);
    mat4_to_nested(py, &m)
}
