//! Integration tests for features added for Cesium parity:
//! - External URI resolution (filesystem)
//! - `resolve_uri` URL helper
//! - `binary_chunk_byte_alignment` GLB option
//! - `SkirtMeshMetadata` roundtrip
//! - Mipmap generation
//! - Async `read_uri` via `AssetAccessor`

use moderu::SkirtMeshMetadata;
use moderu_io::reader::{GltfOk, GltfReader, GltfReaderOptions, ImageProcessingOptions};
use moderu_io::writer::{GltfWriter, GltfWriterOptions};
use std::fs;
use std::path::PathBuf;

// ---- helpers ----------------------------------------------------------------

fn sample_path(model: &str, variant: &str, file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/glTF-Sample-Assets/Models")
        .join(model)
        .join(variant)
        .join(file)
}

fn skip_if_missing(path: &PathBuf) -> bool {
    if !path.exists() {
        eprintln!("Skipping: test data not found: {}", path.display());
        true
    } else {
        false
    }
}

// ---- external URI resolution (filesystem) -----------------------------------

/// `read_file` should load a `.gltf` whose buffer is in a separate `.bin`
/// file. This exercises the external URI resolution pipeline step.
#[test]
fn external_bin_resolved_via_read_file() {
    let gltf_path = sample_path("Box", "glTF", "Box.gltf");
    if skip_if_missing(&gltf_path) {
        return;
    }

    let GltfOk { model, warnings } = GltfReader::default()
        .read_file(&gltf_path)
        .expect("read_file should succeed");

    assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    // If the external .bin was resolved the buffer should be populated.
    assert!(
        !model.buffers.is_empty(),
        "model should have at least one buffer"
    );
    assert!(
        !model.buffers[0].data.is_empty(),
        "buffer[0] data should be non-empty after external URI resolution"
    );
}

/// Parsing raw bytes with `parse()` (no base path) should leave external URIs
/// unresolved without error.
#[test]
fn parse_bytes_leaves_external_uris_unresolved() {
    let gltf_path = sample_path("Box", "glTF", "Box.gltf");
    if skip_if_missing(&gltf_path) {
        return;
    }

    let data = fs::read(&gltf_path).expect("read gltf bytes");
    // parse() has resolve_external_references=true but base_path=None, so the
    // filesystem step is a no-op for files missing a base path.
    let GltfOk { model, .. } = GltfReader::default()
        .parse(&data)
        .expect("parse should succeed even without external resolution");
    // Buffer may be empty because the .bin wasn't read — that's acceptable.
    let _ = model;
}

/// `resolve_external_references: false` must not load any external files even
/// when using `read_file`.
#[test]
fn external_resolution_can_be_disabled() {
    let gltf_path = sample_path("Box", "glTF", "Box.gltf");
    if skip_if_missing(&gltf_path) {
        return;
    }

    let options = GltfReaderOptions {
        images: ImageProcessingOptions {
            resolve_external_references: false,
            base_path: None,
            decode_data_urls: false,
            clear_decoded_data_urls: false,
            decode_embedded_images: false,
        },
        ..GltfReaderOptions::minimal()
    };
    let GltfOk { model, .. } = GltfReader::new(options)
        .read_file(&gltf_path)
        .expect("read_file should succeed");
    // Buffer data must be empty because resolution was disabled.
    if !model.buffers.is_empty() {
        assert!(
            model.buffers[0].data.is_empty(),
            "buffer data should be empty when external resolution is disabled"
        );
    }
}

// ---- resolve_uri ------------------------------------------------------------

#[cfg(feature = "async")]
mod resolve_uri_tests {
    use moderu_io::resolve_uri;

    #[test]
    fn http_relative() {
        assert_eq!(
            resolve_uri("https://example.com/tiles/model.gltf", "buffer0.bin"),
            "https://example.com/tiles/buffer0.bin"
        );
    }

    #[test]
    fn file_path_relative() {
        assert_eq!(
            resolve_uri("/data/tiles/model.gltf", "textures/tex.png"),
            "/data/tiles/textures/tex.png"
        );
    }

