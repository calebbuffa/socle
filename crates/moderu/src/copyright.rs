//! Copyright string parsing for glTF assets.

use crate::GltfModel;

/// Parse a semicolon-separated copyright field into individual credits.
///
/// Reads `model.asset.copyright`, splits on `';'`, trims whitespace, and
/// filters out empty segments.
pub fn parse_gltf_copyright(gltf: &GltfModel) -> Vec<&str> {
    gltf.asset
        .copyright
        .as_deref()
        .map(|s| {
            s.split(';')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Parse a bare semicolon-separated copyright string into individual credits.
///
/// Duplicates the `parseGltfCopyright(string_view)` overload in Cesium.
pub fn parse_copyright_string(s: &str) -> Vec<&str> {
    s.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}
