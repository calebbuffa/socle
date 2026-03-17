//! Auto-generated from i3s-spec. Do not edit manually.
//!
//! Module: material

use serde::{Deserialize, Serialize};

/// Possible values for `MaterialDefinitionInfo::type`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialDefinitionInfoType {
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "water")]
    Water,
    #[serde(rename = "billboard")]
    Billboard,
    #[serde(rename = "leafcard")]
    Leafcard,
    #[serde(rename = "reference")]
    Reference,
}

impl Default for MaterialDefinitionInfoType {
    fn default() -> Self {
        Self::Standard
    }
}

/// Possible values for `MaterialDefinitions::alphaMode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialDefinitionsAlphaMode {
    #[serde(rename = "opaque")]
    Opaque,
    #[serde(rename = "mask")]
    Mask,
    #[serde(rename = "blend")]
    Blend,
}

impl Default for MaterialDefinitionsAlphaMode {
    fn default() -> Self {
        Self::Opaque
    }
}

/// Possible values for `MaterialDefinitions::cullFace`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialDefinitionsCullFace {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "front")]
    Front,
    #[serde(rename = "back")]
    Back,
}

impl Default for MaterialDefinitionsCullFace {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `MaterialParams::renderMode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialParamsRenderMode {
    #[serde(rename = "textured")]
    Textured,
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "untextured")]
    Untextured,
    #[serde(rename = "wireframe")]
    Wireframe,
}

impl Default for MaterialParamsRenderMode {
    fn default() -> Self {
        Self::Textured
    }
}

/// Possible values for `Texture::wrap`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureWrap {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "repeat")]
    Repeat,
    #[serde(rename = "mirror")]
    Mirror,
}

impl Default for TextureWrap {
    fn default() -> Self {
        Self::None
    }
}

/// Possible values for `Texture::channels`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureChannels {
    #[serde(rename = "rgb")]
    Rgb,
    #[serde(rename = "rgba")]
    Rgba,
}

impl Default for TextureChannels {
    fn default() -> Self {
        Self::Rgb
    }
}

/// Possible values for `TextureDefinitionInfo::channels`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureDefinitionInfoChannels {
    #[serde(rename = "rgb")]
    Rgb,
    #[serde(rename = "rgba")]
    Rgba,
}

impl Default for TextureDefinitionInfoChannels {
    fn default() -> Self {
        Self::Rgb
    }
}

/// Possible values for `TextureSetDefinitionFormat::format`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextureSetDefinitionFormatFormat {
    #[serde(rename = "jpg")]
    Jpg,
    #[serde(rename = "png")]
    Png,
    #[serde(rename = "dds")]
    Dds,
    #[serde(rename = "ktx-etc2")]
    KtxEtc2,
    #[serde(rename = "ktx2")]
    Ktx2,
}

impl Default for TextureSetDefinitionFormatFormat {
    fn default() -> Self {
        Self::Jpg
    }
}

/// An image is a binary resource, containing a single raster that can be used to texture a feature
/// or symbol. An image represents one specific texture LoD. For details on texture organization,
/// please refer to the section on [texture resources](texture.cmn.md).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Image {
    /// A unique ID for each image. Generated using the BuildID function.
    pub id: String,
    /// width of this image, in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    /// The maximum size of a single pixel in world units. This property is used by the client to pick the image to load and render.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixel_in_world_units: Option<f64>,
    /// The href to the image(s), one per encoding, in the same order as the encodings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub href: Option<Vec<String>>,
    /// The byte offset of this image's encodings. There is one per encoding, in the same order as the encodings, in the block in which this texture image resides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<Vec<f64>>,
    /// The length in bytes of this image's encodings. There is one per encoding, in the same order as the encodings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<Vec<f64>>,
}

/// Materials describe how a feature or a set of features is to be rendered, including shading and
/// color.  Part of [sharedResource](sharedResource.cmn.md) that is deprecated with 1.7.
#[deprecated]
pub type MaterialDefinition = std::collections::HashMap<String, MaterialDefinitionInfo>;

/// Material information describes how a feature or a set of features is to be rendered, including
/// shading and color. The following table provides the set of attributes and parameters for the
/// `type`: `standard` material.  Part of [sharedResource](sharedResource.cmn.md) that is
/// deprecated with 1.7.
#[deprecated]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialDefinitionInfo {
    /// A name for the material as assigned in the creating application.
    pub name: String,
    /// Indicates the material type, chosen from the supported values.Possible values are:`standard``water``billboard``leafcard``reference`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<MaterialDefinitionInfoType>,
    /// The href that resolves to the shared resource bundle in which the material definition is contained.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "$ref")]
    pub ref_: Option<String>,
    /// Parameter defined for the material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<MaterialParams>,
}

/// The materialDefinitions object in I3S version 1.7 and higher are feature-compatible with [glTF
/// material](https://github.com/KhronosGroup/glTF/tree/master/specification/2.0#materials) but
/// with the following exceptions. I3S material colors properties (baseColorFactor, emissiveFactor
/// etc.) are assumed to be in the same color space as the textures, most commonly sRGB while in
/// glTF they are interpreted as
/// [linear](https://github.com/KhronosGroup/glTF/tree/master/specification/2.0#metallic-roughness-
/// material). glTF has separate definitions for properties like strength for [occlusionTextureInfo
/// ](https://github.com/KhronosGroup/glTF/blob/master/specification/2.0/schema/material.occlusionT
/// extureInfo.schema.json) and scale for [normalTextureInfo](https://github.com/KhronosGroup/glTF/
/// blob/master/specification/2.0/schema/material.normalTextureInfo.schema.json). Further I3S has
/// only one [texture definition](materialTexture.cmn.md) with factor that replaces strength and
/// scale.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialDefinitions {
    /// A set of parameter values that are used to define the metallic-roughness material model from Physically-Based Rendering (PBR) methodology. When not specified, all the default values of pbrMetallicRoug...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pbr_metallic_roughness: Option<PbrMetallicroughness>,
    /// The normal texture map. They are a special kind of texture that allow you to add surface detail such as bumps, grooves, and scratches to a model which catch the light as if they are represented by rea...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_texture: Option<MaterialTexture>,
    /// The occlusion texture map. The occlusion map is used to provide information about which areas of the model should receive high or low indirect lighting
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occlusion_texture: Option<MaterialTexture>,
    /// The emissive texture map. A texture that receives no lighting, so the pixels are shown at full intensity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_texture: Option<MaterialTexture>,
    /// The emissive color of the material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive_factor: Option<[f64; 3]>,
    /// Defines the meaning of the alpha-channel/alpha-mask.Possible values are:`opaque`: The rendered output is fully opaque and any alpha value is ignored.`mask`: The rendered output is either fully opaque ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_mode: Option<MaterialDefinitionsAlphaMode>,
    /// The alpha cutoff value of the material (only applies when alphaMode=`mask`) default = `0.25`.  If the alpha value is greater than or equal to the `alphaCutoff` value then it is rendered as fully opaqu...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alpha_cutoff: Option<f64>,
    /// Specifies whether the material is double sided. For lighting, the opposite normals will be used when original normals are facing away from the camera. default=`false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_sided: Option<bool>,
    /// Winding order is counterclockwise.Possible values are:`none`: Default. **Must** be none if `doubleSided=True`.`front`: Cull front faces (i.e. faces with counter-clockwise winding order).`back`: Cull b...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cull_face: Option<MaterialDefinitionsCullFace>,
}

