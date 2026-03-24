use std::collections::HashMap;
use std::fmt::Write;

use heck::{ToSnakeCase, ToUpperCamelCase};

use crate::config::Config;
use crate::schema::{JsonSchema, SchemaCache};

/// A resolved Rust property (field) for a generated struct.
#[derive(Debug)]
pub struct RustProperty {
    /// The original JSON property name (e.g. "meshes").
    pub json_name: String,
    /// The Rust field name in snake_case (e.g. "meshes").
    pub rust_name: String,
    /// The Rust type expression (e.g. "Vec<Mesh>").
    pub rust_type: String,
    /// The serde default expression, if any.
    pub default_expr: Option<String>,
    /// Documentation string.
    pub doc: Option<String>,
    /// Whether this needs `#[serde(rename = "...")]`.
    pub needs_rename: bool,
    /// Whether to include `skip_serializing_if`.
    pub skip_serializing_if: Option<String>,
    /// Additional types discovered (schemas to also generate).
    pub discovered_schemas: Vec<DiscoveredSchema>,
}

#[derive(Debug)]
pub struct DiscoveredSchema {
    pub title: String,
    pub schema: JsonSchema,
    pub source_path: Option<std::path::PathBuf>,
}

/// A complete generated Rust struct definition.
#[derive(Debug)]
pub struct GeneratedStruct {
    /// The Rust struct name.
    pub name: String,
    /// The original schema title.
    pub title: String,
    /// Doc comment.
    pub doc: Option<String>,
    /// Extension name constant, if applicable.
    pub extension_name: Option<String>,
    /// Fields (including inherited ones, fully flattened).
    pub fields: Vec<RustProperty>,
}

/// Resolve the Rust type name for a schema, applying config overrides.
pub fn rust_type_name(config: &Config, title: &str) -> String {
    if let Some(class_config) = config.classes.get(title) {
        if let Some(ref override_name) = class_config.override_name {
            return override_name.clone();
        }
    }
    title.to_upper_camel_case()
}

/// Map a single JSON Schema property to a Rust field.
pub fn resolve_property(
    cache: &mut SchemaCache,
    config: &Config,
    parent_schema: &JsonSchema,
    _parent_name: &str,
    property_name: &str,
    property_json: &serde_json::Value,
    required: &[String],
) -> Option<RustProperty> {
    let details: JsonSchema = serde_json::from_value(property_json.clone()).ok()?;

    let is_required = required.contains(&property_name.to_string());
    let has_default = details.default.is_some();
    let make_optional = !is_required && !has_default;

    let snake_name = property_name.to_snake_case();
    let rust_name = make_safe_identifier(&snake_name);
    let needs_rename = rust_name != property_name;

    let doc = details
        .description
        .clone()
        .or_else(|| details.detailed_description.clone());

    // Resolve the type.
    let (rust_type, default_expr, skip_if, discovered) =
        resolve_type(cache, config, &details, make_optional, parent_schema)?;

    Some(RustProperty {
        json_name: property_name.to_string(),
        rust_name,
        rust_type,
        default_expr,
        doc,
        needs_rename,
        skip_serializing_if: skip_if,
        discovered_schemas: discovered,
    })
}

