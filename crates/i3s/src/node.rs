//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: node

use serde::{Deserialize, Serialize};

use crate::core::Resource;
use crate::feature::Features;
use crate::geometry::Mesh;
use crate::spatial::Obb;

/// Possible values for `LodSelection::metricType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LodSelectionMetricType {
    #[serde(rename = "maxScreenThreshold")]
    Maxscreenthreshold,
    #[serde(rename = "maxScreenThresholdSQ")]
    Maxscreenthresholdsq,
    #[serde(rename = "screenSpaceRelative")]
    Screenspacerelative,
    #[serde(rename = "distanceRangeFromDefaultCamera")]
    Distancerangefromdefaultcamera,
    #[serde(rename = "effectiveDensity")]
    Effectivedensity,
}

impl Default for LodSelectionMetricType {
    fn default() -> Self {
        Self::Maxscreenthreshold
    }
}

/// Possible values for `NodePageDefinition::lodSelectionMetricType`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodePageDefinitionLodSelectionMetricType {
    #[serde(rename = "maxScreenThreshold")]
    Maxscreenthreshold,
    #[serde(rename = "maxScreenThresholdSQ")]
    Maxscreenthresholdsq,
}

impl Default for NodePageDefinitionLodSelectionMetricType {
    fn default() -> Self {
        Self::Maxscreenthreshold
    }
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
