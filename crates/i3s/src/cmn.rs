//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: cmn

use serde::{Deserialize, Serialize};

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
    IntegratedMesh,
    Point,
    PointCloud,
    Building,
}

impl Default for SceneLayerType {
    fn default() -> Self {
        Self::ThreeDObject
    }
}

/// Possible values for `Domain::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DomainType {
    #[serde(rename = "codedValue")]
    CodedValue,
    #[serde(rename = "range")]
    Range,
}

impl Default for DomainType {
    fn default() -> Self {
        Self::CodedValue
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

/// Possible values for `HeightModelInfo::heightModel`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightModelInfoHeightModel {
    #[serde(rename = "gravity_related_height")]
    GravityRelatedHeight,
    #[serde(rename = "ellipsoidal")]
    Ellipsoidal,
}

impl Default for HeightModelInfoHeightModel {
    fn default() -> Self {
        Self::GravityRelatedHeight
    }
}

/// Possible values for `HeightModelInfo::heightUnit`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightModelInfoHeightUnit {
    #[serde(rename = "meter")]
    Meter,
    #[serde(rename = "us-foot")]
    UsFoot,
    #[serde(rename = "foot")]
    Foot,
    #[serde(rename = "clarke-foot")]
    ClarkeFoot,
    #[serde(rename = "clarke-yard")]
    ClarkeYard,
    #[serde(rename = "clarke-link")]
    ClarkeLink,
    #[serde(rename = "sears-yard")]
    SearsYard,
    #[serde(rename = "sears-foot")]
    SearsFoot,
    #[serde(rename = "sears-chain")]
    SearsChain,
    #[serde(rename = "benoit-1895-b-chain")]
    Benoit1895BChain,
    #[serde(rename = "indian-yard")]
    IndianYard,
    #[serde(rename = "indian-1937-yard")]
    Indian1937Yard,
    #[serde(rename = "gold-coast-foot")]
    GoldCoastFoot,
    #[serde(rename = "sears-1922-truncated-chain")]
    Sears1922TruncatedChain,
    #[serde(rename = "us-inch")]
    UsInch,
    #[serde(rename = "us-mile")]
    UsMile,
    #[serde(rename = "us-yard")]
    UsYard,
    #[serde(rename = "millimeter")]
    Millimeter,
    #[serde(rename = "decimeter")]
    Decimeter,
    #[serde(rename = "centimeter")]
    Centimeter,
    #[serde(rename = "kilometer")]
    Kilometer,
}

impl Default for HeightModelInfoHeightUnit {
    fn default() -> Self {
        Self::Meter
    }
}

/// Possible values for `AttributeStorageInfo::ordering`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttributeStorageInfoOrdering {
    #[serde(rename = "attributeByteCounts")]
    AttributeByteCounts,
    #[serde(rename = "attributeValues")]
    AttributeValues,
    ObjectIds,
}

impl Default for AttributeStorageInfoOrdering {
    fn default() -> Self {
        Self::AttributeByteCounts
    }
}

/// Possible values for `CompressedAttributes::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompressedAttributesEncoding {
    #[serde(rename = "draco")]
    Draco,
}

impl Default for CompressedAttributesEncoding {
    fn default() -> Self {
        Self::Draco
    }
}

/// Possible values for `CompressedAttributes::attributes`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompressedAttributesAttributes {
    #[serde(rename = "position")]
    Position,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "uv0")]
    Uv0,
    #[serde(rename = "color")]
    Color,
    #[serde(rename = "uv-region")]
    UvRegion,
    #[serde(rename = "feature-index")]
    FeatureIndex,
}

impl Default for CompressedAttributesAttributes {
    fn default() -> Self {
        Self::Position
    }
}

/// Possible values for `DefaultGeometrySchema::geometryType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefaultGeometrySchemaGeometryType {
    #[serde(rename = "triangles")]
    Triangles,
}

impl Default for DefaultGeometrySchemaGeometryType {
    fn default() -> Self {
        Self::Triangles
    }
}

/// Possible values for `DefaultGeometrySchema::topology`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DefaultGeometrySchemaTopology {
    PerAttributeArray,
    Indexed,
}

impl Default for DefaultGeometrySchemaTopology {
    fn default() -> Self {
        Self::PerAttributeArray
    }
}

/// Possible values for `ElevationInfo::mode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElevationInfoMode {
    #[serde(rename = "relativeToGround")]
    RelativeToGround,
    #[serde(rename = "absoluteHeight")]
    AbsoluteHeight,
    #[serde(rename = "onTheGround")]
    OnTheGround,
    #[serde(rename = "relativeToScene")]
    RelativeToScene,
}

impl Default for ElevationInfoMode {
    fn default() -> Self {
        Self::RelativeToGround
    }
}

/// Possible values for `GeometryColor::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryColorType {
    UInt8,
}

impl Default for GeometryColorType {
    fn default() -> Self {
        Self::UInt8
    }
}

/// Possible values for `GeometryColor::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryColorEncoding {
    #[serde(rename = "normalized")]
    Normalized,
}

impl Default for GeometryColorEncoding {
    fn default() -> Self {
        Self::Normalized
    }
}

/// Possible values for `GeometryColor::binding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryColorBinding {
    #[serde(rename = "per-vertex")]
    PerVertex,
}

impl Default for GeometryColorBinding {
    fn default() -> Self {
        Self::PerVertex
    }
}

/// Possible values for `GeometryDefinition::topology`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryDefinitionTopology {
    #[serde(rename = "triangle")]
    Triangle,
}

impl Default for GeometryDefinitionTopology {
    fn default() -> Self {
        Self::Triangle
    }
}

/// Possible values for `GeometryFaceRange::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryFaceRangeType {
    UInt32,
}

impl Default for GeometryFaceRangeType {
    fn default() -> Self {
        Self::UInt32
    }
}

/// Possible values for `GeometryFaceRange::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryFaceRangeEncoding {
    #[serde(rename = "none")]
    None,
}

impl Default for GeometryFaceRangeEncoding {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `GeometryFaceRange::binding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryFaceRangeBinding {
    #[serde(rename = "per-feature")]
    PerFeature,
}

impl Default for GeometryFaceRangeBinding {
    fn default() -> Self {
        Self::PerFeature
    }
}

/// Possible values for `GeometryFeatureID::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryFeatureIDType {
    UInt16,
    UInt32,
    UInt64,
}

impl Default for GeometryFeatureIDType {
    fn default() -> Self {
        Self::UInt16
    }
}

/// Possible values for `GeometryFeatureID::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryFeatureIDEncoding {
    #[serde(rename = "none")]
    None,
}

impl Default for GeometryFeatureIDEncoding {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `GeometryFeatureID::binding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryFeatureIDBinding {
    #[serde(rename = "per-feature")]
    PerFeature,
}

impl Default for GeometryFeatureIDBinding {
    fn default() -> Self {
        Self::PerFeature
    }
}

/// Possible values for `GeometryNormal::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryNormalType {
    Float32,
}

impl Default for GeometryNormalType {
    fn default() -> Self {
        Self::Float32
    }
}

/// Possible values for `GeometryNormal::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryNormalEncoding {
    #[serde(rename = "none")]
    None,
}

