//! Main glTF writer API.

use super::error::{WriteError, WriteResult};
use super::glb::{GlbHeader, write_bin_chunk, write_json_chunk};
use moderu::GltfModel;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Options for how to write a glTF and what encodings to apply.
#[derive(Clone, Debug)]
pub struct GltfWriterOptions {
    /// Enable pretty-printing JSON output.
    pub pretty_print: bool,

    /// Byte alignment for GLB chunks.
    ///
    /// The GLB spec requires 4-byte alignment. Some extensions (e.g.
    /// `EXT_mesh_features`) require 8-byte alignment. Defaults to `4`.
    pub binary_chunk_byte_alignment: usize,

    /// Apply Draco mesh compression.
    #[cfg(feature = "draco")]
    pub draco: bool,

    /// Apply Meshopt compression.
    #[cfg(feature = "meshopt")]
    pub meshopt: bool,

    /// Apply SPZ (Gaussian splatting) compression.
    #[cfg(feature = "spz")]
    pub spz: bool,

    /// Apply KTX2 texture compression.
    #[cfg(feature = "ktx2")]
    pub ktx2: bool,

    /// Apply image format encoding.
    #[cfg(feature = "image")]
    pub image: bool,
}

impl Default for GltfWriterOptions {
    fn default() -> Self {
        GltfWriterOptions {
            pretty_print: true,
            binary_chunk_byte_alignment: 4,
            #[cfg(feature = "draco")]
            draco: false,
            #[cfg(feature = "meshopt")]
            meshopt: false,
            #[cfg(feature = "spz")]
            spz: false,
            #[cfg(feature = "ktx2")]
            ktx2: false,
            #[cfg(feature = "image")]
            image: false,
        }
    }
}

/// Main glTF writer for saving models to JSON and GLB formats.
pub struct GltfWriter {
    pub options: GltfWriterOptions,
}

impl GltfWriter {
    /// Create a new writer with specific options.
    pub fn with_options(options: GltfWriterOptions) -> Self {
        Self { options }
    }

    /// Apply codec encodings specified in options to the model.
    ///
    /// # Arguments
    /// * `model` - The glTF model to encode (modified in-place)
    /// * `options` - Encoding options
    ///
    /// # Errors
    /// Returns `WriteError::Codec` if any enabled codec fails.
    pub fn encode(&self, model: &mut GltfModel) -> WriteResult<()> {
        #[cfg(feature = "draco")]
        if self.options.draco {
            moderu_codec::draco::encode(model).map_err(|reason| WriteError::Codec {
                codec: "KHR_draco_mesh_compression",
                reason: reason.to_string(),
            })?;
        }

        #[cfg(feature = "meshopt")]
        if self.options.meshopt {
            moderu_codec::meshopt::encode(model).map_err(|reason| WriteError::Codec {
                codec: "EXT_meshopt_compression",
                reason: reason.to_string(),
            })?;
        }

        #[cfg(feature = "spz")]
        if self.options.spz {
            moderu_codec::spz::encode(model).map_err(|reason| WriteError::Codec {
                codec: "KHR_gaussian_splatting_compression_spz",
                reason: reason.to_string(),
            })?;
        }

        #[cfg(feature = "ktx2")]
        if self.options.ktx2 {
            moderu_codec::ktx2::encode(model).map_err(|reason| WriteError::Codec {
                codec: "EXT_texture_ktx2",
                reason: reason.to_string(),
            })?;
        }

        #[cfg(feature = "image")]
        if self.options.image {
            moderu_codec::image::encode(model).map_err(|reason| WriteError::Codec {
                codec: "image",
                reason: reason.to_string(),
            })?;
        }

        Ok(())
    }

    /// Write a glTF model to a JSON file with a companion `.bin` sidecar.
    ///
    /// If buffer data is present, it is written to `<path>.bin` and the JSON
    /// is updated to reference it. The original `model` is not mutated.
    pub fn write_json_with_bin<P: AsRef<Path>>(
        &self,
        model: &GltfModel,
        path: P,
    ) -> WriteResult<()> {
        let path = path.as_ref();
        let mut model_clone;
        let model_to_write: &GltfModel;

        if let Some(buf) = model.buffers.first().filter(|b| !b.data.is_empty()) {
            let bin_path = path.with_extension("bin");
            let bin_name = bin_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("buffer0.bin")
                .to_string();
            let bin_len = buf.data.len();
            std::fs::write(&bin_path, &buf.data)?;

            model_clone = model.clone();
            if let Some(b) = model_clone.buffers.first_mut() {
                b.uri = Some(bin_name);
                b.byte_length = bin_len;
            }
            model_to_write = &model_clone;
        } else {
            model_to_write = model;
        }

        self.write_json(model_to_write, path)
    }

