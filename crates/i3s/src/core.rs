//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: core

use serde::{Deserialize, Serialize};

use crate::display::CachedDrawingInfo;
use crate::display::DrawingInfo;
use crate::display::PopupInfo;
use crate::feature::AttributeStorageInfo;
use crate::feature::Field;
use crate::feature::RangeInfo;
use crate::feature::StatisticsInfo;
use crate::feature::TimeInfo;
use crate::geometry::DefaultGeometrySchema;
use crate::geometry::GeometryDefinition;
use crate::geometry::GeometryDefinitionPsl;
use crate::material::MaterialDefinition;
use crate::material::MaterialDefinitions;
use crate::material::Texture;
use crate::material::TextureSetDefinition;
use crate::node::NodePageDefinition;
use crate::spatial::ElevationInfo;
use crate::spatial::FullExtent;
use crate::spatial::HeightModelInfo;
use crate::spatial::SpatialReference;

/// Capabilities supported by a scene layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SceneLayerCapabilities {
    View,
    Query,
    Edit,
    Extract,
}

impl Default for SceneLayerCapabilities {
    fn default() -> Self {
        Self::View
    }
}

/// I3S scene layer type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SceneLayerType {
    #[serde(rename = "3DObject")]
    ThreeDObject,
    #[serde(rename = "IntegratedMesh")]
    Integratedmesh,
    Point,
    #[serde(rename = "PointCloud")]
    Pointcloud,
    Building,
}

impl Default for SceneLayerType {
    fn default() -> Self {
        Self::ThreeDObject
    }
}

/// Possible values for `Store::resourcePattern`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoreResourcePattern {
    #[serde(rename = "3dNodeIndexDocument")]
    ThreeDNodeIndexDocument,
    #[serde(rename = "SharedResource")]
    Sharedresource,
    #[serde(rename = "featureData")]
    Featuredata,
    Geometry,
    Texture,
    Attributes,
}

impl Default for StoreResourcePattern {
    fn default() -> Self {
        Self::ThreeDNodeIndexDocument
    }
}

/// Possible values for `Store::normalReferenceFrame`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoreNormalReferenceFrame {
    #[serde(rename = "east-north-up")]
    EastNorthUp,
    #[serde(rename = "earth-centered")]
    EarthCentered,
    #[serde(rename = "vertex-reference-frame")]
    VertexReferenceFrame,
}

impl Default for StoreNormalReferenceFrame {
    fn default() -> Self {
        Self::EastNorthUp
    }
}

/// Possible values for `Store::lodType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoreLodType {
    #[serde(rename = "MeshPyramid")]
    Meshpyramid,
    #[serde(rename = "AutoThinning")]
    Autothinning,
    Clustering,
    Generalizing,
}

impl Default for StoreLodType {
    fn default() -> Self {
        Self::Meshpyramid
    }
}

/// Possible values for `Store::lodModel`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoreLodModel {
    #[serde(rename = "node-switching")]
    NodeSwitching,
    #[serde(rename = "none")]
    None,
}

impl Default for StoreLodModel {
    fn default() -> Self {
        Self::NodeSwitching
    }
}

/// Possible values for `StorePsl::resourcePattern`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorePslResourcePattern {
    #[serde(rename = "3dNodeIndexDocument")]
    ThreeDNodeIndexDocument,
    #[serde(rename = "SharedResource")]
    Sharedresource,
    #[serde(rename = "featureData")]
    Featuredata,
    Geometry,
    Texture,
    Attributes,
}

impl Default for StorePslResourcePattern {
    fn default() -> Self {
        Self::ThreeDNodeIndexDocument
    }
}

/// Possible values for `StorePsl::normalReferenceFrame`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorePslNormalReferenceFrame {
    #[serde(rename = "east-north-up")]
    EastNorthUp,
    #[serde(rename = "earth-centered")]
    EarthCentered,
    #[serde(rename = "vertex-reference-frame")]
    VertexReferenceFrame,
}