impl Default for GeometryNormalEncoding {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `GeometryNormal::binding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryNormalBinding {
    #[serde(rename = "per-vertex")]
    PerVertex,
}

impl Default for GeometryNormalBinding {
    fn default() -> Self {
        Self::PerVertex
    }
}

/// Possible values for `GeometryPosition::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryPositionType {
    Float32,
}

impl Default for GeometryPositionType {
    fn default() -> Self {
        Self::Float32
    }
}

/// Possible values for `GeometryPosition::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryPositionEncoding {
    #[serde(rename = "none")]
    None,
}

impl Default for GeometryPositionEncoding {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `GeometryPosition::binding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryPositionBinding {
    #[serde(rename = "per-vertex")]
    PerVertex,
}

impl Default for GeometryPositionBinding {
    fn default() -> Self {
        Self::PerVertex
    }
}

/// Possible values for `GeometryUV::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryUVType {
    Float32,
}

impl Default for GeometryUVType {
    fn default() -> Self {
        Self::Float32
    }
}

/// Possible values for `GeometryUV::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryUVEncoding {
    #[serde(rename = "none")]
    None,
}

impl Default for GeometryUVEncoding {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `GeometryUV::binding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryUVBinding {
    #[serde(rename = "per-vertex")]
    PerVertex,
}

impl Default for GeometryUVBinding {
    fn default() -> Self {
        Self::PerVertex
    }
}

/// Possible values for `GeometryUVRegion::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryUVRegionType {
    UInt16,
}

impl Default for GeometryUVRegionType {
    fn default() -> Self {
        Self::UInt16
    }
}

/// Possible values for `GeometryUVRegion::encoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryUVRegionEncoding {
    #[serde(rename = "normalized")]
    Normalized,
}

impl Default for GeometryUVRegionEncoding {
    fn default() -> Self {
        Self::Normalized
    }
}

/// Possible values for `GeometryUVRegion::binding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryUVRegionBinding {
    #[serde(rename = "per-vertex")]
    PerVertex,
    #[serde(rename = "per-uvregion")]
    PerUvregion,
}

impl Default for GeometryUVRegionBinding {
    fn default() -> Self {
        Self::PerVertex
    }
}

/// Possible values for `HeaderAttribute::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeaderAttributeType {
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
}

impl Default for HeaderAttributeType {
    fn default() -> Self {
        Self::UInt8
    }
}

/// Possible values for `HeaderValue::valueType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeaderValueType {
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
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
    AttributeValuesByteCount,
}

impl Default for HeaderValueProperty {
    fn default() -> Self {
        Self::Count
    }
}

/// Possible values for `LodSelection::metricType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LodSelectionMetricType {
    #[serde(rename = "maxScreenThreshold")]
    MaxScreenThreshold,
    #[serde(rename = "maxScreenThresholdSQ")]
    MaxScreenThresholdSQ,
    #[serde(rename = "screenSpaceRelative")]
    ScreenSpaceRelative,
    #[serde(rename = "distanceRangeFromDefaultCamera")]
    DistanceRangeFromDefaultCamera,
    #[serde(rename = "effectiveDensity")]
    EffectiveDensity,
}

impl Default for LodSelectionMetricType {
    fn default() -> Self {
        Self::MaxScreenThreshold
    }
}

/// Possible values for `MaterialDefinitionInfo::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialDefinitionInfoType {
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "water")]
    Water,
    #[serde(rename = "billboard")]
    Billboard,
    #[serde(rename = "leafcard")]
    Leafcard,
    #[serde(rename = "reference")]
    Reference,
}

impl Default for MaterialDefinitionInfoType {
    fn default() -> Self {
        Self::Standard
    }
}

/// Possible values for `MaterialDefinitions::alphaMode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialDefinitionsAlphaMode {
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "mask")]
    Mask,
    #[serde(rename = "blend")]
    Blend,
}

impl Default for MaterialDefinitionsAlphaMode {
    fn default() -> Self {
        Self::Opaque
    }
}

/// Possible values for `MaterialDefinitions::cullFace`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialDefinitionsCullFace {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "front")]
    Front,
    #[serde(rename = "back")]
    Back,
}

impl Default for MaterialDefinitionsCullFace {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `MaterialParams::renderMode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialParamsRenderMode {
    #[serde(rename = "textured")]
    Textured,
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "untextured")]
    Untextured,
    #[serde(rename = "wireframe")]
    Wireframe,
}

impl Default for MaterialParamsRenderMode {
    fn default() -> Self {
        Self::Textured
    }
}

/// Possible values for `NodePageDefinition::lodSelectionMetricType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodePageDefinitionLodSelectionMetricType {
    #[serde(rename = "maxScreenThreshold")]
    MaxScreenThreshold,
    #[serde(rename = "maxScreenThresholdSQ")]
    MaxScreenThresholdSQ,
}

impl Default for NodePageDefinitionLodSelectionMetricType {
    fn default() -> Self {
        Self::MaxScreenThreshold
    }
}

/// Possible values for `Store::resourcePattern`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StoreResourcePattern {
    #[serde(rename = "3dNodeIndexDocument")]
    NodeIndexDocument,
    SharedResource,
    #[serde(rename = "featureData")]
    FeatureData,
    Geometry,
    Texture,
    Attributes,
}

impl Default for StoreResourcePattern {
    fn default() -> Self {
        Self::NodeIndexDocument
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
    MeshPyramid,
    AutoThinning,
    Clustering,
    Generalizing,
}

impl Default for StoreLodType {
    fn default() -> Self {
        Self::MeshPyramid
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

/// Possible values for `Texture::wrap`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureWrap {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "repeat")]
    Repeat,
    #[serde(rename = "mirror")]
    Mirror,
}

impl Default for TextureWrap {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `Texture::channels`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureChannels {
    #[serde(rename = "rgb")]
    Rgb,
    #[serde(rename = "rgba")]
    Rgba,
}

impl Default for TextureChannels {
    fn default() -> Self {
        Self::Rgb
    }
}

/// Possible values for `TextureDefinitionInfo::channels`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureDefinitionInfoChannels {
    #[serde(rename = "rgb")]
    Rgb,
    #[serde(rename = "rgba")]
    Rgba,
}

impl Default for TextureDefinitionInfoChannels {
    fn default() -> Self {
        Self::Rgb
    }
}

/// Possible values for `TextureSetDefinitionFormat::format`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureSetDefinitionFormatFormat {
    #[serde(rename = "jpg")]
    Jpg,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "dds")]
    Dds,
    #[serde(rename = "ktx-etc2")]
    KtxEtc2,
    #[serde(rename = "ktx2")]
    Ktx2,
}

impl Default for TextureSetDefinitionFormatFormat {
    fn default() -> Self {
        Self::Jpg
    }
}

/// Possible values for `Value::timeEncoding`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueTimeEncoding {
    #[serde(rename = "ECMA_ISO8601")]
    ECMAIS8601,
}

impl Default for ValueTimeEncoding {
    fn default() -> Self {
        Self::ECMAIS8601
    }
}

