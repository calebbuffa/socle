use std::sync::Arc;

use glam::{DMat4, DVec3};
use moderu;
use orkester::{Handle, Task};
use orkester_io::{AssetAccessor, AssetRequest};
use selekt::NodeDescriptor;

use crate::AsyncRuntime;
use crate::StratumOptions;

pub struct ChildrenResult {
    pub children: Vec<Node>,
    pub state: ContentLoadResultState,
}

pub enum UnloadContentResult {
    /// Content remains in the loaded node list.
    Keep,
    /// Node should be removed from the loaded node list.
    Remove,
    /// Node should be removed from the loaded node list
    /// and its children cleared as well.
    RemoveAndClearChildren,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ContentAddress {
    /// http or file URL.
    Uri(String),
    /// x, y, level
    Quadtree(u32, u32, u8),
    /// x, y, z, level
    Octree(u32, u32, u32, u8),
    /// Opaque numeric ID
    Index(u64),
}
pub trait ContentLoader: Send + Sync {
    fn load(
        &self,
        request: ContentLoadRequest,
    ) -> Task<Result<ContentLoadResult, Box<dyn std::error::Error + Send + Sync>>>;

    fn create_children(
        &self,
        node: ContentAddress,
        ellipsoid: terra::Ellipsoid,
    ) -> Task<Result<Vec<NodeDescriptor>, Box<dyn std::error::Error + Send + Sync>>>;
}

pub trait ContentManager {
    fn load_content(&self, node: &Node, options: &StratumOptions);
    fn update_content(&self, node: &Node, options: &StratumOptions);
    fn unload_content(&self, node: &Node) -> UnloadContentResult;
    fn root_available_handle(&self) -> Handle<()>;
    fn root(&self) -> Option<SlabIndex>;

    fn attach_overlay(
        &self,
        _node: &Node,
        _overlay_id: kasane::OverlayId,
        _uv_index: u32,
        _tile: &kasane::RasterOverlayTile,
        _translation: [f64; 2],
        _scale: [f64; 2],
    ) {
    }

    fn detach_overlay(&self, _node: &Node, _overlay_id: kasane::OverlayId) {}
}

pub struct ContentOptions {}

pub struct ContentLoadRequest {
    pub address: ContentAddress,
    pub runtime: AsyncRuntime,
    pub accessor: Arc<dyn AssetAccessor>,
    pub ellipsoid: terra::Ellipsoid,
    pub options: ContentOptions,
    pub headers: Vec<(String, String)>,
}

pub enum ContentKind {
    Unknown,
    Empty,
    External(String),
    Content(moderu::GltfModel),
}

pub enum ContentLoadResultState {
    /// The operation is successful and all the fields in `ContentLoadResult` are applied to the tile.
    Success,
    /// The operation is failed and __none__ of the fields in `ContentLoadResult` are applied to the tile.
    Failed,
    /// The operation requires the client to retry later due to some background work happening and __none__ of the fields in `ContentLoadResult` are applied to the tile.
    RetryLater,
}

pub struct ContentLoadResult {
    pub kind: ContentKind,
    pub bounds: zukei::SpatialBounds,
    pub state: ContentLoadResultState,
    pub request: AssetRequest,
}

impl ContentLoadResult {
    pub fn failed(request: AssetRequest) -> Self {
        Self {
            kind: ContentKind::Unknown,
            bounds: zukei::SpatialBounds::Empty,
            state: ContentLoadResultState::Failed,
            request,
        }
    }

    pub fn retry_later(request: AssetRequest) -> Self {
        Self {
            kind: ContentKind::Unknown,
            bounds: zukei::SpatialBounds::Empty,
            state: ContentLoadResultState::RetryLater,
            request,
        }
    }
}

pub struct NodeTransform {
    pub translation: DVec3,
    pub rotation: glam::DQuat,
    pub scale: DVec3,
}

impl NodeTransform {
    pub fn matrix(&self) -> DMat4 {
        let t = DMat4::from_translation(self.translation);
        let r = DMat4::from_quat(self.rotation);
        let s = DMat4::from_scale(self.scale);
        t * r * s
    }

    /// Compose `parent * self` (i.e., apply self in parent space)
    pub fn compose(&self, parent: &NodeTransform) -> NodeTransform {
        NodeTransform {
            translation: parent.translation + parent.rotation * (parent.scale * self.translation),
            rotation: parent.rotation * self.rotation,
            scale: parent.scale * self.scale,
        }
    }

    pub const IDENTITY: Self = Self {
        translation: DVec3::ZERO,
        rotation: glam::DQuat::IDENTITY,
        scale: DVec3::ONE,
    };
}

// parent * child
impl std::ops::Mul for NodeTransform {
    type Output = NodeTransform;

    fn mul(self, rhs: Self) -> Self::Output {
        NodeTransform {
            translation: self.translation + self.rotation * (self.scale * rhs.translation),
            rotation: self.rotation * rhs.rotation,
            scale: self.scale * rhs.scale,
        }
    }
}

impl std::ops::Mul<&NodeTransform> for NodeTransform {
    type Output = NodeTransform;

    fn mul(self, rhs: &NodeTransform) -> Self::Output {
        NodeTransform {
            translation: self.translation + self.rotation * (self.scale * rhs.translation),
            rotation: self.rotation * rhs.rotation,
            scale: self.scale * rhs.scale,
        }
    }
}

impl Default for NodeTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

pub enum NodeRefine {
    Add,
    Replace,
}

pub struct LodErrorDescriptor {
    pub value: f64,
    pub name: String,
}

pub enum NodeLoadStatus {
    Unloading,
    FailedTemporarily,
    Unloaded,
    ContentLoading,
    ContentLoaded,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlabIndex(pub u64);

pub struct Node {
    pub address: Option<ContentAddress>,
    pub parent: Option<SlabIndex>,
    pub children: Vec<SlabIndex>,
    pub model: Option<Box<moderu::GltfModel>>,
    pub transform: NodeTransform,
    pub bounds: zukei::SpatialBounds,
    pub refine: NodeRefine,
    pub lod_descriptor: LodErrorDescriptor,
    pub status: NodeLoadStatus,
}
