use serde::Deserialize;
use std::collections::HashMap;

/// Generator configuration — equivalent to the Node.js `glTF.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Per-class overrides keyed by the schema `title`.
    #[serde(default)]
    pub classes: HashMap<String, ClassConfig>,

    /// List of extensions to include.
    #[serde(default)]
    pub extensions: Vec<ExtensionConfig>,
}

impl Config {
    /// Looks up the extension name for a schema title by checking whether the
    /// title contains any known extension name from the `extensions` list.
    pub fn find_extension_name(&self, title: &str) -> Option<&str> {
        self.extensions
            .iter()
            .find(|e| title.contains(&e.extension_name))
            .map(|e| e.extension_name.as_str())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassConfig {
    /// Rename the schema title to this Rust type name.
    pub override_name: Option<String>,

    /// If true, this type will NOT be generated — it is either a base class
    /// whose properties are inlined, or a type alias (like `serde_json::Value`).
    #[serde(default)]
    pub skip: bool,

    /// The official extension name string.
    pub extension_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionConfig {
    /// The official extension name (e.g. "KHR_draco_mesh_compression").
    pub extension_name: String,

    /// Relative path to the extension's JSON Schema file.
    /// If omitted, schema-gen will search `--extension-dir` for a directory
    /// matching the extension name and find `*.schema.json` files inside it.
    pub schema: Option<String>,

    /// Which glTF object(s) this extension attaches to.
    #[serde(default)]
    pub attach_to: Vec<String>,
}
