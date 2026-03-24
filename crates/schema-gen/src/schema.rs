use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A loaded JSON Schema, lightly parsed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonSchema {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(rename = "$ref")]
    pub reference: Option<String>,
    #[serde(default)]
    pub all_of: Vec<serde_json::Value>,
    #[serde(rename = "additionalProperties")]
    pub additional_properties: Option<serde_json::Value>,
    #[serde(default)]
    pub items: Option<Box<serde_json::Value>>,
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<serde_json::Value>>,
    #[serde(rename = "anyOf")]
    pub any_of: Option<Vec<serde_json::Value>>,
    /// The set of string values for gltfDetailedDescription, if any.
    #[serde(rename = "gltf_detailedDescription")]
    pub detailed_description: Option<String>,
    /// Default value for the property.
    pub default: Option<serde_json::Value>,
    /// Minimum value.
    pub minimum: Option<serde_json::Value>,
    /// Maximum value.
    pub maximum: Option<serde_json::Value>,
}

/// Caches loaded schemas and resolves `$ref` paths relative to the
/// current context.
pub struct SchemaCache {
    schema_paths: Vec<PathBuf>,
    extension_paths: Vec<PathBuf>,
    cache: HashMap<PathBuf, JsonSchema>,
    /// Raw JSON cache for fragment resolution.
    raw_cache: HashMap<PathBuf, serde_json::Value>,
    /// Stack of source directories for resolving relative `$ref`.
    context_stack: Vec<PathBuf>,
}

impl SchemaCache {
    pub fn new(schema_paths: Vec<PathBuf>, extension_paths: Vec<PathBuf>) -> Self {
        Self {
            schema_paths,
            extension_paths,
            cache: HashMap::new(),
            raw_cache: HashMap::new(),
            context_stack: Vec::new(),
        }
    }

    /// Load a schema by name, searching relative to the current context first,
    /// then the configured schema paths.
    pub fn load(&mut self, name: &str) -> Option<JsonSchema> {
        self.load_with_path(name).map(|(s, _)| s)
    }

    /// Like `load`, but also returns the canonical path the schema was loaded from.
    pub fn load_with_path(&mut self, name: &str) -> Option<(JsonSchema, Option<PathBuf>)> {
        // Strip fragment (e.g. #/definitions/foo)
        let (file_part, fragment) = split_fragment(name);

        if file_part.is_empty() {
            // Self-reference fragment (e.g. "#/definitions/foo").
            if let Some(frag) = &fragment {
                if let Some(ctx) = self.context_stack.last().cloned() {
                    for (path, raw) in &self.raw_cache {
                        if path.parent() == Some(ctx.as_path()) {
                            let schema = resolve_fragment_raw(raw, frag)?;
                            return Some((schema, Some(path.clone())));
                        }
                    }
                }
            }
            return None;
        }

        let mut search = Vec::new();

        // Relative to current context
        if let Some(ctx) = self.context_stack.last() {
            search.push(ctx.join(file_part));
        }

        // All schema paths
        for base in &self.schema_paths {
            search.push(base.join(file_part));
        }

        let (schema, canonical) = self.load_from_paths(&search)?;

        if let Some(frag) = fragment {
            if let Some(raw) = self.raw_cache.get(&canonical) {
                let resolved = resolve_fragment_raw(raw, &frag)?;
                Some((resolved, Some(canonical)))
            } else {
                None
            }
        } else {
            Some((schema, Some(canonical)))
        }
    }

    /// Load an extension schema by name, prioritizing paths containing the
    /// extension name.
    pub fn load_extension(
        &mut self,
        schema_name: &str,
        extension_name: &str,
    ) -> Option<JsonSchema> {
        let (file_part, fragment) = split_fragment(schema_name);

        let mut search: Vec<PathBuf> = self
            .extension_paths
            .iter()
            .map(|base| base.join(file_part))
            .collect();

        // Prioritize paths containing the extension name.
        search.sort_by(|a, b| {
            let a_has = a.to_string_lossy().contains(extension_name);
            let b_has = b.to_string_lossy().contains(extension_name);
            b_has.cmp(&a_has)
        });

        let (schema, canonical) = self.load_from_paths(&search)?;

        if let Some(frag) = fragment {
            if let Some(raw) = self.raw_cache.get(&canonical) {
                resolve_fragment_raw(raw, &frag)
            } else {
                None
            }
        } else {
            Some(schema)
        }
    }

    fn load_from_paths(&mut self, paths: &[PathBuf]) -> Option<(JsonSchema, PathBuf)> {
        for p in paths {
            let canonical = match std::fs::canonicalize(p) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some(cached) = self.cache.get(&canonical) {
                return Some((cached.clone(), canonical));
            }

            if let Ok(contents) = std::fs::read_to_string(&canonical) {
                if let Ok(schema) = serde_json::from_str::<JsonSchema>(&contents) {
                    // Also cache the raw JSON for fragment resolution.
                    if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&contents) {
                        self.raw_cache.insert(canonical.clone(), raw);
                    }
                    self.cache.insert(canonical.clone(), schema.clone());
                    return Some((schema, canonical));
                }
            }
        }
        None
    }

    pub fn push_context(&mut self, source_path: &Path) {
        if let Some(parent) = source_path.parent() {
            self.context_stack.push(parent.to_path_buf());
        }
    }

    pub fn pop_context(&mut self) {
        self.context_stack.pop();
    }
}

fn split_fragment(name: &str) -> (&str, Option<String>) {
    if let Some(idx) = name.find("#/") {
        let file_part = &name[..idx];
        let fragment = name[idx + 2..].to_string();
        // Empty file_part means self-reference (e.g. "#/definitions/foo").
        // Return empty string so the caller knows not to load a file.
        (file_part, Some(fragment))
    } else {
        (name, None)
    }
}

fn resolve_fragment(schema: &JsonSchema, fragment: &str) -> Option<JsonSchema> {
    let _ = (schema, fragment);
    None
}

fn resolve_fragment_raw(raw: &serde_json::Value, fragment: &str) -> Option<JsonSchema> {
    // Walk into the raw JSON to resolve paths like "definitions/numericValue".
    let mut current = raw;
    for part in fragment.split('/') {
        current = current.get(part)?;
    }
    serde_json::from_value(current.clone()).ok()
}
