//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: feature

use serde::{Deserialize, Serialize};

use crate::geometry::Geometry;

/// Possible values for `Domain::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainType {
    #[serde(rename = "codedValue")]
    Codedvalue,
    #[serde(rename = "range")]
    Range,
}

impl Default for DomainType {
    fn default() -> Self {
        Self::Codedvalue
    }
}

/// Possible values for `Domain::fieldType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainFieldType {
    #[serde(rename = "esriFieldTypeDate")]
    Date,
    #[serde(rename = "esriFieldTypeSingle")]
    Single,
    #[serde(rename = "esriFieldTypeDouble")]
    Double,
    #[serde(rename = "esriFieldTypeInteger")]
    Integer,
    #[serde(rename = "esriFieldTypeSmallInteger")]
    SmallInteger,
    #[serde(rename = "esriFieldTypeString")]
    String,
}

impl Default for DomainFieldType {
    fn default() -> Self {
        Self::Date
    }
}

/// Possible values for `Domain::mergePolicy`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainMergePolicy {
    #[serde(rename = "esriMPTDefaultValue")]
    MPTDefaultValue,
    #[serde(rename = "esriMPTSumValues")]
    MPTSumValues,
    #[serde(rename = "esriMPTAreaWeighted")]
    MPTAreaWeighted,
}

impl Default for DomainMergePolicy {
    fn default() -> Self {
        Self::MPTDefaultValue
    }
}

/// Possible values for `Domain::splitPolicy`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainSplitPolicy {
    #[serde(rename = "esriSPTGeometryRatio")]
    SPTGeometryRatio,
    #[serde(rename = "esriSPTDuplicate")]
    SPTDuplicate,
    #[serde(rename = "esriSPTDefaultValue")]
    SPTDefaultValue,
}

impl Default for DomainSplitPolicy {
    fn default() -> Self {
        Self::SPTGeometryRatio
    }
}

/// Possible values for `Field::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldType {
    #[serde(rename = "esriFieldTypeDate")]
    Date,
    #[serde(rename = "esriFieldTypeSingle")]
    Single,
    #[serde(rename = "esriFieldTypeDouble")]
    Double,
    #[serde(rename = "esriFieldTypeGUID")]
    GUID,
    #[serde(rename = "esriFieldTypeGlobalID")]
    GlobalID,
    #[serde(rename = "esriFieldTypeInteger")]
    Integer,
    #[serde(rename = "esriFieldTypeOID")]
    OID,
    #[serde(rename = "esriFieldTypeSmallInteger")]
    SmallInteger,
    #[serde(rename = "esriFieldTypeString")]
    String,
}

impl Default for FieldType {
    fn default() -> Self {
        Self::Date
    }
}

/// Possible values for `AttributeStorageInfo::ordering`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttributeStorageInfoOrdering {
    #[serde(rename = "attributeByteCounts")]
    Attributebytecounts,
    #[serde(rename = "attributeValues")]
    Attributevalues,
    #[serde(rename = "ObjectIds")]
    Objectids,
}

impl Default for AttributeStorageInfoOrdering {
    fn default() -> Self {
        Self::Attributebytecounts
    }
}

/// Possible values for `HeaderAttribute::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeaderAttributeType {
    #[serde(rename = "UInt8")]
    Uint8,
    #[serde(rename = "UInt16")]
    Uint16,
    #[serde(rename = "UInt32")]
    Uint32,
    #[serde(rename = "UInt64")]
    Uint64,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
}

impl Default for HeaderAttributeType {
    fn default() -> Self {
        Self::Uint8
    }
}

/// Possible values for `HeaderValue::valueType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeaderValueType {
    Int8,
    #[serde(rename = "UInt8")]
    Uint8,
    Int16,
    #[serde(rename = "UInt16")]
    Uint16,
    Int32,
    #[serde(rename = "UInt32")]
    Uint32,
    Float32,
    Float64,
    String,
}

impl Default for HeaderValueType {
    fn default() -> Self {
        Self::Int8
    }
}

/// Possible values for `HeaderValue::property`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeaderValueProperty {
    #[serde(rename = "count")]
    Count,
    #[serde(rename = "attributeValuesByteCount")]
    Attributevaluesbytecount,
}

impl Default for HeaderValueProperty {
    fn default() -> Self {
        Self::Count
    }
}

/// Possible values for `Value::timeEncoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueTimeEncoding {
    #[serde(rename = "ECMA_ISO8601")]
    EcmaIso8601,
}

impl Default for ValueTimeEncoding {
    fn default() -> Self {
        Self::EcmaIso8601
    }
}

