//! Python bindings for `i3s-selection`: SceneLayerExternals, SceneLayer,
//! ViewState, ViewUpdateResult, IPrepareRendererResources, etc.
//!
//! Mirrors cesium-native's `Tiles3dSelectionBindings.cpp` patterns:
//! - `SceneLayerExternals` ↔ `TilesetExternals` — user-constructed
//! - `IPrepareRendererResources` — Python subclass-able base
//! - `SceneLayer` ↔ `Tileset` — constructed with externals + URL/path

use std::sync::Arc;

use glam::DVec3;
use numpy::ndarray::Array2;
use numpy::{IntoPyArray, PyArray1, PyArray2};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use i3s_async::{I3sAssetAccessor, resolver::{ResourceUriResolver, RestUriResolver, SlpkUriResolver}};
use i3s_geospatial::crs::SceneCoordinateSystem;
use i3s_selection::content::NodeContent;
use i3s_selection::externals::SceneLayerExternals;
use i3s_selection::node_state::NodeLoadState;
use i3s_selection::options::SelectionOptions;
use i3s_selection::prepare::{
    NoopPrepareRendererResources, PrepareRendererResources, RendererResources,
};
use i3s_selection::scene_layer::SceneLayer;
use i3s_selection::update_result::ViewUpdateResult;
use i3s_selection::view_state::ViewState;

use crate::async_support::{PyAsyncSystem, PyFuture};
use crate::geospatial::{PyEllipsoid, PyWkidTransform};
use crate::numpy_conv;

/// Base class for preparing renderer resources from decoded I3S content.
///
/// Mirrors cesium-native's ``IPrepareRendererResources``. Subclass this
/// and override methods to create GPU-ready resources from decoded nodes.
///
/// The default implementation is a no-op (suitable for headless use).
///
/// Example::
///
///     class MyResources(IPrepareRendererResources):
///         def prepare_in_load_thread(self, node_id, content):
///             return my_cpu_processing(content)
///
///         def prepare_in_main_thread(self, node_id, content, load_result):
///             return my_gpu_upload(content, load_result)
///
///         def free(self, node_id, resources):
///             pass
#[pyclass(name = "IPrepareRendererResources", subclass)]
pub struct PyIPrepareRendererResources;

#[pymethods]
impl PyIPrepareRendererResources {
    #[new]
    fn new() -> Self {
        Self
    }
}

/// Bridge: implements the Rust `PrepareRendererResources` trait by calling
/// Python methods on a user-provided `IPrepareRendererResources` subclass.
struct PyPrepareRendererResourcesBridge {
    py_obj: Py<PyAny>,
}

// Safety: Py<PyAny> is Send+Sync when detached from GIL.
unsafe impl Send for PyPrepareRendererResourcesBridge {}
unsafe impl Sync for PyPrepareRendererResourcesBridge {}

impl PrepareRendererResources for PyPrepareRendererResourcesBridge {
    fn prepare_in_load_thread(
        &self,
        node_id: u32,
        content: &NodeContent,
        _crs_transform: Option<&Arc<dyn i3s_geospatial::crs::CrsTransform>>,
    ) -> Option<RendererResources> {
        Python::attach(|py| {
            let py_content = PyNodeContent {
                inner: Arc::new(content.clone()),
            };
            let result = self
                .py_obj
                .call_method(py, "prepare_in_load_thread", (node_id, py_content), None)
                .ok()?;
            if result.is_none(py) {
                None
            } else {
                Some(Box::new(result) as RendererResources)
            }
        })
    }

    fn prepare_in_main_thread(
        &self,
        node_id: u32,
        content: &NodeContent,
        load_thread_result: Option<RendererResources>,
        _crs_transform: Option<&Arc<dyn i3s_geospatial::crs::CrsTransform>>,
    ) -> Option<RendererResources> {
        Python::attach(|py| {
            let py_content = PyNodeContent {
                inner: Arc::new(content.clone()),
            };
            let load_result: Py<PyAny> = match load_thread_result {
                Some(res) => match res.downcast::<Py<PyAny>>() {
                    Ok(obj) => *obj,
                    Err(_) => py.None().into(),
                },
                None => py.None().into(),
            };
            let result = self
                .py_obj
                .call_method(
                    py,
                    "prepare_in_main_thread",
                    (node_id, py_content, load_result),
                    None,
                )
                .ok()?;
            if result.is_none(py) {
                None
            } else {
                Some(Box::new(result) as RendererResources)
            }
        })
    }