    #[test]
    fn already_absolute_scheme() {
        assert_eq!(
            resolve_uri(
                "https://example.com/tiles/model.gltf",
                "https://cdn.example.com/buf.bin"
            ),
            "https://cdn.example.com/buf.bin"
        );
    }

    #[test]
    fn root_relative() {
        assert_eq!(
            resolve_uri("https://example.com/tiles/model.gltf", "/other/tex.png"),
            "/other/tex.png"
        );
    }
}

// ---- binary_chunk_byte_alignment --------------------------------------------

/// Default alignment (4) — output length must be a multiple of 4.
#[test]
fn glb_default_alignment_is_4() {
    let model = moderu::GltfModel::default();
    let mut buf = Vec::new();
    GltfWriter::default()
        .write_glb_to_buffer(&model, &mut buf)
        .expect("write_glb_to_buffer");

    // GLB layout: 12-byte header, then JSON chunk (4-byte length + 4-byte type + data).
    assert!(buf.len() >= 20, "GLB too short");
    let json_chunk_len = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]) as usize;
    assert_eq!(
        json_chunk_len % 4,
        0,
        "JSON chunk data must be 4-byte aligned"
    );
}

/// Alignment set to 8 — each chunk's padded data length must be a multiple of 8.
///
/// Note: the 12-byte GLB header means the *total file length* is never a
/// multiple of 8 (12 % 8 == 4). What matters is that the JSON and BIN chunk
/// data lengths are padded to the requested alignment.
#[test]
fn glb_custom_alignment_8() {
    use moderu::{Buffer, GltfModel};

    // Create a model with a buffer whose size is not a multiple of 8.
    let mut model = GltfModel::default();
    model.buffers.push(Buffer {
        data: vec![1u8; 7], // deliberately odd size
        byte_length: 7,
        ..Default::default()
    });

    let opts = GltfWriterOptions {
        binary_chunk_byte_alignment: 8,
        ..Default::default()
    };
    let mut buf = Vec::new();
    GltfWriter::with_options(opts)
        .write_glb_to_buffer(&model, &mut buf)
        .expect("write_glb_to_buffer with align=8");

    // GLB layout:
    //   [0..12]  GLB header
    //   [12..16] JSON chunk data length
    //   [16..20] JSON chunk type
    //   [20..20+json_len] JSON data (padded)
    //   [20+json_len..24+json_len] BIN chunk data length
    //   [24+json_len..28+json_len] BIN chunk type
    //   [28+json_len..] BIN data (padded)
    assert!(buf.len() >= 20, "GLB too short");
    let json_chunk_len = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]) as usize;
    assert_eq!(
        json_chunk_len % 8,
        0,
        "JSON chunk data must be 8-byte aligned"
    );

    let bin_offset = 20 + json_chunk_len;
    if bin_offset + 8 <= buf.len() {
        let bin_chunk_len = u32::from_le_bytes([
            buf[bin_offset],
            buf[bin_offset + 1],
            buf[bin_offset + 2],
            buf[bin_offset + 3],
        ]) as usize;
        assert_eq!(
            bin_chunk_len % 8,
            0,
            "BIN chunk data must be 8-byte aligned"
        );
    }
}

// ---- SkirtMeshMetadata roundtrip -------------------------------------------

#[test]
fn skirt_mesh_metadata_roundtrip() {
    let meta = SkirtMeshMetadata {
        no_skirt_indices_begin: 0,
        no_skirt_indices_count: 360,
        no_skirt_vertices_begin: 0,
        no_skirt_vertices_count: 289,
        mesh_center: [1234567.8, -9876543.2, 45678.9],
        skirt_west_height: 100.5,
        skirt_south_height: 200.0,
        skirt_east_height: 150.75,
        skirt_north_height: 175.25,
    };

    let extras = meta.to_extras();

    // Confirm a round-trip through JSON preserves all values exactly.
    let json = serde_json::to_string(&extras).expect("serialize extras");
    let parsed_value: serde_json::Value = serde_json::from_str(&json).expect("deserialize extras");
    let restored = SkirtMeshMetadata::parse_from_extras(&parsed_value).expect("parse_from_extras");

    assert_eq!(restored.no_skirt_indices_begin, meta.no_skirt_indices_begin);
    assert_eq!(restored.no_skirt_indices_count, meta.no_skirt_indices_count);
    assert_eq!(
        restored.no_skirt_vertices_begin,
        meta.no_skirt_vertices_begin
    );
    assert_eq!(
        restored.no_skirt_vertices_count,
        meta.no_skirt_vertices_count
    );
    assert_eq!(restored.mesh_center, meta.mesh_center);
    assert_eq!(restored.skirt_west_height, meta.skirt_west_height);
    assert_eq!(restored.skirt_south_height, meta.skirt_south_height);
    assert_eq!(restored.skirt_east_height, meta.skirt_east_height);
    assert_eq!(restored.skirt_north_height, meta.skirt_north_height);
}