impl Default for StorePslNormalReferenceFrame {
    fn default() -> Self {
        Self::EastNorthUp
    }
}

/// Possible values for `StorePsl::lodType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorePslLodType {
    #[serde(rename = "MeshPyramid")]
    Meshpyramid,
    #[serde(rename = "AutoThinning")]
    Autothinning,
    Clustering,
    Generalizing,
}

impl Default for StorePslLodType {
    fn default() -> Self {
        Self::Meshpyramid
    }
}

/// Possible values for `StorePsl::lodModel`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StorePslLodModel {
    #[serde(rename = "node-switching")]
    NodeSwitching,
    #[serde(rename = "none")]
    None,
}

impl Default for StorePslLodModel {
    fn default() -> Self {
        Self::NodeSwitching
    }
}

/// The metadata.json contains information regarding the creation and storing of i3s in SLPK to
/// support clients with i3s service creation and processing of the data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Metadata {
    /// Total number of nodes in the SLPK.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_count: Option<f64>,
}

/// Object to provide time stamp when the I3S service or the source of the service was created or
/// updated.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct ServiceUpdateTimeStamp {
    /// Specifies the Unix epoch counting from 1 January 1970 in milliseconds. Time stamp is created when the I3S service was created or updated.
    pub last_update: f64,
}

/// Scanning an SLPK (ZIP store) containing millions of documents is usually inefficient and slow.
/// A hash table file may be added to the SLPK to improve first load and file scanning
/// performances.  A hash table is a data structure that implements an associative array abstract
/// data type, a structure that can map keys to values. A hash table uses a hash function to
/// compute an index, also called a hash code, into an array of buckets or slots, from which the
/// desired value can be found (Wikipedia).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct SlpkHashtable {}

/// The 3DSceneLayerInfo describes the properties of a layer in a store. The store object describes
/// the exact physical storage of a layer and enables the client to detect when multiple layers are
/// served from the same store. Every scene layer contains 3DSceneLayerInfo. If features based
/// scene layers, such as 3D objects or point scene layers, may include the default symbology. This
/// is as specified in the drawingInfo, which contains styling information for a feature layer.
/// When generating 3D Objects or Integrated Mesh scene layers, the root node never has any
/// geometry. Any node's children represent a higher LoD quality than an ancestor node.  Nodes
/// without geometry at the top of the tree are allowable since the lowest LoD of a
/// feature/geometry is not to shown.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct SceneLayerInfo {
    /// Unique numeric ID of the layer.
    pub id: i64,
    /// The relative URL to the 3DSceneLayerResource. Only present as part of the SceneServiceInfo resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// The user-visible layer typePossible values are:`3DObject``IntegratedMesh`
    pub layer_type: SceneLayerType,
    /// The spatialReference of the layer including the vertical coordinate reference system (CRS). Well Known Text (WKT) for CRS is included to support custom CRS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_reference: Option<SpatialReference>,
    /// Enables consuming clients to quickly determine whether this layer is compatible (with respect to its horizontal and vertical coordinate system) with existing content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_model_info: Option<HeightModelInfo>,
    /// The ID of the last update session in which any resource belonging to this layer has been updated.
    pub version: String,
    /// The name of this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The time of the last update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_update_time_stamp: Option<ServiceUpdateTimeStamp>,
    /// The display alias to be used for this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Description string for this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Copyright and usage information for the data in this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright_text: Option<String>,
    /// Capabilities supported by this layer.Possible values for each array string:`View`: View is supported.`Query`: Query is supported.`Edit`: Edit is defined.`Extract`: Extract is defined.
    pub capabilities: SceneLayerCapabilities,
    /// ZFactor to define conversion factor for elevation unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_factor: Option<f64>,
    /// Indicates if any styling information represented as drawingInfo is captured as part of the binary mesh representation.  This helps provide optimal client-side access. Currently the color component of ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_drawing_info: Option<CachedDrawingInfo>,
    /// An object containing drawing information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing_info: Option<DrawingInfo>,
    /// An object containing elevation drawing information. If absent, any content of the scene layer is drawn at its z coordinate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation_info: Option<ElevationInfo>,
    /// PopupInfo of the scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popup_info: Option<PopupInfo>,
    /// Indicates if client application will show the popup information. Default is FALSE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_popup: Option<bool>,
    /// The store object describes the exact physical storage of a layer and enables the client to detect when multiple layers are served from the same store.
    pub store: Store,
    /// A collection of objects that describe each attribute field regarding its field name, datatype, and a user friendly name {name,type,alias}. It includes all fields that are included as part of the scene...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<Field>>,
    /// Provides the schema and layout used for storing attribute content in binary format in I3S.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_storage_info: Option<Vec<AttributeStorageInfo>>,
    /// Contains the statistical information for a layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics_info: Option<Vec<StatisticsInfo>>,
    /// The paged-access index description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_pages: Option<NodePageDefinition>,
    /// List of materials classes used in this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_definitions: Option<Vec<MaterialDefinitions>>,
    /// Defines the set of textures that can be referenced by meshes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_set_definitions: Option<Vec<TextureSetDefinition>>,
    /// Define the layouts of mesh geometry and its attributes.
    pub geometry_definitions: Vec<GeometryDefinition>,
    /// 3D extent. If ```layer.fullExtent.spatialReference``` is specified, it must match ```layer.spatialReference```.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_extent: Option<FullExtent>,
    /// Time info represents the temporal data of a time-aware layer. The time info class provides information such as date fields that store the start and end times for each feature and the total time span f...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_info: Option<TimeInfo>,
    /// Range info is used to filter features of a layer withing a min and max range. The min and max range is created from the statistical information of the range field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_info: Option<RangeInfo>,
}

