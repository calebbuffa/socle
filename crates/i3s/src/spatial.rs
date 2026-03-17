//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: spatial

use serde::{Deserialize, Serialize};

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

/// Possible values for `ElevationInfo::mode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElevationInfoMode {
    #[serde(rename = "relativeToGround")]
    Relativetoground,
    #[serde(rename = "absoluteHeight")]
    Absoluteheight,
    #[serde(rename = "onTheGround")]
    Ontheground,
    #[serde(rename = "relativeToScene")]
    Relativetoscene,
}

impl Default for ElevationInfoMode {
    fn default() -> Self {
        Self::Relativetoground
    }
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
