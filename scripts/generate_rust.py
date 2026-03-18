"""
Generate Rust source files from the i3s-spec JSON intermediate representation.

Reads generated/i3s_spec.json and writes crates/i3s/src/{module}.rs files.
"""

import json
import re
import sys
import textwrap
from pathlib import Path

WORD_RE = re.compile(r"[A-Z]+(?=[A-Z][a-z0-9]|$)|[A-Z]?[a-z]+|[0-9]+")

LEADING_DIGIT_WORDS = {
    "0": "Zero",
    "1": "One",
    "2": "Two",
    "3": "Three",
    "4": "Four",
    "5": "Five",
    "6": "Six",
    "7": "Seven",
    "8": "Eight",
    "9": "Nine",
}

PRIMITIVE_MAP = {
    "string": "String",
    "integer": "i64",
    "number": "f64",
    "boolean": "bool",
}

# Rust keywords that need to be escaped with r#
RUST_KEYWORDS = {
    "type",
    "ref",
    "mod",
    "self",
    "super",
    "crate",
    "enum",
    "struct",
    "fn",
    "let",
    "mut",
    "const",
    "static",
    "use",
    "as",
    "if",
    "else",
    "match",
    "loop",
    "while",
    "for",
    "in",
    "return",
    "break",
    "continue",
    "move",
    "box",
    "where",
    "impl",
    "trait",
    "pub",
    "async",
    "await",
    "dyn",
    "abstract",
    "become",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "try",
}


def to_snake_case(name: str) -> str:
    """Convert camelCase to snake_case."""
    # Insert underscore before uppercase letters
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", s)
    return s.lower()


def to_pascal_case(name: str) -> str:
    """Convert a name to PascalCase."""
    name = re.sub(r"^3D", "ThreeD", name)
    words = []
    for token in re.split(r"[^0-9A-Za-z]+", name):
        if not token:
            continue
        words.extend(WORD_RE.findall(token) or [token])

    if not words:
        return name

    result = "".join(
        word if len(word) > 1 and word.isupper() else word[0].upper() + word[1:]
        for word in words
    )

    if result and result[0].isdigit():
        result = LEADING_DIGIT_WORDS.get(result[0], "N") + result[1:]

    return result


def sanitize_enum_variant(value: str) -> str:
    """Convert an enum value string to a valid Rust PascalCase identifier."""
    # Handle esriFieldType* pattern
    if value.startswith("esriFieldType"):
        return value[len("esriFieldType") :]
    if value.startswith("esri"):
        return value[4:]

    result = to_pascal_case(value)

    # Handle special cases
    if result == "3DObject" or value == "3DObject":
        return "ThreeDObject"
    if result == "3dNodeIndexDocument" or value == "3dNodeIndexDocument":
        return "NodeIndexDocument"

    return result if result else "Unknown"


# Fields that need a serde(rename) because the JSON name is not valid snake_case
FIELD_RENAME_MAP = {
    "$ref": "ref_",
}

# Enum name overrides: (struct_rust_name, prop_name) -> shared enum name.
# Multiple structs can map to the same shared enum; it is generated once
# with merged variants from SHARED_ENUMS.
ENUM_NAME_OVERRIDES = {
    ("SceneLayerInfo", "layerType"): "SceneLayerType",
    ("SceneLayerInfoPsl", "layerType"): "SceneLayerType",
    ("SceneLayerInfo", "capabilities"): "SceneLayerCapabilities",
    ("SceneLayerInfoPsl", "capabilities"): "SceneLayerCapabilities",
    ("Layer", "layerType"): "SceneLayerType",
    ("Layer", "capabilities"): "SceneLayerCapabilities",
    ("PointCloudLayer", "layerType"): "SceneLayerType",
    ("PointCloudLayer", "capabilities"): "SceneLayerCapabilities",
    ("StorePsl", "resourcePattern"): "StoreResourcePattern",
    ("StorePsl", "normalReferenceFrame"): "StoreNormalReferenceFrame",
    ("StorePsl", "lodType"): "StoreLodType",
    ("StorePsl", "lodModel"): "StoreLodModel",
}