/// The 3DSceneLayerInfo object describes the properties of a layer in a store. Every scene layer
/// contains 3DSceneLayerInfo. For features based scene layers, such as 3D objects or point scene
/// layers, may include the default symbology, as specified in the drawingInfo, which contains
/// stylization information for a feature layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct SceneLayerInfoPsl {
    /// Unique numeric ID of the layer.
    pub id: i64,
    /// The relative URL to the 3DSceneLayerResource. Only present as part of the SceneServiceInfo resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// The user-visible layer type.Must be:`Point`
    pub layer_type: SceneLayerType,
    /// The spatialReference of the layer including the vertical coordinate reference system (CRS). Well Known Text (WKT) for CRS is included to support custom CRS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_reference: Option<SpatialReference>,
    /// Enables consuming clients to quickly determine whether this layer is compatible (with respect to its horizontal and vertical coordinate system) with existing content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_model_info: Option<HeightModelInfo>,
    /// The ID of the last update session in which any resource belonging to this layer has been updated.
    pub version: String,
    /// The name of this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The time of the last update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_update_time_stamp: Option<ServiceUpdateTimeStamp>,
    /// The display alias to be used for this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Description string for this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Copyright and usage information for the data in this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright_text: Option<String>,
    /// Capabilities supported by this layer.Possible values for each array string:`View`: View is supported.`Query`: Query is supported.`Edit`: Edit is defined.`Extract`: Extract is defined.
    pub capabilities: SceneLayerCapabilities,
    /// ZFactor to define conversion factor for elevation unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_factor: Option<f64>,
    /// Indicates if any styling information represented as drawingInfo is captured as part of the binary mesh representation.  This helps provide optimal client-side access. Currently the color component of ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_drawing_info: Option<CachedDrawingInfo>,
    /// An object containing drawing information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing_info: Option<DrawingInfo>,
    /// An object containing elevation drawing information. If absent, any content of the scene layer is drawn at its z coordinate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation_info: Option<ElevationInfo>,
    /// PopupInfo of the scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popup_info: Option<PopupInfo>,
    /// Indicates if client application will show the popup information. Default is FALSE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_popup: Option<bool>,
    /// The store object describes the exact physical storage of a layer and enables the client to detect when multiple layers are served from the same store.
    pub store: StorePsl,
    /// A collection of objects that describe each attribute field regarding its field name, datatype, and a user friendly name {name,type,alias}. It includes all fields that are included as part of the scene...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<Field>>,
    /// Provides the schema and layout used for storing attribute content in binary format in I3S.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_storage_info: Option<Vec<AttributeStorageInfo>>,
    /// Contains the statistical information for a layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics_info: Option<Vec<StatisticsInfo>>,
    /// The paged-access index description. For legacy purposes, this property is called pointNodePages in [Point Scene Layers](3DSceneLayer.psl.md).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_node_pages: Option<NodePageDefinition>,
    /// Define the layouts of point geometry and its attributes.
    pub geometry_definition: GeometryDefinitionPsl,
    /// 3D extent. If ```layer.fullExtent.spatialReference``` is specified, it must match ```layer.spatialReference```.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_extent: Option<FullExtent>,
    /// Time info represents the temporal data of a time-aware layer. The time info class provides information such as date fields that store the start and end times for each feature and the total time span f...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_info: Option<TimeInfo>,
    /// Range info is used to filter features of a layer withing a min and max range. The min and max range is created from the statistical information of the range field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_info: Option<RangeInfo>,
}

