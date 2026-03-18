//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: pcsl

use serde::{Deserialize, Serialize};

use crate::cmn::Field;
use crate::cmn::HeightModelInfo;
use crate::cmn::Obb;
use crate::cmn::SceneLayerCapabilities;
use crate::cmn::SceneLayerType;
use crate::cmn::ServiceUpdateTimeStamp;
use crate::cmn::SpatialReference;

/// Possible values for `PointCloudAttributeInfo::ordering`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudAttributeInfoOrdering {
    #[serde(rename = "attributeValues")]
    AttributeValues,
}

impl Default for PointCloudAttributeInfoOrdering {
    fn default() -> Self {
        Self::AttributeValues
    }
}

/// Possible values for `PointCloudAttributeInfo::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudAttributeInfoEncoding {
    #[serde(rename = "embedded-elevation")]
    EmbeddedElevation,
    #[serde(rename = "lepcc-intensity")]
    LepccIntensity,
    #[serde(rename = "lepcc-rgb")]
    LepccRgb,
}

impl Default for PointCloudAttributeInfoEncoding {
    fn default() -> Self {
        Self::EmbeddedElevation
    }
}

/// Possible values for `PointCloudDefaultGeometrySchema::geometryType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudDefaultGeometrySchemaGeometryType {
    #[serde(rename = "points")]
    Points,
}

impl Default for PointCloudDefaultGeometrySchemaGeometryType {
    fn default() -> Self {
        Self::Points
    }
}

/// Possible values for `PointCloudDefaultGeometrySchema::topology`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudDefaultGeometrySchemaTopology {
    PerAttributeArray,
}

impl Default for PointCloudDefaultGeometrySchemaTopology {
    fn default() -> Self {
        Self::PerAttributeArray
    }
}

/// Possible values for `PointCloudDefaultGeometrySchema::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudDefaultGeometrySchemaEncoding {
    #[serde(rename = "lepcc-xyz")]
    LepccXyz,
}

impl Default for PointCloudDefaultGeometrySchemaEncoding {
    fn default() -> Self {
        Self::LepccXyz
    }
}

/// Possible values for `PointCloudDefaultGeometrySchema::ordering`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudDefaultGeometrySchemaOrdering {
    #[serde(rename = "position")]
    Position,
}

impl Default for PointCloudDefaultGeometrySchemaOrdering {
    fn default() -> Self {
        Self::Position
    }
}

/// Possible values for `PointCloudIndex::boundingVolumeType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudIndexBoundingVolumeType {
    #[serde(rename = "obb")]
    Obb,
}

impl Default for PointCloudIndexBoundingVolumeType {
    fn default() -> Self {
        Self::Obb
    }
}

/// Possible values for `PointCloudIndex::lodSelectionMetricType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudIndexLodSelectionMetricType {
    #[serde(rename = "density-threshold")]
    DensityThreshold,
}

impl Default for PointCloudIndexLodSelectionMetricType {
    fn default() -> Self {
        Self::DensityThreshold
    }
}

/// Possible values for `PointCloudStore::profile`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudStoreProfile {
    PointCloud,
}

impl Default for PointCloudStoreProfile {
    fn default() -> Self {
        Self::PointCloud
    }
}

/// Possible values for `PointCloudValue::valueType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointCloudValueType {
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Float32,
    Float64,
}

impl Default for PointCloudValueType {
    fn default() -> Self {
        Self::Int8
    }
}

/// List of attributes included for this layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudAttributeInfo {
    /// Represents the attribute key. Key is the same as `id' used in the resource URL to fetch the binary buffers.
    pub key: String,
    /// The attribute name. Must be unique for this layer.
    pub name: String,
    /// Mapping between attribute to point. Only 1-to-1 is currently supported.Possible values for each array string:`attributeValues`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<PointCloudAttributeInfoOrdering>,
    /// Encoding (i.e. compression) for the attribute binary buffer if different from GZIP or no-compression.Possible values are:`embedded-elevation`: No binary buffer but stats for this pseudo attribute will...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<PointCloudAttributeInfoEncoding>,
    /// Represents the description for value encoding, for example scalar or vector encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_values: Option<PointCloudValue>,
}