/// Parameters describing the material.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialParams {
    /// Indicates transparency of this material; 0 = opaque, 1 = fully transparent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparency: Option<f64>,
    /// Indicates reflectivity of this material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reflectivity: Option<f64>,
    /// Indicates shininess of this material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shininess: Option<f64>,
    /// Ambient color of this material. Ambient color is the color of an object where it is in shadow. This color is what the object reflects when illuminated by ambient light rather than direct light.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<Vec<f64>>,
    /// Diffuse color of this material. Diffuse color is the most instinctive meaning of the color of an object. It is that essential color that the object reveals under pure white light. It is perceived as t...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffuse: Option<Vec<f64>>,
    /// Specular color of this material. Specular color is the color of the light of a specular reflection (specular reflection is the type of reflection that is characteristic of light reflected from a shiny...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specular: Option<Vec<f64>>,
    /// Rendering mode.Possible values are:`textured``solid``untextured``wireframe`
    pub render_mode: MaterialParamsRenderMode,
    /// TRUE if features with this material should cast shadows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cast_shadows: Option<bool>,
    /// TRUE if features with this material should receive shadows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receive_shadows: Option<bool>,
    /// Indicates the material culling options {back, front, *none*}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cull_face: Option<String>,
    /// This flag indicates that the vertex color attribute of the geometry should be used to color the geometry for rendering. If texture is present, the vertex colors are multiplied by this color. e.g. `pix...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_colors: Option<bool>,
    /// This flag indicates that the geometry has uv region vertex attributes. These are used for adressing subtextures in a texture atlas. The uv coordinates are relative to this subtexture in this case.  Th...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_regions: Option<bool>,
    /// Indicates whether Vertex Colors also contain a transparency channel.  Default is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_vertex_color_alpha: Option<bool>,
}

