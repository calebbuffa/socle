//! Async external URI resolution using an [`orkester_io::AssetAccessor`].
//!
//! Mirrors [`super::external_refs`] but fetches resources via an asset accessor
//! instead of `std::fs`, supporting both file paths and HTTP/S URLs.

use super::error::{GltfError, Warning, Warnings};
use moderu::{Buffer, BufferView, GltfModel};
use orkester_io::AssetAccessor;
use std::sync::Arc;

/// Fetch raw bytes from `uri` through `accessor`.
///
/// The accessor is responsible for routing any URI scheme (file, http, https,
/// SLPK archive, etc.). Awaiting `Task<Result<AssetResponse, io::Error>>`
/// yields `Result<Result<..>, AsyncError>`, so we unwrap both layers.
pub async fn fetch_bytes(
    accessor: &Arc<dyn AssetAccessor>,
    uri: &str,
) -> Result<Vec<u8>, GltfError> {
    let inner = accessor
        .get(uri, &[])
        .await
        .map_err(|e| GltfError::Fetch(format!("task error fetching '{}': {}", uri, e)))?;

    let response =
        inner.map_err(|e| GltfError::Fetch(format!("I/O error fetching '{}': {}", uri, e)))?;

    if response.status < 200 || response.status >= 300 {
        return Err(GltfError::Fetch(format!(
            "HTTP {} for '{}'",
            response.status, uri
        )));
    }

    Ok(response.into_decompressed_data())
}

/// Resolve all external (non-`data:`) URIs in `model.buffers` and
/// `model.images` by fetching them through `accessor`.
///
/// `base_uri` is the URI of the `.gltf` file itself; relative URIs are
/// resolved against it using [`resolve_uri`].
pub async fn resolve_external_refs_async(
    model: &mut GltfModel,
    base_uri: &str,
    accessor: &Arc<dyn AssetAccessor>,
    warnings: &mut Warnings,
) {
    resolve_buffers_async(model, base_uri, accessor, warnings).await;
    resolve_images_async(model, base_uri, accessor, warnings).await;
}

async fn resolve_buffers_async(
    model: &mut GltfModel,
    base_uri: &str,
    accessor: &Arc<dyn AssetAccessor>,
    warnings: &mut Warnings,
) {
    for i in 0..model.buffers.len() {
        if !model.buffers[i].data.is_empty() {
            continue;
        }
        let rel = match model.buffers[i].uri.as_deref() {
            Some(u) if !u.starts_with("data:") => u.to_owned(),
            _ => continue,
        };

        let resolved = resolve_uri(base_uri, &rel);
        match fetch_bytes(accessor, &resolved).await {
            Ok(data) => {
                model.buffers[i].byte_length = data.len();
                model.buffers[i].data = data;
            }
            Err(e) => {
                warnings.push(Warning(format!("buffer[{}]: {}", i, e)));
            }
        }
    }
}

async fn resolve_images_async(
    model: &mut GltfModel,
    base_uri: &str,
    accessor: &Arc<dyn AssetAccessor>,
    warnings: &mut Warnings,
) {
    let to_load: Vec<(usize, String)> = model
        .images
        .iter()
        .enumerate()
        .filter_map(|(i, img)| {
            if img.buffer_view.is_some() {
                return None;
            }
            let rel = img.uri.as_deref()?;
            if rel.starts_with("data:") {
                return None;
            }
            Some((i, resolve_uri(base_uri, rel)))
        })
        .collect();

    for (img_idx, resolved) in to_load {
        match fetch_bytes(accessor, &resolved).await {
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
                warnings.push(Warning(format!("image[{}]: {}", img_idx, e)));
            }
        }
    }
}

/// Resolve a relative URI against a base URI.
///
/// Works for both HTTP(S) URLs and file paths (including Windows paths with
/// backslash separators):
/// - If `relative` already has a scheme (`://`) it is returned as-is.
/// - If `relative` starts with `/` or `\` it is returned as-is (root-relative).
/// - Otherwise the last path separator (`/` or `\`) of `base_uri` is found,
///   everything after it is stripped, and `relative` is appended.
///
/// ```text
/// resolve_uri("https://example.com/tiles/model.gltf", "buffer0.bin")
///   => "https://example.com/tiles/buffer0.bin"
///
/// resolve_uri("/data/tiles/model.gltf", "textures/tex.png")
///   => "/data/tiles/textures/tex.png"
///
/// resolve_uri(r"C:\tiles\model.gltf", "buffer0.bin")
///   => r"C:\tiles\buffer0.bin"
/// ```
pub fn resolve_uri(base_uri: &str, relative: &str) -> String {
    outil::resolve_url(base_uri, relative)
}

#[cfg(test)]
mod tests {
    use super::resolve_uri;

    #[test]
    fn test_resolve_relative_http() {
        assert_eq!(
            resolve_uri("https://example.com/tiles/model.gltf", "buffer0.bin"),
            "https://example.com/tiles/buffer0.bin"
        );
    }

    #[test]
    fn test_resolve_relative_file() {
        assert_eq!(
            resolve_uri("/data/tiles/model.gltf", "textures/tex.png"),
            "/data/tiles/textures/tex.png"
        );
    }

    #[test]
    fn test_resolve_relative_windows_path() {
        // Mixed separators as produced by PathBuf::to_str() on Windows.
        assert_eq!(
            resolve_uri(
                r"C:\Users\foo\tiles/data/glTF-Sample-Assets/Models\Box\glTF\Box.gltf",
                "Box0.bin"
            ),
            r"C:\Users\foo\tiles/data/glTF-Sample-Assets/Models\Box\glTF\Box0.bin"
        );
    }

    #[test]
    fn test_resolve_absolute_uri() {
        assert_eq!(
            resolve_uri(
                "https://example.com/tiles/model.gltf",
                "https://cdn.example.com/buf.bin"
            ),
            "https://cdn.example.com/buf.bin"
        );
    }

    #[test]
    fn test_resolve_root_relative() {
        assert_eq!(
            resolve_uri("https://example.com/tiles/model.gltf", "/other/tex.png"),
            "/other/tex.png"
        );
    }
}
