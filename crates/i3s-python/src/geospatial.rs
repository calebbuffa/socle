//! Python bindings for `i3s-geospatial`: Ellipsoid, Cartographic, projections, CRS.
//!
//! Every method that takes a position accepts both a single `(3,)` array and
//! an `(N, 3)` batch array, returning the matching shape. Batch paths release the GIL.

use glam::DVec3;
use numpy::ndarray::{Array2, ArrayView2};
use numpy::{IntoPyArray, PyArray1, PyArray2};
use pyo3::Py;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use i3s_geospatial::bounding_region::BoundingRegion;
use i3s_geospatial::cartographic::Cartographic;
use i3s_geospatial::crs::WkidTransform;
use i3s_geospatial::ellipsoid::Ellipsoid;
use i3s_geospatial::globe_rectangle::GlobeRectangle;
use i3s_geospatial::local_horizontal_cs::{LocalDirection, LocalHorizontalCoordinateSystem};
use i3s_geospatial::projection::{
    TransverseMercatorParams, from_transverse_mercator, from_web_mercator, to_transverse_mercator,
    to_web_mercator,
};
use i3s_geospatial::transforms::{enu_frame as rs_enu_frame, enu_matrix_at as rs_enu_matrix_at};

use crate::numpy_conv;

/// A geographic position: longitude/latitude in radians, height in meters.
#[pyclass(name = "Cartographic", skip_from_py_object)]
#[derive(Clone)]
pub struct PyCartographic {
    pub inner: Cartographic,
}

#[pymethods]
impl PyCartographic {
    #[new]
    #[pyo3(signature = (longitude, latitude, height = 0.0))]
    fn new(longitude: f64, latitude: f64, height: f64) -> Self {
        Self {
            inner: Cartographic::new(longitude, latitude, height),
        }
    }

    /// Create from degrees. Accepts scalars or an (N,3) numpy array.
    #[staticmethod]
    #[pyo3(signature = (longitude_degrees, latitude_degrees = None, height = 0.0))]
    fn from_degrees<'py>(
        py: Python<'py>,
        longitude_degrees: &Bound<'py, PyAny>,
        latitude_degrees: Option<f64>,
        height: f64,
    ) -> PyResult<Py<PyAny>> {
        // Batch path: (N,3) array of [lon_deg, lat_deg, height]
        if numpy_conv::is_points_array(longitude_degrees, 3) {
            let ro = numpy_conv::require_points_array(longitude_degrees, 3)?;
            let view: ArrayView2<f64> = ro.as_array();
            let n = view.nrows();
            let mut output = Array2::<f64>::zeros((n, 3));
            let deg2rad = std::f64::consts::PI / 180.0;

            py.detach(|| {
                for i in 0..n {
                    output[[i, 0]] = view[[i, 0]] * deg2rad;
                    output[[i, 1]] = view[[i, 1]] * deg2rad;
                    output[[i, 2]] = view[[i, 2]];
                }
            });

            return Ok(output.into_pyarray(py).into_any().unbind());
        }

        // Scalar path
        let lon: f64 = longitude_degrees.extract()?;
        let lat = latitude_degrees.unwrap_or(0.0);
        Ok(PyCartographic {
            inner: Cartographic::from_degrees(lon, lat, height),
        }
        .into_pyobject(py)?
        .into_any()
        .unbind())
    }

    #[getter]
    fn longitude(&self) -> f64 {
        self.inner.longitude
    }
    #[setter]
    fn set_longitude(&mut self, v: f64) {
        self.inner.longitude = v;
    }
    #[getter]
    fn latitude(&self) -> f64 {
        self.inner.latitude
    }
    #[setter]
    fn set_latitude(&mut self, v: f64) {
        self.inner.latitude = v;
    }
    #[getter]
    fn height(&self) -> f64 {
        self.inner.height
    }
    #[setter]
    fn set_height(&mut self, v: f64) {
        self.inner.height = v;
    }

    fn __repr__(&self) -> String {
        format!(
            "Cartographic(longitude={}, latitude={}, height={})",
            self.inner.longitude, self.inner.latitude, self.inner.height
        )
    }

    fn __eq__(&self, other: &PyCartographic) -> bool {
        self.inner == other.inner
    }
}