/// Attribute domains are rules that describe the legal values of a field type, providing a method
/// for enforcing data integrity. Attribute domains are used to constrain the values allowed in a
/// particular attribute. Using domains helps ensure data integrity by limiting the choice of
/// values for a particular field. Attribute domains can be shared across scene layers like 3D
/// Object scene layers or Building Scene Layers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Domain {
    /// Type of domainPossible values are:`codedValue``range`
    pub r#type: DomainType,
    /// Name of the domain. Must be unique per Scene Layer.
    pub name: String,
    /// Description of the domain
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The field type is the type of attribute field with which the domain can be associated.Possible values are:`esriFieldTypeDate``esriFieldTypeSingle``esriFieldTypeDouble``esriFieldTypeInteger``esriFieldT...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_type: Option<DomainFieldType>,
    /// Range of the domain. Only numeric types are possible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[f64; 2]>,
    /// Range of the domain. Only string types are possible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coded_values: Option<Vec<DomainCodedValue>>,
    /// Merge policy for the domain. Not used by Scene Layers.Possible values are:`esriMPTDefaultValue``esriMPTSumValues``esriMPTAreaWeighted`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merge_policy: Option<DomainMergePolicy>,
    /// Split policy for the domain. Not used by Scene Layers. Possible values are:`esriSPTGeometryRatio``esriSPTDuplicate``esriSPTDefaultValue`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub split_policy: Option<DomainSplitPolicy>,
}

/// Attribute domains are rules that describe the legal values of a field type, providing a method
/// for enforcing data integrity. Attribute domains are used to constrain the values allowed in any
/// particular attribute. Whenever a domain is associated with an attribute field, only the values
/// within that domain are valid for the field. Using domains helps ensure data integrity by
/// limiting the choice of values for a particular field. The domain code value contains the coded
/// values for a domain as well as an associated description of what that value represents.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct DomainCodedValue {
    /// Text representation of the domain value.
    pub name: String,
    /// Coded value (i.e. field value).
    pub code: String,
}

/// A collection of objects describing each attribute field.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Field {
    /// Name of the field.
    pub name: String,
    /// Type of the field.Possible values are:`esriFieldTypeDate``esriFieldTypeSingle``esriFieldTypeDouble``esriFieldTypeGUID``esriFieldTypeGlobalID``esriFieldTypeInteger``esriFieldTypeOID``esriFieldTypeSmall...
    pub r#type: FieldType,
    /// Alias of the field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Array of domains defined for a field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<Domain>,
}

/// The attributeStorageInfo object describes the structure of the binary attribute data resource
/// of a layer, which is the same for every node in the layer. The following examples show how
/// different attribute types are represented as a binary buffer.  # Examples of attribute
/// resources
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct AttributeStorageInfo {
    /// The unique field identifier key.
    pub key: String,
    /// The name of the field.
    pub name: String,
    /// Declares the headers of the binary attribute data.
    pub header: Vec<HeaderValue>,
    /// Possible values for each array string:`attributeByteCounts`: Should only be present when working with string data types.`attributeValues`: Should always be present. `ObjectIds`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ordering: Option<AttributeStorageInfoOrdering>,
    /// Represents the description for value encoding. For example: scalar or vector encoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_values: Option<Value>,
    /// For string types only. Represents the byte count of the string, including the null character.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_byte_counts: Option<Value>,
    /// Stores the object-id values of each feature within the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_ids: Option<Value>,
}

/// Declaration of the attributes per feature in the geometry, such as feature ID or face range.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FeatureAttribute {
    /// ID of the feature attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    /// Describes the face range of the feature attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_range: Option<Value>,
}

/// The FeatureData JSON file(s) contain geographical features with a set of attributes, accessors
/// to geometry attributes, and other references to styling or materials. FeatureData is only used
/// by point scene layers. For other scene layer types, such as 3D object scene layer or integrated
/// mesh scene layer, clients read [defaultGeometrySchema](defaultGeometrySchema.cmn.md) to access
/// the geometry buffer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FeatureData {
    /// Feature ID, unique within the Node. If lodType is FeatureTree, the ID must be unique in the store.
    pub id: f64,
    /// An array of two or three doubles, giving the x,y(,z) (easting/northing/elevation) position of this feature's minimum bounding sphere center, in the vertexCRS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// An array of three doubles, providing an optional, 'semantic' pivot offset that can be used to e.g. correctly drape tree symbols.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot_offset: Option<[f64; 3]>,
    /// An array of six doubles, corresponding to xmin, ymin, zmin, xmax, ymax and zmax of the minimum bounding box of the feature, expressed in the vertexCRS, without offset. The mbb can be used with the Fea...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbb: Option<[f64; 6]>,
    /// The name of the Feature Class this feature belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// The list of GIS attributes the feature has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<FeatureAttribute>,
    /// The list of geometries the feature has. A feature always has at least one Geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometries: Option<Geometry>,
}

/// Declaration of the attributes per feature in the geometry, such as feature ID or face range.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Features {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_data: Option<Vec<FeatureData>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_data: Option<Vec<Geometry>>,
}