/// Example for LiDAR data:  | Bit Number | Label | description | |:--:|:--:|:--| | 0| Synthetic |
/// If set then this point was created by a technique other than LIDAR collection such as digitized
/// from a photogrammetric stereo model or by traversing a waveform| | 1| Key-Point |If set, this
/// point is considered to be a model key-point and thus generally should not be withheld in a
/// thinning algorithm.| |2| Withheld |If set, this point should not be included in processing
/// (synonymous with Deleted).| |3| Overlap | If set, this point is within the overlap region of
/// two or more swaths or takes. Setting this bit is not mandatory (unless, of course, it is
/// mandated by a particular delivery specification) but allows Classification of overlap points to
/// be preserved.| |4 | Scan Channel 0 | Scanner Channel is used to indicate the channel (scanner
/// head) of a multichannel system. Channel 0 is used for single scanner systems | |5 | Scan
/// Channel 1 |  Scanner Channel is used to    indicate the channel (scanner head) of a
/// multichannel system.| |6| Scan Direction |The Scan Direction Flag denotes the direction at
/// which the scanner mirror was traveling at the time of the output pulse. A bit value of 1 is a
/// positive scan direction, and a bit value of 0 is a negative scan direction (where positive scan
/// direction is a scan moving from the left side of the in-track direction to the right side and
/// negative the opposite). | |7| Edge of flight line | The Edge of Flight Line data bit has a
/// value of 1 only when the point is at the end of a scan. It is the last point on a given scan
/// line before it changes direction or the mirror facet changes. Note that this field has no
/// meaning for 360&deg; Field of View scanners (such as Mobile LIDAR scanners) and should not be
/// set |
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudBitFieldLabel {
    /// Bit number (0 is LSB)
    pub bit_number: i64,
    /// Label string
    pub label: String,
}

/// Attribute description as field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudDefaultGeometrySchema {
    /// The type of primitive. Only points are supported for point cloud scene layer.Must be:`points`
    pub geometry_type: PointCloudDefaultGeometrySchemaGeometryType,
    /// The header in binary buffers. Currently not supported for point cloud scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// This property is currently **ignored* for point cloud scene layer since it only contains geometry position without vertex attributes.Must be:`PerAttributeArray`
    pub topology: PointCloudDefaultGeometrySchemaTopology,
    /// Only 'lepcc-xyz' compression is currently supported.Must be:`lepcc-xyz`
    pub encoding: PointCloudDefaultGeometrySchemaEncoding,
    /// Currently the geometry contains XYZ only, so vertex attribute must only list 'position'.Possible values for each array string:`position`: vertex coordinates
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<PointCloudDefaultGeometrySchemaOrdering>,
    /// The vertex buffer description.
    pub vertex_attributes: PointCloudVertexAttributes,
}

/// The drawingInfo object contains drawing information for a point cloud scene layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudDrawingInfo {
    /// An object defining the symbology for the layer. [See more](https://developers.arcgis.com/web-scene-specification/objects/pointCloudRenderers/) information about supported renderer types in ArcGIS clie...
    pub renderer: String,
}

/// The elevationInfo defines how content in a scene layer is aligned to the ground.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudElevationInfo {
    /// The mode of the elevation. Point cloud scene layer supports absoluteHeight.
    pub mode: String,
    /// The offset the point cloud scene layer. The elevation unit is the coordinate systems units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,
}