fn resolve_type(
    cache: &mut SchemaCache,
    config: &Config,
    details: &JsonSchema,
    make_optional: bool,
    parent_schema: &JsonSchema,
) -> Option<(
    String,
    Option<String>,
    Option<String>,
    Vec<DiscoveredSchema>,
)> {
    let mut discovered = Vec::new();

    // Check for enum first.
    if details.enum_values.is_some() || details.any_of.is_some() {
        // Enums — for now, just use the underlying primitive type.
        // Integer enums become i32, string enums become String.
        let base_type = match details.schema_type.as_deref() {
            Some("string") => "String",
            Some("integer") => "i32",
            _ => "serde_json::Value",
        };
        return if make_optional {
            Some((
                format!("Option<{base_type}>"),
                None,
                Some("Option::is_none".into()),
                discovered,
            ))
        } else {
            let default_expr = details
                .default
                .as_ref()
                .map(|d| format_default(base_type, d));
            Some((base_type.to_string(), default_expr, None, discovered))
        };
    }

    // Primitive types.
    match details.schema_type.as_deref() {
        Some("integer") => {
            let ty = if make_optional { "Option<i64>" } else { "i64" };
            let default_expr = if !make_optional {
                details.default.as_ref().map(|d| format_default("i64", d))
            } else {
                None
            };
            let skip_if = if make_optional {
                Some("Option::is_none".into())
            } else {
                None
            };
            return Some((ty.to_string(), default_expr, skip_if, discovered));
        }
        Some("number") => {
            let ty = if make_optional { "Option<f64>" } else { "f64" };
            let default_expr = if !make_optional {
                details.default.as_ref().map(|d| format_default("f64", d))
            } else {
                None
            };
            let skip_if = if make_optional {
                Some("Option::is_none".into())
            } else {
                None
            };
            return Some((ty.to_string(), default_expr, skip_if, discovered));
        }
        Some("boolean") => {
            let ty = if make_optional {
                "Option<bool>"
            } else {
                "bool"
            };
            let default_expr = if !make_optional {
                details.default.as_ref().map(|d| format_default("bool", d))
            } else {
                None
            };
            let skip_if = if make_optional {
                Some("Option::is_none".into())
            } else {
                None
            };
            return Some((ty.to_string(), default_expr, skip_if, discovered));
        }
        Some("string") => {
            let ty = if make_optional {
                "Option<String>"
            } else {
                "String"
            };
            let default_expr = if !make_optional {
                details
                    .default
                    .as_ref()
                    .map(|d| format_default("String", d))
            } else {
                None
            };
            let skip_if = if make_optional {
                Some("Option::is_none".into())
            } else {
                None
            };
            return Some((ty.to_string(), default_expr, skip_if, discovered));
        }
        Some("array") => {
            // Resolve item type.
            if let Some(items) = &details.items {
                let item_schema: JsonSchema = serde_json::from_value(items.as_ref().clone())
                    .unwrap_or(JsonSchema {
                        title: None,
                        description: None,
                        schema_type: Some("object".into()),
                        properties: HashMap::new(),
                        required: Vec::new(),
                        reference: None,
                        all_of: Vec::new(),
                        additional_properties: None,
                        items: None,
                        enum_values: None,
                        any_of: None,
                        detailed_description: None,
                        default: None,
                        minimum: None,
                        maximum: None,
                    });

                let (inner_type, _, _, inner_discovered) =
                    resolve_type(cache, config, &item_schema, false, parent_schema)?;
                discovered.extend(inner_discovered);

                let ty = format!("Vec<{inner_type}>");
                let skip_if = Some("Vec::is_empty".into());
                return Some((ty, None, skip_if, discovered));
            }
            return Some((
                "Vec<serde_json::Value>".to_string(),
                None,
                Some("Vec::is_empty".into()),
                discovered,
            ));
        }
        Some("object") if details.additional_properties.is_some() => {
            // Dictionary / map type.
            if let Some(ap) = &details.additional_properties {
                let ap_schema: JsonSchema =
                    serde_json::from_value(ap.clone()).unwrap_or(JsonSchema {
                        title: None,
                        description: None,
                        schema_type: Some("string".into()),
                        properties: HashMap::new(),
                        required: Vec::new(),
                        reference: None,
                        all_of: Vec::new(),
                        additional_properties: None,
                        items: None,
                        enum_values: None,
                        any_of: None,
                        detailed_description: None,
                        default: None,
                        minimum: None,
                        maximum: None,
                    });
                let (val_type, _, _, inner_discovered) =
                    resolve_type(cache, config, &ap_schema, false, parent_schema)?;
                discovered.extend(inner_discovered);

                let ty = format!("HashMap<String, {val_type}>");
                let skip_if = Some("HashMap::is_empty".into());
                return Some((ty, None, skip_if, discovered));
            }
        }
        _ => {}
    }

    // $ref — reference to another schema.
    if let Some(ref reference) = details.reference {
        if let Some((ref_schema, ref_path)) = cache.load_with_path(reference) {
            let title = ref_schema.title.as_deref().unwrap_or("Unknown");

            // glTF Id → Option<i32> (absent means "not set").
            if title == "glTF Id" {
                if make_optional {
                    return Some((
                        "Option<i32>".to_string(),
                        None,
                        Some("Option::is_none".into()),
                        discovered,
                    ));
                } else {
                    // Required glTF Ids are just i32.
                    return Some(("i32".to_string(), None, None, discovered));
                }
            }

            // If the referenced schema is a primitive, resolve it directly.
            if ref_schema
                .schema_type
                .as_deref()
                .is_some_and(|t| t != "object")
            {
                return resolve_type(cache, config, &ref_schema, make_optional, parent_schema);
            }

            let type_name = rust_type_name(config, title);

            // Skipped types (base classes whose properties are inlined).
            if config.classes.get(title).is_some_and(|c| c.skip) {
                // Don't emit as a field — properties are inlined by generate_struct.
                return None;
            }

            discovered.push(DiscoveredSchema {
                title: title.to_string(),
                schema: ref_schema.clone(),
                source_path: ref_path,
            });

            let ty = if make_optional {
                format!("Option<{type_name}>")
            } else {
                type_name
            };
            let skip_if = if make_optional {
                Some("Option::is_none".into())
            } else {
                None
            };
            return Some((ty, None, skip_if, discovered));
        }
    }

    // allOf with single entry — unwrap.
    if details.all_of.len() == 1 {
        if let Ok(inner) = serde_json::from_value::<JsonSchema>(details.all_of[0].clone()) {
            return resolve_type(cache, config, &inner, make_optional, parent_schema);
        }
    }

    // Fallback: serde_json::Value.
    if make_optional {
        Some((
            "Option<serde_json::Value>".to_string(),
            None,
            Some("Option::is_none".into()),
            discovered,
        ))
    } else {
        Some(("serde_json::Value".to_string(), None, None, discovered))
    }
}