    fn free(&self, node_id: u32, resources: Option<RendererResources>) {
        Python::attach(|py| {
            let py_resources: Py<PyAny> = match resources {
                Some(res) => match res.downcast::<Py<PyAny>>() {
                    Ok(obj) => *obj,
                    Err(_) => py.None().into(),
                },
                None => py.None().into(),
            };
            let _ = self
                .py_obj
                .call_method(py, "free", (node_id, py_resources), None);
        });
    }
}

/// Bridge: implements the Rust `CrsTransform` trait by calling a Python
/// object's `to_ecef` method. This allows any Python class with
/// `def to_ecef(self, positions: ndarray) -> ndarray` to serve as a
/// CRS transform for the scene layer.
struct PyCrsTransformBridge {
    py_obj: Py<PyAny>,
}

// Safety: Py<PyAny> is Send+Sync when detached from GIL.
unsafe impl Send for PyCrsTransformBridge {}
unsafe impl Sync for PyCrsTransformBridge {}

impl i3s_geospatial::crs::CrsTransform for PyCrsTransformBridge {
    fn to_ecef(&self, positions: &mut [DVec3]) {
        Python::attach(|py| {
            let n = positions.len();
            // SAFETY: DVec3 is #[repr(C)] {x: f64, y: f64, z: f64} —
            // identical layout to (N, 3) float64.  One flat memcpy.
            let input_flat: &[f64] =
                unsafe { std::slice::from_raw_parts(positions.as_ptr() as *const f64, n * 3) };
            let py_input = Array2::from_shape_vec((n, 3), input_flat.to_vec())
                .expect("DVec3 layout")
                .into_pyarray(py);

            let result = match self.py_obj.call_method(py, "to_ecef", (&py_input,), None) {
                Ok(r) => r,
                Err(e) => {
                    e.print(py);
                    return;
                }
            };

            // Read back — contiguous memcpy when possible.
            if let Ok(arr) = result.extract::<numpy::PyReadonlyArray2<f64>>(py) {
                let view = arr.as_array();
                let out_flat: &mut [f64] = unsafe {
                    std::slice::from_raw_parts_mut(positions.as_mut_ptr() as *mut f64, n * 3)
                };
                if let Some(src) = view.as_slice() {
                    out_flat.copy_from_slice(src);
                } else {
                    // Non-contiguous fallback (e.g. Fortran-order)
                    for i in 0..n.min(view.nrows()) {
                        out_flat[i * 3] = view[[i, 0]];
                        out_flat[i * 3 + 1] = view[[i, 1]];
                        out_flat[i * 3 + 2] = view[[i, 2]];
                    }
                }
            }
        });
    }
}

/// Extract an `Arc<dyn CrsTransform>` from a Python argument.
///
/// Accepts either a `WkidTransform` instance (fast, pure-Rust) or any
/// Python object with a `to_ecef(positions: ndarray) -> ndarray` method
/// (e.g. `ProjTransform`).
fn extract_crs_transform(
    obj: &Bound<'_, PyAny>,
) -> PyResult<Arc<dyn i3s_geospatial::crs::CrsTransform>> {
    // Try the fast path: concrete WkidTransform
    if let Ok(wkid) = obj.extract::<PyRef<PyWkidTransform>>() {
        return Ok(Arc::new(wkid.inner.clone()));
    }

    // Protocol path: any object with to_ecef()
    if obj.hasattr("to_ecef")? {
        let py_obj: Py<PyAny> = obj.clone().unbind();
        return Ok(Arc::new(PyCrsTransformBridge { py_obj }));
    }

    Err(PyRuntimeError::new_err(
        "crs_transform must be a WkidTransform or an object with a to_ecef(positions) method",
    ))
}

/// External dependencies for a ``SceneLayer``.
///
/// Mirrors cesium-native's ``TilesetExternals``. Construct one and pass
/// it to ``SceneLayer``.
///
/// Example::
///
///     tp = NativeTaskProcessor(4)
///     async_system = AsyncSystem(tp)
///     externals = SceneLayerExternals(async_system)
///
///     # Or with custom renderer resources:
///     externals = SceneLayerExternals(async_system, my_resources)
#[pyclass(name = "SceneLayerExternals")]
pub struct PySceneLayerExternals {
    pub inner: SceneLayerExternals,
}

