//! Integration tests for round-trip glTF write/read operations.

use moderu_io::reader::{GltfOk, GltfReader};
use moderu_io::writer::GltfWriter;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to construct path to test data directory.
fn test_data_path(model_name: &str, variant: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("moderu-reader/tests/data/glTF-Sample-Assets/Models")
        .join(model_name)
        .join(variant)
        .join(format!("{}.gltf", model_name))
}

#[test]
fn test_round_trip_box_gltf() {
    let path = test_data_path("Box", "glTF");
    if !path.exists() {
        eprintln!("Skipping test, test data not found: {:?}", path);
        return;
    }

    let reader = GltfReader::default();
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk {
        model: original, ..
    } = reader.parse(&data).expect("Errors loading Box.gltf");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path().join("box_roundtrip.gltf");

    let writer = GltfWriter::default();
    writer
        .write_json(&original, &temp_path)
        .expect("Failed to write JSON");

    let reloaded_data = fs::read(&temp_path).expect("Failed to read temp file");
    let GltfOk {
        model: reloaded, ..
    } = reader
        .parse(&reloaded_data)
        .expect("Errors reloading from JSON");

    assert_eq!(original.asset.version, reloaded.asset.version);
    assert_eq!(original.buffers.len(), reloaded.buffers.len());
    assert_eq!(original.buffer_views.len(), reloaded.buffer_views.len());
    assert_eq!(original.accessors.len(), reloaded.accessors.len());
    assert_eq!(original.meshes.len(), reloaded.meshes.len());
}

#[test]
fn test_round_trip_box_glb() {
    let path = test_data_path("Box", "glTF");
    if !path.exists() {
        eprintln!("Skipping test, test data not found: {:?}", path);
        return;
    }

    let reader = GltfReader::default();
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk {
        model: original, ..
    } = reader.parse(&data).expect("Errors loading Box.gltf");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path().join("box_roundtrip.glb");

    let writer = GltfWriter::default();
    writer
        .write_glb(&original, &temp_path)
        .expect("Failed to write GLB");

    assert!(temp_path.exists());
    let file_size = fs::metadata(&temp_path).unwrap().len();
    assert!(file_size > 100, "GLB file seems too small: {}", file_size);

    let reloaded_data = fs::read(&temp_path).expect("Failed to read temp GLB");
    let GltfOk {
        model: reloaded, ..
    } = reader
        .parse_glb(&reloaded_data)
        .expect("Errors reloading from GLB");

    assert_eq!(original.asset.version, reloaded.asset.version);
    assert_eq!(original.buffers.len(), reloaded.buffers.len());
    assert_eq!(original.buffer_views.len(), reloaded.buffer_views.len());
    assert_eq!(original.accessors.len(), reloaded.accessors.len());
    assert_eq!(original.meshes.len(), reloaded.meshes.len());
}

#[test]
fn test_round_trip_duck_gltf() {
    let path = test_data_path("Duck", "glTF");
    if !path.exists() {
        eprintln!("Skipping test, test data not found: {:?}", path);
        return;
    }

    let reader = GltfReader::default();
    let data = fs::read(&path).expect("Failed to read Duck.gltf");
    let GltfOk {
        model: original, ..
    } = reader.parse(&data).expect("Errors loading Duck.gltf");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path().join("duck_roundtrip.gltf");

    let writer = GltfWriter::default();
    writer
        .write_json(&original, &temp_path)
        .expect("Failed to write JSON");

    let reloaded_data = fs::read(&temp_path).expect("Failed to read temp file");
    let GltfOk {
        model: reloaded, ..
    } = reader
        .parse(&reloaded_data)
        .expect("Errors reloading from JSON");

    assert_eq!(original.asset.version, reloaded.asset.version);
    assert_eq!(original.meshes.len(), reloaded.meshes.len());
    assert_eq!(original.materials.len(), reloaded.materials.len());
}

