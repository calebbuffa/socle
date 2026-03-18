//! Python bindings for the `i3s` spec crate — I3S layer document types.
//!
//! Every struct and enum from the i3s crate is exposed here. Fixed-size
//! numeric arrays ([f64; N]) are returned as numpy arrays. Every class has
//! `to_dict()` / `__dict__` for full camelCase spec-JSON access.

use numpy::{IntoPyArray, PyArray1};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pythonize::pythonize;
use serde::Serialize;

use i3s::bld::Layer as BuildingLayer;
use i3s::cmn::{
    AttributeStorageInfo, ElevationInfo, Field, FieldType, FullExtent, GeometryDefinition,
    HeightModelInfo, LodSelection, LodSelectionMetricType, MaterialDefinitions, NodePageDefinition,
    Obb, SceneLayerCapabilities, SceneLayerInfo, SceneLayerType, SpatialReference, Store,
    TextureSetDefinition,
};
use i3s::pcsl::PointCloudLayer;
use i3s::psl::{SceneLayerInfoPsl, StorePsl};
use i3s_selection::layer_info::LayerInfo;

fn to_dict_via_serde<T: Serialize>(py: Python<'_>, value: &T) -> PyResult<Py<PyDict>> {
    pythonize(py, value)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
        .and_then(|obj| {
            obj.downcast_into::<PyDict>()
                .map_err(|_| {
                    pyo3::exceptions::PyValueError::new_err(
                        "pythonize produced a non-dict (enum variant without struct body)",
                    )
                })
                .map(|b| b.unbind())
        })
}

fn arr3_to_numpy(py: Python<'_>, v: [f64; 3]) -> Bound<'_, PyArray1<f64>> {
    numpy::ndarray::Array1::from(v.to_vec()).into_pyarray(py)
}

fn arr4_to_numpy(py: Python<'_>, v: [f64; 4]) -> Bound<'_, PyArray1<f64>> {
    numpy::ndarray::Array1::from(v.to_vec()).into_pyarray(py)
}

fn serde_str<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// I3S scene layer type discriminant.
#[pyclass(name = "SceneLayerType", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PySceneLayerType {
    ThreeDObject = 0,
    IntegratedMesh = 1,
    Point = 2,
    PointCloud = 3,
    Building = 4,
}

#[pymethods]
impl PySceneLayerType {
    fn __repr__(&self) -> &str {
        match self {
            Self::ThreeDObject => "SceneLayerType.ThreeDObject",
            Self::IntegratedMesh => "SceneLayerType.IntegratedMesh",
            Self::Point => "SceneLayerType.Point",
            Self::PointCloud => "SceneLayerType.PointCloud",
            Self::Building => "SceneLayerType.Building",
        }
    }
}

impl From<SceneLayerType> for PySceneLayerType {
    fn from(t: SceneLayerType) -> Self {
        match serde_str(&t).as_str() {
            "3DObject" => Self::ThreeDObject,
            "IntegratedMesh" => Self::IntegratedMesh,
            "Point" => Self::Point,
            "PointCloud" => Self::PointCloud,
            "Building" => Self::Building,
            _ => Self::Point,
        }
    }
}

/// I3S scene layer capabilities (View, Query, Edit, Extract).
#[pyclass(name = "SceneLayerCapabilities", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PySceneLayerCapabilities {
    View = 0,
    Query = 1,
    Edit = 2,
    Extract = 3,
}

#[pymethods]
impl PySceneLayerCapabilities {
    fn __repr__(&self) -> &str {
        match self {
            Self::View => "SceneLayerCapabilities.View",
            Self::Query => "SceneLayerCapabilities.Query",
            Self::Edit => "SceneLayerCapabilities.Edit",
            Self::Extract => "SceneLayerCapabilities.Extract",
        }
    }
}

impl From<SceneLayerCapabilities> for PySceneLayerCapabilities {
    fn from(c: SceneLayerCapabilities) -> Self {
        match c {
            SceneLayerCapabilities::View => Self::View,
            SceneLayerCapabilities::Query => Self::Query,
            SceneLayerCapabilities::Edit => Self::Edit,
            SceneLayerCapabilities::Extract => Self::Extract,
        }
    }
}