#[pymethods]
impl PySceneLayerExternals {
    /// Create externals with the given async system and optional renderer resources.
    ///
    /// Parameters
    /// ----------
    /// async_system : AsyncSystem
    ///     The async system for dispatching work.
    /// prepare_renderer_resources : IPrepareRendererResources, optional
    ///     Custom renderer resource preparer. If None, uses a no-op.
    #[new]
    #[pyo3(signature = (async_system, prepare_renderer_resources=None))]
    fn new(async_system: &PyAsyncSystem, prepare_renderer_resources: Option<Py<PyAny>>) -> Self {
        let prepare: Arc<dyn PrepareRendererResources> = match prepare_renderer_resources {
            Some(py_obj) => Arc::new(PyPrepareRendererResourcesBridge { py_obj }),
            None => Arc::new(NoopPrepareRendererResources),
        };
        Self {
            inner: SceneLayerExternals {
                async_system: async_system.inner.clone(),
                prepare_renderer_resources: prepare,
            },
        }
    }

    /// The async system.
    #[getter]
    fn async_system(&self) -> PyAsyncSystem {
        PyAsyncSystem {
            inner: self.inner.async_system.clone(),
        }
    }

    fn __repr__(&self) -> String {
        "SceneLayerExternals(...)".to_string()
    }
}

/// Camera view state for LOD selection.
///
/// Mirrors cesium-native's ``ViewState``. Positions are ECEF.
#[pyclass(name = "ViewState", from_py_object)]
#[derive(Clone)]
pub struct PyViewState {
    pub inner: ViewState,
}

#[pymethods]
impl PyViewState {
    #[new]
    fn new(
        position: &Bound<'_, PyAny>,
        direction: &Bound<'_, PyAny>,
        up: &Bound<'_, PyAny>,
        viewport_width: u32,
        viewport_height: u32,
        fov_y: f64,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: ViewState {
                position: numpy_conv::to_dvec3(position)?,
                direction: numpy_conv::to_dvec3(direction)?,
                up: numpy_conv::to_dvec3(up)?,
                viewport_width,
                viewport_height,
                fov_y,
            },
        })
    }

    #[getter]
    fn position<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.position)
    }

    #[getter]
    fn direction<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.direction)
    }

    #[getter]
    fn up<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.inner.up)
    }

    #[getter]
    fn viewport_width(&self) -> u32 {
        self.inner.viewport_width
    }

    #[getter]
    fn viewport_height(&self) -> u32 {
        self.inner.viewport_height
    }

    #[getter]
    fn fov_y(&self) -> f64 {
        self.inner.fov_y
    }

    fn __repr__(&self) -> String {
        let p = self.inner.position;
        format!(
            "ViewState(position=[{:.1}, {:.1}, {:.1}], {}x{}, fov_y={:.3})",
            p.x, p.y, p.z, self.inner.viewport_width, self.inner.viewport_height, self.inner.fov_y
        )
    }
}

/// LOD selection and loading options.
///
/// Mirrors cesium-native's ``TilesetOptions``.
#[pyclass(name = "SelectionOptions", skip_from_py_object)]
#[derive(Clone)]
pub struct PySelectionOptions {
    pub inner: SelectionOptions,
}

#[pymethods]
impl PySelectionOptions {
    #[new]
    fn new() -> Self {
        Self {
            inner: SelectionOptions::default(),
        }
    }

    #[getter]
    fn max_simultaneous_loads(&self) -> usize {
        self.inner.max_simultaneous_loads
    }
    #[setter]
    fn set_max_simultaneous_loads(&mut self, v: usize) {
        self.inner.max_simultaneous_loads = v;
    }

    #[getter]
    fn maximum_cached_bytes(&self) -> usize {
        self.inner.maximum_cached_bytes
    }
    #[setter]
    fn set_maximum_cached_bytes(&mut self, v: usize) {
        self.inner.maximum_cached_bytes = v;
    }

