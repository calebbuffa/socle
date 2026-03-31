//! External buffer and image URI resolution for non-GLB (`.gltf`) assets.
//!
//! When a `.gltf` file references external files (e.g. `"buffer0.bin"` or
//! `"Duck.png"`), this module loads those files from the filesystem relative
//! to a provided base directory.

use super::error::{Warning, Warnings};
use moderu::{Buffer, BufferView, GltfModel};
use std::path::Path;

/// Resolve all external (non-`data:`) URIs in `model.buffers` and
/// `model.images` by loading them from the filesystem.
///
/// `base_path` is the directory that relative URIs are resolved against
/// (typically the directory containing the `.gltf` file).
///
/// Already-populated buffers (non-empty `data`) and images that already have a
/// `buffer_view` set are skipped so that calling this after GLB parsing is safe.
pub fn resolve_external_refs(model: &mut GltfModel, base_path: &Path, warnings: &mut Warnings) {
    resolve_buffers(model, base_path, warnings);
    resolve_images(model, base_path, warnings);
}

fn resolve_buffers(model: &mut GltfModel, base_path: &Path, warnings: &mut Warnings) {
    for (i, buffer) in model.buffers.iter_mut().enumerate() {
        // Skip if already populated (e.g. GLB BIN chunk) or no URI.
        if !buffer.data.is_empty() {
            continue;
        }
        let Some(uri) = buffer.uri.as_deref() else {
            continue;
        };
        // data: URIs are handled by the data-URL decode step.
        if uri.starts_with("data:") {
            continue;
        }

        let file_path = base_path.join(uri);
        match std::fs::read(&file_path) {
            Ok(data) => {
                buffer.byte_length = data.len();
                buffer.data = data;
            }
            Err(e) => {
                warnings.push(Warning(format!(
                    "buffer[{i}]: failed to load external URI '{}': {e}",
                    file_path.display()
                )));
            }
        }
    }
}

fn resolve_images(model: &mut GltfModel, base_path: &Path, warnings: &mut Warnings) {
    // Collect images to load: index + resolved path.
    // We cannot mutate model.images while we also push to model.buffers, so
    // collect first then apply.
    let mut to_load: Vec<(usize, std::path::PathBuf)> = Vec::new();

    for (i, image) in model.images.iter().enumerate() {
        // Already resolved — has a buffer_view or no external URI.
        if image.buffer_view.is_some() {
            continue;
        }
        let Some(uri) = image.uri.as_deref() else {
            continue;
        };
        if uri.starts_with("data:") {
            continue;
        }
        to_load.push((i, base_path.join(uri)));
    }

    for (img_idx, file_path) in to_load {
        match std::fs::read(&file_path) {
            Ok(data) => {
                let buf_idx = model.buffers.len();
                let bv_idx = model.buffer_views.len();
                let byte_len = data.len();

                model.buffers.push(Buffer {
                    data,
                    byte_length: byte_len,
                    ..Default::default()
                });
                model.buffer_views.push(BufferView {
                    buffer: buf_idx,
                    byte_length: byte_len,
                    ..Default::default()
                });
                model.images[img_idx].buffer_view = Some(bv_idx);
            }
            Err(e) => {
                warnings.push(Warning(format!(
                    "image[{img_idx}]: failed to load external URI '{}': {e}",
                    file_path.display()
                )));
            }
        }
    }
}