/// I3S attribute field type (esriFieldType* values).
#[pyclass(name = "FieldType", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PyFieldType {
    Date = 0,
    Single = 1,
    Double = 2,
    GUID = 3,
    GlobalID = 4,
    Integer = 5,
    OID = 6,
    SmallInteger = 7,
    String = 8,
}

#[pymethods]
impl PyFieldType {
    fn __repr__(&self) -> &str {
        match self {
            Self::Date => "FieldType.Date",
            Self::Single => "FieldType.Single",
            Self::Double => "FieldType.Double",
            Self::GUID => "FieldType.GUID",
            Self::GlobalID => "FieldType.GlobalID",
            Self::Integer => "FieldType.Integer",
            Self::OID => "FieldType.OID",
            Self::SmallInteger => "FieldType.SmallInteger",
            Self::String => "FieldType.String",
        }
    }
}

impl From<FieldType> for PyFieldType {
    fn from(t: FieldType) -> Self {
        match t {
            FieldType::Date => Self::Date,
            FieldType::Single => Self::Single,
            FieldType::Double => Self::Double,
            FieldType::GUID => Self::GUID,
            FieldType::GlobalID => Self::GlobalID,
            FieldType::Integer => Self::Integer,
            FieldType::OID => Self::OID,
            FieldType::SmallInteger => Self::SmallInteger,
            FieldType::String => Self::String,
        }
    }
}

/// LoD selection metric type.
#[pyclass(name = "LodSelectionMetricType", eq, eq_int, skip_from_py_object)]
#[derive(Clone, PartialEq)]
pub enum PyLodSelectionMetricType {
    MaxScreenThreshold = 0,
    MaxScreenThresholdSQ = 1,
    ScreenSpaceRelative = 2,
    DistanceRangeFromDefaultCamera = 3,
    EffectiveDensity = 4,
}

#[pymethods]
impl PyLodSelectionMetricType {
    fn __repr__(&self) -> &str {
        match self {
            Self::MaxScreenThreshold => "LodSelectionMetricType.MaxScreenThreshold",
            Self::MaxScreenThresholdSQ => "LodSelectionMetricType.MaxScreenThresholdSQ",
            Self::ScreenSpaceRelative => "LodSelectionMetricType.ScreenSpaceRelative",
            Self::DistanceRangeFromDefaultCamera => {
                "LodSelectionMetricType.DistanceRangeFromDefaultCamera"
            }
            Self::EffectiveDensity => "LodSelectionMetricType.EffectiveDensity",
        }
    }
}

impl From<LodSelectionMetricType> for PyLodSelectionMetricType {
    fn from(t: LodSelectionMetricType) -> Self {
        match serde_str(&t).as_str() {
            "maxScreenThreshold" => Self::MaxScreenThreshold,
            "maxScreenThresholdSQ" => Self::MaxScreenThresholdSQ,
            "screenSpaceRelative" => Self::ScreenSpaceRelative,
            "distanceRangeFromDefaultCamera" => Self::DistanceRangeFromDefaultCamera,
            "effectiveDensity" => Self::EffectiveDensity,
            _ => Self::MaxScreenThreshold,
        }
    }
}

/// I3S spatial reference — a CRS defined by WKID or WKT.
#[pyclass(name = "SpatialReference")]
#[derive(Clone)]
pub struct PySpatialReference {
    pub inner: SpatialReference,
}