/// Possible values for `VestedGeometryParams::topology`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VestedGeometryParamsTopology {
    PerAttributeArray,
    InterleavedArray,
    Indexed,
}

impl Default for VestedGeometryParamsTopology {
    fn default() -> Self {
        Self::PerAttributeArray
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

/// The I3S standard accommodates declaration of a vertical coordinate system that may either be
/// ellipsoidal or gravity-related. This allows for a diverse range of fields and applications
/// where the definition of elevation/height is important.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct HeightModelInfo {
    /// Represents the height model type.Possible values are:`gravity_related_height``ellipsoidal`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_model: Option<HeightModelInfoHeightModel>,
    /// Represents the vertical coordinate system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert_crs: Option<String>,
    /// Represents the unit of the height.Possible values are:`meter``us-foot``foot``clarke-foot``clarke-yard``clarke-link``sears-yard``sears-foot``sears-chain``benoit-1895-b-chain``indian-yard``indian-1937-y...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_unit: Option<HeightModelInfoHeightUnit>,
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

/// An Oriented Bounding Box (OBB) is a compact bounding volume representation, tightly fitting the
/// geometries it represents. An OBBs' invariance to translation and rotation, makes it ideal as
/// the optimal and default bounding volume representation in I3S.  When constructing an OBB for
/// I3S use, there are two considerations an implementer needs to be make based on the Coordinate
/// Reference System (CRS) of the layer:
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Obb {
    /// The center point of the oriented bounding box. For a global scene, such as the XY coordinate system in WGS1984, the center is specified in latitude/longitude in decimal degrees, elevation (Z) in meter...
    pub center: [f64; 3],
    /// Half size of the oriented bounding box in units of the CRS. For a global scene, such as the XY coordinate system in WGS1984, the center is specified in latitude/longitude in decimal degrees, elevation...
    pub half_size: [f64; 3],
    /// Orientation of the oriented bounding box as a 4-component quaternion. For a global scene, the quaternion is in an Earth-Centric-Earth-Fixed (ECEF) Cartesian space. ( Z+ : North, Y+ : East, X+: lon=lat...
    pub quaternion: [f64; 4],
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

/// The spatialReference object is located at the top level of the JSON hierarchy.  A spatial
/// reference can be defined using a Well-Known ID (WKID) or Well-Known Text (WKT). The default
/// tolerance and resolution values for the associated Coordinate Reference System (CRS) are used.
/// A spatial reference can optionally include a definition for a vertical coordinate system (VCS),
/// which is used to interpret a geometries z values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct SpatialReference {
    /// The current WKID value of the vertical coordinate system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_vcs_wkid: Option<i64>,
    /// Identifies the current WKID value associated with the same spatial reference. For example a WKID of '102100' (Web Mercator) has a latestWKid of '3857'.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_wkid: Option<i64>,
    /// The WKID value of the vertical coordinate system.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_wkid: Option<i64>,
    /// WKID, or Well-Known ID, of the CRS. Specify either WKID or WKT of the CRS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wkid: Option<i64>,
    /// WKT, or Well-Known Text, of the CRS. Specify either WKT or WKID of the CRS but not both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wkt: Option<String>,
}

/// The 3dNodeIndexDocument JSON file describes a single index node within a [store](store.cmn.md).
/// The store object describes the exact physical storage of a layer and enables the client to
/// detect when multiple layers are served from the same store. The file includes links to other
/// nodes (e.g. children, sibling, and parent), links to feature data, geometry data, texture data
/// resources, metadata (e.g. metrics used for LoD selection), and spatial extent. The node is the
/// root object in the 3dNodeIndexDocument. There is always exactly one node object in a
/// 3dNodeIndexDocument.  Depending on the geometry and LoD model, a node document can be tuned
/// towards being light-weight or heavy-weight. Clients decide which data to retrieve. The bounding
/// volume information for the node, its parent, siblings, and children provide enough data for a
/// simple visualization.  For example, the centroids of a bounding volume could be rendered as
/// point features.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct NodeIndexDocument {
    /// Tree-key ID. A unique identifier of a node within the scene layer. At 1.7 the tree-key is the integer id of the node represented as a string.
    pub id: String,
    /// Explicit level of this node within the index tree. The lowest level is 0, which is always the root node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<i64>,
    /// The version (store update session ID) of this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The center point of the minimum bounding sphere. An array of four doubles, corresponding to x, y, z and radius of the minimum bounding sphere of a node. For a global scene, i.e. ellipsoidal coordinate...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbs: Option<[f64; 4]>,
    /// Describes oriented bounding box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obb: Option<Obb>,
    /// Creation date of this node in UTC, presented as a string in the format YYYY-MM-DDThh:mm:ss.sTZD, with a fixed 'Z' time zone (see http://www.w3.org/TR/NOTE-datetime).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Expiration date of this node in UTC, presented as a string in the format YYYY-MM-DDThh:mm:ss.sTZD, with a fixed 'Z' time zone (see http://www.w3.org/TR/NOTE-datetime).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    /// Optional, 3D (4x4) transformation matrix expressed as a linear array of 16 values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 16]>,
    /// Reference to the parent node of a node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node: Option<NodeReference>,
    /// Reference to the child nodes of a node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<NodeReference>>,
    /// Reference to the neighbor (same level, spatial proximity) nodes of a node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neighbors: Option<Vec<NodeReference>>,
    /// Resource reference describing a shared resource document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shared_resource: Option<Resource>,
    /// Resource reference describing a FeatureData document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_data: Option<Vec<Resource>>,
    /// Resource reference describing a geometry resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_data: Option<Vec<Resource>>,
    /// Resource reference describing a texture resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_data: Option<Vec<Resource>>,
    /// Resource reference describing a featureData document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_data: Option<Vec<Resource>>,
    /// Metrics for LoD selection, to be evaluated by the client. *This property was previously optional which was a documentation error.
    pub lod_selection: Vec<LodSelection>,
    /// **Deprecated.** A list of summary information on the features present in this node, used for pre-visualisation and LoD switching in featureTree LoD stores.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub features: Option<Vec<Features>>,
}

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
    pub ordering: Option<Vec<AttributeStorageInfoOrdering>>,
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

/// The cachedDrawingInfo object indicates if the *drawingInfo* object is captured as part of the
/// binary scene layer representation. This object is used for the 3D Object and Integrated Mesh
/// scene layer if no [drawingInfo](drawingInfo.cmn.md) is defined.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct CachedDrawingInfo {
    /// If true, the drawingInfo is captured as part of the binary scene layer representation.
    pub color: bool,
}

/// I3S version 1.7 supports compressing the geometryBuffer of Integrated Mesh and 3D Object Layers
/// using [Draco](https://github.com/google/draco) compression. Draco compression is optimized for
/// compressing and decompressing 3D geometric meshes and point clouds.  Draco reduces the size of
/// the geometryBuffer payload, thereby reducing storage size and optimizing transmission rate.
/// All *vertexAttributes* of a Meshpyramids profile can be compressed with Draco.  *The ArcGIS
/// platform currently is compatible with version 1.3.5 of
/// [Draco](https://github.com/google/draco/blob/master/README.md#version-135-release).*
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct CompressedAttributes {
    /// Must be:`draco`
    pub encoding: CompressedAttributesEncoding,
    /// Possible values for each array string:`position`: `Draco` _double_ meta-data `i3s-scale_x`, `i3s-scale_y`. If present, must be applied to `x` and `y` coordinates to reverse `XY`/`Z` ratio preserving s...
    pub attributes: Vec<CompressedAttributesAttributes>,
}

