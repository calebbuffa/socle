//! REST-based `AssetAccessor` using HTTP GET.

use std::collections::HashMap;

use i3s_util::{I3sError, Result};
use reqwest::Client;

use crate::accessor::AssetAccessor;
use crate::request::{AssetRequest, AssetResponse, Headers};

/// Fetches resources via HTTP — the REST transport layer.
///
/// This accessor does not know about I3S-specific URI patterns.
/// Pair with [`RestUriResolver`](crate::resolver::RestUriResolver) for
/// I3S URI construction.
pub struct RestAssetAccessor {
    client: Client,
}

impl RestAssetAccessor {
    /// Create a new REST accessor with a default `reqwest::Client`.
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Create a new REST accessor with a custom `reqwest::Client`.
    pub fn with_client(client: Client) -> Self {
        Self { client }
    }
}

impl AssetAccessor for RestAssetAccessor {
    async fn get(&self, uri: &str) -> Result<AssetRequest> {
        let resp = self
            .client
            .get(uri)
            .send()
            .await
            .map_err(|e| I3sError::Network(e.to_string()))?;

        let status_code = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let headers: Headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let data = resp
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| I3sError::Network(e.to_string()))?;

        Ok(AssetRequest {
            method: "GET".into(),
            uri: uri.to_string(),
            request_headers: HashMap::new(),
            response: AssetResponse {
                status_code,
                content_type,
                headers,
                data,
            },
        })
    }
}
