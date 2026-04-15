//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: bld

use serde::{Deserialize, Serialize};

use crate::cmn::FullExtent;
use crate::cmn::HeightModelInfo;
use crate::cmn::SceneLayerCapabilities;
use crate::cmn::SceneLayerType;
use crate::cmn::SpatialReference;

/// Possible values for `AttributeStatistics::modelName`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AttributeStatisticsModelName {
    #[serde(rename = "category")]
    Category,
    #[serde(rename = "family")]
    Family,
    #[serde(rename = "familyType")]
    FamilyType,
    #[serde(rename = "bldgLevel")]
    BldgLevel,
    #[serde(rename = "createdPhase")]
    CreatedPhase,
    #[serde(rename = "demolishedPhase")]
    DemolishedPhase,
    #[serde(rename = "discipline")]
    Discipline,
    #[serde(rename = "assemblyCode")]
    AssemblyCode,
    #[serde(rename = "omniClass")]
    OmniClass,
    #[serde(rename = "systemClassifications")]
    SystemClassifications,
    #[serde(rename = "systemType")]
    SystemType,
    #[serde(rename = "systemName")]
    SystemName,
    #[serde(rename = "systemClass")]
    SystemClass,
    #[serde(rename = "custom")]
    Custom,
    #[serde(other)]
    Unknown,
}

impl Default for AttributeStatisticsModelName {
    fn default() -> Self {
        Self::Category
    }
}

/// Possible values for `FilterAuthoringInfo::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterAuthoringInfoType {
    #[serde(rename = "checkbox")]
    Checkbox,
    #[serde(other)]
    Unknown,
}

impl Default for FilterAuthoringInfoType {
    fn default() -> Self {
        Self::Checkbox
    }
}

/// Possible values for `FilterModeSolid::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterModeSolidType {
    #[serde(rename = "solid")]
    Solid,
    #[serde(other)]
    Unknown,
}

impl Default for FilterModeSolidType {
    fn default() -> Self {
        Self::Solid
    }
}

/// Possible values for `FilterModeWireFrame::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FilterModeWireFrameType {
    #[serde(rename = "wireFrame")]
    WireFrame,
    #[serde(other)]
    Unknown,
}

impl Default for FilterModeWireFrameType {
    fn default() -> Self {
        Self::WireFrame
    }
}

/// Possible values for `Sublayer::discipline`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SublayerDiscipline {
    Mechanical,
    Architectural,
    Piping,
    Electrical,
    Structural,
    Infrastructure,
    #[serde(other)]
    Unknown,
}

impl Default for SublayerDiscipline {
    fn default() -> Self {
        Self::Mechanical
    }
}

/// Possible values for `Sublayer::layerType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SublayerLayerType {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "3DObject")]
    ThreeDObject,
    Point,
    #[serde(other)]
    Unknown,
}

impl Default for SublayerLayerType {
    fn default() -> Self {
        Self::Group
    }
}

/// Concatenated attribute statistics. If needed, the type of the attribute (string or number) may
/// be inferred from `mostFrequentValues` and/or `min`/`max` fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct AttributeStatistics {
    /// Name of the field.
    pub field_name: String,
    /// Label of the field name. If label is empty, the label and fieldName are identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// A fixed string of building information, similar to a filter. Used by client applications to define specific behavior for the modelName. The [default filter types](./defaultFilterTypes.bld.md) define t...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<AttributeStatisticsModelName>,
    /// Minimum value. Numeric attributes only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Maximum value. Numeric attributes only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Most frequent value, if applicable for this attribute. Truncated to 256 entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub most_frequent_values: Option<Vec<i64>>,
    /// List of sublayers where this attribute may be found.
    pub sub_layer_ids: Vec<i64>,
}

/// The filter object can be applied to a building scene layer. Filter allows client applications
/// to reduce the drawn elements of a building to specific types and values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Filter {
    /// Global ID as unique identifier of the filter.
    pub id: String,
    /// Name of the filter.
    pub name: String,
    /// Description of the filter.
    pub description: String,
    /// Indicates if a filter is the default filter. Clients use the default filter to show the current state of a building. For example, if 'created' is the default filter, all elements in the 'created' phas...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default_filter: Option<bool>,
    /// Defines if a filter is visible within the client application. Used to exclude filters that are overwritten from a group of filters shown in the client application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_visible: Option<bool>,
    /// Array of filter blocks defining the filter. A filter contains at least one filter block.
    pub filter_blocks: Vec<FilterBlock>,
    /// Authoring Info used to generate user interface for authoring clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_authoring_info: Option<FilterAuthoringInfo>,
}