/// The defaultGeometry schema is used in stores where all arrayBufferView geometry declarations
/// use the same pattern for face and vertex elements. This schema reduces redundancies of
/// arrayBufferView geometry declarations in a store and reuses the geometryAttribute type from
/// featureData. Only valueType and valuesPerElement are required.  # Geometry buffer
/// |fieldName|type|description| ----|------------|----| |vertexCount|UINT32|Number of vertices|
/// |featureCount|UINT32|Number of features.| |position|Float32[3*vertex count]|Vertex x,y,z
/// positions.| |normal|Float32[3*vertex count]|Normals x,y,z vectors.| |uv0|Float32[2*vertex
/// count]|Texture coordinates.| |color|UInt8[4*vertex count|RGBA colors. |id|UInt64[feature
/// count]|Feature IDs.| |faceRange|UInt32[2*feature count|Inclusive
/// [range](../1.7/geometryFaceRange.cmn.md) of the mesh triangles belonging to each feature in the
/// featureID array.| |region|UINT16[4*vertex count]|UV [region](../1.7/geometryUVRegion.cmn.md)
/// for repeated textures.|
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct DefaultGeometrySchema {
    /// Low-level default geometry type. If defined, all geometries in the store are expected to have this type.Must be:`triangles`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_type: Option<DefaultGeometrySchemaGeometryType>,
    /// Declares the topology of embedded geometry attributes. When 'Indexed', the indices must also be declared in the geometry schema ('faces') and precede the vertexAttribute data.Possible values are:`PerA...
    pub topology: DefaultGeometrySchemaTopology,
    /// Defines header fields in the geometry resources of this store that precede the vertex (and index) data.
    pub header: Vec<HeaderAttribute>,
    /// Defines the ordering of the vertex Attributes.
    pub ordering: Vec<String>,
    /// Declaration of the attributes per vertex in the geometry, such as position, normals or texture coordinates.
    pub vertex_attributes: VertexAttribute,
    /// Declaration of the indices into vertex attributes that define faces in the geometry, such as position, normals or texture coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<VertexAttribute>,
    /// Provides the order of the keys in featureAttributes, if present.
    pub feature_attribute_order: Vec<String>,
    /// Declaration of the attributes per feature in the geometry, such as feature ID or face range.
    pub feature_attributes: FeatureAttribute,
}

/// The drawingInfo object contains drawing information for a scene layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct DrawingInfo {
    /// An object defining the symbology for the layer. [See more](https://developers.arcgis.com/web-scene-specification/objects/drawingInfo/) information about supported renderer types in ArcGIS clients.
    pub renderer: String,
    /// Scale symbols for the layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale_symbols: Option<bool>,
}

/// An object defining where a feature is placed within a scene. For example, on the ground or at
/// an absolute height. [See more](https://developers.arcgis.com/web-scene-
/// specification/objects/elevationInfo/) information on elevation in ArcGIS clients.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct ElevationInfo {
    /// Possible values are:`relativeToGround``absoluteHeight``onTheGround``relativeToScene`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<ElevationInfoMode>,
    /// Offset is always added to the result of the above logic except for onTheGround where offset is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,
    /// A string value indicating the unit for the values in elevationInfo
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
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
    pub position: Option<Vec<f64>>,
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

/// The 3D spatial extent of the object it describes in the given spatial reference. The
/// coordinates of the extent can span across the antimeridian (180th meridian). For example, scene
/// layers in a geographic coordinate system covering New Zealand may have a larger xmin value than
/// xmax value. The fullExtent is used by clients to zoom to a scene layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FullExtent {
    /// An object containing the WKID or WKT identifying the spatial reference of the layer's geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_reference: Option<SpatialReference>,
    /// The most east x coordinate.
    pub xmin: f64,
    /// The most south y coordinate.
    pub ymin: f64,
    /// The most west x coordinate.
    pub xmax: f64,
    /// The most north y coordinate.
    pub ymax: f64,
    /// The minimum height z coordinate.
    pub zmin: f64,
    /// The maximum height z coordinate.
    pub zmax: f64,
}

/// This is the common container class for all types of geometry definitions used in I3S.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Geometry {
    /// Unique ID of the geometry in this store.
    pub id: f64,
    /// The type denotes whether the following geometry is defined by using array buffer views (ArrayBufferView), as an internal reference (GeometryReference), as a reference to a shared Resource (SharedResou...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// 3D (4x4) transformation matrix expressed as a linear array of 16 values.  Used for methods such as translation, scaling, and rotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transformation: Option<[f64; 16]>,
    /// The parameters for a geometry, as an Embedded GeometryParams object, an ArrayBufferView, a GeometryReference object, or a SharedResourceReference object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<GeometryParams>,
}

/// Each geometryAttribute object is an accessor, i.e. a view, into an array buffer. There are two
/// types of geometryAttributes - vertexAttributes and faceAttributes. The vertexAttributes
/// describe valid properties for a single vertex, and faceAttributes describe faces and other
/// structures by providing a set of indices. For example, the <code>faces.position</code> index
/// attribute is used to define which vertex positions make up a face.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryAttribute {
    /// The starting byte position where the required bytes begin. Only used with the Geometry **arrayBufferView**.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<f64>,
    /// The element type, from {UInt8, UInt16, Int16, Int32, Int64 or Float32, Float64}.
    pub value_type: String,
    /// The short number of values need to make a valid element (such as 3 for a xyz position).
    pub values_per_element: f64,
}

/// Mesh Geometry Description  **Important**: The order of the vertex attributes in the buffer is
/// **fixed** to simplify binary parsing:   ``` position normal uv0 uv1 color uvRegion featureId
/// faceRange ``` or  ``` compressedAttributes ```  **Important:** - Attribute that are present are
/// stored continuously in the corresponding geometry buffers. - All vertex attributes ( **except**
/// `compressedAttributes`) have a fixed size that may be computed as: `#component * sizeof( type )
/// * {# of vertices or #features}` where `#component` is the number of components such as
/// `position`,`normal`, etc.  Furthermore,`type` is the datatype of the variable used and `sizeof`
/// returns the size of the datatype in bytes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryBuffer {
    /// The number of bytes to skip from the beginning of the binary buffer. Useful to describe 'legacy' buffer that have a header. Default=`0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    /// Vertex positions relative to oriented-bounding-box center.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<GeometryPosition>,
    /// Face/vertex normal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<GeometryNormal>,
    /// First set of UV coordinates. Only applies to textured mesh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv0: Option<GeometryUV>,
    /// The colors attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<GeometryColor>,
    /// UV regions, used for repeated textures in texture atlases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_region: Option<GeometryUVRegion>,
    /// FeatureId attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_id: Option<GeometryFeatureID>,
    /// Face range for a feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_range: Option<GeometryFaceRange>,
    /// Compressed attributes. **Cannot** be combined with any other attributes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compressed_attributes: Option<CompressedAttributes>,
}