    /// Write a glTF model to a JSON file.
    ///
    /// # Arguments
    /// * `model` - The glTF model to write
    /// * `path` - Path where the JSON file will be saved
    /// * `options` - Writer options
    ///
    /// # Errors
    /// Returns `WriteError` if serialization or I/O fails.
    pub fn write_json<P: AsRef<Path>>(&self, model: &GltfModel, path: P) -> WriteResult<()> {
        let json_str = if self.options.pretty_print {
            serde_json::to_string_pretty(model)?
        } else {
            serde_json::to_string(model)?
        };

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(json_str.as_bytes())?;
        writer.flush()?;

        Ok(())
    }

    /// Write a glTF model to a GLB (binary) file.
    ///
    /// # Arguments
    /// * `model` - The glTF model to write
    /// * `buffers` - Runtime buffer data (buffers[0] becomes the GLB BIN chunk)
    /// * `path` - Path where the GLB file will be saved
    /// * `options` - Writer options
    ///
    /// # Errors
    /// Returns `WriteError` if serialization, buffer preparation, or I/O fails.
    pub fn write_glb<P: AsRef<Path>>(&self, model: &GltfModel, path: P) -> WriteResult<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        self.write_glb_internal(model, writer)
    }

    /// Write a glTF model to a GLB buffer in memory.
    ///
    /// # Arguments
    /// * `model` - The glTF model to write
    /// * `buffers` - Runtime buffer data (buffers[0] becomes the GLB BIN chunk)
    /// * `out` - Vector where GLB data will be written
    /// * `options` - Writer options
    ///
    /// # Errors
    /// Returns `WriteError` if serialization fails.
    pub fn write_glb_to_buffer(&self, model: &GltfModel, out: &mut Vec<u8>) -> WriteResult<()> {
        self.write_glb_internal(model, out)
    }

    fn write_glb_internal<W: Write>(&self, model: &GltfModel, mut writer: W) -> WriteResult<()> {
        // Serialize model to JSON
        let json_str = if self.options.pretty_print {
            serde_json::to_string_pretty(model)?
        } else {
            serde_json::to_string(model)?
        };
        let json_bytes = json_str.as_bytes();

        // Use the first runtime buffer as the GLB BIN chunk.
        let bin_data: &[u8] = model
            .buffers
            .first()
            .map(|b| b.data.as_slice())
            .unwrap_or(&[]);

        // Calculate file length: 12 (header) + json chunk + bin chunk
        let align = self.options.binary_chunk_byte_alignment.max(1);
        let json_chunk_size = ((json_bytes.len() + align - 1) / align) * align + 8; // padded + header
        let bin_chunk_size = if !bin_data.is_empty() {
            ((bin_data.len() + align - 1) / align) * align + 8 // padded + header
        } else {
            0
        };
        let total_length = 12 + json_chunk_size + bin_chunk_size;

        // Write GLB header
        let header = GlbHeader::new(total_length as u32);
        header.write(&mut writer)?;

        // Write JSON chunk
        write_json_chunk(&mut writer, json_bytes, align)?;

        // Write binary chunk if present
        if !bin_data.is_empty() {
            write_bin_chunk(&mut writer, bin_data, align)?;
        }

        Ok(())
    }
}

impl Default for GltfWriter {
    fn default() -> Self {
        Self {
            options: GltfWriterOptions::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_options_default() {
        let opts = GltfWriterOptions::default();
        assert!(opts.pretty_print);
        #[cfg(feature = "draco")]
        assert!(!opts.draco);
    }

    #[test]
    fn test_options_builder() {
        let writer = GltfWriter {
            options: GltfWriterOptions {
                pretty_print: false,
                ..Default::default()
            },
        };
        assert!(!writer.options.pretty_print);
    }

    #[test]
    fn test_write_glb_to_buffer() {
        let model = GltfModel::default();
        let mut buf = Vec::new();
        let writer = GltfWriter::default();

        writer.write_glb_to_buffer(&model, &mut buf).unwrap();

        // Check GLB magic number
        assert!(buf.len() > 12);
        assert_eq!(
            u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            0x46546C67 // "glTF"
        );
    }
}
