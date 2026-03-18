//! URI resolution for I3S resources.

use crate::resource::TextureRequestFormat;

/// Maps I3S resource concepts to transport-specific URI strings.
pub trait ResourceUriResolver: Send + Sync {
    /// URI for the scene layer document (JSON).
    fn layer_uri(&self) -> String;

    /// URI for a node page by page ID (JSON).
    fn node_page_uri(&self, page_id: u32) -> String;

    /// URI for geometry data for a node.
    fn geometry_uri(&self, node_id: u32, geometry_id: u32) -> String;

    /// URI for a texture for a node in the requested format.
    fn texture_uri(&self, node_id: u32, texture_id: u32, format: TextureRequestFormat) -> String;

    /// URI for an attribute buffer for a node.
    fn attribute_uri(&self, node_id: u32, attribute_id: u32) -> String;

    /// URI for statistics for an attribute (JSON).
    fn statistics_uri(&self, attribute_id: u32) -> String;
}

/// URI resolver for I3S REST service endpoints.
pub struct RestUriResolver {
    base_uri: String,
}

impl RestUriResolver {
    pub fn new(base_uri: &str) -> Self {
        Self {
            base_uri: base_uri.trim_end_matches('/').to_string(),
        }
    }
}

impl ResourceUriResolver for RestUriResolver {
    fn layer_uri(&self) -> String {
        self.base_uri.clone()
    }

    fn node_page_uri(&self, page_id: u32) -> String {
        format!("{}/nodepages/{page_id}", self.base_uri)
    }

    fn geometry_uri(&self, node_id: u32, geometry_id: u32) -> String {
        format!("{}/nodes/{node_id}/geometries/{geometry_id}", self.base_uri)
    }

    fn texture_uri(&self, node_id: u32, texture_id: u32, _format: TextureRequestFormat) -> String {
        // REST endpoint negotiates format via Accept header
        format!("{}/nodes/{node_id}/textures/{texture_id}", self.base_uri)
    }

    fn attribute_uri(&self, node_id: u32, attribute_id: u32) -> String {
        format!(
            "{}/nodes/{node_id}/attributes/f_{attribute_id}/0",
            self.base_uri
        )
    }

    fn statistics_uri(&self, attribute_id: u32) -> String {
        format!("{}/statistics/f_{attribute_id}/0", self.base_uri)
    }
}

/// URI resolver for SLPK (Scene Layer Package) archive entry paths.
///
/// SLPK files are ZIP archives with a well-defined internal path structure.
/// Each entry may be individually gzip-compressed (with a `.gz` suffix) —
/// the [`SlpkAssetAccessor`](crate::slpk::SlpkAssetAccessor) handles
/// transparent decompression.
pub struct SlpkUriResolver;

impl ResourceUriResolver for SlpkUriResolver {
    fn layer_uri(&self) -> String {
        "3dSceneLayer.json".into()
    }

    fn node_page_uri(&self, page_id: u32) -> String {
        format!("nodepages/{page_id}.json")
    }

    fn geometry_uri(&self, node_id: u32, geometry_id: u32) -> String {
        format!("nodes/{node_id}/geometries/{geometry_id}.bin")
    }

    fn texture_uri(&self, node_id: u32, texture_id: u32, format: TextureRequestFormat) -> String {
        let ext = format.extension();
        format!("nodes/{node_id}/textures/{texture_id}.{ext}")
    }

    fn attribute_uri(&self, node_id: u32, attribute_id: u32) -> String {
        format!("nodes/{node_id}/attributes/f_{attribute_id}/0.bin")
    }

    fn statistics_uri(&self, attribute_id: u32) -> String {
        format!("statistics/f_{attribute_id}/0.json")
    }
}