/// The color vertex attribute. Assumed to be Standard RGB (sRGB space). sRGB is a color space that
/// defines a range of colors that can be displayed on screen on in print. It is the most widely
/// used color space and is supported by most operating systems, software programs, monitors, and
/// printers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryColor {
    /// The color channel values.Must be:`UInt8`
    pub r#type: GeometryColorType,
    /// Number of colors. Must be `1` (opaque grayscale: `{R,R,R,255}`),`3`(opaque color `{R,G,B,255}`) or `4` ( transparent color `{R,G,B,A}`).
    pub component: i64,
    /// Encoding of the vertex attribute.Must be:`normalized`: Default. Assumes 8-bit unsigned color per channel [0,255] -> [0,1].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<GeometryColorEncoding>,
    /// Must be:`per-vertex`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GeometryColorBinding>,
}

/// The geometry definitions used in I3S version 1.7.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryDefinition {
    /// Defines the topology type of the mesh.Must be:`triangle`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<GeometryDefinitionTopology>,
    /// Array of geometry representation(s) for this class of meshes. When multiple representations are listed, Clients should select the most compact they support (e.g. Draco compressed mesh). For compatibil...
    pub geometry_buffers: Vec<GeometryBuffer>,
}

/// `faceRange` is an inclusive range of faces of the geometry that belongs to a specific feature.
/// For each feature, `faceRange` indicates its first and last triangles as a pair of integer
/// indices in the face list.  **Notes**: - [`featureID`](geometryFeatureID.cmn.md) attribute is
/// required - This attributes is only supported when topology is `triangle` - Vertices in the
/// geometry buffer must be grouped by `feature_id` - for _un-indexed triangle meshes_,
/// `vertex_index = face_index * 3 `  **Example**  ![Thematic 3D Object Scene Layer without
/// textures](../../docs/img/faceRange.png)  _Mesh with 2 features._  ![Thematic 3D Object Scene
/// Layer without textures](../../docs/img/faceRance_Triangles.png)  _Grouped vertices in the
/// geometry buffer._
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryFaceRange {
    /// Data type for the index rangeMust be:`UInt32`
    pub r#type: GeometryFaceRangeType,
    /// Pair of indices marking first and last triangles for a feature.
    pub component: i64,
    /// Must be:`none`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<GeometryFaceRangeEncoding>,
    /// Must be:`per-feature`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GeometryFaceRangeBinding>,
}

/// FeatureID attribute helps to identify a part of a mesh belonging to a particular GIS `feature`.
/// This ID may be used to query additional information from a `FeatureService`. For example, if a
/// 3D Object scene layer has a building with ID 1 all triangles in the faceRange for this feature
/// will belong to this feature_id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryFeatureID {
    /// A feature integer ID.Possible values are:`UInt16``UInt32``UInt64`
    pub r#type: GeometryFeatureIDType,
    /// must be 1
    pub component: i64,
    /// Must be:`none`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<GeometryFeatureIDEncoding>,
    /// Must be:`per-feature`: Default for `geometryBuffer.featureId`. One `feature_id` per feature. **Requirement**: a) [`FaceRange`](geometryFaceRange.cmn.md) attribute must be **present** to map features-t...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GeometryFeatureIDBinding>,
}

/// Normal attribute. Defines the normals of the geometry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryNormal {
    /// Must be:`Float32`
    pub r#type: GeometryNormalType,
    /// Number of coordinates per vertex position. Must be 3.
    pub component: i64,
    /// EncodingMust be:`none`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<GeometryNormalEncoding>,
    /// Must be:`per-vertex`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GeometryNormalBinding>,
}

/// The abstract parent object for all geometryParams classes (geometryReferenceParams,
/// vestedGeometryParamas, singleComponentParams). It does not have properties of its own.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryParams {}

/// Position vertex attribute.  Relative to the center of oriented-bounded box of the node.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryPosition {
    /// Vertex positions relative to Oriented-bounding-box center.Must be:`Float32`
    pub r#type: GeometryPositionType,
    /// Number of coordinates per vertex position. Must be 3.
    pub component: i64,
    /// Encoding. Must be:`none`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<GeometryPositionEncoding>,
    /// Must be:`per-vertex`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GeometryPositionBinding>,
}

/// Instead of owning a geometry exclusively, a feature can reference part of a geometry defined
/// for the node. This allows to pre-aggregate geometries for many features. In this case,
/// geometryReferenceParams must be used.  This allows for a single geometry to be
/// shared(referenced) by multiple features.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryReferenceParams {
    /// In-document absolute reference to full geometry definition (Embedded or ArrayBufferView) using the I3S json pointer syntax. For example, /geometryData/1.  See [OGC I3S Specification](https://docs.open...
    pub href: String,
    /// The type denotes whether the following geometry is defined by using array buffer views (arrayBufferView), as an internal reference (geometryReference), as a reference to a shared Resource (sharedResou...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Inclusive range of faces in this geometry that belongs to this feature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_range: Option<Vec<f64>>,
    /// True if this geometry participates in an LoD tree. Always true in mesh-pyramids profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_geometry: Option<bool>,
}

/// Defines the texture coordinates of the geometry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryUV {
    /// Must be:`Float32`
    pub r#type: GeometryUVType,
    /// Number of texture coordinates. Must be 2.
    pub component: i64,
    /// Must be:`none`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<GeometryUVEncoding>,
    /// Must be:`per-vertex`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GeometryUVBinding>,
}

/// UV region for repeated textures. UV regions are required to properly wrap UV coordinates of
/// repeated-texture in texture atlases.  The texture must be written in the atlas with extra
/// border texels to reduce texture sampling artifacts.  UV regions are defined as a four-component
/// array per vertex : [u_min, v_min, u_max, v_max ], where each component is in the range [0,1]
/// encoded using `normalized UInt16`.  UV could be "wrapped" in the shader like the following: ```
/// hlsl // UV for this texel is uv in [0, n] uv = frac(uv) * (region.zw - region.xy) + region.xy;
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct GeometryUVRegion {
    /// Color channel values.Must be:`UInt16`
    pub r#type: GeometryUVRegionType,
    /// The `default =4`, must be 4.
    pub component: i64,
    /// EncodingMust be:`normalized`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<GeometryUVRegionEncoding>,
    /// bindingPossible values are:`per-vertex`: default`per-uvregion`: Only valid in conjonction with [`compressedAttributes`](compressedAttributes.cmn.md) when `uvRegionIndex` attribute is present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<GeometryUVRegionBinding>,
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

/// An image is a binary resource, containing a single raster that can be used to texture a feature
/// or symbol. An image represents one specific texture LoD. For details on texture organization,
/// please refer to the section on [texture resources](texture.cmn.md).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Image {
    /// A unique ID for each image. Generated using the BuildID function.
    pub id: String,
    /// width of this image, in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// The maximum size of a single pixel in world units. This property is used by the client to pick the image to load and render.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixel_in_world_units: Option<f64>,
    /// The href to the image(s), one per encoding, in the same order as the encodings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<Vec<String>>,
    /// The byte offset of this image's encodings. There is one per encoding, in the same order as the encodings, in the block in which this texture image resides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<Vec<f64>>,
    /// The length in bytes of this image's encodings. There is one per encoding, in the same order as the encodings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Vec<f64>>,
}

