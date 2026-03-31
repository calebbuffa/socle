//! [`TilesetReader`] — deserialize a `tileset.json` byte payload into a
//! [`Tileset`] with structured errors and warnings.
//!
//! # Example
//!
//! ```no_run
//! use tiles3d::TilesetReader;
//!
//! let json = br#"{"asset":{"version":"1.1"},"geometricError":0,"root":{"boundingVolume":{"sphere":[0,0,0,1]},"geometricError":0}}"#;
//! let result = TilesetReader::read_from_slice(json);
//! if result.ok() {
//!     let tileset = result.tileset.unwrap();
//!     println!("{}", tileset.asset.version);
//! }
//! for w in &result.warnings {
//!     eprintln!("warning: {w}");
//! }
//! ```

use crate::generated::Tileset;

/// A single diagnostic message produced while reading or validating a tileset.
#[derive(Debug, Clone)]
pub struct ReadIssue(pub String);

impl std::fmt::Display for ReadIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The result of [`TilesetReader::read_from_slice`].
pub struct TilesetReadResult {
    /// The parsed tileset. `None` if a fatal parse error occurred.
    pub tileset: Option<Tileset>,
    /// Non-fatal warnings (e.g. unknown field, negative geometric error).
    pub warnings: Vec<ReadIssue>,
    /// Fatal errors that prevented successful parsing.
    pub errors: Vec<ReadIssue>,
}

impl TilesetReadResult {
    /// Returns `true` if parsing succeeded with no fatal errors.
    pub fn ok(&self) -> bool {
        self.errors.is_empty() && self.tileset.is_some()
    }
}

/// Parses 3D Tiles `tileset.json` payloads.
pub struct TilesetReader;

impl TilesetReader {
    /// Parse a tileset from a raw JSON byte slice.
    pub fn read_from_slice(data: &[u8]) -> TilesetReadResult {
        let mut warnings = Vec::new();
        match serde_json::from_slice::<Tileset>(data) {
            Ok(tileset) => {
                let errors = validate(&tileset, &mut warnings);
                TilesetReadResult {
                    tileset: Some(tileset),
                    warnings,
                    errors,
                }
            }
            Err(e) => TilesetReadResult {
                tileset: None,
                warnings,
                errors: vec![ReadIssue(e.to_string())],
            },
        }
    }

    /// Parse a tileset from a JSON string slice.
    pub fn read_from_str(s: &str) -> TilesetReadResult {
        Self::read_from_slice(s.as_bytes())
    }
}

/// Validate a successfully-parsed [`Tileset`].
///
/// Returns fatal errors; pushes non-fatal warnings into `warnings`.
fn validate(tileset: &Tileset, warnings: &mut Vec<ReadIssue>) -> Vec<ReadIssue> {
    let mut errors = Vec::new();

    if tileset.asset.version.is_empty() {
        errors.push(ReadIssue(
            "asset.version is required and must not be empty".into(),
        ));
    }

    if tileset.geometric_error < 0.0 {
        warnings.push(ReadIssue(format!(
            "geometricError is negative ({}); expected >= 0",
            tileset.geometric_error
        )));
    }

    if tileset.root.geometric_error < 0.0 {
        warnings.push(ReadIssue(format!(
            "root.geometricError is negative ({}); expected >= 0",
            tileset.root.geometric_error
        )));
    }

    // extensionsRequired must be a subset of extensionsUsed.
    for ext in &tileset.extensions_required {
        if !tileset.extensions_used.contains(ext) {
            warnings.push(ReadIssue(format!(
                "extensionsRequired contains '{}' which is not listed in extensionsUsed",
                ext
            )));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_json() -> &'static [u8] {
        br#"{
            "asset": { "version": "1.1" },
            "geometricError": 100.0,
            "root": {
                "boundingVolume": { "sphere": [0, 0, 0, 1000] },
                "geometricError": 100.0,
                "refine": "ADD"
            }
        }"#
    }

    #[test]
    fn parses_minimal_tileset() {
        let r = TilesetReader::read_from_slice(minimal_json());
        assert!(r.ok(), "errors: {:?}", r.errors);
        let ts = r.tileset.unwrap();
        assert_eq!(ts.asset.version, "1.1");
        assert_eq!(ts.geometric_error, 100.0);
    }

    #[test]
    fn warns_on_negative_geometric_error() {
        let json = br#"{
            "asset": { "version": "1.1" },
            "geometricError": -1.0,
            "root": {
                "boundingVolume": { "sphere": [0, 0, 0, 1] },
                "geometricError": 0.0
            }
        }"#;
        let r = TilesetReader::read_from_slice(json);
        assert!(r.ok());
        assert!(
            r.warnings
                .iter()
                .any(|w| w.0.contains("geometricError is negative"))
        );
    }

    #[test]
    fn errors_on_invalid_json() {
        let r = TilesetReader::read_from_slice(b"not json");
        assert!(!r.ok());
        assert!(!r.errors.is_empty());
    }
}