// Ellipsoid

/// Reference ellipsoid.
#[pyclass(name = "Ellipsoid", skip_from_py_object)]
#[derive(Clone)]
pub struct PyEllipsoid {
    pub inner: Ellipsoid,
}

#[pymethods]
impl PyEllipsoid {
    #[new]
    fn new(x: f64, y: f64, z: f64) -> Self {
        Self {
            inner: Ellipsoid::new(DVec3::new(x, y, z)),
        }
    }

    /// WGS84 reference ellipsoid.
    #[staticmethod]
    fn wgs84() -> Self {
        Self {
            inner: Ellipsoid::WGS84,
        }
    }

    /// Unit sphere.
    #[staticmethod]
    fn unit_sphere() -> Self {
        Self {
            inner: Ellipsoid::UNIT_SPHERE,
        }
    }

    /// Radii as a (3,) numpy array.
    #[getter]
    fn radii<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.radii)
    }

    /// Semi-major axis (equatorial radius).
    #[getter]
    fn semi_major_axis(&self) -> f64 {
        self.inner.semi_major_axis()
    }

    /// Semi-minor axis (polar radius).
    #[getter]
    fn semi_minor_axis(&self) -> f64 {
        self.inner.semi_minor_axis()
    }

    /// Maximum radius.
    #[getter]
    fn maximum_radius(&self) -> f64 {
        self.inner.radii.max_element()
    }

    /// Minimum radius.
    #[getter]
    fn minimum_radius(&self) -> f64 {
        self.inner.radii.min_element()
    }

    /// Geodetic surface normal. Accepts (3,) or (N,3).
    fn geodetic_surface_normal<'py>(
        &self,
        py: Python<'py>,
        position: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if numpy_conv::is_points_array(position, 3) {
            let ellipsoid = self.inner;
            let result = numpy_conv::batch_transform(py, position, 3, 3, move |inp, out| {
                let v =
                    ellipsoid.geodetic_surface_normal_cartesian(DVec3::new(inp[0], inp[1], inp[2]));
                out[0] = v.x;
                out[1] = v.y;
                out[2] = v.z;
            })?;
            return Ok(result.into_any().unbind());
        }
        let v = numpy_conv::to_dvec3(position)?;
        let n = self.inner.geodetic_surface_normal_cartesian(v);
        Ok(numpy_conv::dvec3_to_numpy(py, n).into_any().unbind())
    }

    /// Convert cartographic to ECEF. Accepts Cartographic, (3,), or (N,3).
    fn cartographic_to_cartesian<'py>(
        &self,
        py: Python<'py>,
        cartographic: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Batch: (N,3)
        if numpy_conv::is_points_array(cartographic, 3) {
            let ellipsoid = self.inner;
            let result = numpy_conv::batch_transform(py, cartographic, 3, 3, move |inp, out| {
                let c = Cartographic::new(inp[0], inp[1], inp[2]);
                let v = ellipsoid.cartographic_to_cartesian(c);
                out[0] = v.x;
                out[1] = v.y;
                out[2] = v.z;
            })?;
            return Ok(result.into_any().unbind());
        }
        // Single Cartographic object
        if let Ok(c) = cartographic.extract::<PyRef<PyCartographic>>() {
            let v = self.inner.cartographic_to_cartesian(c.inner);
            return Ok(numpy_conv::dvec3_to_numpy(py, v).into_any().unbind());
        }
        // Single (3,) array
        let v = numpy_conv::to_dvec3(cartographic)?;
        let c = Cartographic::new(v.x, v.y, v.z);
        let result = self.inner.cartographic_to_cartesian(c);
        Ok(numpy_conv::dvec3_to_numpy(py, result).into_any().unbind())
    }

    /// Convert ECEF to cartographic. Accepts (3,) or (N,3).
    /// Returns None/NaN for points at the ellipsoid center.
    fn cartesian_to_cartographic<'py>(
        &self,
        py: Python<'py>,
        cartesian: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Batch: (N,3)
        if numpy_conv::is_points_array(cartesian, 3) {
            let ellipsoid = self.inner;
            let result = numpy_conv::batch_transform(py, cartesian, 3, 3, move |inp, out| {
                let v = DVec3::new(inp[0], inp[1], inp[2]);
                if let Some(c) = ellipsoid.cartesian_to_cartographic(v) {
                    out[0] = c.longitude;
                    out[1] = c.latitude;
                    out[2] = c.height;
                } else {
                    out[0] = f64::NAN;
                    out[1] = f64::NAN;
                    out[2] = f64::NAN;
                }
            })?;
            return Ok(result.into_any().unbind());
        }
        // Single (3,)
        let v = numpy_conv::to_dvec3(cartesian)?;
        match self.inner.cartesian_to_cartographic(v) {
            Some(c) => Ok(PyCartographic { inner: c }
                .into_pyobject(py)?
                .into_any()
                .unbind()),
            None => Ok(py.None()),
        }
    }

    /// Scale a Cartesian point to the geodetic surface. Accepts (3,) or (N,3).
    fn scale_to_geodetic_surface<'py>(
        &self,
        py: Python<'py>,
        cartesian: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if numpy_conv::is_points_array(cartesian, 3) {
            let ellipsoid = self.inner;
            let result = numpy_conv::batch_transform(py, cartesian, 3, 3, move |inp, out| {
                let v = DVec3::new(inp[0], inp[1], inp[2]);
                if let Some(r) = ellipsoid.scale_to_geodetic_surface(v) {
                    out[0] = r.x;
                    out[1] = r.y;
                    out[2] = r.z;
                } else {
                    out[0] = f64::NAN;
                    out[1] = f64::NAN;
                    out[2] = f64::NAN;
                }
            })?;
            return Ok(result.into_any().unbind());
        }
        let v = numpy_conv::to_dvec3(cartesian)?;
        match self.inner.scale_to_geodetic_surface(v) {
            Some(r) => Ok(numpy_conv::dvec3_to_numpy(py, r).into_any().unbind()),
            None => Ok(py.None()),
        }
    }

    fn __eq__(&self, other: &PyEllipsoid) -> bool {
        self.inner == other.inner
    }

    fn __repr__(&self) -> String {
        let r = self.inner.radii;
        format!("Ellipsoid(x={}, y={}, z={})", r.x, r.y, r.z)
    }
}