/// The filter authoring info object contains metadata about the authoring process for creating a
/// filter object. This allows the authoring client to save specific, overridable settings.  The
/// next time it is accessed with an authoring client, the selections are remembered. Non-authoring
/// clients can ignore it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FilterAuthoringInfo {
    /// Declares type or filter authoring info.Must be:`checkbox`: Client UI with checkbox representation for each filter type and filter value.
    pub r#type: FilterAuthoringInfoType,
    /// Array of filter block authoring info.
    pub filter_blocks: Vec<FilterBlockAuthoringInfo>,
}

/// A filter block defines what elements will be filtered with a specific filter mode.  To ensure
/// performance on client applications, it is not recommended to declare multiple filter blocks
/// with the same filter mode. Filter blocks are contained in a filter for a building scene layer.
/// Each filter includes at least one filter block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FilterBlock {
    /// Title of the filter block.
    pub title: String,
    /// Filter mode defines how features are drawn. For example, the filter mode of a filter can be solid or wire frame.
    pub filter_mode: FilterMode,
    /// Filter query expression for a building scene layer.
    pub filter_expression: String,
}

/// The filter authoring info object contains metadata about the authoring process for creating a
/// filter block object. This allows the authoring client to save specific, overridable settings.
/// The next time it is accessed via an authoring client, their selections are remembered. Non-
/// authoring clients can ignore it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FilterBlockAuthoringInfo {
    /// Array of defined filter types. Each filter type has an array of filter values.
    pub filter_types: Vec<FilterType>,
}

/// Filter mode represents the way elements draw when participating in a filter block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FilterMode {}

/// Shows all elements that comply with the filter block of a filter in a building scene layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FilterModeSolid {
    /// Declares filter mode of type solid.Must be:`solid`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<FilterModeSolidType>,
}

/// Shows all elements that comply with the filter block of a filter in a building scene layer.
/// The elements are drawn with an edge line.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FilterModeWireFrame {
    /// Declares filter mode of type wire frame.Must be:`wireFrame`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<FilterModeWireFrameType>,
    /// An object defining solid edges of a feature. [See more](https://developers.arcgis.com/web-scene-specification/objects/edges/) information on supported edges in ArcGIS clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edges: Option<String>,
}

/// The file authoring information for a filter, including the filter type and its value settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct FilterType {
    /// Represents the filter type name. Name is a unique identifier.
    pub filter_type: String,
    /// Array of filter values. Filter values are the attributes that can be stored for individual fields in a layer.
    pub filter_values: Vec<String>,
}

