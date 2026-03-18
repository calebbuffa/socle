//! Unified I3S asset accessor — handles both HTTP(S) and local SLPK archives.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flate2::read::GzDecoder;
use i3s_util::{I3sError, Result};
use zip::ZipArchive;

use crate::accessor::AssetAccessor;
use crate::request::{AssetRequest, AssetResponse};

type SlpkArchive = Arc<Mutex<ZipArchive<BufReader<File>>>>;

/// Unified asset accessor for I3S scene layers (HTTP and SLPK).
pub struct I3sAssetAccessor {
    http_agent: ureq::Agent,
    slpk_cache: Mutex<HashMap<PathBuf, SlpkArchive>>,
}

impl I3sAssetAccessor {
    /// Create a new accessor with default HTTP settings.
    pub fn new() -> Self {
        Self {
            http_agent: ureq::Agent::new_with_config(
                ureq::config::Config::builder()
                    .user_agent("i3s-native/0.1")
                    .build(),
            ),
            slpk_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Create an accessor with a custom [`ureq::Agent`] (e.g. with custom TLS or proxy).
    pub fn with_agent(agent: ureq::Agent) -> Self {
        Self {
            http_agent: agent,
            slpk_cache: Mutex::new(HashMap::new()),
        }
    }

    fn fetch_http(&self, uri: &str) -> Result<AssetRequest> {
        let response = self
            .http_agent
            .get(uri)
            .call()
            .map_err(|e| I3sError::Network(e.to_string()))?;

        let status_code = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let headers: HashMap<String, String> = response
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
            .collect();

        let data = response
            .into_body()
            .read_to_vec()
            .map_err(|e| I3sError::Network(e.to_string()))?;

        Ok(AssetRequest {
            method: "GET".into(),
            uri: uri.to_string(),
            request_headers: HashMap::new(),
            response: AssetResponse {
                status_code: status_code.as_u16(),
                content_type,
                headers,
                data,
            },
        })
    }

    // SLPK (ZIP archive) fetch

    /// Fetch an entry from an SLPK archive.
    ///
    /// SLPK URIs produced by [`SlpkUriResolver`](crate::resolver::SlpkUriResolver)
    /// are of the form `<archive_path>/<entry_path>`, e.g.:
    ///   `layers/0/3dSceneLayer.json`
    ///
    /// The archive itself is identified by the SLPK file path registered via
    /// [`register_slpk`](Self::register_slpk). The entry path is the remainder
    /// after the archive prefix is stripped.
    ///
    /// SLPK entries are individually gzip-compressed. We try the `.gz` suffixed
    /// name first, then the bare name.
    fn fetch_slpk(&self, slpk_path: &str, entry_path: &str) -> Result<AssetRequest> {
        let archive = self.get_or_open_archive(slpk_path)?;
        let mut guard = archive
            .lock()
            .map_err(|_| I3sError::InvalidData("SLPK archive lock poisoned".into()))?;

        let gz_name = format!("{entry_path}.gz");
        let gz_found = guard.by_name(&gz_name).is_ok();
        let name: &str = if gz_found { &gz_name } else { entry_path };

        match guard.by_name(name) {
            Ok(mut entry) => {
                let mut raw = Vec::new();
                entry.read_to_end(&mut raw).map_err(I3sError::Io)?;

                let data = if entry.name().ends_with(".gz") {
                    let mut dec = GzDecoder::new(&raw[..]);
                    let mut out = Vec::new();
                    dec.read_to_end(&mut out).map_err(I3sError::Io)?;
                    out
                } else {
                    raw
                };

                Ok(AssetRequest {
                    method: "GET".into(),
                    uri: format!("{slpk_path}/{entry_path}"),
                    request_headers: HashMap::new(),
                    response: AssetResponse {
                        status_code: 200,
                        content_type: guess_content_type(entry_path),
                        headers: HashMap::new(),
                        data,
                    },
                })
            }
            Err(_) => Ok(AssetRequest {
                method: "GET".into(),
                uri: format!("{slpk_path}/{entry_path}"),
                request_headers: HashMap::new(),
                response: AssetResponse {
                    status_code: 404,
                    content_type: String::new(),
                    headers: HashMap::new(),
                    data: Vec::new(),
                },
            }),
        }
    }

    fn get_or_open_archive(&self, path: &str) -> Result<SlpkArchive> {
        let key = PathBuf::from(path);
        {
            let cache = self
                .slpk_cache
                .lock()
                .map_err(|_| I3sError::InvalidData("SLPK cache lock poisoned".into()))?;
            if let Some(arc) = cache.get(&key) {
                return Ok(Arc::clone(arc));
            }
        }
        // Open outside the lock to avoid blocking other threads
        let archive = open_slpk(Path::new(path))?;
        let mut cache = self
            .slpk_cache
            .lock()
            .map_err(|_| I3sError::InvalidData("SLPK cache lock poisoned".into()))?;
        let arc = Arc::new(Mutex::new(archive));
        cache.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    /// Pre-open and cache an SLPK archive for use by [`SlpkUriResolver`].
    ///
    /// Optional — the accessor will open archives lazily on first access.
    /// Call this eagerly if you want to surface open errors before loading begins.
    pub fn register_slpk(&self, path: &Path) -> Result<()> {
        self.get_or_open_archive(&path.to_string_lossy())?;
        Ok(())
    }
}

impl Default for I3sAssetAccessor {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetAccessor for I3sAssetAccessor {
    /// Fetch a resource by URI.
    ///
    /// - `http://` / `https://` URIs are fetched via blocking HTTP (ureq).
    /// - All other URIs are treated as `<slpk_path>/<entry_path>` and read
    ///   from the corresponding ZIP archive.
    fn get(&self, uri: &str) -> Result<AssetRequest> {
        if uri.starts_with("http://") || uri.starts_with("https://") {
            self.fetch_http(uri)
        } else {
            // Split on first '/' to get archive path and entry path.
            // SlpkUriResolver produces URIs like "path/to/layer.slpk/layers/0/..."
            // but SceneLayer::open passes the slpk path separately via the resolver;
            // here the full URI is just the entry path relative to the archive root.
            // The SlpkUriResolver embeds the slpk path as the root of all URIs.
            match uri.find('/') {
                Some(sep) => self.fetch_slpk(&uri[..sep], &uri[sep + 1..]),
                None => self.fetch_slpk(uri, ""),
            }
        }
    }
}

fn open_slpk(path: &Path) -> Result<ZipArchive<BufReader<File>>> {
    let file = File::open(path).map_err(I3sError::Io)?;
    ZipArchive::new(BufReader::new(file))
        .map_err(|e| I3sError::InvalidData(format!("failed to read SLPK ZIP: {e}")))
}

fn guess_content_type(uri: &str) -> String {
    match uri.rsplit('.').next() {
        Some("json") => "application/json",
        Some("bin") => "application/octet-stream",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("ktx2") => "image/ktx2",
        Some("dds") => "image/vnd-ms.dds",
        _ => "application/octet-stream",
    }
    .to_string()
}
