"""Type stubs for ``i3s.selection`` — SceneLayer, ViewState, LOD selection."""

from __future__ import annotations

from enum import IntEnum
from typing import Any

import numpy as np
import numpy.typing as npt

from .async_ import AsyncSystem
from .geometry import OrientedBoundingBox
from .geospatial import CrsTransform, Ellipsoid, SceneCoordinateSystem
from .spec import LayerInfo, SpatialReference

class LodMetric(IntEnum):
    MaxScreenThreshold = 0
    MaxScreenThresholdSQ = 1
    DensityThreshold = 2

class INodeExcluder:
    """Base class for excluding nodes from LOD traversal.

    Subclass and override :meth:`should_exclude` to skip specific nodes.
    """

    def __init__(self) -> None: ...
    def start_new_frame(self) -> None: ...
    def should_exclude(self, obb: Any) -> bool: ...

class IPrepareRendererResources:
    """Base class for preparing renderer resources from decoded I3S content.

    Subclass and override methods to create GPU-ready resources.
    """

    def __init__(self) -> None: ...
    def prepare_in_load_thread(self, node_id: int, content: NodeContent) -> Any: ...
    def prepare_in_main_thread(
        self, node_id: int, content: NodeContent, load_result: Any
    ) -> Any: ...
    def free(self, node_id: int, resources: Any) -> None: ...

class SceneLayerExternals:
    """External dependencies for a SceneLayer."""

    def __init__(
        self,
        async_system: AsyncSystem,
        prepare_renderer_resources: IPrepareRendererResources | None = None,
        excluders: list[INodeExcluder] | None = None,
    ) -> None: ...
    @property
    def async_system(self) -> AsyncSystem: ...
    def __repr__(self) -> str: ...

class ViewState:
    """Camera view state for LOD selection. Positions are ECEF."""

    def __init__(
        self,
        position: npt.NDArray[np.float64],
        direction: npt.NDArray[np.float64],
        up: npt.NDArray[np.float64],
        viewport_width: int,
        viewport_height: int,
        fov_y: float,
    ) -> None: ...
    @property
    def position(self) -> npt.NDArray[np.float64]: ...
    @property
    def direction(self) -> npt.NDArray[np.float64]: ...
    @property
    def up(self) -> npt.NDArray[np.float64]: ...
    @property
    def viewport_width(self) -> int: ...
    @property
    def viewport_height(self) -> int: ...
    @property
    def fov_y(self) -> float: ...
    def __repr__(self) -> str: ...

class SelectionOptions:
    """LOD selection and loading options."""

    def __init__(self) -> None: ...

    max_simultaneous_loads: int
    maximum_cached_bytes: int
    preload_ancestors: bool
    preload_siblings: bool
    loading_descendant_limit: int
    forbid_holes: bool
    enable_frustum_culling: bool
    enable_fog_culling: bool
    lod_threshold_multiplier: float

    def __repr__(self) -> str: ...

class ViewUpdateResult:
    """Result of a single frame's LOD selection."""

    @property
    def nodes_to_render(self) -> npt.NDArray[np.uint32]: ...
    @property
    def nodes_to_unload(self) -> npt.NDArray[np.uint32]: ...
    @property
    def load_request_count(self) -> int: ...
    @property
    def pages_needed_count(self) -> int: ...
    @property
    def tiles_visited(self) -> int: ...
    @property
    def tiles_culled(self) -> int: ...
    @property
    def tiles_kicked(self) -> int: ...
    @property
    def max_depth_visited(self) -> int: ...
    def __repr__(self) -> str: ...

class NodeLoadState(IntEnum):
    Unloaded = 0
    Loading = 1
    Loaded = 2
    Failed = 3

class RenderNode:
    """A node selected for rendering, with its OBB transform."""

    @property
    def node_id(self) -> int: ...
    @property
    def center(self) -> npt.NDArray[np.float64]: ...
    @property
    def quaternion(self) -> npt.NDArray[np.float64]: ...
    @property
    def half_size(self) -> npt.NDArray[np.float64]: ...
    @property
    def bounding_radius(self) -> float: ...
    def __repr__(self) -> str: ...

class GeometryData:
    """Decoded geometry buffer content."""

    @property
    def vertex_count(self) -> int: ...
    @property
    def feature_count(self) -> int: ...
    @property
    def positions(self) -> npt.NDArray[np.float32]: ...
    @property
    def normals(self) -> npt.NDArray[np.float32] | None: ...
    @property
    def uv0(self) -> npt.NDArray[np.float32] | None: ...
    @property
    def colors(self) -> npt.NDArray[np.uint8] | None: ...
    @property
    def uv_region(self) -> npt.NDArray[np.uint16] | None: ...
    @property
    def feature_ids(self) -> npt.NDArray[np.uint64] | None: ...
    @property
    def face_ranges(self) -> npt.NDArray[np.uint32] | None: ...
    def __repr__(self) -> str: ...

class NodeContent:
    """Loaded node content: geometry + texture + attribute data."""

    @property
    def geometry(self) -> GeometryData: ...
    @property
    def texture_data(self) -> bytes: ...
    @property
    def byte_size(self) -> int: ...
    def __repr__(self) -> str: ...

class SceneLayer:
    """An I3S scene layer.

    Accepts REST URLs (``http://``, ``https://``) and local ``.slpk``
    file paths.  Source type is auto-detected.

    ``__init__`` returns immediately; bootstrap I/O (layer JSON + node page 0)
    runs on the worker thread pool.  Poll :attr:`is_ready` or :attr:`root_obb`
    to know when the layer is usable, or simply drive the frame loop — ``tick``
    / ``update_view`` are no-ops until the bootstrap resolves.
    """
    """

    def __init__(
        self,
        externals: SceneLayerExternals,
        url: str,
        crs_transform: CrsTransform | None = None,
        options: SelectionOptions | None = None,
    ) -> None: ...
    @property
    def is_ready(self) -> bool:
        """``True`` once bootstrap has resolved (layer JSON + node page 0 loaded)."""
        ...

    @property
    def crs(self) -> SceneCoordinateSystem: ...
    @property
    def frame(self) -> int: ...
    @property
    def load_progress(self) -> float: ...
    @property
    def ellipsoid(self) -> Ellipsoid: ...
    @property
    def options(self) -> SelectionOptions: ...
    @options.setter
    def options(self, value: SelectionOptions) -> None: ...
    @property
    def cached_bytes(self) -> int: ...
    @property
    def layer_info(self) -> LayerInfo | None:
        """The typed I3S layer document (metadata from ``3DSceneLayer.json``).
        Returns ``None`` until :attr:`is_ready` is ``True``."""
        ...

    @property
    def spatial_reference(self) -> SpatialReference | None:
        """The spatial reference of this layer, or ``None`` if not yet loaded."""
        ...

    def update_view(self, view_states: list[ViewState]) -> ViewUpdateResult: ...
    def load_nodes(self, result: ViewUpdateResult) -> None: ...
    def tick(self, view_states: list[ViewState]) -> ViewUpdateResult: ...
    def update_view_offline(self, view_states: list[ViewState]) -> ViewUpdateResult: ...
    @property
    def root_obb(self) -> OrientedBoundingBox | None:
        """OBB of the root node in I3S spec coordinates.
        Returns ``None`` until :attr:`is_ready` is ``True``."""
        ...
    def nodes_to_render(self) -> list[RenderNode]: ...
    def node_load_state(self, node_id: int) -> NodeLoadState | None: ...
    def node_content(self, node_id: int) -> NodeContent | None: ...
    def __repr__(self) -> str: ...