/// LoD (Level of Detail) selection.  A client needs information to determine whether a node's
/// contents are "good enough" to render in the current 3D view under constraints such as
/// resolution, screen size, bandwidth and available memory and target minimum quality goals.
/// Multiple LoD selection metrics can be included.  These metrics are used by clients to determine
/// the optimal resource access patterns. Each I3S profile definition provides additional details
/// on LoD Selection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct LodSelection {
    /// Possible values are:`maxScreenThreshold`: A per-node value for the maximum pixel size as measured in screen pixels. This value indicates the upper limit for the screen size of the diameter of the node...
    pub metric_type: LodSelectionMetricType,
    /// Maximum metric value, expressed in the CRS of the vertex coordinates or in reference to other constants such as screen size.
    pub max_error: f64,
}

/// Materials describe how a feature or a set of features is to be rendered, including shading and
/// color.  Part of [sharedResource](sharedResource.cmn.md) that is deprecated with 1.7.
#[deprecated]
pub type MaterialDefinition = std::collections::HashMap<String, MaterialDefinitionInfo>;

/// Material information describes how a feature or a set of features is to be rendered, including
/// shading and color. The following table provides the set of attributes and parameters for the
/// `type`: `standard` material.  Part of [sharedResource](sharedResource.cmn.md) that is
/// deprecated with 1.7.
#[deprecated]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialDefinitionInfo {
    /// A name for the material as assigned in the creating application.
    pub name: String,
    /// Indicates the material type, chosen from the supported values.Possible values are:`standard``water``billboard``leafcard``reference`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<MaterialDefinitionInfoType>,
    /// The href that resolves to the shared resource bundle in which the material definition is contained.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "$ref")]
    pub ref_: Option<String>,
    /// Parameter defined for the material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<MaterialParams>,
}

/// The materialDefinitions object in I3S version 1.7 and higher are feature-compatible with [glTF
/// material](https://github.com/KhronosGroup/glTF/tree/master/specification/2.0#materials) but
/// with the following exceptions. I3S material colors properties (baseColorFactor, emissiveFactor
/// etc.) are assumed to be in the same color space as the textures, most commonly sRGB while in
/// glTF they are interpreted as
/// [linear](https://github.com/KhronosGroup/glTF/tree/master/specification/2.0#metallic-roughness-
/// material). glTF has separate definitions for properties like strength for [occlusionTextureInfo
/// ](https://github.com/KhronosGroup/glTF/blob/master/specification/2.0/schema/material.occlusionT
/// extureInfo.schema.json) and scale for [normalTextureInfo](https://github.com/KhronosGroup/glTF/
/// blob/master/specification/2.0/schema/material.normalTextureInfo.schema.json). Further I3S has
/// only one [texture definition](materialTexture.cmn.md) with factor that replaces strength and
/// scale.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialDefinitions {
    /// A set of parameter values that are used to define the metallic-roughness material model from Physically-Based Rendering (PBR) methodology. When not specified, all the default values of pbrMetallicRoug...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pbr_metallic_roughness: Option<PbrMetallicroughness>,
    /// The normal texture map. They are a special kind of texture that allow you to add surface detail such as bumps, grooves, and scratches to a model which catch the light as if they are represented by rea...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_texture: Option<MaterialTexture>,
    /// The occlusion texture map. The occlusion map is used to provide information about which areas of the model should receive high or low indirect lighting
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occlusion_texture: Option<MaterialTexture>,
    /// The emissive texture map. A texture that receives no lighting, so the pixels are shown at full intensity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_texture: Option<MaterialTexture>,
    /// The emissive color of the material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_factor: Option<[f64; 3]>,
    /// Defines the meaning of the alpha-channel/alpha-mask.Possible values are:`opaque`: The rendered output is fully opaque and any alpha value is ignored.`mask`: The rendered output is either fully opaque ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_mode: Option<MaterialDefinitionsAlphaMode>,
    /// The alpha cutoff value of the material (only applies when alphaMode=`mask`) default = `0.25`.  If the alpha value is greater than or equal to the `alphaCutoff` value then it is rendered as fully opaqu...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_cutoff: Option<f64>,
    /// Specifies whether the material is double sided. For lighting, the opposite normals will be used when original normals are facing away from the camera. default=`false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_sided: Option<bool>,
    /// Winding order is counterclockwise.Possible values are:`none`: Default. **Must** be none if `doubleSided=True`.`front`: Cull front faces (i.e. faces with counter-clockwise winding order).`back`: Cull b...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cull_face: Option<MaterialDefinitionsCullFace>,
}

/// Parameters describing the material.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialParams {
    /// Indicates transparency of this material; 0 = opaque, 1 = fully transparent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparency: Option<f64>,
    /// Indicates reflectivity of this material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflectivity: Option<f64>,
    /// Indicates shininess of this material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shininess: Option<f64>,
    /// Ambient color of this material. Ambient color is the color of an object where it is in shadow. This color is what the object reflects when illuminated by ambient light rather than direct light.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<Vec<f64>>,
    /// Diffuse color of this material. Diffuse color is the most instinctive meaning of the color of an object. It is that essential color that the object reveals under pure white light. It is perceived as t...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse: Option<Vec<f64>>,
    /// Specular color of this material. Specular color is the color of the light of a specular reflection (specular reflection is the type of reflection that is characteristic of light reflected from a shiny...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular: Option<Vec<f64>>,
    /// Rendering mode.Possible values are:`textured``solid``untextured``wireframe`
    pub render_mode: MaterialParamsRenderMode,
    /// TRUE if features with this material should cast shadows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cast_shadows: Option<bool>,
    /// TRUE if features with this material should receive shadows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_shadows: Option<bool>,
    /// Indicates the material culling options {back, front, *none*}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cull_face: Option<String>,
    /// This flag indicates that the vertex color attribute of the geometry should be used to color the geometry for rendering. If texture is present, the vertex colors are multiplied by this color. e.g. `pix...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_colors: Option<bool>,
    /// This flag indicates that the geometry has uv region vertex attributes. These are used for adressing subtextures in a texture atlas. The uv coordinates are relative to this subtexture in this case.  Th...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_regions: Option<bool>,
    /// Indicates whether Vertex Colors also contain a transparency channel.  Default is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_vertex_color_alpha: Option<bool>,
}

/// The material texture definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialTexture {
    /// The index in [layer.textureSetDefinitions](3DSceneLayer.cmn.md).
    pub texture_set_definition_id: i64,
    /// The set index of texture's TEXCOORD attribute used for texture coordinate mapping. Default is 0. Deprecated.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<i64>,
    /// The _normal texture_: scalar multiplier applied to each normal vector of the normal texture. For _occlusion texture_,scalar multiplier controlling the amount of occlusion applied. Default=`1`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor: Option<f64>,
}