    #[getter]
    fn preload_ancestors(&self) -> bool {
        self.inner.preload_ancestors
    }
    #[setter]
    fn set_preload_ancestors(&mut self, v: bool) {
        self.inner.preload_ancestors = v;
    }

    #[getter]
    fn preload_siblings(&self) -> bool {
        self.inner.preload_siblings
    }
    #[setter]
    fn set_preload_siblings(&mut self, v: bool) {
        self.inner.preload_siblings = v;
    }

    #[getter]
    fn loading_descendant_limit(&self) -> u32 {
        self.inner.loading_descendant_limit
    }
    #[setter]
    fn set_loading_descendant_limit(&mut self, v: u32) {
        self.inner.loading_descendant_limit = v;
    }

    #[getter]
    fn forbid_holes(&self) -> bool {
        self.inner.forbid_holes
    }
    #[setter]
    fn set_forbid_holes(&mut self, v: bool) {
        self.inner.forbid_holes = v;
    }

    #[getter]
    fn enable_frustum_culling(&self) -> bool {
        self.inner.enable_frustum_culling
    }
    #[setter]
    fn set_enable_frustum_culling(&mut self, v: bool) {
        self.inner.enable_frustum_culling = v;
    }

    #[getter]
    fn enable_fog_culling(&self) -> bool {
        self.inner.enable_fog_culling
    }
    #[setter]
    fn set_enable_fog_culling(&mut self, v: bool) {
        self.inner.enable_fog_culling = v;
    }

    #[getter]
    fn lod_threshold_multiplier(&self) -> f64 {
        self.inner.lod_threshold_multiplier
    }
    #[setter]
    fn set_lod_threshold_multiplier(&mut self, v: f64) {
        self.inner.lod_threshold_multiplier = v;
    }

    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

/// Result of a single frame's LOD selection.
///
/// Mirrors cesium-native's ``ViewUpdateResult``.
#[pyclass(name = "ViewUpdateResult")]
pub struct PyViewUpdateResult {
    inner: ViewUpdateResult,
}

#[pymethods]
impl PyViewUpdateResult {
    /// Node IDs selected for rendering this frame.
    #[getter]
    fn nodes_to_render<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<u32>> {
        numpy::ndarray::Array1::from_vec(self.inner.nodes_to_render.clone()).into_pyarray(py)
    }

    /// Node IDs that should be unloaded.
    #[getter]
    fn nodes_to_unload<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<u32>> {
        numpy::ndarray::Array1::from_vec(self.inner.nodes_to_unload.clone()).into_pyarray(py)
    }

    /// Number of load requests generated.
    #[getter]
    fn load_request_count(&self) -> usize {
        self.inner.load_requests.len()
    }

    /// Number of node pages needed.
    #[getter]
    fn pages_needed_count(&self) -> usize {
        self.inner.pages_needed.len()
    }

    /// Traversal statistics.
    #[getter]
    fn tiles_visited(&self) -> u32 {
        self.inner.stats.tiles_visited
    }

    #[getter]
    fn tiles_culled(&self) -> u32 {
        self.inner.stats.tiles_culled
    }

    #[getter]
    fn tiles_kicked(&self) -> u32 {
        self.inner.stats.tiles_kicked
    }

    #[getter]
    fn max_depth_visited(&self) -> u32 {
        self.inner.stats.max_depth_visited
    }

    fn __repr__(&self) -> String {
        format!(
            "ViewUpdateResult(render={}, unload={}, requests={}, visited={}, culled={})",
            self.inner.nodes_to_render.len(),
            self.inner.nodes_to_unload.len(),
            self.inner.load_requests.len(),
            self.inner.stats.tiles_visited,
            self.inner.stats.tiles_culled,
        )
    }
}

#[pyclass(name = "NodeLoadState", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PyNodeLoadState {
    Unloaded = 0,
    Loading = 1,
    Loaded = 2,
    Failed = 3,
}

impl From<NodeLoadState> for PyNodeLoadState {
    fn from(s: NodeLoadState) -> Self {
        match s {
            NodeLoadState::Unloaded => PyNodeLoadState::Unloaded,
            NodeLoadState::Loading => PyNodeLoadState::Loading,
            NodeLoadState::Loaded => PyNodeLoadState::Loaded,
            NodeLoadState::Failed => PyNodeLoadState::Failed,
        }
    }
}

