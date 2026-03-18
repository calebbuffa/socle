//! Building scene layer — a composite of sublayer `SceneLayer`s.
//!
//! A Building scene layer (BLD profile) is not a single node tree. Instead it
//! contains a hierarchy of **sublayers**, each of which is either:
//! - A **group** (container for other sublayers)
//! - A **3DObject** sublayer (has its own node tree, geometry, etc.)
//! - A **Point** sublayer (has its own node tree)
//!
//! This module wraps the building layer document and manages the collection of
//! sublayer `SceneLayer`s. Each visible 3DObject/Point sublayer gets its own
//! `SceneLayer::open` call with a URI resolver scoped to that sublayer.
//!
//! ## I3S Building Layer URL structure
//!
//! ```text
//! .../SceneServer/layers/0              ← BuildingLayer JSON
//! .../SceneServer/layers/0/sublayers/1  ← Sublayer JSON (3DObject)
//! .../SceneServer/layers/0/sublayers/1/nodepages/0
//! .../SceneServer/layers/0/sublayers/1/nodes/0/geometries/0
//! ```

use std::sync::Arc;

use i3s::bld::{Layer as BuildingLayer, Sublayer, SublayerLayerType};

use i3s_async::{AssetAccessor, ResourceUriResolver};
use i3s_geospatial::crs::CrsTransform;
use i3s_reader::json::read_json;
use i3s_util::Result;

use crate::externals::SceneLayerExternals;
use crate::options::SelectionOptions;
use crate::scene_layer::SceneLayer;
use crate::update_result::ViewUpdateResult;
use crate::view_state::ViewState;

/// A URI resolver for a building sublayer, relative to the parent layer.
struct SublayerUriResolver {
    inner: Arc<dyn ResourceUriResolver>,
    sublayer_id: i64,
}

impl ResourceUriResolver for SublayerUriResolver {
    fn layer_uri(&self) -> String {
        format!("{}/sublayers/{}", self.inner.layer_uri(), self.sublayer_id)
    }

    fn node_page_uri(&self, page_id: u32) -> String {
        format!(
            "{}/sublayers/{}/nodepages/{page_id}",
            self.inner.layer_uri(),
            self.sublayer_id
        )
    }

    fn geometry_uri(&self, node_id: u32, geometry_id: u32) -> String {
        format!(
            "{}/sublayers/{}/nodes/{node_id}/geometries/{geometry_id}",
            self.inner.layer_uri(),
            self.sublayer_id
        )
    }

    fn texture_uri(
        &self,
        node_id: u32,
        texture_id: u32,
        format: i3s_async::TextureRequestFormat,
    ) -> String {
        let _ = format;
        format!(
            "{}/sublayers/{}/nodes/{node_id}/textures/{texture_id}",
            self.inner.layer_uri(),
            self.sublayer_id
        )
    }

    fn attribute_uri(&self, node_id: u32, attribute_id: u32) -> String {
        format!(
            "{}/sublayers/{}/nodes/{node_id}/attributes/f_{attribute_id}/0",
            self.inner.layer_uri(),
            self.sublayer_id
        )
    }

    fn statistics_uri(&self, attribute_id: u32) -> String {
        format!(
            "{}/sublayers/{}/statistics/f_{attribute_id}/0",
            self.inner.layer_uri(),
            self.sublayer_id
        )
    }
}

/// Metadata for a single sublayer within the building hierarchy.
#[derive(Debug)]
pub struct SublayerEntry {
    /// The sublayer's ID within the building layer.
    pub sublayer_id: i64,
    /// The sublayer's display name.
    pub name: String,
    /// Whether this sublayer is a group (container) or a leaf (has geometry).
    pub is_group: bool,
    /// IDs of child sublayers (for groups).
    pub child_ids: Vec<i64>,
    /// Whether this sublayer is visible by default.
    pub visible: bool,
    /// Whether this sublayer has been opened as a SceneLayer.
    pub opened: bool,
}

/// A Building scene layer.
///
/// Manages the building layer document and its flattened sublayer hierarchy.
/// Each non-empty 3DObject/Point sublayer can be independently opened,
/// selected, and loaded.
pub struct BuildingSceneLayer<A: AssetAccessor + 'static> {
    /// The building layer info document.
    pub info: BuildingLayer,
    /// The accessor for fetching data.
    accessor: Arc<A>,
    /// The base URI resolver for the building layer.
    resolver: Arc<dyn ResourceUriResolver>,
    /// External dependencies.
    externals: SceneLayerExternals,
    /// Selection options (shared with sublayers).
    options: SelectionOptions,
    /// CRS transform for local-to-ECEF conversion.
    crs_transform: Option<Arc<dyn CrsTransform>>,
    /// Flattened sublayer entries (including groups).
    sublayer_entries: Vec<SublayerEntry>,
    /// Opened sublayer SceneLayers, keyed by sublayer_id position in entries.
    sublayers: Vec<Option<SceneLayer>>,
}

