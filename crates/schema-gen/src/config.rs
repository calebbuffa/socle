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

    /// Custom type definitions (enums, etc.) to generate at module level.
    #[serde(default, rename = "customTypes")]
    pub custom_types: HashMap<String, CustomTypeConfig>,

    /// Additional root schema files to process, relative to `--schema-dir`.
    /// These are seeded into the BFS queue alongside the main root schema.
    #[serde(default, rename = "additionalSchemas")]
    pub additional_schemas: Vec<String>,
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

    /// Per-property type overrides: property name -> Rust type string.
    /// Example: { "componentType": "u32", "mode": "u32" }
    #[serde(default)]
    pub property_overrides: HashMap<String, String>,

    /// Extra fields to inject into the generated struct.
    /// These are emitted with `#[serde(skip)]` just before `extensions`/`extras`.
    #[serde(default)]
    pub extra_fields: Vec<ExtraFieldConfig>,
}

/// A single extra field to inject into a generated struct.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraFieldConfig {
    /// Field name (Rust snake_case).
    pub name: String,
    /// Rust type expression, e.g. `"Vec<u8>"`.
    pub rust_type: String,
    /// Default expression used in `impl Default`. Defaults to `Default::default()`.
    #[serde(default)]
    pub default_expr: Option<String>,
    /// Doc comment (single line).
    #[serde(default)]
    pub doc: Option<String>,
    /// If true (default), emit `#[serde(skip)]` so the field is excluded from
    /// JSON (de)serialization. Set to false for fields that exist in real JSON
    /// payloads but are absent from the JSON Schema.
    #[serde(default = "default_true")]
    pub skip_serde: bool,
}

fn default_true() -> bool {
    true
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomTypeConfig {
    /// Type kind: "enum" for now.
    pub kind: String,

    /// For enums: list of variant names.
    #[serde(default)]
    pub variants: Vec<String>,

    /// Doc comment for the type.
    #[serde(default)]
    pub doc: Option<String>,

    /// For numeric enums: deserialize from u32 indices instead of string names.
    #[serde(default)]
    pub numeric: bool,

    /// For numeric enums: maps variant names to actual numeric values.
    /// If not provided, uses 0-based indices.
    #[serde(default)]
    pub numeric_values: std::collections::HashMap<String, u32>,
}
