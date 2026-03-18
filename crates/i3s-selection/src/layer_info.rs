//! Layer info enum — typed access to the different I3S layer document formats.
//!
//! I3S has four layer document types:
//! - [`SceneLayerInfo`] — 3DObject and IntegratedMesh (CMN profile)
//! - [`SceneLayerInfoPsl`] — Point (PSL profile)
//! - [`PointCloudLayer`] — PointCloud (PCSL profile)
//! - [`BuildingLayer`] — Building (BLD profile)
//!
//! This enum wraps them and provides uniform access to common fields.

use i3s::bld::Layer as BuildingLayer;
use i3s::cmn::{SceneLayerInfo, SceneLayerType, SpatialReference};
use i3s::pcsl::PointCloudLayer;
use i3s::psl::SceneLayerInfoPsl;

/// The typed layer document for an I3S scene layer.
#[derive(Debug, Clone)]
pub enum LayerInfo {
    /// 3DObject or IntegratedMesh scene layer.
    Mesh(SceneLayerInfo),
    /// Point scene layer (PSL profile).
    Point(SceneLayerInfoPsl),
    /// Point cloud scene layer (PCSL profile).
    PointCloud(PointCloudLayer),
    /// Building scene layer (BLD profile).
    Building(BuildingLayer),
}

impl LayerInfo {
    /// The layer type discriminant.
    pub fn layer_type(&self) -> SceneLayerType {
        match self {
            LayerInfo::Mesh(info) => info.layer_type.clone(),
            LayerInfo::Point(info) => info.layer_type.clone(),
            LayerInfo::PointCloud(_) => SceneLayerType::PointCloud,
            LayerInfo::Building(_) => SceneLayerType::Building,
        }
    }

    /// The spatial reference, if present.
    pub fn spatial_reference(&self) -> Option<&SpatialReference> {
        match self {
            LayerInfo::Mesh(info) => info.spatial_reference.as_ref(),
            LayerInfo::Point(info) => info.spatial_reference.as_ref(),
            LayerInfo::PointCloud(info) => Some(&info.spatial_reference),
            LayerInfo::Building(info) => Some(&info.spatial_reference),
        }
    }

    /// Access as a mesh (3DObject / IntegratedMesh) layer info.
    pub fn as_mesh(&self) -> Option<&SceneLayerInfo> {
        match self {
            LayerInfo::Mesh(info) => Some(info),
            _ => None,
        }
    }

    /// Access as a Point (PSL) layer info.
    pub fn as_point(&self) -> Option<&SceneLayerInfoPsl> {
        match self {
            LayerInfo::Point(info) => Some(info),
            _ => None,
        }
    }

    /// Access as a PointCloud (PCSL) layer info.
    pub fn as_point_cloud(&self) -> Option<&PointCloudLayer> {
        match self {
            LayerInfo::PointCloud(info) => Some(info),
            _ => None,
        }
    }

    /// Access as a Building (BLD) layer info.
    pub fn as_building(&self) -> Option<&BuildingLayer> {
        match self {
            LayerInfo::Building(info) => Some(info),
            _ => None,
        }
    }
}