# Shared enums with their complete variant list (merged from all profiles).
SHARED_ENUMS = {
    "SceneLayerType": {
        "values": ["3DObject", "IntegratedMesh", "Point", "PointCloud", "Building"],
        "doc": "I3S scene layer type.",
    },
    "SceneLayerCapabilities": {
        "values": ["View", "Query", "Edit", "Extract"],
        "doc": "Capabilities supported by a scene layer.",
    },
}

# Shared enums are emitted in one owner module and imported elsewhere.
SHARED_ENUM_OWNER_MODULE = {
    "SceneLayerType": "cmn",
    "SceneLayerCapabilities": "cmn",
    "StoreResourcePattern": "cmn",
    "StoreNormalReferenceFrame": "cmn",
    "StoreLodType": "cmn",
    "StoreLodModel": "cmn",
}


def rust_field_name(name: str) -> str:
    """Convert a property name to a snake_case Rust field name."""
    if name in FIELD_RENAME_MAP:
        return FIELD_RENAME_MAP[name]
    snake = to_snake_case(name)
    # Handle Rust keywords
    if snake in RUST_KEYWORDS:
        return f"r#{snake}"
    return snake


def resolve_type(
    type_info: dict,
    parent_rust_name: str,
    prop_name: str,
    module_types: dict,
    current_module: str,
    enums_collector: list,
    imports_collector: set,
) -> str:
    """Resolve a type descriptor to a Rust type string."""
    kind = type_info["kind"]

    if kind == "primitive":
        return PRIMITIVE_MAP.get(type_info["type"], "String")

    if kind == "fixed_array":
        elem = PRIMITIVE_MAP.get(type_info["element_type"], "f64")
        return f"[{elem}; {type_info['size']}]"

    if kind == "array":
        elem = type_info["element"]
        inner = resolve_type(
            elem,
            parent_rust_name,
            prop_name,
            module_types,
            current_module,
            enums_collector,
            imports_collector,
        )
        return f"Vec<{inner}>"

    if kind == "reference":
        ref_base = type_info.get("base_name", type_info.get("name", ""))
        ref_profile = type_info.get("profile", "cmn")

        # Find the actual rust name and module for this reference
        ref_key = f"{ref_base.lower()}:{ref_profile}"
        if ref_key in module_types:
            ref_info = module_types[ref_key]
            ref_rust_name = ref_info["rust_name"]
            ref_module = ref_info["module"]
            if ref_module != current_module:
                imports_collector.add((ref_module, ref_rust_name))
            return ref_rust_name
        else:
            # Fallback: PascalCase the reference name
            rust_name = to_pascal_case(type_info.get("name", ref_base))
            return rust_name

    return "String"


def generate_enum(
    enum_name: str, values: list[str], parent_type: str, prop_name: str, doc: str = ""
) -> str:
    """Generate a Rust enum definition."""
    lines = []
    if doc:
        lines.append(f"/// {doc}")
    else:
        lines.append(f"/// Possible values for `{parent_type}::{prop_name}`.")
    lines.append("#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]")
    lines.append(f"pub enum {enum_name} {{")
    for val in values:
        variant = sanitize_enum_variant(val)
        # Only add rename if variant != value
        if variant != val:
            lines.append(f'    #[serde(rename = "{val}")]')
        lines.append(f"    {variant},")
    lines.append("}")
    lines.append("")
    # Default impl: first variant
    if values:
        first_variant = sanitize_enum_variant(values[0])
        lines.append(f"impl Default for {enum_name} {{")
        lines.append("    fn default() -> Self {")
        lines.append(f"        Self::{first_variant}")
        lines.append("    }")
        lines.append("}")
        lines.append("")
    return "\n".join(lines)