/// A node selected for rendering, with its OBB transform.
#[pyclass(name = "RenderNode")]
pub struct PyRenderNode {
    pub node_id: u32,
    pub center: DVec3,
    pub quaternion: glam::DQuat,
    pub half_size: DVec3,
    pub bounding_radius: f64,
}

#[pymethods]
impl PyRenderNode {
    #[getter]
    fn node_id(&self) -> u32 {
        self.node_id
    }

    #[getter]
    fn center<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.center)
    }

    #[getter]
    fn quaternion<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dquat_to_numpy(py, self.quaternion)
    }

    #[getter]
    fn half_size<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        numpy_conv::dvec3_to_numpy(py, self.half_size)
    }

    #[getter]
    fn bounding_radius(&self) -> f64 {
        self.bounding_radius
    }

    fn __repr__(&self) -> String {
        format!(
            "RenderNode(id={}, center=[{:.1}, {:.1}, {:.1}], radius={:.1})",
            self.node_id, self.center.x, self.center.y, self.center.z, self.bounding_radius
        )
    }
}

/// Decoded geometry buffer content: positions, normals, UVs, etc.
///
/// All array properties return numpy arrays via contiguous `memcpy`
/// (no element-by-element copy).  The data is shared via `Arc` so
/// accessing `NodeContent.geometry` is O(1).
#[pyclass(name = "GeometryData")]
pub struct PyGeometryData {
    /// Shared reference — avoids cloning large buffers from NodeContent.
    _owner: Arc<NodeContent>,
}

impl PyGeometryData {
    fn geom(&self) -> &i3s_reader::geometry::GeometryData {
        &self._owner.geometry
    }
}

#[pymethods]
impl PyGeometryData {
    #[getter]
    fn vertex_count(&self) -> u32 {
        self.geom().vertex_count
    }

    #[getter]
    fn feature_count(&self) -> u32 {
        self.geom().feature_count
    }

    /// Positions as (N, 3) float32 numpy array.
    #[getter]
    fn positions<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray2<f32>> {
        numpy_conv::f32x3_to_pyarray2(py, &self.geom().positions)
    }

    /// Normals as (N, 3) float32 numpy array, or None.
    #[getter]
    fn normals<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray2<f32>>> {
        self.geom()
            .normals
            .as_ref()
            .map(|v| numpy_conv::f32x3_to_pyarray2(py, v))
    }

    /// UV0 as (N, 2) float32 numpy array, or None.
    #[getter]
    fn uv0<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray2<f32>>> {
        self.geom()
            .uv0
            .as_ref()
            .map(|v| numpy_conv::f32x2_to_pyarray2(py, v))
    }

    /// Vertex colors as (N, 4) uint8 numpy array, or None.
    #[getter]
    fn colors<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray2<u8>>> {
        self.geom()
            .colors
            .as_ref()
            .map(|v| numpy_conv::u8x4_to_pyarray2(py, v))
    }

    /// UV region as (N, 4) uint16 numpy array, or None.
    #[getter]
    fn uv_region<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray2<u16>>> {
        self.geom()
            .uv_region
            .as_ref()
            .map(|v| numpy_conv::u16x4_to_pyarray2(py, v))
    }

    /// Feature IDs as (N,) uint64 numpy array, or None.
    #[getter]
    fn feature_ids<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<u64>>> {
        self.geom()
            .feature_ids
            .as_ref()
            .map(|fids| numpy::ndarray::Array1::from_vec(fids.clone()).into_pyarray(py))
    }

    /// Face ranges as (N, 2) uint32 numpy array, or None.
    #[getter]
    fn face_ranges<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray2<u32>>> {
        self.geom()
            .face_ranges
            .as_ref()
            .map(|v| numpy_conv::u32x2_to_pyarray2(py, v))
    }

    fn __repr__(&self) -> String {
        format!(
            "GeometryData(vertices={}, features={})",
            self.geom().vertex_count,
            self.geom().feature_count
        )
    }
}


/// Loaded node content: geometry + texture + attribute data.
///
/// Content is reference-counted (`Arc`) so accessing sub-fields like
/// ``geometry`` is O(1) — no deep clone.
#[pyclass(name = "NodeContent")]
pub struct PyNodeContent {
    inner: Arc<NodeContent>,
}