/// Resource objects are pointers to different types of resources related to a node, such as the
/// feature data, the geometry attributes and indices, textures and shared resources.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Resource {
    /// The relative URL to the referenced resource.
    pub href: String,
    /// **Deprecated.** The list of layer names that indicates which layer features in the bundle belongs to. The client can use this information to selectively download bundles.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_content: Option<Vec<String>>,
    /// **Deprecated.** Only applicable for featureData resources. Provides inclusive indices of the features list in this node that indicate which features of the node are located in this bundle.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_range: Option<Vec<f64>>,
    /// **Deprecated.** Only applicable for textureData resources. TRUE if the bundle contains multiple textures. If FALSE or not set, clients can interpret the entire bundle as a single image.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_texture_bundle: Option<String>,
    /// **Deprecated.** Only applicable for geometryData resources. Represents the count of elements in vertexAttributes; multiply by the sum of bytes required for each element as defined in the defaultGeomet...
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_elements: Option<Vec<f64>>,
    /// **Deprecated.** Only applicable for geometryData resources. Represents the count of elements in faceAttributes; multiply by the sum of bytes required for each element as defined in the defaultGeometry...
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_elements: Option<Vec<f64>>,
}

/// The resources folder is a location for additional symbology.  In styles subfolder, symbols may
/// be user defined.  In this folder, root.json.gz must be defined.  Root carries information such
/// as a name(which is unique), itemtype, and more.  <b>Example of root.json</b> ``` { "items": [ {
/// "name": "5fe9e487e2230d61de71aff13744c5e9", "title": "", "itemType": "pointSymbol",
/// "dimensionality": "volumetric", "formats": [ "web3d", "cim" ], "cimRef":
/// "./cim/5fe9e487e2230d61de71aff13744c5e9.json.gz", "webRef":
/// "./web/5fe9e487e2230d61de71aff13744c5e9.json.gz", "formatInfos": [ { "type": "gltf", "href":
/// "./gltf/5fe9e487e2230d61de71aff13744c5e9.json.gz" } ], "thumbnail": { "href":
/// "./thumbnails/5fe9e487e2230d61de71aff13744c5e9.png" } } ], "cimVersion": "2.0.0" } ```   If a
/// symbol is defined, it is placed in a folder based on the type(gltf,jpeg,png) and given a
/// symbolLayer json.  The symbolLayer json is named based on the unique symbol name, and the
/// resource property in the symbolLayer json is an href to an image or glb file.  The supported
/// symbol resource types are JPEG, PNG, glb.gz.  The glb file type is a binary representation of
/// 3D models saved in the gltf, then compressed with gzip.  <b>Example of the resource symbolLayer
/// json</b> ``` { "name": "5fe9e487e2230d61de71aff13744c5e9", "type": "PointSymbol3D",
/// "symbolLayers": [ { "type": "Object", "anchorPosition": [ 0, 0, -0.5 ], "width":
/// 26.685164171278601, "height": 20, "depth": 64.389789603982777, "heading": -90, "anchor":
/// "relative", "resource": { "href": "./resource/5fe9e487e2230d61de71aff13744c5e9.glb.gz" } } ] }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Resources {}

