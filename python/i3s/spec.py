"""i3s.spec — I3S spec layer document types.

Re-exports compiled types from ``i3s._native.spec``. Every type mirrors the
I3S specification JSON at https://github.com/Esri/i3s-spec. Use ``to_dict()``
or ``__dict__`` for the full camelCase spec document as a Python dict.
"""

from i3s import _native  # type: ignore[attr-defined]

_mod = _native.spec

SceneLayerType = _mod.SceneLayerType
SceneLayerCapabilities = _mod.SceneLayerCapabilities
FieldType = _mod.FieldType
LodSelectionMetricType = _mod.LodSelectionMetricType
SpatialReference = _mod.SpatialReference
OrientedBoundingBox = _mod.OrientedBoundingBox
FullExtent = _mod.FullExtent
HeightModelInfo = _mod.HeightModelInfo
ElevationInfo = _mod.ElevationInfo
Field = _mod.Field
AttributeStorageInfo = _mod.AttributeStorageInfo
LodSelection = _mod.LodSelection
NodePageDefinition = _mod.NodePageDefinition
GeometryDefinition = _mod.GeometryDefinition
MaterialDefinitions = _mod.MaterialDefinitions
TextureSetDefinition = _mod.TextureSetDefinition
Store = _mod.Store
StorePsl = _mod.StorePsl
MeshLayerInfo = _mod.MeshLayerInfo
PointLayerInfo = _mod.PointLayerInfo
PointCloudLayerInfo = _mod.PointCloudLayerInfo
BuildingLayerInfo = _mod.BuildingLayerInfo
LayerInfo = _mod.LayerInfo

__all__ = [
    "SceneLayerType",
    "SceneLayerCapabilities",
    "FieldType",
    "LodSelectionMetricType",
    "SpatialReference",
    "OrientedBoundingBox",
    "FullExtent",
    "HeightModelInfo",
    "ElevationInfo",
    "Field",
    "AttributeStorageInfo",
    "LodSelection",
    "NodePageDefinition",
    "GeometryDefinition",
    "MaterialDefinitions",
    "TextureSetDefinition",
    "Store",
    "StorePsl",
    "MeshLayerInfo",
    "PointLayerInfo",
    "PointCloudLayerInfo",
    "BuildingLayerInfo",
    "LayerInfo",
]
