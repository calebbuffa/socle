//! glTF 2.0 reader with full codec pipeline.
//!
//! Parses GLB / glTF JSON and runs every post-processing step:
//!
//! 1. **Data URI decoding** — `data:` buffer/image URIs → raw bytes
//! 2. **Image decoding** — PNG / JPEG / WebP → `Image` pixels *(feature: `image`)*
//! 3. **Draco** — `KHR_draco_mesh_compression` *(feature: `draco`)*
//! 4. **meshopt** — `EXT_meshopt_compression` *(feature: `meshopt`)*
//! 5. **SPZ** — `KHR_gaussian_splatting_compression_spz` *(feature: `spz`)*
//! 6. **Dequantization** — `KHR_mesh_quantization`
//! 7. **Texture transform** — `KHR_texture_transform`
//!
//! All codecs are behind feature flags so you only link what you need.
//!
//! ## Quick start
//!
//! ```ignore
//! let model = GltfReader::default().read_file("model.glb")?.model;
//! for img in &model.images {
//!     upload_to_gpu(img.pixels.data.as_slice(), img.pixels.width, img.pixels.height);
//! }
//! ```
mod error;
mod external_refs;
mod glb;
mod image_ops;
mod pipeline;
mod uri;

mod dequantize;
mod khr_texture_transform;

#[cfg(feature = "async")]
pub mod async_external_refs;

pub use error::{GltfError, Warning, Warnings};
pub use image_ops::blit_image;

// Re-export view types directly from moderu — no intermediate view module.
pub use glam::{Vec2, Vec3};
pub use moderu::{
    AccessorData, AccessorDataView, AccessorViewError, AnimationSampler, ExtensionRegistry,
    NodeTransform, SceneError, SceneGraph, SceneNode, Transform, TransformCache, TransformSOA,
    TypedExtension,
};

use moderu::GltfModel;

/// Image processing options for data URIs and embedded images.
#[derive(Clone, Debug)]
pub struct ImageProcessingOptions {
    /// Resolve external (non-`data:`) buffer and image URIs from the filesystem.
    ///
    /// Requires `base_path` to be set. Automatically set to `true` when using
    /// [`GltfReader::read_file`].
    pub resolve_external_references: bool,
    /// Base directory used to resolve relative URIs when
    /// `resolve_external_references` is `true`. Automatically derived from the
    /// file path when using [`GltfReader::read_file`].
    pub base_path: Option<std::path::PathBuf>,
    /// Decode `data:` URIs in buffers and images.
    pub decode_data_urls: bool,
    /// Clear the URI string after decoding a data URL (saves memory).
    pub clear_decoded_data_urls: bool,
    /// Decode embedded images (PNG / JPEG / WebP) to pixel data.
    pub decode_embedded_images: bool,
}

impl Default for ImageProcessingOptions {
    fn default() -> Self {
        Self {
            resolve_external_references: true,
            base_path: None,
            decode_data_urls: true,
            clear_decoded_data_urls: true,
            decode_embedded_images: true,
        }
    }
}

/// Mesh codec decompression options.
#[derive(Clone, Debug)]
pub struct MeshCodecOptions {
    /// Decompress `KHR_draco_mesh_compression` primitives.
    pub decode_draco: bool,
    /// Decompress `EXT_meshopt_compression` buffer views.
    pub decode_meshopt: bool,
    /// Decompress `KHR_gaussian_splatting_compression_spz` splats.
    pub decode_spz: bool,
}

impl Default for MeshCodecOptions {
    fn default() -> Self {
        Self {
            decode_draco: true,
            decode_meshopt: true,
            decode_spz: true,
        }
    }
}

/// Mesh data processing options (quantization, transforms).
#[derive(Clone, Debug)]
pub struct MeshProcessingOptions {
    /// Dequantize `KHR_mesh_quantization` attributes to float.
    pub dequantize: bool,
    /// Apply `KHR_texture_transform` to UV coordinates.
    pub apply_texture_transform: bool,
}

impl Default for MeshProcessingOptions {
    fn default() -> Self {
        Self {
            dequantize: true,
            apply_texture_transform: true,
        }
    }
}

/// Options controlling which post-processing steps run.
///
/// All fields are public — configure them directly or use [`GltfReaderOptions::minimal`].
#[derive(Clone, Debug)]
pub struct GltfReaderOptions {
    /// Image processing settings (data URIs, embedded images).
    pub images: ImageProcessingOptions,
    /// Mesh codec decompression settings.
    pub codecs: MeshCodecOptions,
    /// Mesh post-processing settings (quantization, texture transforms).
    pub mesh: MeshProcessingOptions,
}

impl Default for GltfReaderOptions {
    fn default() -> Self {
        Self {
            images: ImageProcessingOptions::default(),
            codecs: MeshCodecOptions::default(),
            mesh: MeshProcessingOptions::default(),
        }
    }
}

impl GltfReaderOptions {
    /// All post-processing disabled — parse JSON/GLB only.
    pub fn minimal() -> Self {
        Self {
            images: ImageProcessingOptions {
                resolve_external_references: false,
                base_path: None,
                decode_data_urls: false,
                clear_decoded_data_urls: false,
                decode_embedded_images: false,
            },
            codecs: MeshCodecOptions {
                decode_draco: false,
                decode_meshopt: false,
                decode_spz: false,
            },
            mesh: MeshProcessingOptions {
                dequantize: false,
                apply_texture_transform: false,
            },
        }
    }
}