/// The store object describes the exact physical storage of a layer and enables the client to
/// detect when multiple layers are served from the same store. Storing multiple layers in a single
/// store - and thus having them share resources - enables efficient serving of many layers of the
/// same content type, but with different attribute schemas or different symbology applied.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Store {
    /// A store ID, unique across a SceneServer. Enables the client to discover which layers are part of a common store, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Indicates which profile this scene store fulfills.{point, meshpyramid, pointcloud}
    pub profile: String,
    /// Indicates the resources needed for rendering and the required order in which the client should load them. Possible values for each array string:`3dNodeIndexDocument`: JSON file describes a single inde...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pattern: Option<StoreResourcePattern>,
    /// Relative URL to root node resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_node: Option<String>,
    /// Format version of this resource. Used here again if this store hasn't been served by a 3D Scene Server.
    pub version: String,
    /// The 2D spatial extent (xmin, ymin, xmax, ymax) of this store, in the horizontal indexCRS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<[f64; 4]>,
    /// The horizontal CRS used for all minimum bounding spheres (mbs) in this store. The CRS is identified by an OGC URL. Needs to be identical to the spatial reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_crs: Option<String>,
    /// The horizontal CRS used for all 'vertex positions' in this store. The CRS is identified by an OGC URL. Needs to be identical to the spatial reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_crs: Option<String>,
    /// Describes the coordinate reference frame used for storing normals. Although not required, it is recommended to re-compute the normal component of the binary geometry buffer if this property is not pre...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_reference_frame: Option<StoreNormalReferenceFrame>,
    /// Deprecated in 1.7. MIME type for the encoding used for the Node Index Documents. Example: application/vnd.esri.I3S.json+gzip; version=1.6.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nid_encoding: Option<String>,
    /// Deprecated in 1.7. MIME type for the encoding used for the Feature Data Resources. For example: application/vnd.esri.I3S.json+gzip; version=1.6.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_encoding: Option<String>,
    /// Deprecated in 1.7. MIME type for the encoding used for the Geometry Resources. For example: application/octet-stream; version=1.6.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_encoding: Option<String>,
    /// Deprecated in 1.7. MIME type for the encoding used for the Attribute Resources. For example: application/octet-stream; version=1.6.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_encoding: Option<String>,
    /// Deprecated in 1.7. MIME type(s) for the encoding used for the Texture Resources.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_encoding: Option<Vec<String>>,
    /// Deprecated in 1.7. Optional field to indicate which LoD generation scheme is used in this store.Possible values are:`MeshPyramid`: Used for integrated mesh and 3D scene layer.`AutoThinning`: Use for p...
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_type: Option<StoreLodType>,
    /// Deprecated in 1.7. Optional field to indicate the [LoD switching](lodSelection.cmn.md) mode.Possible values are:`node-switching`: A parent node is substituted for its children nodes when its lod thres...
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_model: Option<StoreLodModel>,
    /// Deprecated in 1.7. Information on the Indexing Scheme (QuadTree, R-Tree, Octree, ...) used.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexing_scheme: Option<String>,
    /// A common, global ArrayBufferView definition that can be used if the schema of vertex attributes and face attributes is consistent in an entire cache; this is a requirement for meshpyramids caches.
    pub default_geometry_schema: DefaultGeometrySchema,
    /// Deprecated in 1.7. A common, global TextureDefinition to be used for all textures in this store. The default texture definition uses a reduced profile of the full TextureDefinition, with the following...
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_texture_definition: Option<Vec<Texture>>,
    /// Deprecated in 1.7. If a store uses only one material, it can be defined here entirely as a MaterialDefinition.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_material_definition: Option<MaterialDefinition>,
}