/// CRS-to-ECEF transform for common WKID-based coordinate systems.
#[pyclass(name = "WkidTransform", skip_from_py_object)]
#[derive(Clone)]
pub struct PyWkidTransform {
    pub(crate) inner: WkidTransform,
}

#[pymethods]
impl PyWkidTransform {
    /// Create from an EPSG code. Returns None if unsupported.
    #[staticmethod]
    fn from_wkid(wkid: i64) -> Option<Self> {
        WkidTransform::from_wkid(wkid).map(|inner| Self { inner })
    }

    /// Create with a custom ellipsoid.
    #[staticmethod]
    fn from_wkid_with_ellipsoid(wkid: i64, ellipsoid: &PyEllipsoid) -> Option<Self> {
        WkidTransform::from_wkid_with_ellipsoid(wkid, ellipsoid.inner).map(|inner| Self { inner })
    }

    /// Transform positions to ECEF. Accepts (3,) or (N,3).
    fn to_ecef<'py>(&self, py: Python<'py>, positions: &Bound<'py, PyAny>) -> PyResult<Py<PyAny>> {
        use i3s_geospatial::crs::CrsTransform;

        // Batch: (N,3)
        if numpy_conv::is_points_array(positions, 3) {
            let ro = numpy_conv::require_points_array(positions, 3)?;
            let view: ArrayView2<f64> = ro.as_array();
            let n = view.nrows();
            let mut output = Array2::<f64>::zeros((n, 3));
            let xform = self.inner.clone();

            py.detach(|| {
                // Work in chunks to avoid per-point allocations
                let mut buf = vec![DVec3::ZERO; n];
                for i in 0..n {
                    buf[i] = DVec3::new(view[[i, 0]], view[[i, 1]], view[[i, 2]]);
                }
                xform.to_ecef(&mut buf);
                for i in 0..n {
                    output[[i, 0]] = buf[i].x;
                    output[[i, 1]] = buf[i].y;
                    output[[i, 2]] = buf[i].z;
                }
            });

            return Ok(output.into_pyarray(py).into_any().unbind());
        }