#[pymethods]
impl PySpatialReference {
    #[getter]
    fn wkid(&self) -> Option<i64> {
        self.inner.wkid
    }
    #[getter]
    fn latest_wkid(&self) -> Option<i64> {
        self.inner.latest_wkid
    }
    #[getter]
    fn vcs_wkid(&self) -> Option<i64> {
        self.inner.vcs_wkid
    }
    #[getter]
    fn latest_vcs_wkid(&self) -> Option<i64> {
        self.inner.latest_vcs_wkid
    }
    #[getter]
    fn wkt(&self) -> Option<&str> {
        self.inner.wkt.as_deref()
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        match (self.inner.wkid, &self.inner.wkt) {
            (Some(w), _) => format!("SpatialReference(wkid={})", w),
            (_, Some(_)) => "SpatialReference(wkt=<...>)".to_string(),
            _ => "SpatialReference()".to_string(),
        }
    }
}

/// I3S Oriented Bounding Box in geographic / CRS coordinates.
#[pyclass(name = "OrientedBoundingBox")]
#[derive(Clone)]
pub struct PyOrientedBoundingBox {
    pub inner: Obb,
}

#[pymethods]
impl PyOrientedBoundingBox {
    #[getter]
    fn center<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        arr3_to_numpy(py, self.inner.center)
    }
    #[getter]
    fn half_size<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        arr3_to_numpy(py, self.inner.half_size)
    }
    #[getter]
    fn quaternion<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<f64>> {
        arr4_to_numpy(py, self.inner.quaternion)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        let c = &self.inner.center;
        format!(
            "OrientedBoundingBox(center=[{:.4}, {:.4}, {:.1}])",
            c[0], c[1], c[2]
        )
    }
}

/// 3-D spatial extent of a scene layer.
#[pyclass(name = "FullExtent")]
#[derive(Clone)]
pub struct PyFullExtent {
    pub inner: FullExtent,
}

#[pymethods]
impl PyFullExtent {
    #[getter]
    fn xmin(&self) -> f64 {
        self.inner.xmin
    }
    #[getter]
    fn ymin(&self) -> f64 {
        self.inner.ymin
    }
    #[getter]
    fn xmax(&self) -> f64 {
        self.inner.xmax
    }
    #[getter]
    fn ymax(&self) -> f64 {
        self.inner.ymax
    }
    #[getter]
    fn zmin(&self) -> f64 {
        self.inner.zmin
    }
    #[getter]
    fn zmax(&self) -> f64 {
        self.inner.zmax
    }
    #[getter]
    fn spatial_reference(&self) -> Option<PySpatialReference> {
        self.inner
            .spatial_reference
            .clone()
            .map(|sr| PySpatialReference { inner: sr })
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "FullExtent(x=[{:.4}, {:.4}], y=[{:.4}, {:.4}], z=[{:.1}, {:.1}])",
            self.inner.xmin,
            self.inner.xmax,
            self.inner.ymin,
            self.inner.ymax,
            self.inner.zmin,
            self.inner.zmax,
        )
    }
}

/// Height model and vertical CRS information.
#[pyclass(name = "HeightModelInfo")]
#[derive(Clone)]
pub struct PyHeightModelInfo {
    pub inner: HeightModelInfo,
}

#[pymethods]
impl PyHeightModelInfo {
    #[getter]
    fn height_model(&self) -> Option<String> {
        self.inner.height_model.as_ref().map(serde_str)
    }
    #[getter]
    fn vert_crs(&self) -> Option<&str> {
        self.inner.vert_crs.as_deref()
    }
    #[getter]
    fn height_unit(&self) -> Option<String> {
        self.inner.height_unit.as_ref().map(serde_str)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "HeightModelInfo(model={:?}, unit={:?})",
            self.inner.height_model, self.inner.height_unit
        )
    }
}

/// Elevation placement mode for a scene layer.
#[pyclass(name = "ElevationInfo")]
#[derive(Clone)]
pub struct PyElevationInfo {
    pub inner: ElevationInfo,
}

#[pymethods]
impl PyElevationInfo {
    #[getter]
    fn mode(&self) -> Option<String> {
        self.inner.mode.as_ref().map(serde_str)
    }
    #[getter]
    fn offset(&self) -> Option<f64> {
        self.inner.offset
    }
    #[getter]
    fn unit(&self) -> Option<&str> {
        self.inner.unit.as_deref()
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!("ElevationInfo(mode={:?})", self.inner.mode)
    }
}