#[test]
fn test_glb_buffer_structure() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path().join("test.glb");

    let model = moderu::GltfModel::default();
    let writer = GltfWriter::default();
    writer
        .write_glb_to_buffer(&model, &mut Vec::new())
        .expect("Failed to write GLB to buffer");

    // Also test writing to file
    writer
        .write_glb(&model, &temp_path)
        .expect("Failed to write GLB file");

    let data = fs::read(&temp_path).expect("Failed to read GLB file");

    // Check magic number
    assert!(data.len() >= 12, "GLB file too small");
    assert_eq!(
        u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        0x46546C67, // "glTF"
        "Invalid GLB magic"
    );

    // Check version
    assert_eq!(
        u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
        2,
        "Invalid GLB version"
    );

    // File length should match
    let declared_length = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
    assert_eq!(declared_length, data.len(), "GLB length mismatch");
}

#[test]
fn test_options_configuration() {
    let writer = moderu_io::writer::GltfWriter {
        options: moderu_io::writer::GltfWriterOptions {
            pretty_print: false,
            ..Default::default()
        },
    };

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path().join("test.gltf");

    let model = moderu::GltfModel::default();
    writer
        .write_json(&model, &temp_path)
        .expect("Failed to write JSON");

    let content = fs::read_to_string(&temp_path).expect("Failed to read file");
    assert!(!content.contains("  "));
}

#[test]
fn test_round_trip_with_draco_codec() {
    let path = test_data_path("Box", "glTF");
    if !path.exists() {
        eprintln!("Skipping test, test data not found: {:?}", path);
        return;
    }

    let reader = GltfReader::default();
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk {
        model: original, ..
    } = reader.parse(&data).expect("Errors loading Box.gltf");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path().join("box_draco.glb");

    let writer = GltfWriter::default();
    #[allow(unused_mut)]
    let mut writer = writer;
    #[cfg(feature = "draco")]
    {
        writer.options.draco = true;
    }
    #[cfg(feature = "meshopt")]
    {
        writer.options.meshopt = false;
    }

    writer
        .write_glb(&original, &temp_path)
        .expect("Failed to write GLB with Draco");

    assert!(temp_path.exists(), "GLB file was not created");
    let file_size = fs::metadata(&temp_path).unwrap().len();
    assert!(file_size > 100, "GLB file seems too small: {}", file_size);

    let reloaded_data = fs::read(&temp_path).expect("Failed to read temp GLB");
    let GltfOk {
        model: reloaded, ..
    } = reader
        .parse_glb(&reloaded_data)
        .expect("Errors reloading from GLB with codec");

    assert_eq!(original.asset.version, reloaded.asset.version);
    assert_eq!(original.meshes.len(), reloaded.meshes.len());
}

#[test]
fn test_round_trip_with_meshopt_codec() {
    let path = test_data_path("Box", "glTF");
    if !path.exists() {
        eprintln!("Skipping test, test data not found: {:?}", path);
        return;
    }

    let reader = GltfReader::default();
    let data = fs::read(&path).expect("Failed to read Box.gltf");
    let GltfOk {
        model: original, ..
    } = reader.parse(&data).expect("Errors loading Box.gltf");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path().join("box_meshopt.glb");

    let writer = GltfWriter::default();
    #[allow(unused_mut)]
    let mut writer = writer;
    #[cfg(feature = "draco")]
    {
        writer.options.draco = false;
    }
    #[cfg(feature = "meshopt")]
    {
        writer.options.meshopt = true;
    }

    writer
        .write_glb(&original, &temp_path)
        .expect("Failed to write GLB with Meshopt");

    assert!(temp_path.exists(), "GLB file was not created");
    let file_size = fs::metadata(&temp_path).unwrap().len();
    assert!(file_size > 100, "GLB file seems too small: {}", file_size);

    let reloaded_data = fs::read(&temp_path).expect("Failed to read temp GLB");
    let GltfOk {
        model: reloaded, ..
    } = reader
        .parse_glb(&reloaded_data)
        .expect("Errors reloading from GLB with Meshopt");

    assert_eq!(original.asset.version, reloaded.asset.version);
    assert_eq!(original.meshes.len(), reloaded.meshes.len());
}

#[test]
fn test_codec_options_struct() {
    // Verify options can be set directly via public fields on the writer.
    let mut writer = moderu_io::writer::GltfWriter::default();
    writer.options.pretty_print = true;
    #[cfg(feature = "draco")]
    {
        writer.options.draco = true;
    }
    #[cfg(feature = "meshopt")]
    {
        writer.options.meshopt = true;
    }
    let _ = writer;
}
