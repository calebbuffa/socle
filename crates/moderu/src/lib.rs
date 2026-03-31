mod generated;

mod accessor;
mod animation;
mod builder;
mod compaction;
mod copyright;
pub mod extensions;
mod geometry;
mod image;
mod merge;
mod metadata;
mod property;
mod sampler;
mod scene;
mod semantics;
mod texture;

pub use generated::*;

pub use accessor::{
    AccessorData, AccessorDataIter, AccessorDataView, AccessorTypedView, AccessorViewError,
    AccessorViewIter, AccessorWriter, ComponentType, append_accessor, get_feature_id_as_u64,
    get_instancing_translation, get_normal_accessor, get_position_accessor, get_texcoord_accessor,
    resolve_accessor, resolve_accessor_checked, resolve_accessor_mut, resolve_accessor_owned,
};

pub use animation::{
    AnimationClip, AnimationError, AnimationPlayer, AnimationSampler, AnimationTarget,
    InterpolationMode, TargetProperty,
};

pub use builder::{
    AccessorIndex, BufferViewIndex, GltfData, GltfModelBuilder, MeshBuilder, MeshIndex,
    PrimitiveBuilder,
};

// extensions — individual types accessible via moderu::extensions::*
// Re-export only the trait and registry at top level to avoid name conflicts.
pub use extensions::{ExtensionRegistry, TypedExtension};

pub use image::{
    GpuCompressedPixelFormat, Image, Ktx2TranscodeTargets, MipPosition, Rectangle,
    SupportedGpuCompressedPixelFormats,
};

pub use metadata::{
    Class, ClassProperty, EXT_STRUCTURAL_METADATA, EnumValue, ExtStructuralMetadata, FeatureId,
    PropertyAttribute, PropertyAttributeIter, PropertyAttributeProperty,
    PropertyAttributePropertyView, PropertyAttributeView, PropertyTable, PropertyTableIter,
    PropertyTableProperty, PropertyTablePropertyView, PropertyTableView, PropertyTexture,
    PropertyTextureProperty, PropertyTexturePropertyView, PropertyTextureView, Schema, SchemaEnum,
};

pub use property::{
    IntoF64, MetadataConvert, MetadataValue, PropertyArrayCopy, PropertyArrayIter,
    PropertyArrayView, PropertyComponentType, PropertyElement, PropertyMat2, PropertyMat3,
    PropertyMat4, PropertyType, PropertyViewStatus, TransformProperty, VariablePropertyArrayView,
    apply_scale, normalize_i8, normalize_i16, normalize_i32, normalize_i64,
    normalize_u8, normalize_u16, normalize_u32, normalize_u64, transform_value,
};

pub use sampler::{FilterMode, WrapMode};

pub use scene::{
    NodeTransform, SceneError, SceneGraph, SceneNode, SceneNodeIterator, SceneRootIterator,
    Transform, TransformCache, TransformSOA,
};

pub use semantics::{InstanceAttribute, VertexAttribute};

pub use texture::{
    FeatureIdTextureView, KhrTextureTransform, TextureView, TextureViewError, TextureViewOptions,
    base_color_index, has_transform, normal_map_index, texcoord_index,
};

pub use compaction::{
    collapse_to_single_buffer, compact_buffer, compact_buffers, move_buffer_content,
    remove_unused_accessors, remove_unused_buffer_views, remove_unused_buffers,
    remove_unused_images, remove_unused_materials, remove_unused_meshes, remove_unused_samplers,
    remove_unused_textures,
};

pub use copyright::{parse_copyright_string, parse_gltf_copyright};

pub use geometry::{
    BoundingBox, RayGltfHit, SkirtMeshMetadata, apply_gltf_up_axis_transform, apply_rtc_center,
    compute_bounding_box, get_node_transform, intersect_ray_gltf, set_node_transform,
};
