//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: psl

use serde::{Deserialize, Serialize};

use crate::cmn::AttributeStorageInfo;
use crate::cmn::CachedDrawingInfo;
use crate::cmn::CompressedAttributes;
use crate::cmn::DefaultGeometrySchema;
use crate::cmn::DrawingInfo;
use crate::cmn::ElevationInfo;
use crate::cmn::Field;
use crate::cmn::FullExtent;
use crate::cmn::HeightModelInfo;
use crate::cmn::MaterialDefinition;
use crate::cmn::NodePageDefinition;
use crate::cmn::PopupInfo;
use crate::cmn::RangeInfo;
use crate::cmn::SceneLayerCapabilities;
use crate::cmn::SceneLayerType;
use crate::cmn::ServiceUpdateTimeStamp;
use crate::cmn::SpatialReference;
use crate::cmn::StatisticsInfo;
use crate::cmn::StoreLodModel;
use crate::cmn::StoreLodType;
use crate::cmn::StoreNormalReferenceFrame;
use crate::cmn::StoreResourcePattern;
use crate::cmn::Texture;
use crate::cmn::TimeInfo;

/// Possible values for `GeometryDefinitionPsl::topology`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryDefinitionPslTopology {
    #[serde(rename = "point")]
    Point,
    #[serde(other)]
    Unknown,
}

impl Default for GeometryDefinitionPslTopology {
    fn default() -> Self {
        Self::Point
    }
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
    pub capabilities: Vec<SceneLayerCapabilities>,
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

/// Point Geometry Description  ``` compressedAttributes ```  **Important:** - Attribute that are
/// present are stored continuously in the corresponding geometry buffers. - Point Geometry are
/// always compressed
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryBufferPsl {
    /// Compressed attributes. **Cannot** be combined with any other attributes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressed_attributes: Option<CompressedAttributes>,
}

/// The geometry definitions used in [Point Scene Layer]() I3S version 1.7.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryDefinitionPsl {
    /// Defines the topology type of the point.Must be:`point`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<GeometryDefinitionPslTopology>,
    /// Array of geometry representation(s) for this class of points.  Must be compressed.
    pub geometry_buffers: Vec<GeometryBufferPsl>,
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
pub struct Resources {
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
    pub resource_pattern: Option<Vec<StoreResourcePattern>>,
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
    pub lod_type: Option<StoreLodType>,
    /// Optional field to indicate the [LoD switching](lodSelection.cmn.md) mode.Possible values are:`node-switching`: A parent node is substituted for its children nodes when its lod threshold is exceeded. T...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_model: Option<StoreLodModel>,
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