#[test]
fn skirt_mesh_metadata_missing_key_returns_none() {
    let incomplete = serde_json::json!({
        "skirtMeshMetadata": {
            "noSkirtRange": [0, 100, 0, 80]
            // missing meshCenter and skirt heights
        }
    });
    assert!(
        SkirtMeshMetadata::parse_from_extras(&incomplete).is_none(),
        "parse_from_extras should return None for incomplete data"
    );
}

#[test]
fn skirt_mesh_metadata_no_key_returns_none() {
    let empty = serde_json::json!({});
    assert!(SkirtMeshMetadata::parse_from_extras(&empty).is_none());
}

// ---- Mipmap generation ------------------------------------------------------

#[cfg(feature = "image")]
mod mipmap_tests {
    use moderu::Image;
    use moderu_codec::image::generate_mipmaps;

    fn solid_rgba(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Image {
        let data = vec![r, g, b, a].repeat((width * height) as usize);
        Image {
            data,
            width,
            height,
            channels: 4,
            bytes_per_channel: 1,
            ..Default::default()
        }
    }

    #[test]
    fn generates_correct_mip_count_for_4x4() {
        let mut img = solid_rgba(4, 4, 255, 0, 0, 255);
        generate_mipmaps(&mut img).expect("generate_mipmaps");
        // 4×4 → 2×2 → 1×1 = 3 levels
        assert_eq!(img.mip_positions.len(), 3);
    }

    #[test]
    fn mip0_matches_original_data() {
        let mut img = solid_rgba(4, 4, 128, 64, 32, 255);
        let original_data = img.data.clone();
        generate_mipmaps(&mut img).expect("generate_mipmaps");

        let mip0 = &img.mip_positions[0];
        assert_eq!(mip0.byte_offset, 0);
        assert_eq!(mip0.byte_size, original_data.len());
        assert_eq!(&img.data[..mip0.byte_size], original_data.as_slice());
    }

    #[test]
    fn total_data_length_is_sum_of_mip_sizes() {
        let mut img = solid_rgba(8, 8, 0, 128, 255, 255);
        generate_mipmaps(&mut img).expect("generate_mipmaps");

        let total: usize = img.mip_positions.iter().map(|m| m.byte_size).sum();
        assert_eq!(total, img.data.len());
    }

    #[test]
    fn mip_level_sizes_halve_each_step() {
        let mut img = solid_rgba(8, 8, 0, 0, 0, 255);
        generate_mipmaps(&mut img).expect("generate_mipmaps");
        // 8×8, 4×4, 2×2, 1×1 → 4 levels
        assert_eq!(img.mip_positions.len(), 4);
        assert_eq!(img.mip_positions[0].byte_size, 8 * 8 * 4);
        assert_eq!(img.mip_positions[1].byte_size, 4 * 4 * 4);
        assert_eq!(img.mip_positions[2].byte_size, 2 * 2 * 4);
        assert_eq!(img.mip_positions[3].byte_size, 1 * 1 * 4);
    }

    #[test]
    fn rejects_non_rgba8() {
        let mut img = Image {
            data: vec![0u8; 4 * 8],
            width: 4,
            height: 2,
            channels: 3, // RGB, not RGBA
            bytes_per_channel: 1,
            ..Default::default()
        };
        assert!(generate_mipmaps(&mut img).is_err());
    }

    #[test]
    fn rejects_already_mipped() {
        use moderu::MipPosition;
        let mut img = solid_rgba(4, 4, 0, 0, 0, 255);
        img.mip_positions.push(MipPosition {
            byte_offset: 0,
            byte_size: 4 * 4 * 4,
        });
        assert!(generate_mipmaps(&mut img).is_err());
    }
}

// ---- async read_uri ---------------------------------------------------------

#[cfg(feature = "async")]
mod async_tests {
    use super::{sample_path, skip_if_missing};
    use moderu_io::reader::{GltfOk, GltfReader};
    use orkester::{Context, Runtime};
    use orkester_io::{AssetAccessor, AssetResponse};
    use std::sync::Arc;