/// An I3S attribute field descriptor.
#[pyclass(name = "Field")]
#[derive(Clone)]
pub struct PyField {
    pub inner: Field,
}

#[pymethods]
impl PyField {
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn alias(&self) -> Option<&str> {
        self.inner.alias.as_deref()
    }
    #[getter]
    fn field_type(&self) -> PyFieldType {
        self.inner.r#type.clone().into()
    }
    #[getter]
    fn field_type_str(&self) -> String {
        serde_str(&self.inner.r#type)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "Field(name={:?}, type={:?})",
            self.inner.name,
            self.field_type_str()
        )
    }
}

/// Binary attribute storage descriptor for a layer field.
#[pyclass(name = "AttributeStorageInfo")]
#[derive(Clone)]
pub struct PyAttributeStorageInfo {
    pub inner: AttributeStorageInfo,
}

#[pymethods]
impl PyAttributeStorageInfo {
    #[getter]
    fn key(&self) -> &str {
        &self.inner.key
    }
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn ordering(&self) -> Option<String> {
        self.inner.ordering.as_ref().map(serde_str)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "AttributeStorageInfo(key={:?}, name={:?})",
            self.inner.key, self.inner.name
        )
    }
}

/// LoD selection metric for a node.
#[pyclass(name = "LodSelection")]
#[derive(Clone)]
pub struct PyLodSelection {
    pub inner: LodSelection,
}

#[pymethods]
impl PyLodSelection {
    #[getter]
    fn metric_type(&self) -> PyLodSelectionMetricType {
        self.inner.metric_type.clone().into()
    }
    #[getter]
    fn metric_type_str(&self) -> String {
        serde_str(&self.inner.metric_type)
    }
    #[getter]
    fn max_error(&self) -> f64 {
        self.inner.max_error
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "LodSelection(metric={:?}, max_error={})",
            self.metric_type_str(),
            self.inner.max_error
        )
    }
}

/// Node page definition — describes how nodes are organized into fixed-size pages.
#[pyclass(name = "NodePageDefinition")]
#[derive(Clone)]
pub struct PyNodePageDefinition {
    pub inner: NodePageDefinition,
}

#[pymethods]
impl PyNodePageDefinition {
    #[getter]
    fn nodes_per_page(&self) -> i64 {
        self.inner.nodes_per_page
    }
    #[getter]
    fn root_index(&self) -> Option<i64> {
        self.inner.root_index
    }
    #[getter]
    fn lod_selection_metric_type(&self) -> String {
        serde_str(&self.inner.lod_selection_metric_type)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "NodePageDefinition(nodes_per_page={})",
            self.inner.nodes_per_page
        )
    }
}

/// Geometry definition for a mesh node (topology + buffer layout).
#[pyclass(name = "GeometryDefinition")]
#[derive(Clone)]
pub struct PyGeometryDefinition {
    pub inner: GeometryDefinition,
}

#[pymethods]
impl PyGeometryDefinition {
    #[getter]
    fn topology(&self) -> Option<String> {
        self.inner.topology.as_ref().map(serde_str)
    }
    #[getter]
    fn geometry_buffers(&self) -> &str {
        &self.inner.geometry_buffers
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!("GeometryDefinition(topology={:?})", self.topology())
    }
}

/// Material definition (PBR metallic-roughness, supports glTF materials).
#[pyclass(name = "MaterialDefinitions")]
#[derive(Clone)]
pub struct PyMaterialDefinitions {
    pub inner: MaterialDefinitions,
}

#[pymethods]
impl PyMaterialDefinitions {
    #[getter]
    fn alpha_mode(&self) -> Option<String> {
        self.inner.alpha_mode.as_ref().map(serde_str)
    }
    #[getter]
    fn alpha_cutoff(&self) -> Option<f64> {
        self.inner.alpha_cutoff
    }
    #[getter]
    fn double_sided(&self) -> Option<bool> {
        self.inner.double_sided
    }
    #[getter]
    fn cull_face(&self) -> Option<String> {
        self.inner.cull_face.as_ref().map(serde_str)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!("MaterialDefinitions(alpha_mode={:?})", self.alpha_mode())
    }
}