/// Mesh object. Mesh geometry for a node. Clients have to use the `resource` identifiers written
/// in each node to access the resources. While content creator may choose to match `resource` with
/// the node id this is not required by the I3S specification and clients should not make this
/// assumption.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Mesh {
    /// The material definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<MeshMaterial>,
    /// The geometry definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<MeshGeometry>,
    /// The attribute set definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<MeshAttribute>,
}

/// Mesh attributes for a node.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MeshAttribute {
    /// The resource identifier to be used to locate attribute resources of this mesh. i.e. `layers/0/nodes//attributes/...`
    pub resource: i64,
}

/// Mesh geometry for a node.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MeshGeometry {
    /// The index in [layer.geometryDefinitions](geometryDefinition.cmn.md) array
    pub definition: i64,
    /// The resource locator to be used to query geometry resources: `layers/0/nodes/{this.resource}/geometries/{layer.geometryDefinitions[this.definition].geometryBuffers[0 or 1]}`.
    pub resource: i64,
    /// Number of vertices in the geometry buffer of this mesh for the **umcompressed mesh buffer**. Please note that `Draco` compressed meshes may have less vertices due to de-duplication (actual number of v...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_count: Option<i64>,
    /// Number of features for this mesh. Default=`0`. (Must omit or set to `0` if mesh doesn't use `features`.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_count: Option<i64>,
}

/// Mesh geometry for a node.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MeshMaterial {
    /// The index in [layer.materialDefinitions](3DSceneLayer.cmn.md) array.
    pub definition: i64,
    /// Resource id for the material textures. i.e: `layers/0/nodes/{material.resource}/textures/{tex_name}`. Is **required** if material declares any textures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<i64>,
    /// Estimated number of texel for the highest resolution base color texture. i.e. `texture.mip0.width*texture.mip0.height`. Useful to estimate the resource cost of this node and/or texel-resolution based ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texel_count_hint: Option<i64>,
}

/// The node object.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Node {
    /// The index in the node array. May be **different than** material, geometry and attribute `resource` id. See [`mesh`](mesh.cmn.md) for more information.
    pub index: i64,
    /// The index of the parent node in the node array.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_index: Option<i64>,
    /// When to switch LoD. See [`nodepages[i].lodSelectionMetricType`](nodePageDefinition.cmn.md) for more information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lod_threshold: Option<f64>,
    /// Oriented bounding box for this node.
    pub obb: Obb,
    /// index of the children nodes indices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<i64>>,
    /// The mesh for this node. **WARNING:** only **SINGLE** mesh is supported at version 1.7 (i.e. `length` **must** be 0 or 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<Mesh>,
}

/// The node page object representing the tree as a flat array of nodes where internal nodes
/// reference their children by their array indices.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct NodePage {
    /// Array of nodes.
    pub nodes: Vec<Node>,
}

/// Nodes are stored contiguously in what can be considered a _flat_ array of nodes. This array can
/// be accessed by fixed-size pages of nodes for better request efficiency. All pages contains
/// exactly `layer.nodePages.nodesPerPage` nodes, except for the last page (that may contain less).
/// We use an integer ID to map a node to its page as follow: ``` page_id         = floor( node_id
/// / node_per_page) node_id_in_page = modulo( node_id, node_per_page) ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct NodePageDefinition {
    /// Number of nodes per page for this layer. **Must be a power-of-two** less than `4096`
    pub nodes_per_page: i64,
    /// Index of the root node.  Default = 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_index: Option<i64>,
    /// Defines the meaning of `nodes[].lodThreshold` for this layer.Possible values are:`maxScreenThreshold`: A per-node value for the maximum area of the projected bounding volume on screen in pixel.`maxScr...
    pub lod_selection_metric_type: NodePageDefinitionLodSelectionMetricType,
}

/// A nodeReference is a pointer to another node - the parent, a child or a neighbor. A
/// nodeReference contains a relative URL to the referenced NID, and a set of meta information
/// which helps determines if a client loads the data and maintains store consistency.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct NodeReference {
    /// Tree Key ID of the referenced node represented as string.
    pub id: String,
    /// An array of four doubles, corresponding to x, y, z and radius of the [minimum bounding sphere](mbs.cmn.md) of a node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mbs: Option<[f64; 4]>,
    /// Number of values per element.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    /// Version (store update session ID) of the referenced node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Number of features in the referenced node and its descendants, down to the leaf nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_count: Option<f64>,
    /// Describes oriented bounding box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obb: Option<Obb>,
}

/// Feature-compatible with [glTF
/// material](https://github.com/KhronosGroup/glTF/tree/master/specification/2.0#materials). With
/// the exception of emissive texture.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PbrMetallicroughness {
    /// The material's base color factor. default=`[1,1,1,1]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_factor: Option<[f64; 4]>,
    /// The base color texture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<MaterialTexture>,
    /// The metalness of the material. default=`1.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_factor: Option<f64>,
    /// The roughness of the material. default=`1.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roughness_factor: Option<f64>,
    /// The metallic-roughness texture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture: Option<MaterialTexture>,
}

/// Defines the look and feel of popup windows when a user clicks or queries a feature. [See
/// more](https://developers.arcgis.com/web-scene-specification/objects/popupInfo/) information on
/// popup information in ArcGIS clients.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PopupInfo {
    /// A string that appears at the top of the popup window as a title
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// A string that appears in the body of the popup window as a description. It is also possible to specify the description as HTML-formatted content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// List of Arcade expressions added to the pop-up. [See more](https://developers.arcgis.com/web-scene-specification/objects/popupExpressionInfo/) information on supported in ArcGIS clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression_infos: Option<String>,
    /// Array of fieldInfo information properties. This information is provided by the service layer definition. [See more](https://developers.arcgis.com/web-scene-specification/objects/fieldInfo/) informatio...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_infos: Option<String>,
    /// Array of various mediaInfo to display. Can be of type image, piechart, barchart, columnchart, or linechart. The order given is the order in which it displays. [See more](https://developers.arcgis.com/...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_infos: Option<String>,
    /// An array of popupElement objects that represent an ordered list of popup elements. [See more](https://developers.arcgis.com/web-scene-specification/objects/popupElement/) information on supported in A...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popup_elements: Option<String>,
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

/// **Shared Resources are deprecated for v1.7.  They must be included for backwards compatibility,
/// but are not used.**  Shared resources are models or textures that can be shared among features
/// within the same layer. They are stored as a JSON file. Each node has a shared resource that is
/// used by other features in the node or by features in the subtree of the current node. This
/// approach ensures an optimal distribution of shared resources across nodes, while maintaining
/// the node-based updating process. The SharedResource class collects Material definitions,
/// Texture definitions, Shader definitions and geometry symbols that need to be instanced.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct SharedResources {
    /// Materials describe how a Feature or a set of Features is to be rendered.
    pub material_definitions: MaterialDefinition,
    /// A Texture is a set of images, with some parameters specific to the texture/uv mapping to geometries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_definitions: Option<TextureDefinition>,
}