fn format_default(rust_type: &str, value: &serde_json::Value) -> String {
    match rust_type {
        "i64" | "i32" => {
            if let Some(n) = value.as_i64() {
                return n.to_string();
            }
            if let Some(n) = value.as_f64() {
                return format!("{}", n as i64);
            }
            "0".to_string()
        }
        "f64" => {
            if let Some(n) = value.as_f64() {
                // Ensure it has a decimal point.
                let s = format!("{n}");
                if s.contains('.') { s } else { format!("{s}.0") }
            } else {
                "0.0".to_string()
            }
        }
        "bool" => value.as_bool().unwrap_or(false).to_string(),
        "String" => {
            if let Some(s) = value.as_str() {
                format!("\"{s}\".to_string()")
            } else {
                "String::new()".to_string()
            }
        }
        _ => "Default::default()".to_string(),
    }
}

/// Returns true if the given default expression is the same as `Default::default()`
/// for the given Rust type, meaning we can use plain `#[serde(default)]`.
fn is_type_default(default_expr: &str, rust_type: &str) -> bool {
    match rust_type {
        "i64" | "i32" => default_expr == "0",
        "f64" => default_expr == "0" || default_expr == "0.0",
        "bool" => default_expr == "false",
        "String" => default_expr == "String::new()",
        _ => default_expr == "Default::default()",
    }
}