/// Texture set definition — available formats for a texture set.
#[pyclass(name = "TextureSetDefinition")]
#[derive(Clone)]
pub struct PyTextureSetDefinition {
    pub inner: TextureSetDefinition,
}

#[pymethods]
impl PyTextureSetDefinition {
    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!("TextureSetDefinition(formats={})", self.inner.formats.len())
    }
}

/// Physical storage descriptor for a scene layer store.
#[pyclass(name = "Store")]
#[derive(Clone)]
pub struct PyStore {
    pub inner: Store,
}

#[pymethods]
impl PyStore {
    #[getter]
    fn id(&self) -> Option<&str> {
        self.inner.id.as_deref()
    }
    #[getter]
    fn profile(&self) -> &str {
        &self.inner.profile
    }
    #[getter]
    fn root_node(&self) -> Option<&str> {
        self.inner.root_node.as_deref()
    }
    #[getter]
    fn version(&self) -> &str {
        &self.inner.version
    }
    #[getter]
    fn index_crs(&self) -> Option<&str> {
        self.inner.index_crs.as_deref()
    }
    #[getter]
    fn vertex_crs(&self) -> Option<&str> {
        self.inner.vertex_crs.as_deref()
    }
    #[getter]
    fn extent<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner.extent.map(|e| arr4_to_numpy(py, e))
    }
    #[getter]
    fn normal_reference_frame(&self) -> Option<String> {
        self.inner.normal_reference_frame.as_ref().map(serde_str)
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "Store(profile={:?}, version={:?})",
            self.inner.profile, self.inner.version
        )
    }
}

/// Physical storage descriptor for a Point scene layer store.
#[pyclass(name = "StorePsl")]
#[derive(Clone)]
pub struct PyStorePsl {
    pub inner: StorePsl,
}

#[pymethods]
impl PyStorePsl {
    #[getter]
    fn id(&self) -> Option<&str> {
        self.inner.id.as_deref()
    }
    #[getter]
    fn profile(&self) -> &str {
        &self.inner.profile
    }
    #[getter]
    fn root_node(&self) -> Option<&str> {
        self.inner.root_node.as_deref()
    }
    #[getter]
    fn version(&self) -> &str {
        &self.inner.version
    }
    #[getter]
    fn index_crs(&self) -> Option<&str> {
        self.inner.index_crs.as_deref()
    }
    #[getter]
    fn vertex_crs(&self) -> Option<&str> {
        self.inner.vertex_crs.as_deref()
    }
    #[getter]
    fn extent<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f64>>> {
        self.inner.extent.map(|e| arr4_to_numpy(py, e))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "StorePsl(profile={:?}, version={:?})",
            self.inner.profile, self.inner.version
        )
    }
}

/// Metadata for a 3DObject or IntegratedMesh scene layer.
///
/// Returned by ``LayerInfo.as_mesh()``. Use ``to_dict()`` or ``__dict__``
/// for the full I3S spec document.
#[pyclass(name = "MeshLayerInfo")]
#[derive(Clone)]
pub struct PyMeshLayerInfo {
    pub inner: SceneLayerInfo,
}

