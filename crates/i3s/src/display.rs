//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: display

use serde::{Deserialize, Serialize};

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