def generate_struct(
    type_def: dict, module_types: dict, current_module: str
) -> tuple[str, list[str], set]:
    """Generate Rust struct + any inline enums. Returns (code, enum_codes, imports)."""
    rust_name = type_def["rust_name"]
    properties = type_def["properties"]
    description = type_def.get("description", "")

    enum_codes = []
    imports: set[tuple[str, str]] = set()

    # Check for (identifier) map pattern: single property named "(identifier)"
    # means this type is actually a HashMap<String, ValueType>
    if len(properties) == 1 and properties[0]["name"] == "(identifier)":
        prop = properties[0]
        value_type = resolve_type(
            prop["type"],
            rust_name,
            prop["name"],
            module_types,
            current_module,
            enum_codes,
            imports,
        )
        lines = []
        if description:
            for desc_line in textwrap.wrap(description, width=95):
                lines.append(f"/// {desc_line}")
        if type_def.get("deprecated"):
            lines.append("#[deprecated]")
        lines.append(
            f"pub type {rust_name} = std::collections::HashMap<String, {value_type}>;"
        )
        lines.append("")
        return "\n".join(lines), enum_codes, imports

    lines = []

    # Doc comment
    if description:
        for desc_line in textwrap.wrap(description, width=95):
            lines.append(f"/// {desc_line}")

    # Deprecation attribute
    if type_def.get("deprecated"):
        lines.append("#[deprecated]")

    # Derives
    lines.append("#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]")
    lines.append('#[serde(rename_all = "camelCase")]')
    lines.append('#[cfg_attr(feature = "strict", serde(deny_unknown_fields))]')
    lines.append(f"pub struct {rust_name} {{")

    for prop in properties:
        prop_name = prop["name"]
        field_name = rust_field_name(prop_name)
        required = prop["required"]
        enum_values = prop.get("enum_values")
        type_info = prop["type"]

        # If this property has enum values, generate a dedicated enum
        if enum_values:
            override_key = (rust_name, prop_name)
            if override_key in ENUM_NAME_OVERRIDES:
                # Shared enum — generated at module level, not here
                rust_type = ENUM_NAME_OVERRIDES[override_key]
                owner_module = SHARED_ENUM_OWNER_MODULE.get(rust_type)
                if owner_module and owner_module != current_module:
                    imports.add((owner_module, rust_type))
            else:
                prop_pascal = to_pascal_case(prop_name)
                # Collapse stutter: if struct name ends with a prefix of the
                # property name, merge the overlap.
                # e.g. Layer + LayerType -> LayerType (not LayerLayerType)
                #      PointCloudValue + ValueType -> PointCloudValueType
                # Guard: if overlap swallows the entire prop name the enum
                # would collide with the struct itself, so skip collapse.
                overlap = 0
                for i in range(1, min(len(rust_name), len(prop_pascal)) + 1):
                    if rust_name.endswith(prop_pascal[:i]):
                        overlap = i
                if overlap >= len(prop_pascal):
                    overlap = 0
                enum_name = rust_name + prop_pascal[overlap:]
                enum_code = generate_enum(enum_name, enum_values, rust_name, prop_name)
                enum_codes.append(enum_code)
                rust_type = enum_name
        else:
            rust_type = resolve_type(
                type_info,
                rust_name,
                prop_name,
                module_types,
                current_module,
                enum_codes,
                imports,
            )

        # Add serde rename if JSON name doesn't match the Rust field name
        needs_rename = prop_name in FIELD_RENAME_MAP

        # Add doc comment for field
        desc = prop.get("description", "")
        if desc:
            short_desc = desc[:200] + "..." if len(desc) > 200 else desc
            lines.append(f"    /// {short_desc}")

        # Deprecation attribute for field
        if prop.get("deprecated"):
            lines.append("    #[deprecated]")

        # Handle optional vs required
        if required:
            if needs_rename:
                lines.append(f'    #[serde(rename = "{prop_name}")]')
            lines.append(f"    pub {field_name}: {rust_type},")
        else:
            if needs_rename:
                lines.append(
                    f'    #[serde(default, skip_serializing_if = "Option::is_none", rename = "{prop_name}")]'
                )
            else:
                lines.append(
                    '    #[serde(default, skip_serializing_if = "Option::is_none")]'
                )
            lines.append(f"    pub {field_name}: Option<{rust_type}>,")

    lines.append("}")
    lines.append("")

    return "\n".join(lines), enum_codes, imports