#[pymethods]
impl PyNodeContent {
    /// Get the geometry data (O(1) — shared reference, no copy).
    #[getter]
    fn geometry(&self) -> PyGeometryData {
        PyGeometryData {
            _owner: self.inner.clone(),
        }
    }

    /// Raw texture bytes (JPEG, PNG, KTX2, etc.).
    #[getter]
    fn texture_data<'py>(&self, py: Python<'py>) -> Bound<'py, pyo3::types::PyBytes> {
        pyo3::types::PyBytes::new(py, &self.inner.texture_data)
    }

    /// Total byte size of this content.
    #[getter]
    fn byte_size(&self) -> usize {
        self.inner.byte_size
    }

    fn __repr__(&self) -> String {
        format!(
            "NodeContent(vertices={}, texture_bytes={}, total_bytes={})",
            self.inner.geometry.vertex_count,
            self.inner.texture_data.len(),
            self.inner.byte_size,
        )
    }
}


/// An I3S scene layer — the central runtime object.
///
/// Equivalent to cesium-native's ``Tileset``.
///
/// **Construction** (mirrors ``Tileset(externals, url, options)``)::
///
///     tp = NativeTaskProcessor(4)
///     async_system = AsyncSystem(tp)
///     externals = SceneLayerExternals(async_system)
///     layer = SceneLayer(externals, "https://tiles.arcgis.com/.../layers/0")
///
/// **Frame loop:**
///
/// 1. ``result = layer.update_view(view_states)`` — LOD selection
/// 2. ``layer.load_nodes(result)`` — dispatch loads, collect completed
/// 3. ``for rn in layer.nodes_to_render(result): ...`` — draw
///
/// **Offline:**
///
/// - ``result = layer.update_view_offline(view_states)`` — blocking
#[pyclass(name = "SceneLayer")]
pub struct PySceneLayer {
    inner: SceneLayer,
    /// Stash the last ViewUpdateResult so Python can iterate render nodes.
    last_result: Option<ViewUpdateResult>,
}