        // Single (3,)
        let v = numpy_conv::to_dvec3(positions)?;
        let mut buf = [v];
        self.inner.to_ecef(&mut buf);
        Ok(numpy_conv::dvec3_to_numpy(py, buf[0]).into_any().unbind())
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

#[pyclass(name = "SceneCoordinateSystem", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PySceneCoordinateSystem {
    Global = 0,
    Local = 1,
}

/// Project cartographic to Web Mercator. (N,3) or (3,) -> (N,3) or (3,).
#[pyfunction]
#[pyo3(signature = (cartographic, ellipsoid = None))]
fn web_mercator_project<'py>(
    py: Python<'py>,
    cartographic: &Bound<'py, PyAny>,
    ellipsoid: Option<&PyEllipsoid>,
) -> PyResult<Py<PyAny>> {
    let e = ellipsoid.map(|e| e.inner).unwrap_or(Ellipsoid::WGS84);

    if numpy_conv::is_points_array(cartographic, 3) {
        let result = numpy_conv::batch_transform(py, cartographic, 3, 3, move |inp, out| {
            let c = Cartographic::new(inp[0], inp[1], inp[2]);
            let (x, y) = to_web_mercator(&c, &e);
            out[0] = x;
            out[1] = y;
            out[2] = inp[2]; // pass height through
        })?;
        return Ok(result.into_any().unbind());
    }
    let v = numpy_conv::to_dvec3(cartographic)?;
    let c = Cartographic::new(v.x, v.y, v.z);
    let (x, y) = to_web_mercator(&c, &e);
    Ok(numpy_conv::dvec3_to_numpy(py, DVec3::new(x, y, v.z))
        .into_any()
        .unbind())
}

/// Unproject Web Mercator to cartographic. (N,3) or (3,) -> (N,3) or (3,).
#[pyfunction]
#[pyo3(signature = (positions, ellipsoid = None))]
fn web_mercator_unproject<'py>(
    py: Python<'py>,
    positions: &Bound<'py, PyAny>,
    ellipsoid: Option<&PyEllipsoid>,
) -> PyResult<Py<PyAny>> {
    let e = ellipsoid.map(|e| e.inner).unwrap_or(Ellipsoid::WGS84);

    if numpy_conv::is_points_array(positions, 3) {
        let result = numpy_conv::batch_transform(py, positions, 3, 3, move |inp, out| {
            let mut c = from_web_mercator(inp[0], inp[1], &e);
            c.height = inp[2];
            out[0] = c.longitude;
            out[1] = c.latitude;
            out[2] = c.height;
        })?;
        return Ok(result.into_any().unbind());
    }
    let v = numpy_conv::to_dvec3(positions)?;
    let mut c = from_web_mercator(v.x, v.y, &e);
    c.height = v.z;
    Ok(
        numpy_conv::dvec3_to_numpy(py, DVec3::new(c.longitude, c.latitude, c.height))
            .into_any()
            .unbind(),
    )
}