/// Objects of this type extend vestedGeometryParams and use one texture and one material. They can
/// be used with aggregated LoD geometries. Component objects provide information on parts of the
/// geometry they belong to, specifically with which material and texture to render them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct SingleComponentParams {
    /// URL - I3S Pointer reference to the material definition in this node's shared resource, from its root element. If present, used for the entire geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// URL - I3S Pointer reference to the material definition in this node's shared resource, from its root element. If present, used for the entire geometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// The ID of the component, only unique within the Geometry.
    pub id: f64,
    /// UUID of the material, as defined in the shared resources bundle, to use for rendering this component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_id: Option<f64>,
    /// Optional ID of the texture, as defined in shared resources, to use with the material to render this component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_id: Option<Vec<f64>>,
    /// Optional ID of a texture atlas region which to use with the texture to render this component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region_id: Option<Vec<f64>>,
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
    pub most_frequent_values: Option<Vec<ValueCount>>,
}

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

/// The texture resource contains texture image files. Textures are stored as a binary resource
/// within a node. I3S supports JPEG and PNG, as well as compressed texture formats S3TC, ETC2, and
/// Basis Universal. When creating a scene layer using textures for example, a 3D Object scene
/// layer, the appropriate texture encoding declaration needs to be provided. This is done using
/// MIME types such as ```image/jpeg``` (for JPEG), ```image/vnd-ms.dds``` (for S3TC) and
/// ```image/ktx2``` (for Basis Universal). Textures should be in RGBA format. RGBA is a three-
/// channel RGB color model supplemented with a 4th alpha chanel.  The integrated mesh and 3D
/// object profile types support textures. The textures file is a binary resource that contains
/// images to be used as textures for the features in the node. A single texture file contains 1 to
/// n textures for a specific level of texture detail. It may contain a single texture or multiple
/// individual textures. These are part of a texture atlas. Textures are expected in the following
/// formats:  |File name convention|Format| |-----|------------| |0_0.jpg|JPEG| |0.bin|PNG|
/// |0_0_1.bin.dds|S3TC| | 0_0_2.ktx|ETC2| |1.ktx2|Basis Universal|  The texture resource must
/// include either a JPEG or PNG texture file.  In I3S version 1.6, the size property will give you
/// the width of a texture. In version 1.7, the texelCountHint can be used to determine the cost of
/// loading a node as well as for use in texel-resolution based LoD switching. (A texel, texture
/// element, or texture pixel is the fundamental unit of a texture map.) Compressed textures(S3TC,
/// ETC, Basis Universal) may contain mipmaps. Mipmaps (also MIP maps) or pyramids are pre-
/// calculated, optimized sequences of images, each of which is a progressively lower resolution
/// representation of the same image. The height and width of each image, or level, in the mipmap
/// is a power of two smaller than the previous level. When compressing textures with mipmaps,  the
/// texture dimensions must of size 2<sup>n</sup> and the smallest size allowed is 4x4, where n =
/// 2. The number and volume of textures tends to be the limiting display factor, especially for
/// web and mobile clients.  The format used depends on the use case. For example, a client might
/// choose to consume JPEG in low bandwidth conditions since JPEG encoded files are efficient to
/// transmit and widely used. Clients constrained for memory or computing resources might choose to
/// directly consume compressed textures for performance reasons.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Texture {
    /// MIMEtype[1..*] The encoding/content type that is used by all images in this map
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<Vec<String>>,
    /// Possible values for each array string:`none``repeat``mirror`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<Vec<TextureWrap>>,
    /// True if the Map represents a texture atlas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<bool>,
    /// The name of the UV set to be used as texture coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_set: Option<String>,
    /// Indicates channels description.Possible values are:`rgb``rgba`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<TextureChannels>,
}

/// A texture is a set of images, with some parameters specific to the texture/uv mapping to
/// geometries.  Part of [sharedResource](sharedResource.cmn.md) that is deprecated with 1.7.
#[deprecated]
pub type TextureDefinition = std::collections::HashMap<String, TextureDefinitionInfo>;

/// A texture is a set of images, with some parameters specific to the texture/uv mapping to
/// geometries.  Part of [sharedResource](sharedResource.cmn.md) that is deprecated with 1.7.
#[deprecated]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct TextureDefinitionInfo {
    /// MIMEtype - The encoding/content type that is used by all images in this map
    pub encoding: Vec<String>,
    /// UV wrapping modes, from {none, repeat, mirror}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<Vec<String>>,
    /// TRUE if the Map represents a texture atlas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<bool>,
    /// The name of the UV set to be used as texture coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_set: Option<String>,
    /// Indicates channels description.Possible values are:`rgb``rgba`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<TextureDefinitionInfoChannels>,
    /// An image is a binary resource, containing a single raster that can be used to texture a feature or symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<Image>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct TextureSetDefinition {
    /// List of formats that are available for this texture set.
    pub formats: Vec<TextureSetDefinitionFormat>,
    /// Set to `true` if this texture is a texture atlas. It is expected that geometries that use this texture have uv regions to specify the subtexture in the atlas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct TextureSetDefinitionFormat {
    /// The location ID for the resource (last segment of the URL path). Must be `"0"` for jpg/png, `"0_0_1"` for DDS, `"0_0_2"` for KTX, and `"1"` for KTX2.
    pub name: String,
    /// The texture format.Possible values are:`jpg`: JPEG compression. No mipmaps. Please note that alpha channel may have been added after the JPEG stream. This alpha channel is alwasy 8bit and zlib compres...
    pub format: TextureSetDefinitionFormatFormat,
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
pub struct ValueCount {
    /// Type of the attribute values after decompression, if applicable. Please note that `string` is not supported for point cloud scene layer attributes.
    pub value: String,
    /// Count of the number of values. May exceed 32 bits.
    pub count: f64,
}

/// The vertexAttribute object describes valid properties for a single vertex.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct VertexAttribute {
    /// The vertex position.
    pub position: GeometryAttribute,
    /// The vertex normal.
    pub normal: GeometryAttribute,
    /// The first set of UV coordinates.
    pub uv0: GeometryAttribute,
    /// The color attribute.
    pub color: GeometryAttribute,
    /// The region attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<GeometryAttribute>,
}

/// This object extends geometryParams and is the abstract parent object for all concrete
/// ('vested') geometryParams objects that directly contain a geometry definition, either as an
/// arrayBufferView or as an embedded geometry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct VestedGeometryParams {
    /// The primitive type of the geometry defined through a vestedGeometryParams object. One of {*triangles*, lines, points}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Declares the typology of embedded geometry attributes or those in a geometry resources. When 'Indexed', the indices (faces) must also be declared.Possible values are:`PerAttributeArray``InterleavedArr...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology: Option<VestedGeometryParamsTopology>,
    /// A list of Vertex Attributes, such as Position, Normals, UV coordinates, and their definitions. While there are standard keywords such as position, uv0..uv9, normal and color, this is an open, extendab...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_attributes: Option<VertexAttribute>,
    /// A list of Face Attributes, such as indices to build faces, and their definitions. While there are standard keywords such as position, uv0..uv9, normal and color, this is an open, extendable list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<GeometryAttribute>,
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
