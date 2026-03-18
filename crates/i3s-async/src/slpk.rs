//! SLPK (Scene Layer Package) `AssetAccessor` — reads from a local ZIP archive.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::{Arc, Mutex};

use flate2::read::GzDecoder;
use i3s_util::{I3sError, Result};
use zip::ZipArchive;

use crate::accessor::AssetAccessor;
use crate::request::{AssetRequest, AssetResponse};

/// Reads I3S resources from a local `.slpk` ZIP archive.
pub struct SlpkAssetAccessor {
    archive: Arc<Mutex<ZipArchive<BufReader<File>>>>,
}

impl SlpkAssetAccessor {
    /// Open an SLPK file.
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let archive = ZipArchive::new(reader)
            .map_err(|e| I3sError::InvalidData(format!("failed to read SLPK ZIP: {e}")))?;
        Ok(Self {
            archive: Arc::new(Mutex::new(archive)),
        })
    }
}

impl AssetAccessor for SlpkAssetAccessor {
    fn get(&self, uri: &str) -> Result<AssetRequest> {
        let mut guard = self
            .archive
            .lock()
            .map_err(|e| I3sError::InvalidData(format!("SLPK archive lock poisoned: {e}")))?;

        // Try gzip-compressed entry first (e.g. "3dSceneLayer.json.gz")
        let gz_name = format!("{uri}.gz");
        let gz_found = guard.by_name(&gz_name).is_ok();
        let name = if gz_found { gz_name.as_str() } else { uri };
        let result = guard.by_name(name);

        match result {
            Ok(mut entry) => {
                let mut raw = Vec::new();
                entry.read_to_end(&mut raw)?;
                let data = if entry.name().ends_with(".gz") {
                    let mut decoder = GzDecoder::new(&raw[..]);
                    let mut decompressed = Vec::new();
                    decoder.read_to_end(&mut decompressed)?;
                    decompressed
                } else {
                    raw
                };

                Ok(AssetRequest {
                    method: "GET".into(),
                    uri: uri.to_string(),
                    request_headers: HashMap::new(),
                    response: AssetResponse {
                        status_code: 200,
                        content_type: guess_content_type(uri),
                        headers: HashMap::new(),
                        data,
                    },
                })
            }
            Err(_) => Ok(AssetRequest {
                method: "GET".into(),
                uri: uri.to_string(),
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