/// The material texture definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MaterialTexture {
    /// The index in [layer.textureSetDefinitions](3DSceneLayer.cmn.md).
    pub texture_set_definition_id: i64,
    /// The set index of texture's TEXCOORD attribute used for texture coordinate mapping. Default is 0. Deprecated.
    #[deprecated]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tex_coord: Option<i64>,
    /// The _normal texture_: scalar multiplier applied to each normal vector of the normal texture. For _occlusion texture_,scalar multiplier controlling the amount of occlusion applied. Default=`1`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub factor: Option<f64>,
}

/// Mesh geometry for a node.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct MeshMaterial {
    /// The index in [layer.materialDefinitions](3DSceneLayer.cmn.md) array.
    pub definition: i64,
    /// Resource id for the material textures. i.e: `layers/0/nodes/{material.resource}/textures/{tex_name}`. Is **required** if material declares any textures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<i64>,
    /// Estimated number of texel for the highest resolution base color texture. i.e. `texture.mip0.width*texture.mip0.height`. Useful to estimate the resource cost of this node and/or texel-resolution based ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texel_count_hint: Option<i64>,
}

/// Feature-compatible with [glTF
/// material](https://github.com/KhronosGroup/glTF/tree/master/specification/2.0#materials). With
/// the exception of emissive texture.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct PbrMetallicroughness {
    /// The material's base color factor. default=`[1,1,1,1]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_factor: Option<[f64; 4]>,
    /// The base color texture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color_texture: Option<MaterialTexture>,
    /// The metalness of the material. default=`1.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_factor: Option<f64>,
    /// The roughness of the material. default=`1.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roughness_factor: Option<f64>,
    /// The metallic-roughness texture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metallic_roughness_texture: Option<MaterialTexture>,
}

/// **Shared Resources are deprecated for v1.7.  They must be included for backwards compatibility,
/// but are not used.**  Shared resources are models or textures that can be shared among features
/// within the same layer. They are stored as a JSON file. Each node has a shared resource that is
/// used by other features in the node or by features in the subtree of the current node. This
/// approach ensures an optimal distribution of shared resources across nodes, while maintaining
/// the node-based updating process. The SharedResource class collects Material definitions,
/// Texture definitions, Shader definitions and geometry symbols that need to be instanced.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct SharedResource {
    /// Materials describe how a Feature or a set of Features is to be rendered.
    pub material_definitions: MaterialDefinition,
    /// A Texture is a set of images, with some parameters specific to the texture/uv mapping to geometries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture_definitions: Option<TextureDefinition>,
}

