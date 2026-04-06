//! Copyright string parsing for glTF assets.

use crate::GltfModel;

impl GltfModel {
    /// Parse the `asset.copyright` field into individual semicolon-separated credits.
    ///
    /// Splits on `';'`, trims whitespace, and filters out empty segments.
    pub fn copyright(&self) -> Vec<&str> {
        self.asset
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
}