/// #### Building scene layer structure  A building scene layer represents a 3D BIM model as a
/// single layer composed of sublayers. The 3D BIM model can be any man made structure organized in
/// discipline layers (groups) such as Architectural, Electrical, Infrastructure, Mechanical,
/// Piping or Structural and category layers representing content such as walls or windows. A
/// building scene layer may contain an overview including only exterior feature in addition to the
/// full model.  The concept of a group (i.e. `layerType='group'`) has been added to organized
/// sublayers into a nested tree structure that can be reflected in the table of content of 3D
/// Clients. If a building scene layer does not contain an overview, the structure should not
/// include an overview or full model, only the disciplines directly.  Please note that: - Groups
/// and sublayers may be referenced **once** (e.g. a sublayer may not be in multiple groups). -
/// Groups and sublayers do not have any resource associated with them. - Sublayer resources are
/// located in the sublayers of the building scene layer:
/// layers/{bim_layer_id}/sublayers/{sub_layer_id}/....  Since a building scene layer may have an
/// associated featureService, care must be taken to match building scene layer sublayer IDs with
/// the service. In practice, if the building scene layer has n sublayers numbered [0,n-1] they
/// need to match the featureService sublayers IDs. Any group layers ID in the scene layer need to
/// be greater.  ``` +-- layers |  +-- 10 (3dSceneLayer.json for layer10, layerType ='building' ) |
/// |  +-- statistics |  |  |   +-- summary.json |  |  +-- sublayers |  |  |  +--0
/// (3dSceneLayer.json for layer0, layerType='3DObject') |  |  |  |  +--nodes |  |  |  |  |  +--0 |
/// |  |  |  |  |  +--3dNodeIndexDocument.json |  |  |  |  |  |  +--geometries (...) |  |  |  |  |
/// |  +--attributes (...) |  |  |  |  |  +--1 |  |  |  |  |  |  +--3dNodeIndexDocument.json |  |
/// |  |  |  |  +--geometries (...) |  |  |  |  |  |  +--attributes (...) |  |  |  |  |  +--(...) |
/// |  |  |  +--statistics |  |  |  +--1 (3dSceneLayer.json for layer1, layerType='3DObject') |  |
/// |  |  +-- (...) |  |  |  +--(... , layerType='3DObject')  ```  #### Building scene layer
/// service: The service definition is identical to other scene layer service definitions and will
/// list a single layer (the BIM layer) e.g: ``` js { "serviceName" : "Esri Campus",
/// "serviceVersion" : "1.8" "supportedBindings" : "REST" "layers": [ { "id" : 10, "layerType" :
/// "Building" // ... // building scene layer JSON definitions (see example below) // ... } ] } ```
/// **Edits** - Group/layer names **must be unique**. - Capabilities that have been removed. -
/// `sublayers.href` and `groups.href` have been removed in favor of IDs. - Removed `fullExtent`
/// from `group` object. - Added back `modelName`. - Added statistics.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Layer {
    /// Identifier for the layer. Building scene layer id is not in the same namespace as sublayer id. **Important**: clients should **not** assume it will be `0`.
    pub id: i64,
    /// Layer name.
    pub name: String,
    /// Version of building scene layer.
    pub version: String,
    /// Alias of the layer name. Can be empty if alias and name are identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Must be:`Building`
    pub layer_type: SceneLayerType,
    /// Description for the layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Copyright information to be displayed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyright_text: Option<String>,
    /// 3d extent. If `layer.fullExtent.spatialReference` is specified, it **must** match `layer.spatialReference`.
    pub full_extent: FullExtent,
    /// The spatialReference of the layer including the vertical coordinate system. WKT is included to support custom spatial references.
    pub spatial_reference: SpatialReference,
    /// An object containing the vertical coordinate system information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height_model_info: Option<HeightModelInfo>,
    /// List of sublayers or group of sublayers.
    pub sublayers: Vec<Sublayer>,
    /// Array of filters defined for the building scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<Vec<Filter>>,
    /// Global ID, filter ID of the currently active filter for the building scene layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_filter_id: Option<String>,
    /// url to statistic summary for the BIM layer. [statistics/summary.json](attributestats.bld.md)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics_h_ref: Option<String>,
    /// Capabilities supported by building scene layer. Overwrites any capabilities on sublayers.Possible values for each array string:`View`: View is supported.`Query`: Query is supported.`Edit`: Edit is def...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<SceneLayerCapabilities>>,
}

/// Statistics for all building scene layer sublayers. Captures statistical information for each
/// field in the building scene layer and the sublayers containing this fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct BuildingStats {
    /// Per-attribute statistics for all sublayers.
    pub summary: Vec<AttributeStatistics>,
}

/// Sublayer of a building scene layer. A building scene layer is composed of an overview and the
/// full model containing the discipline and category layers. These layer types are represented as
/// sublayers. A sublayer may contain other layers or sublayers (i.e. `group`) to form a nested
/// structure.  The order of the layers is inverted, meaning the first layer is on the bottom and
/// the last layer is on the top.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Sublayer {
    /// Identifier for this sublayer. **If** `layerType != 'group'`, resources will be at `/layers/{bim_layer_id}/sublayers/{this.id}/...`
    pub id: i64,
    /// Layer name. **Must be unique** per building scene layer.
    pub name: String,
    /// Alias of the layer name. Can be empty if alias and name are identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    /// Semantics for work discipline groups which can be used to refine the user experience. Possible values are:`Mechanical``Architectural``Piping``Electrical``Structural``Infrastructure`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discipline: Option<SublayerDiscipline>,
    /// A fixed string of sublayer information. Used by client applications to define specific behavior for the modelName. See [list of defined modelNames](subLayerModelName.md) for sublayers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    /// Possible values are:`group``3DObject``Point`
    pub layer_type: SublayerLayerType,
    /// Visibility of the sublayer. Default is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<bool>,
    /// Sublayers contained in this layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sublayers: Option<Vec<Sublayer>>,
    /// Returns true if the layer has no features.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_empty: Option<bool>,
}

/// Building scene layers can be filtered by field types in the building category layer.  These
/// predefined filter types are always included in the statistical information of the building
/// scene layer.  Some filter types are common to all buildings. In addition to the default
/// filters, other fields can be included by setting the modelName to custom.  Filter types are
/// used in the buildings [statistical information](attributestats.bld.md) as well as in the
/// [filter authoring info](filterAuthoringInfo.bld.md). The following list contains all default
/// filter types and modelNames when creating a building scene layer using ArcGIS.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct DefaultFilterTypes {}