/// The texture resource contains texture image files. Textures are stored as a binary resource
/// within a node. I3S supports JPEG and PNG, as well as compressed texture formats S3TC, ETC2, and
/// Basis Universal. When creating a scene layer using textures for example, a 3D Object scene
/// layer, the appropriate texture encoding declaration needs to be provided. This is done using
/// MIME types such as ```image/jpeg``` (for JPEG), ```image/vnd-ms.dds``` (for S3TC) and
/// ```image/ktx2``` (for Basis Universal). Textures should be in RGBA format. RGBA is a three-
/// channel RGB color model supplemented with a 4th alpha chanel.  The integrated mesh and 3D
/// object profile types support textures. The textures file is a binary resource that contains
/// images to be used as textures for the features in the node. A single texture file contains 1 to
/// n textures for a specific level of texture detail. It may contain a single texture or multiple
/// individual textures. These are part of a texture atlas. Textures are expected in the following
/// formats:  |File name convention|Format| |-----|------------| |0_0.jpg|JPEG| |0.bin|PNG|
/// |0_0_1.bin.dds|S3TC| | 0_0_2.ktx|ETC2| |1.ktx2|Basis Universal|  The texture resource must
/// include either a JPEG or PNG texture file.  In I3S version 1.6, the size property will give you
/// the width of a texture. In version 1.7, the texelCountHint can be used to determine the cost of
/// loading a node as well as for use in texel-resolution based LoD switching. (A texel, texture
/// element, or texture pixel is the fundamental unit of a texture map.) Compressed textures(S3TC,
/// ETC, Basis Universal) may contain mipmaps. Mipmaps (also MIP maps) or pyramids are pre-
/// calculated, optimized sequences of images, each of which is a progressively lower resolution
/// representation of the same image. The height and width of each image, or level, in the mipmap
/// is a power of two smaller than the previous level. When compressing textures with mipmaps,  the
/// texture dimensions must of size 2<sup>n</sup> and the smallest size allowed is 4x4, where n =
/// 2. The number and volume of textures tends to be the limiting display factor, especially for
/// web and mobile clients.  The format used depends on the use case. For example, a client might
/// choose to consume JPEG in low bandwidth conditions since JPEG encoded files are efficient to
/// transmit and widely used. Clients constrained for memory or computing resources might choose to
/// directly consume compressed textures for performance reasons.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct Texture {
    /// MIMEtype[1..*] The encoding/content type that is used by all images in this map
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<Vec<String>>,
    /// Possible values for each array string:`none``repeat``mirror`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<TextureWrap>,
    /// True if the Map represents a texture atlas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<bool>,
    /// The name of the UV set to be used as texture coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_set: Option<String>,
    /// Indicates channels description.Possible values are:`rgb``rgba`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<TextureChannels>,
}

/// A texture is a set of images, with some parameters specific to the texture/uv mapping to
/// geometries.  Part of [sharedResource](sharedResource.cmn.md) that is deprecated with 1.7.
#[deprecated]
pub type TextureDefinition = std::collections::HashMap<String, TextureDefinitionInfo>;

/// A texture is a set of images, with some parameters specific to the texture/uv mapping to
/// geometries.  Part of [sharedResource](sharedResource.cmn.md) that is deprecated with 1.7.
#[deprecated]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct TextureDefinitionInfo {
    /// MIMEtype - The encoding/content type that is used by all images in this map
    pub encoding: Vec<String>,
    /// UV wrapping modes, from {none, repeat, mirror}.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<Vec<String>>,
    /// TRUE if the Map represents a texture atlas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<bool>,
    /// The name of the UV set to be used as texture coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv_set: Option<String>,
    /// Indicates channels description.Possible values are:`rgb``rgba`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channels: Option<TextureDefinitionInfoChannels>,
    /// An image is a binary resource, containing a single raster that can be used to texture a feature or symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<Image>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct TextureSetDefinition {
    /// List of formats that are available for this texture set.
    pub formats: Vec<TextureSetDefinitionFormat>,
    /// Set to `true` if this texture is a texture atlas. It is expected that geometries that use this texture have uv regions to specify the subtexture in the atlas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]
pub struct TextureSetDefinitionFormat {
    /// The location ID for the resource (last segment of the URL path). Must be `"0"` for jpg/png, `"0_0_1"` for DDS, `"0_0_2"` for KTX, and `"1"` for KTX2.
    pub name: String,
    /// The texture format.Possible values are:`jpg`: JPEG compression. No mipmaps. Please note that alpha channel may have been added after the JPEG stream. This alpha channel is alwasy 8bit and zlib compres...
    pub format: TextureSetDefinitionFormatFormat,
}