impl<A: AssetAccessor + 'static> BuildingSceneLayer<A> {
    /// Open a building scene layer from the layer JSON document.
    ///
    /// This fetches and parses the building layer document, then catalogs
    /// all sublayers. Sublayer SceneLayers are **not** opened eagerly —
    /// call [`open_sublayer`](Self::open_sublayer) or
    /// [`open_visible_sublayers`](Self::open_visible_sublayers).
    pub async fn open(
        accessor: A,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
    ) -> Result<Self> {
        Self::open_with_transform(accessor, resolver, externals, options, None).await
    }

    /// Like [`open`](Self::open), but with a [`CrsTransform`] for local-to-ECEF conversion.
    ///
    /// See [`SceneLayer::open_with_transform`] for details.
    pub async fn open_with_transform(
        accessor: A,
        resolver: Arc<dyn ResourceUriResolver>,
        externals: SceneLayerExternals,
        options: SelectionOptions,
        crs_transform: Option<Arc<dyn CrsTransform>>,
    ) -> Result<Self> {
        let accessor = Arc::new(accessor);

        let layer_uri = resolver.layer_uri();
        let layer_bytes = accessor.get(&layer_uri)?.into_data()?;
        let info: BuildingLayer = read_json(&layer_bytes)?;

        let mut entries = Vec::new();
        flatten_sublayers(&info.sublayers, &mut entries);

        let sublayers: Vec<Option<SceneLayer>> = (0..entries.len()).map(|_| None).collect();

        Ok(Self {
            info,
            accessor,
            resolver,
            externals,
            options,
            crs_transform,
            sublayer_entries: entries,
            sublayers,
        })
    }

    /// Get the flattened list of sublayer entries.
    pub fn sublayer_entries(&self) -> &[SublayerEntry] {
        &self.sublayer_entries
    }

    /// Open a specific sublayer by its sublayer ID.
    ///
    /// This fetches the sublayer's layer document and first node page.
    /// Only non-group, non-empty sublayers can be opened.
    pub async fn open_sublayer(&mut self, sublayer_id: i64) -> Result<()> {
        let idx = self
            .sublayer_entries
            .iter()
            .position(|e| e.sublayer_id == sublayer_id)
            .ok_or_else(|| {
                i3s_util::I3SError::InvalidData(format!("sublayer {sublayer_id} not found"))
            })?;

        if self.sublayer_entries[idx].is_group {
            return Err(i3s_util::I3SError::InvalidData(
                "cannot open a group sublayer".into(),
            ));
        }

        if self.sublayers[idx].is_some() {
            return Ok(()); // already opened
        }

        let sub_resolver = Arc::new(SublayerUriResolver {
            inner: Arc::clone(&self.resolver),
            sublayer_id,
        });

        let layer = SceneLayer::open_shared(
            Arc::clone(&self.accessor) as Arc<dyn AssetAccessor>,
            sub_resolver as Arc<dyn ResourceUriResolver>,
            self.externals.clone(),
            self.options.clone(),
            self.crs_transform.clone(),
        );

        self.sublayers[idx] = Some(layer);
        self.sublayer_entries[idx].opened = true;
        Ok(())
    }

    /// Open all visible, non-empty, non-group sublayers.
    pub async fn open_visible_sublayers(&mut self) -> Result<()> {
        let ids: Vec<i64> = self
            .sublayer_entries
            .iter()
            .filter(|e| e.visible && !e.is_group)
            .map(|e| e.sublayer_id)
            .collect();

        for id in ids {
            self.open_sublayer(id).await?;
        }
        Ok(())
    }

    /// Run `update_view` on all opened sublayers and merge the results.
    pub fn update_view(&mut self, views: &[ViewState]) -> Vec<(i64, ViewUpdateResult)> {
        let mut results = Vec::new();
        for (idx, entry) in self.sublayer_entries.iter().enumerate() {
            if let Some(layer) = self.sublayers[idx].as_mut() {
                let result = layer.update_view(views);
                results.push((entry.sublayer_id, result));
            }
        }
        results
    }

    /// Run `load_nodes` on all opened sublayers with their paired results.
    pub fn load_nodes(&mut self, results: &[(i64, ViewUpdateResult)]) {
        for (sublayer_id, result) in results {
            if let Some(idx) = self
                .sublayer_entries
                .iter()
                .position(|e| e.sublayer_id == *sublayer_id)
            {
                if let Some(layer) = self.sublayers[idx].as_mut() {
                    layer.load_nodes(result);
                }
            }
        }
    }

    /// Access an opened sublayer's SceneLayer by sublayer ID.
    pub fn sublayer(&self, sublayer_id: i64) -> Option<&SceneLayer> {
        let idx = self
            .sublayer_entries
            .iter()
            .position(|e| e.sublayer_id == sublayer_id)?;
        self.sublayers[idx].as_ref()
    }

    /// Access an opened sublayer's SceneLayer mutably by sublayer ID.
    pub fn sublayer_mut(&mut self, sublayer_id: i64) -> Option<&mut SceneLayer> {
        let idx = self
            .sublayer_entries
            .iter()
            .position(|e| e.sublayer_id == sublayer_id)?;
        self.sublayers[idx].as_mut()
    }
}

/// Recursively flatten the sublayer tree into a list of entries.
fn flatten_sublayers(sublayers: &[Sublayer], out: &mut Vec<SublayerEntry>) {
    for sub in sublayers {
        let is_group = sub.layer_type == SublayerLayerType::Group;
        let visible = sub.visibility.unwrap_or(true);
        let is_empty = sub.is_empty.unwrap_or(false);

        let child_ids = sub
            .sublayers
            .as_ref()
            .map(|subs| subs.iter().map(|s| s.id).collect())
            .unwrap_or_default();

        // Skip truly empty sublayers (no geometry at all)
        if !is_group && is_empty {
            continue;
        }

        out.push(SublayerEntry {
            sublayer_id: sub.id,
            name: sub.name.clone(),
            is_group,
            child_ids,
            visible,
            opened: false,
        });

        if let Some(children) = &sub.sublayers {
            flatten_sublayers(children, out);
        }
    }
}