def generate_module(module_name: str, types: list[dict], module_types: dict) -> str:
    """Generate a complete Rust module file."""
    all_struct_code = []
    all_enum_code = []
    all_imports: set[tuple[str, str]] = set()

    # Determine which shared enums are needed in this module
    shared_needed: set[str] = set()
    for type_def in types:
        rn = type_def["rust_name"]
        for prop in type_def["properties"]:
            key = (rn, prop["name"])
            if key in ENUM_NAME_OVERRIDES:
                shared_needed.add(ENUM_NAME_OVERRIDES[key])

    for type_def in types:
        struct_code, enum_codes, imports = generate_struct(
            type_def, module_types, module_name
        )
        all_struct_code.append(struct_code)
        all_enum_code.extend(enum_codes)
        all_imports.update(imports)

    # Build the file
    lines = []
    lines.append("//! Auto-generated from i3s-spec. Do not edit manually.")
    lines.append("//!")
    lines.append(f"//! Module: {module_name}")
    lines.append("")
    lines.append("use serde::{Deserialize, Serialize};")
    lines.append("")

    # Cross-module imports
    import_lines = set()
    for mod, type_name in sorted(all_imports):
        if mod != module_name:
            import_lines.add(f"use crate::{mod}::{type_name};")

    if import_lines:
        for imp in sorted(import_lines):
            lines.append(imp)
        lines.append("")

    # Shared enums first (generated once with merged variants)
    for enum_name in sorted(shared_needed):
        if enum_name in SHARED_ENUMS:
            owner_module = SHARED_ENUM_OWNER_MODULE.get(enum_name, module_name)
            if owner_module != module_name:
                continue
            info = SHARED_ENUMS[enum_name]
            code = generate_enum(enum_name, info["values"], "", "", doc=info["doc"])
            lines.append(code)

    # Per-struct enums
    for enum_code in all_enum_code:
        lines.append(enum_code)

    # Then structs
    for struct_code in all_struct_code:
        lines.append(struct_code)

    return "\n".join(lines)


def generate_lib_rs(module_names: list[str], modules_data: dict) -> str:
    """Generate lib.rs with module declarations and re-exports."""
    lines = []
    lines.append("//! Auto-generated I3S type definitions.")
    lines.append("//!")
    lines.append(
        "//! This crate provides pure data types for the I3S (Indexed 3D Scene Layer)"
    )
    lines.append("//! specification, organized by feature domain.")
    lines.append("")

    for mod_name in sorted(module_names):
        lines.append(f"pub mod {mod_name};")

    lines.append("")

    return "\n".join(lines)


def build_module_types_index(modules_data: dict) -> dict:
    """Build a lookup index: (base_name:profile) -> {rust_name, module}."""
    index = {}
    for mod_name, mod_data in modules_data.items():
        for t in mod_data["types"]:
            key = f"{t['base_name'].lower()}:{t['profile']}"
            index[key] = {
                "rust_name": t["rust_name"],
                "module": mod_name,
            }
    return index


def main():
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    ir_path = project_root / "generated" / "i3s_spec.json"
    output_dir = project_root / "crates" / "i3s" / "src"

    if not ir_path.exists():
        print(f"ERROR: JSON IR not found at {ir_path}")
        print("Run parse_spec.py first.")
        return 1

    with open(ir_path, encoding="utf-8") as f:
        ir = json.load(f)

    modules_data = ir["modules"]
    module_types = build_module_types_index(modules_data)

    # Generate each module file
    module_names = []
    for mod_name, mod_data in sorted(modules_data.items()):
        types = mod_data["types"]
        if not types:
            continue

        code = generate_module(
            mod_name,
            types,
            module_types,
        )

        # Write the module file
        out_path = output_dir / f"{mod_name}.rs"
        out_path.parent.mkdir(parents=True, exist_ok=True)
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(code)
        print(f"  Generated {out_path.relative_to(project_root)} ({len(types)} types)")
        module_names.append(mod_name)

    # Generate lib.rs
    lib_code = generate_lib_rs(module_names, modules_data)
    lib_path = output_dir / "lib.rs"
    with open(lib_path, "w", encoding="utf-8") as f:
        f.write(lib_code)
    print(f"  Generated {lib_path.relative_to(project_root)}")

    total = sum(len(m["types"]) for m in modules_data.values())
    print(f"\nDone: {total} types across {len(module_names)} modules.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