#[pymethods]
impl PySceneLayer {
    /// Open an I3S scene layer.
    ///
    /// Accepts both REST URLs (``http://``, ``https://``) and local SLPK
    /// file paths.  An optional ``crs_transform`` can be any object with a
    /// ``to_ecef(positions)`` method (e.g. ``WkidTransform``,
    /// ``ProjTransform``, or a custom implementation).
    ///
    /// Parameters
    /// ----------
    /// externals : SceneLayerExternals
    ///     External dependencies (async system, renderer resources).
    /// url : str
    ///     HTTP(S) URL to an I3S layer endpoint, **or** a path to a
    ///     local ``.slpk`` archive.
    /// crs_transform : object, optional
    ///     CRS-to-ECEF transform.  A ``WkidTransform`` (fast, pure-Rust)
    ///     or any object whose ``to_ecef(ndarray) -> ndarray`` method
    ///     converts ``(N, 3) float64`` positions to ECEF metres.
    /// options : SelectionOptions, optional
    ///     LOD selection and loading options.
    #[new]
    #[pyo3(signature = (externals, url, crs_transform=None, options=None))]
    fn new(
        externals: &PySceneLayerExternals,
        url: &str,
        crs_transform: Option<&Bound<'_, PyAny>>,
        options: Option<&PySelectionOptions>,
    ) -> PyResult<Self> {
        let opts = options.map(|o| o.inner.clone()).unwrap_or_default();
        let transform: Option<Arc<dyn i3s_geospatial::crs::CrsTransform>> =
            crs_transform.map(extract_crs_transform).transpose()?;
        let ext = externals.inner.clone();

        // Detect SLPK vs REST based on the URL/path.
        let lower = url.to_ascii_lowercase();
        let is_slpk = lower.ends_with(".slpk")
            || (!lower.starts_with("http://") && !lower.starts_with("https://"));

        let (accessor, resolver): (I3sAssetAccessor, Arc<dyn ResourceUriResolver>) = if is_slpk {
            let mut acc = I3sAssetAccessor::new();
            acc.register_slpk(std::path::Path::new(url))
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            (acc, Arc::new(SlpkUriResolver))
        } else {
            (I3sAssetAccessor::new(), Arc::new(RestUriResolver::new(url)))
        };

        // SAFETY: open() is pure Rust I/O only; no Python objects accessed.
        // Releasing the GIL lets other Python threads make progress.
        let layer = unsafe {
            numpy_conv::without_gil(|| {
                if let Some(xf) = transform {
                    SceneLayer::open_with_transform(accessor, resolver, ext, opts, xf)
                } else {
                    SceneLayer::open(accessor, resolver, ext, opts)
                }
            })
        }
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            inner: layer,
            last_result: None,
        })
    }

    /// Open an I3S scene layer asynchronously.
    ///
    /// Same arguments as ``__init__`` but returns a ``Future`` that
    /// resolves to a ``SceneLayer``.
    #[staticmethod]
    #[pyo3(signature = (externals, url, crs_transform=None, options=None))]
    fn open_async(
        externals: &PySceneLayerExternals,
        url: &str,
        crs_transform: Option<&Bound<'_, PyAny>>,
        options: Option<&PySelectionOptions>,
    ) -> PyResult<PyFuture> {
        let opts = options.map(|o| o.inner.clone()).unwrap_or_default();
        let ext = externals.inner.clone();
        let transform: Option<Arc<dyn i3s_geospatial::crs::CrsTransform>> =
            crs_transform.map(extract_crs_transform).transpose()?;

        let lower = url.to_ascii_lowercase();
        let is_slpk = lower.ends_with(".slpk")
            || (!lower.starts_with("http://") && !lower.starts_with("https://"));

        let (accessor, resolver): (I3sAssetAccessor, Arc<dyn ResourceUriResolver>) = if is_slpk {
            let mut acc = I3sAssetAccessor::new();
            acc.register_slpk(std::path::Path::new(url))
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            (acc, Arc::new(SlpkUriResolver))
        } else {
            (I3sAssetAccessor::new(), Arc::new(RestUriResolver::new(url)))
        };

        let (promise, future) = ext.async_system.create_promise();
        let tp = ext.async_system.task_processor().clone();
        tp.start_task(Box::new(move || {
            let result = (|| -> Result<Py<PyAny>, String> {
                let layer = if let Some(xf) = transform {
                    SceneLayer::open_with_transform(accessor, resolver, ext, opts, xf)
                } else {
                    SceneLayer::open(accessor, resolver, ext, opts)
                }
                .map_err(|e| e.to_string())?;
                Python::attach(|py| {
                    Py::new(
                        py,
                        PySceneLayer {
                            inner: layer,
                            last_result: None,
                        },
                    )
                    .map(|obj| obj.into_any())
                    .map_err(|e: PyErr| e.to_string())
                })
            })();
            match result {
                Ok(obj) => promise.resolve(obj),
                Err(e) => promise.reject(e),
            }
        }));

        Ok(PyFuture::from_rust(future))
    }


    /// The layer's coordinate reference system classification.
    #[getter]
    fn crs(&self) -> crate::geospatial::PySceneCoordinateSystem {
        match self.inner.crs() {
            SceneCoordinateSystem::Global => crate::geospatial::PySceneCoordinateSystem::Global,
            SceneCoordinateSystem::Local => crate::geospatial::PySceneCoordinateSystem::Local,
        }
    }

    /// Current frame counter.
    #[getter]
    fn frame(&self) -> u64 {
        self.inner.frame()
    }

    /// Load progress as a fraction [0.0, 1.0].
    #[getter]
    fn load_progress(&self) -> f64 {
        self.inner.load_progress()
    }

    /// The ellipsoid used for this layer.
    #[getter]
    fn ellipsoid(&self) -> PyEllipsoid {
        PyEllipsoid {
            inner: self.inner.ellipsoid(),
        }
    }

    /// Selection options (read/write).
    #[getter]
    fn options(&self) -> PySelectionOptions {
        PySelectionOptions {
            inner: self.inner.options.clone(),
        }
    }

    #[setter]
    fn set_options(&mut self, opts: &PySelectionOptions) {
        self.inner.options = opts.inner.clone();
    }

    /// Total cached bytes.
    #[getter]
    fn cached_bytes(&mut self) -> usize {
        self.inner.cache().total_bytes()
    }


    /// Run per-frame LOD selection (sync — pure computation).
    ///
    /// Parameters
    /// ----------
    /// view_states : list[ViewState]
    ///     One or more camera states. Multiple for VR / shadow cascades.
    ///
    /// Returns
    /// -------
    /// ViewUpdateResult
    fn update_view(&mut self, view_states: Vec<PyViewState>) -> PyViewUpdateResult {
        let views: Vec<ViewState> = view_states.iter().map(|v| v.inner).collect();
        // SAFETY: update_view is pure Rust traversal (frustum culling, SSE
        // tests).  No Python objects are accessed.
        let result = unsafe {
            numpy_conv::without_gil(|| self.inner.update_view(&views))
        };
        self.last_result = Some(result.clone());
        PyViewUpdateResult { inner: result }
    }

    /// Dispatch content loading, collect completed, finalize.
    ///
    /// Call after ``update_view()`` each frame.
    fn load_nodes(&mut self, result: &PyViewUpdateResult) {
        // SAFETY: load_nodes dispatches I/O to worker threads and collects
        // completed work.  The prepare_renderer_resources callbacks use
        // Python::attach() to reacquire the GIL when needed.
        unsafe {
            numpy_conv::without_gil(|| self.inner.load_nodes(&result.inner))
        }
    }

    /// Convenience: combined update_view + load_nodes.
    fn tick(&mut self, view_states: Vec<PyViewState>) -> PyViewUpdateResult {
        let views: Vec<ViewState> = view_states.iter().map(|v| v.inner).collect();
        // SAFETY: see update_view + load_nodes above.
        let result = unsafe {
            numpy_conv::without_gil(|| {
                let r = self.inner.update_view(&views);
                self.inner.load_nodes(&r);
                r
            })
        };
        self.last_result = Some(result.clone());
        PyViewUpdateResult { inner: result }
    }

    /// Blocking update — wait until all nodes meeting SSE are loaded.
    fn update_view_offline(&mut self, view_states: Vec<PyViewState>) -> PyViewUpdateResult {
        let views: Vec<ViewState> = view_states.iter().map(|v| v.inner).collect();
        // SAFETY: potentially long-running — must release GIL so other
        // threads can make progress and callbacks can reacquire it.
        let result = unsafe {
            numpy_conv::without_gil(|| self.inner.update_view_offline(&views))
        };
        self.last_result = Some(result.clone());
        PyViewUpdateResult { inner: result }
    }

    /// Get nodes selected for rendering as a list of RenderNode.
    ///
    /// Call after ``update_view()`` (uses the stored result).
    fn nodes_to_render(&self) -> PyResult<Vec<PyRenderNode>> {
        let result = self.last_result.as_ref().ok_or_else(|| {
            PyRuntimeError::new_err("No view update result — call update_view() first")
        })?;
        let nodes: Vec<PyRenderNode> = self.inner
            .nodes_to_render(result)
            .map(|rn| PyRenderNode {
                node_id: rn.node_id,
                center: rn.center,
                quaternion: rn.quaternion,
                half_size: rn.half_size,
                bounding_radius: rn.bounding_radius,
            })
            .collect();
        Ok(nodes)
    }

    /// Get the load state of a node.
    fn node_load_state(&self, node_id: u32) -> Option<PyNodeLoadState> {
        self.inner.node_state(node_id).map(|s| s.load_state.into())
    }

    /// Get cached content for a node.
    fn node_content(&mut self, node_id: u32) -> Option<PyNodeContent> {
        self.inner.cache().get(node_id).map(|c| PyNodeContent {
            inner: Arc::new(c.clone()),
        })
    }

    fn __repr__(&self) -> String {
        format!("SceneLayer(type={:?})", self.inner.info.layer_type())
    }
}

// Module registration

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyIPrepareRendererResources>()?;
    m.add_class::<PySceneLayerExternals>()?;
    m.add_class::<PyViewState>()?;
    m.add_class::<PySelectionOptions>()?;
    m.add_class::<PyViewUpdateResult>()?;
    m.add_class::<PyNodeLoadState>()?;
    m.add_class::<PyRenderNode>()?;
    m.add_class::<PyGeometryData>()?;
    m.add_class::<PyNodeContent>()?;
    m.add_class::<PySceneLayer>()?;
    Ok(())
}