/// Create UTM Transverse Mercator parameters.
#[pyfunction]
#[pyo3(signature = (zone, north = true))]
fn utm_params(zone: u8, north: bool) -> PyResult<PyTransverseMercatorParams> {
    if zone == 0 || zone > 60 {
        return Err(PyValueError::new_err("UTM zone must be 1-60"));
    }
    Ok(PyTransverseMercatorParams {
        inner: TransverseMercatorParams::utm(zone, north),
    })
}

/// Transverse Mercator projection parameters (exposed for advanced use).
#[pyclass(name = "TransverseMercatorParams", skip_from_py_object)]
#[derive(Clone)]
pub struct PyTransverseMercatorParams {
    inner: TransverseMercatorParams,
}

#[pymethods]
impl PyTransverseMercatorParams {
    #[getter]
    fn central_meridian(&self) -> f64 {
        self.inner.central_meridian
    }
    #[getter]
    fn scale_factor(&self) -> f64 {
        self.inner.scale_factor
    }
    #[getter]
    fn false_easting(&self) -> f64 {
        self.inner.false_easting
    }
    #[getter]
    fn false_northing(&self) -> f64 {
        self.inner.false_northing
    }
}

/// Forward Transverse Mercator: cartographic -> (easting, northing). (N,3) batch.
#[pyfunction]
fn transverse_mercator_project<'py>(
    py: Python<'py>,
    cartographic: &Bound<'py, PyAny>,
    params: &PyTransverseMercatorParams,
) -> PyResult<Py<PyAny>> {
    let p = params.inner;

    if numpy_conv::is_points_array(cartographic, 3) {
        let result = numpy_conv::batch_transform(py, cartographic, 3, 3, move |inp, out| {
            let c = Cartographic::new(inp[0], inp[1], inp[2]);
            let (e, n) = to_transverse_mercator(&c, &p);
            out[0] = e;
            out[1] = n;
            out[2] = inp[2];
        })?;
        return Ok(result.into_any().unbind());
    }
    let v = numpy_conv::to_dvec3(cartographic)?;
    let c = Cartographic::new(v.x, v.y, v.z);
    let (e, n) = to_transverse_mercator(&c, &p);
    Ok(numpy_conv::dvec3_to_numpy(py, DVec3::new(e, n, v.z))
        .into_any()
        .unbind())
}

/// Inverse Transverse Mercator: (easting, northing) -> cartographic. (N,3) batch.
#[pyfunction]
fn transverse_mercator_unproject<'py>(
    py: Python<'py>,
    positions: &Bound<'py, PyAny>,
    params: &PyTransverseMercatorParams,
) -> PyResult<Py<PyAny>> {
    let p = params.inner;

    if numpy_conv::is_points_array(positions, 3) {
        let result = numpy_conv::batch_transform(py, positions, 3, 3, move |inp, out| {
            let mut c = from_transverse_mercator(inp[0], inp[1], &p);
            c.height = inp[2];
            out[0] = c.longitude;
            out[1] = c.latitude;
            out[2] = c.height;
        })?;
        return Ok(result.into_any().unbind());
    }
    let v = numpy_conv::to_dvec3(positions)?;
    let mut c = from_transverse_mercator(v.x, v.y, &p);
    c.height = v.z;
    Ok(
        numpy_conv::dvec3_to_numpy(py, DVec3::new(c.longitude, c.latitude, c.height))
            .into_any()
            .unbind(),
    )
}

/// A principal compass or vertical direction in a local horizontal coordinate frame.
#[pyclass(name = "LocalDirection", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, PartialEq)]
pub enum PyLocalDirection {
    East = 0,
    North = 1,
    West = 2,
    South = 3,
    Up = 4,
    Down = 5,
}

impl From<PyLocalDirection> for LocalDirection {
    fn from(d: PyLocalDirection) -> Self {
        match d {
            PyLocalDirection::East => LocalDirection::East,
            PyLocalDirection::North => LocalDirection::North,
            PyLocalDirection::West => LocalDirection::West,
            PyLocalDirection::South => LocalDirection::South,
            PyLocalDirection::Up => LocalDirection::Up,
            PyLocalDirection::Down => LocalDirection::Down,
        }
    }
}