/// The store object describes the exact physical storage of a layer and enables the client to
/// detect when multiple layers are served from the same store. Storing multiple layers in a single
/// store - and thus having them share resources - enables efficient serving of many layers of the
/// same content type, but with different attribute schemas or different symbology applied.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct StorePsl {
    /// A store ID, unique across a SceneServer. Enables the client to discover which layers are part of a common store, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Indicates which profile this scene store fulfills.{point, meshpyramid, pointcloud}
    pub profile: String,
    /// Indicates the resources needed for rendering and the required order in which the client should load them. Possible values for each array string:`3dNodeIndexDocument`: JSON file describes a single inde...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_pattern: Option<StorePslResourcePattern>,
    /// Relative URL to root node resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_node: Option<String>,
    /// Format version of this resource. Used here again if this store hasn't been served by a 3D Scene Server.
    pub version: String,
    /// The 2D spatial extent (xmin, ymin, xmax, ymax) of this store, in the horizontal indexCRS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extent: Option<[f64; 4]>,
    /// The horizontal CRS used for all minimum bounding spheres (mbs) in this store. The CRS is identified by an OGC URL. Needs to be identical to the spatial reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_crs: Option<String>,
    /// The horizontal CRS used for all 'vertex positions' in this store. The CRS is identified by an OGC URL. Needs to be identical to the spatial reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_crs: Option<String>,
    /// Describes the coordinate reference frame used for storing normals. Although not required, it is recommended to re-compute the normal component of the binary geometry buffer if this property is not pre...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_reference_frame: Option<StorePslNormalReferenceFrame>,
    /// MIME type for the encoding used for the Node Index Documents. Example: application/vnd.esri.I3S.json+gzip; version=1.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nid_encoding: Option<String>,
    /// MIME type for the encoding used for the Feature Data Resources. For example: application/vnd.esri.I3S.json+gzip; version=1.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_encoding: Option<String>,
    /// MIME type for the encoding used for the Geometry Resources. For example: application/octet-stream; version=1.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_encoding: Option<String>,
    /// MIME type for the encoding used for the Attribute Resources. For example: application/octet-stream; version=1.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_encoding: Option<String>,
    /// MIME type(s) for the encoding used for the Texture Resources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_encoding: Option<Vec<String>>,
    /// Optional field to indicate which LoD generation scheme is used in this store.Possible values are:`MeshPyramid`: Used for integrated mesh and 3D scene layer.`AutoThinning`: Used for point scene layer.`...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_type: Option<StorePslLodType>,
    /// Optional field to indicate the [LoD switching](lodSelection.cmn.md) mode.Possible values are:`node-switching`: A parent node is substituted for its children nodes when its lod threshold is exceeded. T...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_model: Option<StorePslLodModel>,
    /// Information on the Indexing Scheme (QuadTree, R-Tree, Octree, ...) used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexing_scheme: Option<String>,
    /// A common, global ArrayBufferView definition that can be used if the schema of vertex attributes and face attributes is consistent in an entire cache; this is a requirement for meshpyramids caches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_geometry_schema: Option<DefaultGeometrySchema>,
    /// A common, global TextureDefinition to be used for all textures in this store. The default texture definition uses a reduced profile of the full TextureDefinition, with the following attributes being m...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_texture_definition: Option<Vec<Texture>>,
    /// If a store uses only one material, it can be defined here entirely as a MaterialDefinition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_material_definition: Option<MaterialDefinition>,
}

/// An array of four doubles, corresponding to x, y, z and radius of the minimum bounding sphere of
/// a node.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Mbs {
    /// The center point of the minimum bounding sphere. An array of four doubles, corresponding to x, y, z and radius of the minimum bounding sphere of a node. For a global scene, i.e. XY coordinate system i...
    pub mbs: [f64; 4],
}