/// Successful output of a glTF read: the parsed model and any non-fatal warnings.
#[derive(Debug)]
pub struct GltfOk {
    pub model: GltfModel,
    pub warnings: Warnings,
}
/// glTF 2.0 reader. Parses GLB or glTF JSON and runs the codec pipeline.
#[derive(Clone, Debug)]
pub struct GltfReader {
    pub options: GltfReaderOptions,
}

impl Default for GltfReader {
    fn default() -> Self {
        Self {
            options: GltfReaderOptions::default(),
        }
    }
}

impl GltfReader {
    pub fn new(options: GltfReaderOptions) -> Self {
        Self { options }
    }

    /// Read from a file path. Detects GLB vs glTF JSON automatically.
    ///
    /// Automatically sets `options.images.base_path` to the file's parent
    /// directory so that external buffer and image URIs can be resolved.
    pub fn read_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<GltfOk, GltfError> {
        let path = path.as_ref();
        let data = std::fs::read(path)?;
        // Inject the parent directory so the pipeline can resolve external URIs.
        if self.options.images.resolve_external_references {
            let mut opts = self.options.clone();
            opts.images.base_path = path
                .parent()
                .map(|p| p.to_path_buf())
                .or_else(|| Some(std::path::PathBuf::new()));
            let reader = GltfReader { options: opts };
            return if glb::is_glb(&data) {
                reader.parse_glb(&data)
            } else {
                reader.parse_json(&data)
            };
        }
        self.parse(&data)
    }

    /// Parse binary GLB or JSON glTF from raw bytes.
    ///
    /// Automatically detects GLB by the magic header. Falls back to JSON parse.
    pub fn parse(&self, data: &[u8]) -> Result<GltfOk, GltfError> {
        if glb::is_glb(data) {
            self.parse_glb(data)
        } else {
            self.parse_json(data)
        }
    }

    /// Parse a binary GLB container from raw bytes.
    pub fn parse_glb(&self, data: &[u8]) -> Result<GltfOk, GltfError> {
        let (mut model, bin_chunk) = glb::parse_glb(data)?;

        if let Some(bin) = bin_chunk {
            if let Some(b) = model.buffers.first_mut() {
                b.data = bin.to_vec();
            }
        }

        let mut warnings = Warnings::new();
        pipeline::run(&self.options, &mut model, &mut warnings);
        Ok(GltfOk { model, warnings })
    }

    /// Parse glTF JSON from raw bytes.
    pub fn parse_json(&self, data: &[u8]) -> Result<GltfOk, GltfError> {
        let mut model = serde_json::from_slice::<GltfModel>(data).map_err(GltfError::JsonParse)?;
        let mut warnings = Warnings::new();
        pipeline::run(&self.options, &mut model, &mut warnings);
        Ok(GltfOk { model, warnings })
    }

    /// Fetch and parse a glTF asset from a URI using an [`orkester_io::AssetAccessor`].
    ///
    /// This is the async counterpart of [`GltfReader::read_file`], designed for
    /// use with [`orkester`]'s `Runtime` — matching the role of Cesium's
    /// `IAssetAccessor` / `AsyncSystem`.
    ///
    /// The `uri` can be any scheme the `accessor` understands: `https://`,
    /// `file://`, a bare file path, an archive reference, etc.
    ///
    /// The returned [`orkester::Task`] resolves on `Context::BACKGROUND`. It:
    ///
    /// 1. Fetches the primary `.gltf` / `.glb` bytes via `accessor`.
    /// 2. Resolves external buffer and image URIs through `accessor` (if
    ///    `options.images.resolve_external_references` is `true`).
    /// 3. Runs the standard codec pipeline (data-URI decode, image decode,
    ///    Draco, meshopt, SPZ, dequantization, texture transform).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let accessor = Arc::new(MyAccessor::new()); // handles file:// and https://
    /// let Runtime = Runtime::with_threads(4);
    /// let task = GltfReader::default().read_uri(
    ///     "https://example.com/model.glb",
    ///     accessor,
    ///     &Runtime,
    /// );
    /// let GltfOk { model, .. } = task.block().unwrap().unwrap();
    /// ```
    #[cfg(feature = "async")]
    pub fn read_uri(
        &self,
        uri: impl Into<String>,
        accessor: std::sync::Arc<dyn orkester_io::AssetAccessor>,
        Runtime: &orkester::Runtime,
    ) -> orkester::Task<Result<GltfOk, GltfError>> {
        let uri = uri.into();
        let options = self.options.clone();

        Runtime.run_async(orkester::Context::BACKGROUND, move || async move {
            // 1. Fetch the primary .gltf / .glb asset.
            let data = async_external_refs::fetch_bytes(&accessor, &uri).await?;

            // 2. Parse the JSON/GLB envelope (no pipeline yet).
            let mut model = if glb::is_glb(&data) {
                let (mut m, bin) = glb::parse_glb(&data)?;
                if let Some(b) = bin {
                    if let Some(buf) = m.buffers.first_mut() {
                        buf.data = b.to_vec();
                    }
                }
                m
            } else {
                serde_json::from_slice::<GltfModel>(&data).map_err(GltfError::JsonParse)?
            };

            let mut warnings = Warnings::new();

            // 3. Async external URI resolution.
            if options.images.resolve_external_references {
                async_external_refs::resolve_external_refs_async(
                    &mut model,
                    &uri,
                    &accessor,
                    &mut warnings,
                )
                .await;
            }

            // 4. Sync pipeline (steps 1–8). Disable the fs-based external-refs
            //    step (step 0) — it was already handled above.
            let mut pipeline_opts = options.clone();
            pipeline_opts.images.resolve_external_references = false;
            pipeline_opts.images.base_path = None;
            pipeline::run(&pipeline_opts, &mut model, &mut warnings);

            Ok(GltfOk { model, warnings })
        })
    }
}