fn make_safe_identifier(name: &str) -> String {
    let reserved = [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
        "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final",
        "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];
    if reserved.contains(&name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// Process a single schema and generate a Rust struct definition.
/// Properties from base schemas (allOf inheritance) are inlined — no
/// `#[serde(flatten)]`, no separate base struct.
pub fn generate_struct(
    cache: &mut SchemaCache,
    config: &Config,
    schema: &JsonSchema,
    source_path: Option<&std::path::Path>,
) -> Option<GeneratedStruct> {
    let title = schema.title.as_ref()?;
    let class_config = config.classes.get(title.as_str());

    // Skip types marked as skip (base classes, type aliases).
    if class_config.is_some_and(|c| c.skip) {
        return None;
    }

    let name = rust_type_name(config, title);

    if let Some(sp) = source_path {
        cache.push_context(sp);
    }

    // Collect all properties: own + inherited from allOf chain.
    let mut all_properties: HashMap<String, serde_json::Value> = HashMap::new();
    let mut all_required: Vec<String> = schema.required.clone();

    // Walk the allOf chain to collect inherited properties.
    collect_inherited_properties(cache, schema, &mut all_properties, &mut all_required);

    // Own properties override inherited ones, but skip empty objects
    // (used in JSON Schema to signal "this property exists" without redefining it).
    // Also skip `extensions` and `extras` — they're added by render_struct.
    for (k, v) in &schema.properties {
        if k == "extensions" || k == "extras" {
            continue;
        }
        if v.as_object().is_some_and(|o| o.is_empty()) {
            continue;
        }
        all_properties.insert(k.clone(), v.clone());
    }

    // Resolve properties into Rust fields.
    let mut fields = Vec::new();
    let mut prop_names: Vec<_> = all_properties.keys().cloned().collect();
    prop_names.sort();

    for prop_name in &prop_names {
        if let Some(prop_json) = all_properties.get(prop_name) {
            if let Some(field) = resolve_property(
                cache,
                config,
                schema,
                &name,
                prop_name,
                prop_json,
                &all_required,
            ) {
                fields.push(field);
            }
        }
    }

    if source_path.is_some() {
        cache.pop_context();
    }

    Some(GeneratedStruct {
        name,
        title: title.clone(),
        doc: schema.description.clone(),
        extension_name: class_config
            .and_then(|c| c.extension_name.clone())
            .or_else(|| config.find_extension_name(title).map(|s| s.to_string())),
        fields,
    })
}

/// Recursively collect properties from base schemas via allOf inheritance.
/// Walks up the chain: if a schema has `allOf: [{"$ref": "base.schema.json"}]`,
/// load the base schema and collect its properties (and its bases, recursively).
fn collect_inherited_properties(
    cache: &mut SchemaCache,
    schema: &JsonSchema,
    properties: &mut HashMap<String, serde_json::Value>,
    required: &mut Vec<String>,
) {
    for entry in &schema.all_of {
        // Each allOf entry can be a $ref or an inline schema with properties.
        if let Some(obj) = entry.as_object() {
            if let Some(ref_str) = obj.get("$ref").and_then(|v| v.as_str()) {
                if let Some(base_schema) = cache.load(ref_str) {
                    // Check if this base is a skipped type — if so, still
                    // collect its properties (that's the point of inlining).
                    // Recurse first to get the base's own bases.
                    collect_inherited_properties(cache, &base_schema, properties, required);

                    // Then collect the base's own properties.
                    for (k, v) in &base_schema.properties {
                        // Skip extensions/extras — added by render_struct.
                        if k == "extensions" || k == "extras" {
                            continue;
                        }
                        properties.insert(k.clone(), v.clone());
                    }
                    required.extend(base_schema.required.iter().cloned());
                }
            } else {
                // Inline schema in allOf — collect its properties directly.
                if let Ok(inline) = serde_json::from_value::<JsonSchema>(entry.clone()) {
                    for (k, v) in &inline.properties {
                        properties.insert(k.clone(), v.clone());
                    }
                    required.extend(inline.required.iter().cloned());
                }
            }
        }
    }
}

/// Render a `GeneratedStruct` to a Rust source string.
pub fn render_struct(s: &GeneratedStruct) -> String {
    let mut out = String::new();
    let mut default_fns: Vec<(String, String, String)> = Vec::new();

    // Doc comment.
    if let Some(ref doc) = s.doc {
        for line in doc.lines() {
            let _ = writeln!(out, "/// {line}");
        }
    }

    // Derive macros.
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]"
    );

    let _ = writeln!(out, "pub struct {} {{", s.name);

    // Fields.
    for field in &s.fields {
        let _ = writeln!(out);

        // Doc.
        if let Some(ref doc) = field.doc {
            for line in doc.lines() {
                let _ = writeln!(out, "    /// {line}");
            }
        }

        // Serde attributes.
        let mut attrs = Vec::new();

        if field.needs_rename {
            attrs.push(format!("rename = \"{}\"", field.json_name));
        }

        if let Some(ref skip_if) = field.skip_serializing_if {
            attrs.push(format!("skip_serializing_if = \"{skip_if}\""));
        }

        if let Some(ref default_expr) = field.default_expr {
            if is_type_default(default_expr, &field.rust_type) {
                attrs.push("default".to_string());
            } else {
                // Generate a named default function.
                let fn_name = format!("default_{}_{}", s.name.to_snake_case(), field.rust_name);
                default_fns.push((
                    fn_name.clone(),
                    field.rust_type.clone(),
                    default_expr.clone(),
                ));
                attrs.push(format!("default = \"{fn_name}\""));
            }
        } else if field.skip_serializing_if.is_some() {
            attrs.push("default".to_string());
        }

        if !attrs.is_empty() {
            let _ = writeln!(out, "    #[serde({})]", attrs.join(", "));
        }

        let _ = writeln!(out, "    pub {}: {},", field.rust_name, field.rust_type);
    }

    // Extensions + extras (all ExtensibleObject types get these).
    let _ = writeln!(out);
    let _ = writeln!(out, "    /// Extension-specific data.");
    let _ = writeln!(
        out,
        "    #[serde(default, skip_serializing_if = \"HashMap::is_empty\")]"
    );
    let _ = writeln!(
        out,
        "    pub extensions: HashMap<String, serde_json::Value>,"
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "    /// Application-specific data.");
    let _ = writeln!(
        out,
        "    #[serde(default, skip_serializing_if = \"Option::is_none\")]"
    );
    let _ = writeln!(out, "    pub extras: Option<serde_json::Value>,");

    let _ = writeln!(out, "}}");

    // Impl block for constants.
    if let Some(ref ext_name) = s.extension_name {
        let _ = writeln!(out);
        let _ = writeln!(out, "impl {} {{", s.name);
        let _ = writeln!(out, "    pub const EXTENSION_NAME: &str = \"{ext_name}\";");
        let _ = writeln!(out, "}}");
    }

    // Default helper functions.
    for (fn_name, rust_type, default_expr) in &default_fns {
        let _ = writeln!(out);
        let _ = writeln!(out, "fn {fn_name}() -> {rust_type} {{ {default_expr} }}");
    }

    out
}

/// Generate the complete Rust source file for a set of structs.
pub fn render_module(
    module_doc: &str,
    structs: &[GeneratedStruct],
    extra_imports: &[&str],
) -> String {
    let mut out = String::new();

    let _ = writeln!(out, "// This file was generated by schema-gen.");
    let _ = writeln!(out, "// DO NOT EDIT THIS FILE!");
    let _ = writeln!(out);
    let _ = writeln!(out, "//! {module_doc}");
    let _ = writeln!(out);
    let _ = writeln!(out, "#![allow(clippy::all)]");
    let _ = writeln!(out);
    let _ = writeln!(out, "use std::collections::HashMap;");
    let _ = writeln!(out, "use serde::{{Serialize, Deserialize}};");

    for imp in extra_imports {
        let _ = writeln!(out, "use {imp};");
    }

    let _ = writeln!(out);

    for s in structs {
        let _ = writeln!(out, "{}", render_struct(s));
    }

    out
}