#[pymethods]
impl PyMeshLayerInfo {
    #[getter]
    fn id(&self) -> i64 {
        self.inner.id
    }
    #[getter]
    fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }
    #[getter]
    fn alias(&self) -> Option<&str> {
        self.inner.alias.as_deref()
    }
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }
    #[getter]
    fn copyright_text(&self) -> Option<&str> {
        self.inner.copyright_text.as_deref()
    }
    #[getter]
    fn version(&self) -> &str {
        &self.inner.version
    }
    #[getter]
    fn z_factor(&self) -> Option<f64> {
        self.inner.z_factor
    }
    #[getter]
    fn disable_popup(&self) -> Option<bool> {
        self.inner.disable_popup
    }
    #[getter]
    fn href(&self) -> Option<&str> {
        self.inner.href.as_deref()
    }

    #[getter]
    fn layer_type(&self) -> PySceneLayerType {
        self.inner.layer_type.clone().into()
    }
    #[getter]
    fn capabilities(&self) -> PySceneLayerCapabilities {
        self.inner.capabilities.clone().into()
    }
    #[getter]
    fn spatial_reference(&self) -> Option<PySpatialReference> {
        self.inner
            .spatial_reference
            .clone()
            .map(|sr| PySpatialReference { inner: sr })
    }
    #[getter]
    fn height_model_info(&self) -> Option<PyHeightModelInfo> {
        self.inner
            .height_model_info
            .clone()
            .map(|h| PyHeightModelInfo { inner: h })
    }
    #[getter]
    fn elevation_info(&self) -> Option<PyElevationInfo> {
        self.inner
            .elevation_info
            .clone()
            .map(|e| PyElevationInfo { inner: e })
    }
    #[getter]
    fn full_extent(&self) -> Option<PyFullExtent> {
        self.inner
            .full_extent
            .clone()
            .map(|fe| PyFullExtent { inner: fe })
    }
    #[getter]
    fn store(&self) -> PyStore {
        PyStore {
            inner: self.inner.store.clone(),
        }
    }
    #[getter]
    fn fields(&self) -> Vec<PyField> {
        self.inner
            .fields
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|f| PyField { inner: f.clone() })
            .collect()
    }
    #[getter]
    fn attribute_storage_info(&self) -> Vec<PyAttributeStorageInfo> {
        self.inner
            .attribute_storage_info
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|a| PyAttributeStorageInfo { inner: a.clone() })
            .collect()
    }
    #[getter]
    fn node_pages(&self) -> Option<PyNodePageDefinition> {
        self.inner
            .node_pages
            .clone()
            .map(|n| PyNodePageDefinition { inner: n })
    }
    #[getter]
    fn geometry_definitions(&self) -> Vec<PyGeometryDefinition> {
        self.inner
            .geometry_definitions
            .iter()
            .map(|g| PyGeometryDefinition { inner: g.clone() })
            .collect()
    }
    #[getter]
    fn material_definitions(&self) -> Vec<PyMaterialDefinitions> {
        self.inner
            .material_definitions
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|m| PyMaterialDefinitions { inner: m.clone() })
            .collect()
    }
    #[getter]
    fn texture_set_definitions(&self) -> Vec<PyTextureSetDefinition> {
        self.inner
            .texture_set_definitions
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|t| PyTextureSetDefinition { inner: t.clone() })
            .collect()
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "MeshLayerInfo(id={}, name={:?}, version={})",
            self.inner.id, self.inner.name, self.inner.version
        )
    }
}

/// Metadata for a Point (PSL) scene layer. Returned by ``LayerInfo.as_point()``.
#[pyclass(name = "PointLayerInfo")]
#[derive(Clone)]
pub struct PyPointLayerInfo {
    pub inner: SceneLayerInfoPsl,
}