/// The header definition provides the name of each field and the value type. Headers to geometry
/// resources must be uniform across any cache and may only contain fixed-width, single element
/// fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct HeaderAttribute {
    /// The name of the property in the header.
    pub property: String,
    /// The element type of the header property.Possible values are:`UInt8``UInt16``UInt32``UInt64``Int16``Int32``Int64``Float32``Float64`
    pub r#type: HeaderAttributeType,
}

/// Value for attributeByteCount, attributeValues and objectIds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct HeaderValue {
    /// Defines the value type.Possible values are:`Int8``UInt8``Int16``UInt16``Int32``UInt32``Float32``Float64``String`
    pub value_type: HeaderValueType,
    /// Encoding method for the value.Possible values are:`count`: Should always be present and indicates the count of features in the attribute storage.`attributeValuesByteCount`
    pub property: HeaderValueProperty,
}

/// The bin size may be computed as (max-min)/bin count. Please note that stats.histo.min/max is
/// not equivalent to stats.min/max since values smaller than stats.histo.min and greater than
/// stats.histo.max are counted in the first and last bin respectively. The values stats.min and
/// stats.max may be conservative estimates. The bins would be distributed as follows:  ```(-inf,
/// stats.min + bin_size], (stats.min + bin_size, stats.min + 2 * bin_size], ... , (stats.min +
/// (bin_count - 1) * bin_size], (stats.min + (bin_count - 1) * bin_size, +inf)```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Histogram {
    /// Minimum value (i.e. left bound) of the first bin of the histogram.
    pub minimum: f64,
    /// Maximum value (i.e. right bound) of the last bin of the histogram.
    pub maximum: f64,
    /// Array of binned value counts with up to ```n``` values, where ```n``` is the number of bins and **must be less or equal to 256**.
    pub counts: Vec<f64>,
}

/// Range information allows to filter features of a layer within a minimum and maximum range.
/// Range is often used to visualize indoor spaces like picking a floor of a building or visualize
/// rooms belonging to a specific occupation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct RangeInfo {
    /// Field name to used for the range. The statistics of the field will contain the min and max values of all features for this rangeInfo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// A unique name that can be referenced by an application to represent the range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Describes the attribute statistics for the scene layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct StatisticsInfo {
    /// Key indicating the resource of the statistics. For example f_1 for  ./statistics/f_1
    pub key: String,
    /// Name of the field of the statistical information.
    pub name: String,
    /// The URL to the statistics information. For example ./statistics/f_1
    pub href: String,
}

/// Contains statistics about each attribute. Statistics are useful to estimate attribute
/// distribution and range.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Stats {
    /// Contains statistics about each attribute. Statistics are useful to estimate attribute distribution and range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<StatsInfo>,
}

/// Contains statistics about each attribute. Statistics are useful to estimate attribute
/// distribution and range. The content depends on the [field types](field.cmn.md).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct StatsInfo {
    /// Represents the count of the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_values_count: Option<f64>,
    /// Minimum attribute value for the entire layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Maximum attribute value for the entire layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Minimum time string represented according to [time encoding](value.cmn.md). Only used for esriFieldTypeDate i3s version 1.9 or newer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_time_str: Option<String>,
    /// Maximum time string represented according to [time encoding](value.cmn.md). Only used for esriFieldTypeDate i3s version 1.9 or newer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_time_str: Option<String>,
    /// Count for the entire layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<f64>,
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
    pub histogram: Option<Histogram>,
    /// An array of most frequently used values within the point cloud scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub most_frequent_values: Option<Vec<Valuecount>>,
}

/// Time info represents the temporal data of a time-aware layer. The time info provides
/// information such as date fields storing the start and end times for each feature. The statistic
/// of the time fields defines the time extent as a period of time with a definite start and end
/// time. The time encoding is [ECMA ISO8601](ECMA_ISO8601.md). The date time values can be UTC
/// time or local time with offset to UTC. Temporal data is data that represents a state in time.
/// You can to step through periods of time to reveal patterns and trends in your data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct TimeInfo {
    /// The name of the field containing the end time information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time_field: Option<String>,
    /// The name of the field that contains the start time information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_time_field: Option<String>,
}

/// Value for attributeByteCount, attributeValues and objectIds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Value {
    /// Defines the value type.
    pub value_type: String,
    /// Encoding method for the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// Encoding method for the time value. DateTime attribute string formatting must comply with [ECMA-ISO 8601](ECMA_ISO8601.md).Must be:`ECMA_ISO8601`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_encoding: Option<ValueTimeEncoding>,
    /// Number of values per element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values_per_element: Option<f64>,
}

/// A string or numeric value.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Valuecount {
    /// Type of the attribute values after decompression, if applicable. Please note that `string` is not supported for point cloud scene layer attributes.
    pub value: String,
    /// Count of the number of values. May exceed 32 bits.
    pub count: f64,
}
