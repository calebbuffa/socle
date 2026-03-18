//! REST-based `AssetAccessor` using synchronous HTTP GET via [`ureq`].

use std::collections::HashMap;

use i3s_util::{I3sError, Result};

use crate::accessor::AssetAccessor;
use crate::request::{AssetRequest, AssetResponse, Headers};

/// Fetches I3S resources via synchronous HTTP using `ureq`.
pub struct RestAssetAccessor {
    agent: ureq::Agent,
}

impl Default for RestAssetAccessor {
    fn default() -> Self {
        Self::new()
    }
}

impl RestAssetAccessor {
    /// Create a new REST accessor with a default [`ureq::Agent`].
    pub fn new() -> Self {
        Self {
            agent: ureq::Agent::new_with_defaults(),
        }
    }

    /// Create a new REST accessor with a custom [`ureq::Agent`].
    pub fn with_agent(agent: ureq::Agent) -> Self {
        Self { agent }
    }
}

impl AssetAccessor for RestAssetAccessor {
    fn get(&self, uri: &str) -> Result<AssetRequest> {
        let mut resp = match self.agent.get(uri).call() {
            Ok(r) => r,
            Err(ureq::Error::StatusCode(code)) => {
                return Ok(AssetRequest {
                    method: "GET".into(),
                    uri: uri.to_string(),
                    request_headers: HashMap::new(),
                    response: AssetResponse {
                        status_code: code,
                        content_type: String::new(),
                        headers: HashMap::new(),
                        data: Vec::new(),
                    },
                });
            }
            Err(e) => return Err(I3sError::Network(e.to_string())),
        };

        let status_code = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let headers: Headers = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let data = resp
            .body_mut()
            .read_to_vec()
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