#[pymethods]
impl PyPointLayerInfo {
    #[getter]
    fn id(&self) -> i64 {
        self.inner.id
    }
    #[getter]
    fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }
    #[getter]
    fn alias(&self) -> Option<&str> {
        self.inner.alias.as_deref()
    }
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }
    #[getter]
    fn copyright_text(&self) -> Option<&str> {
        self.inner.copyright_text.as_deref()
    }
    #[getter]
    fn version(&self) -> &str {
        &self.inner.version
    }
    #[getter]
    fn z_factor(&self) -> Option<f64> {
        self.inner.z_factor
    }
    #[getter]
    fn disable_popup(&self) -> Option<bool> {
        self.inner.disable_popup
    }
    #[getter]
    fn href(&self) -> Option<&str> {
        self.inner.href.as_deref()
    }

    #[getter]
    fn layer_type(&self) -> PySceneLayerType {
        self.inner.layer_type.clone().into()
    }
    #[getter]
    fn capabilities(&self) -> PySceneLayerCapabilities {
        self.inner.capabilities.clone().into()
    }
    #[getter]
    fn spatial_reference(&self) -> Option<PySpatialReference> {
        self.inner
            .spatial_reference
            .clone()
            .map(|sr| PySpatialReference { inner: sr })
    }
    #[getter]
    fn height_model_info(&self) -> Option<PyHeightModelInfo> {
        self.inner
            .height_model_info
            .clone()
            .map(|h| PyHeightModelInfo { inner: h })
    }
    #[getter]
    fn elevation_info(&self) -> Option<PyElevationInfo> {
        self.inner
            .elevation_info
            .clone()
            .map(|e| PyElevationInfo { inner: e })
    }
    #[getter]
    fn store(&self) -> PyStorePsl {
        PyStorePsl {
            inner: self.inner.store.clone(),
        }
    }
    #[getter]
    fn fields(&self) -> Vec<PyField> {
        self.inner
            .fields
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|f| PyField { inner: f.clone() })
            .collect()
    }
    #[getter]
    fn attribute_storage_info(&self) -> Vec<PyAttributeStorageInfo> {
        self.inner
            .attribute_storage_info
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|a| PyAttributeStorageInfo { inner: a.clone() })
            .collect()
    }
    #[getter]
    fn node_pages(&self) -> Option<PyNodePageDefinition> {
        self.inner
            .point_node_pages
            .clone()
            .map(|n| PyNodePageDefinition { inner: n })
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "PointLayerInfo(id={}, name={:?}, version={})",
            self.inner.id, self.inner.name, self.inner.version
        )
    }
}

/// Metadata for a PointCloud (PCSL) scene layer. Returned by ``LayerInfo.as_point_cloud()``.
#[pyclass(name = "PointCloudLayerInfo")]
#[derive(Clone)]
pub struct PyPointCloudLayerInfo {
    pub inner: PointCloudLayer,
}

#[pymethods]
impl PyPointCloudLayerInfo {
    #[getter]
    fn id(&self) -> i64 {
        self.inner.id
    }
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn alias(&self) -> Option<&str> {
        self.inner.alias.as_deref()
    }
    #[getter]
    fn desc(&self) -> Option<&str> {
        self.inner.desc.as_deref()
    }
    #[getter]
    fn copyright_text(&self) -> Option<&str> {
        self.inner.copyright_text.as_deref()
    }
    #[getter]
    fn layer_type(&self) -> PySceneLayerType {
        PySceneLayerType::PointCloud
    }
    #[getter]
    fn spatial_reference(&self) -> PySpatialReference {
        PySpatialReference {
            inner: self.inner.spatial_reference.clone(),
        }
    }
    #[getter]
    fn height_model_info(&self) -> Option<PyHeightModelInfo> {
        self.inner
            .height_model_info
            .clone()
            .map(|h| PyHeightModelInfo { inner: h })
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "PointCloudLayerInfo(id={}, name={:?})",
            self.inner.id, self.inner.name
        )
    }
}

/// Metadata for a Building (BLD) scene layer. Returned by ``LayerInfo.as_building()``.
#[pyclass(name = "BuildingLayerInfo")]
#[derive(Clone)]
pub struct PyBuildingLayerInfo {
    pub inner: BuildingLayer,
}

