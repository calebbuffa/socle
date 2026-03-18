"""i3s.selection — SceneLayer, ViewState, LOD selection.

Re-exports compiled types from ``i3s._native.selection``.
"""

from i3s import _native as _native  # type: ignore[attr-defined]

_mod = _native.selection

GeometryData = _mod.GeometryData
INodeExcluder = _mod.INodeExcluder
IPrepareRendererResources = _mod.IPrepareRendererResources
LodMetric = _mod.LodMetric
NodeContent = _mod.NodeContent
NodeLoadState = _mod.NodeLoadState
RenderNode = _mod.RenderNode
SceneLayer = _mod.SceneLayer
SceneLayerExternals = _mod.SceneLayerExternals
SelectionOptions = _mod.SelectionOptions
ViewState = _mod.ViewState
ViewUpdateResult = _mod.ViewUpdateResult

__all__ = [
    "GeometryData",
    "INodeExcluder",
    "IPrepareRendererResources",
    "LodMetric",
    "NodeContent",
    "NodeLoadState",
    "RenderNode",
    "SceneLayer",
    "SceneLayerExternals",
    "SelectionOptions",
    "ViewState",
    "ViewUpdateResult",
]