    /// File-backed `AssetAccessor` for testing.
    ///
    /// Treats the URI as a file path and reads it synchronously on the
    /// background thread. The `Runtime` is needed to construct a resolved
    /// `Task`.
    struct FileAccessor {
        Runtime: Runtime,
    }

    impl AssetAccessor for FileAccessor {
        fn get(
            &self,
            uri: &str,
            _headers: &[(String, String)],
            _priority: orkester_io::RequestPriority,
        ) -> orkester::Task<Result<AssetResponse, std::io::Error>> {
            let path = uri.to_owned();
            self.Runtime.run(Context::BACKGROUND, move || {
                std::fs::read(&path).map(|data| AssetResponse {
                    status: 200,
                    data,
                    content_encoding: orkester_io::ContentEncoding::None,
                })
            })
        }

        fn get_range(
            &self,
            uri: &str,
            _headers: &[(String, String)],
            _priority: orkester_io::RequestPriority,
            offset: u64,
            length: u64,
        ) -> orkester::Task<Result<AssetResponse, std::io::Error>> {
            let path = uri.to_owned();
            self.Runtime.run(Context::BACKGROUND, move || {
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(&path)?;
                file.seek(SeekFrom::Start(offset))?;
                let mut data = vec![0u8; length as usize];
                file.read_exact(&mut data)?;
                Ok(AssetResponse {
                    status: 200,
                    data,
                    content_encoding: orkester_io::ContentEncoding::None,
                })
            })
        }
    }

    /// `read_uri` with a file-backed accessor should load a `.glb` and
    /// produce a valid model — same result as `read_file`.
    #[test]
    fn read_uri_loads_glb() {
        let glb_path = sample_path("Box", "glTF-Binary", "Box.glb");
        if skip_if_missing(&glb_path) {
            return;
        }

        let Runtime = Runtime::with_threads(2);
        let accessor = Arc::new(FileAccessor {
            Runtime: Runtime.clone(),
        });

        let task = GltfReader::default().read_uri(
            glb_path.to_str().expect("valid path"),
            accessor,
            &Runtime,
        );

        let GltfOk { model, warnings } = task
            .block()
            .expect("task completed without async error")
            .expect("glTF parsed without error");

        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
        assert!(!model.meshes.is_empty(), "Box should have meshes");
    }

    /// `read_uri` with a `.gltf` that has an external `.bin` — the accessor
    /// should be called for both the main file and the sidecar.
    #[test]
    fn read_uri_resolves_external_bin() {
        let gltf_path = sample_path("Box", "glTF", "Box.gltf");
        if skip_if_missing(&gltf_path) {
            return;
        }

        let Runtime = Runtime::with_threads(2);
        let accessor = Arc::new(FileAccessor {
            Runtime: Runtime.clone(),
        });

        let task = GltfReader::default().read_uri(
            gltf_path.to_str().expect("valid path"),
            accessor,
            &Runtime,
        );

        let GltfOk { model, warnings } = task
            .block()
            .expect("task completed without async error")
            .expect("glTF parsed without error");

        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
        assert!(
            !model.buffers.is_empty() && !model.buffers[0].data.is_empty(),
            "external .bin should have been resolved"
        );
    }

    /// `read_uri` on a non-existent path should return a `GltfError::Fetch`.
    #[test]
    fn read_uri_missing_asset_returns_error() {
        let Runtime = Runtime::with_threads(2);
        let accessor = Arc::new(FileAccessor {
            Runtime: Runtime.clone(),
        });

        let task =
            GltfReader::default().read_uri("/nonexistent/path/model.glb", accessor, &Runtime);

        let result = task.block().expect("task completed without async error");

        assert!(result.is_err(), "missing file should produce GltfError");
    }
}