/// A local horizontal coordinate system anchored to a point on the globe.
///
/// Each axis points in a configurable :class:`LocalDirection` at the origin.
/// Stores ``local_to_ecef`` and ``ecef_to_local`` ``(4, 4)`` matrices.
#[pyclass(name = "LocalHorizontalCoordinateSystem", skip_from_py_object)]
#[derive(Clone)]
pub struct PyLocalHorizontalCoordinateSystem {
    inner: LocalHorizontalCoordinateSystem,
}

#[pymethods]
impl PyLocalHorizontalCoordinateSystem {
    /// Create at a cartographic origin.
    #[new]
    #[pyo3(signature = (origin, x_axis, y_axis, z_axis, scale_to_meters=1.0, ellipsoid=None))]
    fn new(
        origin: &PyCartographic,
        x_axis: PyLocalDirection,
        y_axis: PyLocalDirection,
        z_axis: PyLocalDirection,
        scale_to_meters: f64,
        ellipsoid: Option<&PyEllipsoid>,
    ) -> Self {
        let e = ellipsoid.map(|e| e.inner).unwrap_or(Ellipsoid::WGS84);
        Self {
            inner: LocalHorizontalCoordinateSystem::new(
                origin.inner,
                x_axis.into(),
                y_axis.into(),
                z_axis.into(),
                scale_to_meters,
                &e,
            ),
        }
    }

    /// Create at an ECEF origin (3,) numpy array.
    #[staticmethod]
    #[pyo3(signature = (origin_ecef, x_axis, y_axis, z_axis, scale_to_meters=1.0, ellipsoid=None))]
    fn from_ecef(
        origin_ecef: &Bound<'_, PyAny>,
        x_axis: PyLocalDirection,
        y_axis: PyLocalDirection,
        z_axis: PyLocalDirection,
        scale_to_meters: f64,
        ellipsoid: Option<&PyEllipsoid>,
    ) -> PyResult<Self> {
        let ecef = numpy_conv::to_dvec3(origin_ecef)?;
        let e = ellipsoid.map(|e| e.inner).unwrap_or(Ellipsoid::WGS84);
        Ok(Self {
            inner: LocalHorizontalCoordinateSystem::from_ecef(
                ecef,
                x_axis.into(),
                y_axis.into(),
                z_axis.into(),
                scale_to_meters,
                &e,
            ),
        })
    }

    /// Create from a known ``local_to_ecef`` (4×4) matrix.
    #[staticmethod]
    fn from_matrix(mat: &Bound<'_, PyAny>) -> PyResult<Self> {
        let m = numpy_conv::to_dmat4(mat)?;
        Ok(Self {
            inner: LocalHorizontalCoordinateSystem::from_matrix(m),
        })
    }

    /// ``local_to_ecef`` as a (4, 4) float64 numpy array.
    #[getter]
    fn local_to_ecef_transform<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f64>> {
        numpy_conv::dmat4_to_numpy(py, self.inner.local_to_ecef_transform())
    }

    /// ``ecef_to_local`` as a (4, 4) float64 numpy array.
    #[getter]
    fn ecef_to_local_transform<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f64>> {
        numpy_conv::dmat4_to_numpy(py, self.inner.ecef_to_local_transform())
    }