/// The histogram of the point cloud scene layer. The bin size may be computed as (max-min)/bin
/// count. Please note that stats.histo.min/max is not equivalent to stats.min/max since values
/// smaller than stats.histo.min and greater than stats.histo.max are counted in the first and last
/// bin respectively. The values stats.min and stats.max may be conservative estimates. The bins
/// would be distributed as follows:  ```(-inf, stats.min + bin_size], (stats.min + bin_size,
/// stats.min + 2 * bin_size], ... , (stats.min + (bin_count - 1) * bin_size], (stats.min +
/// (bin_count - 1) * bin_size, +inf)```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudHistogram {
    /// Minimum value (i.e. left bound) of the first bin of the histogram.
    pub minimum: f64,
    /// Maximum value (i.e. right bound) of the last bin of the histogram.
    pub maximum: f64,
    /// Array of binned value counts with up to ```n``` values, where ```n``` is the number of bins and **must be less or equal to 256**.
    pub counts: String,
}

/// Describes the index (i.e. bounding volume tree) of the layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudIndex {
    /// The version of the individual nodes format.
    pub node_version: i64,
    /// The page size describes the number of nodes per paged index document. 64 is currently expected.
    pub nodes_per_page: i64,
    /// The bounding volume type. Only OBB is currently supported.Must be:`obb`: Oriented bounding box
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounding_volume_type: Option<PointCloudIndexBoundingVolumeType>,
    /// Defines how `node.lodThreshold` should be interpretedMust be:`density-threshold`: nodes[i].lodThreshold will represent an 'effective' 2D area for the node. This estimation works best when the point cl...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_selection_metric_type: Option<PointCloudIndexLodSelectionMetricType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

/// Label object for the statistics labels in the point cloud profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudLabel {
    /// Value
    pub value: f64,
    /// Label string
    pub label: String,
}

/// Optionally, the statistics document may contain labeling information for the attribute values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudLabels {
    /// Array of string label/value pairs. Used when attribute represents a set of values. For example, ClassCode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<PointCloudLabel>>,
    /// Array of string label/bitNumber pairs. This is useful when the attribute represent a bitfield. For example, FLAGS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitfield_labels: Option<Vec<PointCloudBitFieldLabel>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudLayer {
    /// A unique identifying number for the layer. For point cloud scene layer, only a single layer is supported, therefore, id is always 0.
    pub id: i64,
    /// String indicating the layer typeMust be:`PointCloud`
    pub layer_type: SceneLayerType,
    /// Represents the layer name.
    pub name: String,
    /// Represents the alias layer name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Description for the layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    /// Copyright information to be displayed with this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright_text: Option<String>,
    /// Capabilities supported by this layer.Possible values for each array string:`View`: View is supported.`Query`: Query is supported.`Extract`: Extract is defined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<SceneLayerCapabilities>,
    /// An object containing the WKID or WKT identifying the spatial reference of the layer's geometry.
    pub spatial_reference: SpatialReference,
    /// An object containing the vertical coordinate system information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_model_info: Option<HeightModelInfo>,
    /// Object to provide time stamp when the I3S service or the source of the service was created or updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service_update_time_stamp: Option<ServiceUpdateTimeStamp>,
    /// The storage for the layer.
    pub store: PointCloudStore,
    /// List of attributes included for this layer.
    pub attribute_storage_info: Vec<PointCloudAttributeInfo>,
    /// An object containing drawing information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drawing_info: Option<PointCloudDrawingInfo>,
    /// An object containing elevation information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation_info: Option<PointCloudElevationInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<Field>>,
}

/// A single bounding volume hierarchy node
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudNode {
    /// Index of the first child of this node. The resourceID must be used to query node resources, like geometry buffer (XYZ)  /nodes//geometry/0  and attribute buffers. One buffer can have one attribute. Av...
    pub resource_id: i64,
    /// Index of the first child of this node.
    pub first_child: i64,
    /// Number of children for this node. Value is 0 if node is a leaf node.
    pub child_count: i64,
    /// Number of points for this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_count: Option<i64>,
    /// Oriented bounding boxes (OBB) are the only supported bounding volumes.
    pub obb: Obb,
    /// This metric may be used as a threshold to split a parent node into its children. See [layer.store.index.lodSelectionMetricType](index.pcsl.md)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_threshold: Option<f64>,
}

