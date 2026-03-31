mod config;
mod generate;
mod schema;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use clap::Parser;

use config::Config;
use generate::{GeneratedStruct, generate_struct, render_module};
use schema::SchemaCache;

#[derive(Parser)]
#[command(name = "schema-gen", about = "Generate Rust structs from JSON Schema")]
struct Args {
    /// Path to the root JSON Schema file.
    #[arg(long)]
    schema: PathBuf,

    /// Configuration file (JSON) with class overrides and extensions.
    #[arg(long)]
    config: PathBuf,

    /// Output directory for generated Rust source files.
    #[arg(long)]
    output: PathBuf,

    /// Additional schema search directories.
    #[arg(long)]
    schema_dir: Vec<PathBuf>,

    /// Extension schema search directories.
    #[arg(long)]
    extension_dir: Vec<PathBuf>,

    /// Module documentation string.
    #[arg(long, default_value = "Generated types.")]
    module_doc: String,
}

/// Find schema files for an extension by searching extension directories
/// for a folder matching the extension name.
fn find_extension_schemas(extension_name: &str, extension_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut results = Vec::new();
    for ext_dir in extension_dirs {
        // Walk subdirectories looking for one that matches the extension name.
        // Layout: ext_dir/{Vendor,Khronos,...}/EXTENSION_NAME/schema/*.schema.json
        // or:     ext_dir/EXTENSION_NAME/schema/*.schema.json
        if let Ok(entries) = std::fs::read_dir(ext_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name == extension_name {
                        // Found the extension directory — look for schema files.
                        collect_schemas_in_dir(&path, &mut results);
                    } else {
                        // Could be a grouping dir (Vendor/, Khronos/); look one level deeper.
                        let nested = path.join(extension_name);
                        if nested.is_dir() {
                            collect_schemas_in_dir(&nested, &mut results);
                        }
                    }
                }
            }
        }
    }
    results
}

/// Collect all `*.schema.json` files under a directory (non-recursive, checks
/// both the directory itself and a `schema/` subdirectory).
fn collect_schemas_in_dir(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    for check_dir in [dir.to_path_buf(), dir.join("schema")] {
        if let Ok(entries) = std::fs::read_dir(&check_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(stem) = p.file_name().and_then(|n| n.to_str()) {
                        if stem.ends_with(".schema.json") {
                            out.push(p);
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let args = Args::parse();

    // Load config.
    let config_str = std::fs::read_to_string(&args.config)
        .unwrap_or_else(|e| panic!("Failed to read config {:?}: {e}", args.config));
    let config: Config =
        serde_json::from_str(&config_str).unwrap_or_else(|e| panic!("Failed to parse config: {e}"));

    // Build schema search paths from the root schema's directory.
    let mut schema_dirs = args.schema_dir.clone();
    if let Some(parent) = args.schema.parent() {
        schema_dirs.insert(0, parent.to_path_buf());
    }

    let mut cache = SchemaCache::new(schema_dirs.clone(), args.extension_dir.clone());

    // Load root schema.
    let root_schema_str = std::fs::read_to_string(&args.schema)
        .unwrap_or_else(|e| panic!("Failed to read root schema {:?}: {e}", args.schema));
    let root_schema: schema::JsonSchema = serde_json::from_str(&root_schema_str)
        .unwrap_or_else(|e| panic!("Failed to parse root schema: {e}"));

    cache.push_context(&args.schema);

    // BFS schema processing — same approach as the Node.js generator.
    let mut queue: VecDeque<(schema::JsonSchema, Option<PathBuf>)> = VecDeque::new();
    queue.push_back((root_schema, Some(args.schema.clone())));

    // Queue additional root schemas declared in the config.
    for rel_path in &config.additional_schemas {
        let mut found = false;
        for dir in &schema_dirs {
            let full_path = dir.join(rel_path);
            if full_path.exists() {
                if let Ok(s) = std::fs::read_to_string(&full_path) {
                    if let Ok(schema) = serde_json::from_str::<schema::JsonSchema>(&s) {
                        queue.push_back((schema, Some(full_path)));
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            eprintln!("Warning: additionalSchema not found: {rel_path}");
        }
    }

    // Queue extension schemas — either from explicit paths or auto-discovered.
    for ext in &config.extensions {
        let schema_paths: Vec<PathBuf> = if let Some(ref explicit) = ext.schema {
            // Explicit path — try each extension dir.
            args.extension_dir
                .iter()
                .map(|d| d.join(explicit))
                .filter(|p| p.exists())
                .collect()
        } else {
            // Auto-discover by extension name.
            find_extension_schemas(&ext.extension_name, &args.extension_dir)
        };

        if schema_paths.is_empty() {
            eprintln!(
                "Warning: no schema found for extension {}",
                ext.extension_name
            );
        }

        for ext_path in schema_paths {
            if let Ok(ext_str) = std::fs::read_to_string(&ext_path) {
                if let Ok(ext_schema) = serde_json::from_str::<schema::JsonSchema>(&ext_str) {
                    queue.push_back((ext_schema, Some(ext_path)));
                }
            }
        }
    }

    let mut generated: Vec<GeneratedStruct> = Vec::new();
    let mut seen: HashMap<String, bool> = HashMap::new();

    while let Some((schema, source_path)) = queue.pop_front() {
        let title = match schema.title {
            Some(ref t) => t.clone(),
            None => continue,
        };

        if seen.contains_key(&title) {
            continue;
        }
        seen.insert(title.clone(), true);

        if let Some(result) = generate_struct(&mut cache, &config, &schema, source_path.as_deref())
        {
            // Collect newly-discovered schemas from field references.
            for field in &result.fields {
                for discovered in &field.discovered_schemas {
                    if !seen.contains_key(&discovered.title) {
                        queue
                            .push_back((discovered.schema.clone(), discovered.source_path.clone()));
                    }
                }
            }

            eprintln!("Generated: {}", result.name);
            generated.push(result);
        }
    }

    cache.pop_context();

    // Sort structs alphabetically for stable output.
    generated.sort_by(|a, b| a.name.cmp(&b.name));

    // Render to a single module file.
    let source = render_module(&args.module_doc, &generated, &[], &config.custom_types);

    // Write output.
    std::fs::create_dir_all(&args.output)
        .unwrap_or_else(|e| panic!("Failed to create output dir {:?}: {e}", args.output));

    let output_file = args.output.join("generated.rs");
    std::fs::write(&output_file, &source)
        .unwrap_or_else(|e| panic!("Failed to write {:?}: {e}", output_file));

    eprintln!("Wrote {} structs to {:?}", generated.len(), output_file);
}