#[pymethods]
impl PyBuildingLayerInfo {
    #[getter]
    fn id(&self) -> i64 {
        self.inner.id
    }
    #[getter]
    fn name(&self) -> &str {
        &self.inner.name
    }
    #[getter]
    fn version(&self) -> &str {
        &self.inner.version
    }
    #[getter]
    fn alias(&self) -> Option<&str> {
        self.inner.alias.as_deref()
    }
    #[getter]
    fn description(&self) -> Option<&str> {
        self.inner.description.as_deref()
    }
    #[getter]
    fn copyright_text(&self) -> Option<&str> {
        self.inner.copyright_text.as_deref()
    }
    #[getter]
    fn layer_type(&self) -> PySceneLayerType {
        PySceneLayerType::Building
    }
    #[getter]
    fn spatial_reference(&self) -> PySpatialReference {
        PySpatialReference {
            inner: self.inner.spatial_reference.clone(),
        }
    }
    #[getter]
    fn full_extent(&self) -> PyFullExtent {
        PyFullExtent {
            inner: self.inner.full_extent.clone(),
        }
    }
    #[getter]
    fn height_model_info(&self) -> Option<PyHeightModelInfo> {
        self.inner
            .height_model_info
            .clone()
            .map(|h| PyHeightModelInfo { inner: h })
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        to_dict_via_serde(py, &self.inner)
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!(
            "BuildingLayerInfo(id={}, name={:?}, version={})",
            self.inner.id, self.inner.name, self.inner.version
        )
    }
}

/// Typed layer document. Obtain via ``SceneLayer.layer_info``.
#[pyclass(name = "LayerInfo")]
pub struct PyLayerInfo {
    pub inner: LayerInfo,
}

#[pymethods]
impl PyLayerInfo {
    #[getter]
    fn layer_type(&self) -> PySceneLayerType {
        self.inner.layer_type().into()
    }
    #[getter]
    fn spatial_reference(&self) -> Option<PySpatialReference> {
        self.inner
            .spatial_reference()
            .cloned()
            .map(|sr| PySpatialReference { inner: sr })
    }

    fn as_mesh(&self) -> Option<PyMeshLayerInfo> {
        self.inner
            .as_mesh()
            .cloned()
            .map(|inner| PyMeshLayerInfo { inner })
    }
    fn as_point(&self) -> Option<PyPointLayerInfo> {
        self.inner
            .as_point()
            .cloned()
            .map(|inner| PyPointLayerInfo { inner })
    }
    fn as_point_cloud(&self) -> Option<PyPointCloudLayerInfo> {
        self.inner
            .as_point_cloud()
            .cloned()
            .map(|inner| PyPointCloudLayerInfo { inner })
    }
    fn as_building(&self) -> Option<PyBuildingLayerInfo> {
        self.inner
            .as_building()
            .cloned()
            .map(|inner| PyBuildingLayerInfo { inner })
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        match &self.inner {
            LayerInfo::Mesh(info) => to_dict_via_serde(py, info),
            LayerInfo::Point(info) => to_dict_via_serde(py, info),
            LayerInfo::PointCloud(info) => to_dict_via_serde(py, info),
            LayerInfo::Building(info) => to_dict_via_serde(py, info),
        }
    }
    #[getter]
    fn __dict__(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        self.to_dict(py)
    }
    fn __repr__(&self) -> String {
        format!("LayerInfo(type={:?})", self.inner.layer_type())
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySceneLayerType>()?;
    m.add_class::<PySceneLayerCapabilities>()?;
    m.add_class::<PyFieldType>()?;
    m.add_class::<PyLodSelectionMetricType>()?;
    m.add_class::<PySpatialReference>()?;
    m.add_class::<PyOrientedBoundingBox>()?;
    m.add_class::<PyFullExtent>()?;
    m.add_class::<PyHeightModelInfo>()?;
    m.add_class::<PyElevationInfo>()?;
    m.add_class::<PyField>()?;
    m.add_class::<PyAttributeStorageInfo>()?;
    m.add_class::<PyLodSelection>()?;
    m.add_class::<PyNodePageDefinition>()?;
    m.add_class::<PyGeometryDefinition>()?;
    m.add_class::<PyMaterialDefinitions>()?;
    m.add_class::<PyTextureSetDefinition>()?;
    m.add_class::<PyStore>()?;
    m.add_class::<PyStorePsl>()?;
    m.add_class::<PyMeshLayerInfo>()?;
    m.add_class::<PyPointLayerInfo>()?;
    m.add_class::<PyPointCloudLayerInfo>()?;
    m.add_class::<PyBuildingLayerInfo>()?;
    m.add_class::<PyLayerInfo>()?;
    Ok(())
}