/// Nodes represent the spatial index of the data as a bounding volume hierarchy. To reduce the
/// number of node-index requests required to traverse this index tree, they are organized in
/// *pages* of [layer.index.nodesPerPage](index.pcsl.md) nodes.  Children must be **contiguous**,
/// in index range, so they may be located using  `firstChild`  and  `childrenCount` fields.
/// **Page Number Computation Example:**  `page_id = floor( node_id /
/// layer.store.index.nodesPerPage )`  Let's say  `node id` = 78 and
/// `layer.store.index.nodesPerPage` = 64.  ``` page_id = floor (78 / 64) = floor (1.22) = 1 ```
/// The `page_id` of this node is `1`.  This is the second page since indexing starts at 0.
/// **IMPORTANT:** Page size must be a power-of-two less than `4096`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudNodePageDefinition {
    /// Array of nodes
    pub nodes: Vec<PointCloudNode>,
}

/// Contains statistics about each attribute. Statistics are useful to estimate attribute
/// distribution and range. By convention,  statistics are stored by attribute at
/// `layers/0/statistics/{attribute_id}`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudStatistics {
    /// Attribute name. Must match the name specified for this attribute in `layer.attributeStorageInfo`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<String>,
    /// Statistics for this attribute
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<PointCloudStats>,
    /// The statistics document may contain labeling information for the attribute values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<PointCloudLabels>,
}

/// Contains statistics about each attribute. Statistics are useful to estimate attribute
/// distribution and range.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudStats {
    /// (Conservative) minimum attribute value for the entire layer.
    pub min: f64,
    /// (Conservative) maximum attribute value for the entire layer.
    pub max: f64,
    /// Count for the entire layer.
    pub count: f64,
    /// Sum of the attribute values over the entire layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sum: Option<f64>,
    /// Representing average or mean value. For example, sum/count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg: Option<f64>,
    /// Representing the standard deviation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stddev: Option<f64>,
    /// Representing variance. For example, stats.stddev *stats.stddev.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variance: Option<f64>,
    /// Represents the histogram.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub histogram: Option<PointCloudHistogram>,
    /// An array of most frequently used values within the point cloud scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub most_frequent_values: Option<Vec<PointCloudValueCount>>,
}

/// Describes storage for the layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudStore {
    /// Id for the store. Not currently used by the point cloud scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Defines the profile type of the scene layer as point cloud scene layer.Must be:`PointCloud`
    pub profile: PointCloudStoreProfile,
    /// Point cloud scene layer store version.
    pub version: String,
    /// 2D extent of the point cloud scene layer in the layers spatial reference units.
    pub extent: [f64; 4],
    /// Describes the index (i.e. bounding volume tree) of the layer.
    pub index: PointCloudIndex,
    /// Attribute description as field.
    pub default_geometry_schema: PointCloudDefaultGeometrySchema,
    /// MIME type for the encoding used for the Geometry Resources. For example: application/octet-stream; version=1.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_encoding: Option<String>,
    /// MIME type for the encoding used for the Attribute Resources. For example: application/octet-stream; version=1.6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_encoding: Option<String>,
}

/// A scalar or vector value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudValue {
    /// Type of the attribute values after decompression, if applicable. Please note that `string` is not supported for point cloud scene layer attributes.Possible values are:`Int8``UInt8``Int16``UInt16``Int3...
    pub value_type: PointCloudValueType,
    /// Number of components.
    pub values_per_element: f64,
}

/// A scalar or vector value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudValueCount {
    /// Type of the attribute values after decompression, if applicable. Please note that `string` is not supported for point cloud scene layer attributes.
    pub value: f64,
    /// Count the number of values. May exceed 32 bit.
    pub count: f64,
}

/// The vertex buffer description.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudVertexAttributes {
    /// Only LEPCC compressed (X,Y,Z) is supported. Decompressed data will be absolute `Float64` position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<PointCloudValue>,
}

/// Scanning an SLPK (ZIP store) containing millions of documents is usually inefficient and slow.
/// A hash table file may be added to the SLPK to improve first load and file scanning
/// performances.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PointCloudSlpkHashtable {}
