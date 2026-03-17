//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: geometry

use serde::{Deserialize, Serialize};

use crate::feature::FeatureAttribute;
use crate::feature::HeaderAttribute;
use crate::material::MeshMaterial;

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
    #[serde(rename = "PerAttributeArray")]
    Perattributearray,
    Indexed,
}

impl Default for DefaultGeometrySchemaTopology {
    fn default() -> Self {
        Self::Perattributearray
    }
}

/// Possible values for `GeometryColor::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryColorType {
    #[serde(rename = "UInt8")]
    Uint8,
}

impl Default for GeometryColorType {
    fn default() -> Self {
        Self::Uint8
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

/// Possible values for `GeometryDefinitionPsl::topology`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryDefinitionPslTopology {
    #[serde(rename = "point")]
    Point,
}

impl Default for GeometryDefinitionPslTopology {
    fn default() -> Self {
        Self::Point
    }
}

/// Possible values for `GeometryFaceRange::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeometryFaceRangeType {
    #[serde(rename = "UInt32")]
    Uint32,
}

impl Default for GeometryFaceRangeType {
    fn default() -> Self {
        Self::Uint32
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
    #[serde(rename = "UInt16")]
    Uint16,
    #[serde(rename = "UInt32")]
    Uint32,
    #[serde(rename = "UInt64")]
    Uint64,
}

impl Default for GeometryFeatureIDType {
    fn default() -> Self {
        Self::Uint16
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
    #[serde(rename = "UInt16")]
    Uint16,
}

impl Default for GeometryUVRegionType {
    fn default() -> Self {
        Self::Uint16
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

/// Possible values for `VestedGeometryParams::topology`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VestedGeometryParamsTopology {
    #[serde(rename = "PerAttributeArray")]
    Perattributearray,
    #[serde(rename = "InterleavedArray")]
    Interleavedarray,
    Indexed,
}

impl Default for VestedGeometryParamsTopology {
    fn default() -> Self {
        Self::Perattributearray
    }
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
    pub attributes: CompressedAttributesAttributes,
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
    pub geometry_buffers: String,
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
    pub geometry_buffers: String,
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