    /// Transform a local-frame position to ECEF.
    fn local_position_to_ecef<'py>(
        &self,
        py: Python<'py>,
        local: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let v = numpy_conv::to_dvec3(local)?;
        Ok(numpy_conv::dvec3_to_numpy(
            py,
            self.inner.local_position_to_ecef(v),
        ))
    }

    /// Transform an ECEF position to the local frame.
    fn ecef_position_to_local<'py>(
        &self,
        py: Python<'py>,
        ecef: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let v = numpy_conv::to_dvec3(ecef)?;
        Ok(numpy_conv::dvec3_to_numpy(
            py,
            self.inner.ecef_position_to_local(v),
        ))
    }

    /// Transform a local-frame direction vector to ECEF.
    fn local_direction_to_ecef<'py>(
        &self,
        py: Python<'py>,
        local: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let v = numpy_conv::to_dvec3(local)?;
        Ok(numpy_conv::dvec3_to_numpy(
            py,
            self.inner.local_direction_to_ecef(v),
        ))
    }

    /// Transform an ECEF direction vector to the local frame.
    fn ecef_direction_to_local<'py>(
        &self,
        py: Python<'py>,
        ecef: &Bound<'_, PyAny>,
    ) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let v = numpy_conv::to_dvec3(ecef)?;
        Ok(numpy_conv::dvec3_to_numpy(
            py,
            self.inner.ecef_direction_to_local(v),
        ))
    }

    /// Return the (4, 4) matrix that maps positions in *self* to positions in *other*.
    fn compute_transformation_to_another_local<'py>(
        &self,
        py: Python<'py>,
        other: &PyLocalHorizontalCoordinateSystem,
    ) -> Bound<'py, PyArray2<f64>> {
        let mat = self
            .inner
            .compute_transformation_to_another_local(&other.inner);
        numpy_conv::dmat4_to_numpy(py, &mat)
    }

    fn __repr__(&self) -> String {
        "LocalHorizontalCoordinateSystem(...)".to_string()
    }
}

/// An axis-aligned rectangle on the globe surface (bounds in radians).
#[pyclass(name = "GlobeRectangle", skip_from_py_object)]
#[derive(Clone)]
pub struct PyGlobeRectangle {
    pub inner: GlobeRectangle,
}

#[pymethods]
impl PyGlobeRectangle {
    /// Create from radian bounds (west, south, east, north).
    #[new]
    fn new(west: f64, south: f64, east: f64, north: f64) -> Self {
        Self {
            inner: GlobeRectangle::new(west, south, east, north),
        }
    }

    /// Create from degree bounds.
    #[staticmethod]
    fn from_degrees(west: f64, south: f64, east: f64, north: f64) -> Self {
        Self {
            inner: GlobeRectangle::from_degrees(west, south, east, north),
        }
    }

    #[getter]
    fn west(&self) -> f64 {
        self.inner.west
    }
    #[getter]
    fn south(&self) -> f64 {
        self.inner.south
    }
    #[getter]
    fn east(&self) -> f64 {
        self.inner.east
    }
    #[getter]
    fn north(&self) -> f64 {
        self.inner.north
    }

    fn width(&self) -> f64 {
        self.inner.width()
    }
    fn height(&self) -> f64 {
        self.inner.height()
    }
    fn center_longitude(&self) -> f64 {
        self.inner.center_longitude()
    }
    fn center_latitude(&self) -> f64 {
        self.inner.center_latitude()
    }
    fn contains(&self, longitude: f64, latitude: f64) -> bool {
        self.inner.contains(longitude, latitude)
    }

    fn __repr__(&self) -> String {
        format!(
            "GlobeRectangle(west={:.4}, south={:.4}, east={:.4}, north={:.4})",
            self.inner.west, self.inner.south, self.inner.east, self.inner.north
        )
    }
}

/// A bounding region on the globe: a geographic rectangle with a height range.
#[pyclass(name = "BoundingRegion", skip_from_py_object)]
#[derive(Clone)]
pub struct PyBoundingRegion {
    pub inner: BoundingRegion,
}

#[pymethods]
impl PyBoundingRegion {
    #[new]
    fn new(rectangle: &PyGlobeRectangle, minimum_height: f64, maximum_height: f64) -> Self {
        Self {
            inner: BoundingRegion::new(rectangle.inner, minimum_height, maximum_height),
        }
    }

    #[getter]
    fn rectangle(&self) -> PyGlobeRectangle {
        PyGlobeRectangle {
            inner: self.inner.rectangle,
        }
    }
    #[getter]
    fn minimum_height(&self) -> f64 {
        self.inner.minimum_height
    }
    #[getter]
    fn maximum_height(&self) -> f64 {
        self.inner.maximum_height
    }

    /// Compute the bounding sphere enclosing this region.
    #[pyo3(signature = (ellipsoid=None))]
    fn to_bounding_sphere(
        &self,
        ellipsoid: Option<&PyEllipsoid>,
    ) -> crate::geometry::PyBoundingSphere {
        let e = ellipsoid.map(|e| e.inner).unwrap_or(Ellipsoid::WGS84);
        let sphere = self.inner.to_bounding_sphere(&e);
        crate::geometry::PyBoundingSphere { inner: sphere }
    }

    fn contains(&self, cartographic: &PyCartographic) -> bool {
        self.inner.contains(&cartographic.inner)
    }

    fn __repr__(&self) -> String {
        format!(
            "BoundingRegion(h=[{:.1}, {:.1}])",
            self.inner.minimum_height, self.inner.maximum_height
        )
    }
}

/// Compute the East-North-Up (ENU) rotation matrix (3×3) at an ECEF position.
///
/// Columns are the East, North, and Up unit vectors in ECEF space.
#[pyfunction]
#[pyo3(name = "enu_frame")]
#[pyo3(signature = (cartesian, ellipsoid=None))]
fn py_enu_frame<'py>(
    py: Python<'py>,
    cartesian: &Bound<'_, PyAny>,
    ellipsoid: Option<&PyEllipsoid>,
) -> PyResult<Bound<'py, numpy::PyArray2<f64>>> {
    let v = numpy_conv::to_dvec3(cartesian)?;
    let e = ellipsoid.map(|e| e.inner).unwrap_or(Ellipsoid::WGS84);
    Ok(numpy_conv::dmat3_to_numpy(py, rs_enu_frame(&e, v)))
}

/// Compute the East-North-Up frame as a (4, 4) matrix with ECEF origin as translation.
///
/// Equivalent to ``Cesium.Transforms.eastNorthUpToFixedFrame()``.
#[pyfunction]
#[pyo3(name = "enu_matrix_at")]
#[pyo3(signature = (cartesian, ellipsoid=None))]
fn py_enu_matrix_at<'py>(
    py: Python<'py>,
    cartesian: &Bound<'_, PyAny>,
    ellipsoid: Option<&PyEllipsoid>,
) -> PyResult<Bound<'py, numpy::PyArray2<f64>>> {
    let v = numpy_conv::to_dvec3(cartesian)?;
    let e = ellipsoid.map(|e| e.inner).unwrap_or(Ellipsoid::WGS84);
    Ok(numpy_conv::dmat4_to_numpy(py, &rs_enu_matrix_at(&e, v)))
}

// Module registration

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCartographic>()?;
    m.add_class::<PyEllipsoid>()?;
    m.add_class::<PyWkidTransform>()?;
    m.add_class::<PySceneCoordinateSystem>()?;
    m.add_class::<PyTransverseMercatorParams>()?;
    m.add_class::<PyLocalDirection>()?;
    m.add_class::<PyLocalHorizontalCoordinateSystem>()?;
    m.add_class::<PyGlobeRectangle>()?;
    m.add_class::<PyBoundingRegion>()?;
    m.add_function(wrap_pyfunction!(web_mercator_project, m)?)?;
    m.add_function(wrap_pyfunction!(web_mercator_unproject, m)?)?;
    m.add_function(wrap_pyfunction!(utm_params, m)?)?;
    m.add_function(wrap_pyfunction!(transverse_mercator_project, m)?)?;
    m.add_function(wrap_pyfunction!(transverse_mercator_unproject, m)?)?;
    m.add_function(wrap_pyfunction!(py_enu_frame, m)?)?;
    m.add_function(wrap_pyfunction!(py_enu_matrix_at, m)?)?;
    Ok(())
}
