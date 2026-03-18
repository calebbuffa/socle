"""
Parse i3s-spec markdown type definitions into a JSON intermediate representation.

Outputs generated/i3s_spec.json.
"""

import json
import os
import re
from pathlib import Path

# Explicit mappings for types. Key = lowercase base name from filename.
MODULE_MAP_CMN = {
    # core
    "3dscenelayer": "core",
    "store": "core",
    "metadata": "core",
    "resource": "core",
    "serviceupdatetimestamp": "core",
    "slpk_hashtable": "core",
    # node
    "node": "node",
    "nodepage": "node",
    "nodepagedefinition": "node",
    "nodereference": "node",
    "3dnodeindexdocument": "node",
    "lodselection": "node",
    # geometry
    "geometry": "geometry",
    "geometryattribute": "geometry",
    "geometrybuffer": "geometry",
    "geometrycolor": "geometry",
    "geometrydefinition": "geometry",
    "geometryfacerange": "geometry",
    "geometryfeatureid": "geometry",
    "geometrynormal": "geometry",
    "geometryparams": "geometry",
    "geometryposition": "geometry",
    "geometryreferenceparams": "geometry",
    "geometryuv": "geometry",
    "geometryuvregion": "geometry",
    "mesh": "geometry",
    "meshattribute": "geometry",
    "meshgeometry": "geometry",
    "defaultgeometryschema": "geometry",
    "vertexattribute": "geometry",
    "singlecomponentparams": "geometry",
    "vestedgeometryparams": "geometry",
    "compressedattributes": "geometry",
    # material
    "materialdefinition": "material",
    "materialdefinitioninfo": "material",
    "materialdefinitions": "material",
    "materialparams": "material",
    "materialtexture": "material",
    "meshmaterial": "material",
    "pbrmetallicroughness": "material",
    "sharedresource": "material",
    "texture": "material",
    "texturedefinition": "material",
    "texturedefinitioninfo": "material",
    "texturesetdefinition": "material",
    "texturesetdefinitionformat": "material",
    "image": "material",
    # feature
    "field": "feature",
    "featuredata": "feature",
    "features": "feature",
    "featureattribute": "feature",
    "attributestorageinfo": "feature",
    "domain": "feature",
    "domaincodedvalue": "feature",
    "statisticsinfo": "feature",
    "stats": "feature",
    "statsinfo": "feature",
    "histogram": "feature",
    "value": "feature",
    "valuecount": "feature",
    "headerattribute": "feature",
    "headervalue": "feature",
    "rangeinfo": "feature",
    "timeinfo": "feature",
    # spatial
    "obb": "spatial",
    "spatialreference": "spatial",
    "fullextent": "spatial",
    "heightmodelinfo": "spatial",
    "elevationinfo": "spatial",
    # display
    "drawinginfo": "display",
    "popupinfo": "display",
    "cacheddrawinginfo": "display",
}

# Explicit Rust name overrides: base_name -> rust_name
RUST_NAME_OVERRIDES = {
    "3DSceneLayer": "SceneLayerInfo",
    "3DNodeIndexDocument": "NodeIndexDocument",
}

# Files to skip (not type definitions)
SKIP_PATTERNS = [
    re.compile(r"_ReadMe\.md$", re.IGNORECASE),
    re.compile(r"ECMA_ISO8601\.md$"),
]


def should_skip(filename: str) -> bool:
    for pat in SKIP_PATTERNS:
        if pat.search(filename):
            return True
    return False


def parse_profile(filename: str) -> str | None:
    """Extract profile from filename: foo.cmn.md -> cmn, foo.bld.md -> bld, etc."""
    m = re.search(r"\.(cmn|bld|psl|pcsl)\.md$", filename, re.IGNORECASE)
    return m.group(1).lower() if m else None


def parse_base_name(filename: str) -> str:
    """Extract base type name from filename: node.cmn.md -> node"""
    return re.sub(r"\.(cmn|bld|psl|pcsl)\.md$", "", filename, flags=re.IGNORECASE)


def get_module(base_name: str, profile: str) -> str:
    """Get the module name based on profile. Organize by profile, not by type category."""
    # All types are organized by their profile:
    # - cmn profile types go to cmn.rs
    # - bld profile types go to bld.rs
    # - psl profile types go to psl.rs
    # - pcsl profile types go to pcsl.rs
    return profile


def to_pascal_case(name: str) -> str:
    """Convert camelCase or snake_case name to PascalCase."""
    # Handle special leading digits: 3DSceneLayer -> ThreeDSceneLayer
    name = re.sub(r"^3D", "ThreeD", name)
    # Handle underscores: slpk_hashtable -> slpkHashtable first
    if "_" in name:
        parts = name.split("_")
        name = parts[0] + "".join(p.capitalize() for p in parts[1:])
    # Capitalize first letter
    return name[0].upper() + name[1:] if name else name


def parse_type_column(type_str: str) -> dict:
    """Parse the Type column from the properties table into a structured type descriptor."""
    type_str = type_str.strip()

    # Reference array: [typeName](file.md)[]
    m = re.match(r"\[([^\]]+)\]\(([^)]+)\)\[\]", type_str)
    if m:
        ref_name = m.group(1)
        ref_file = m.group(2)
        ref_profile = parse_profile(ref_file) or "cmn"
        ref_base = parse_base_name(os.path.basename(ref_file))
        return {
            "kind": "array",
            "element": {
                "kind": "reference",
                "name": ref_name,
                "base_name": ref_base,
                "profile": ref_profile,
            },
        }

    # Reference: [typeName](file.md)
    m = re.match(r"\[([^\]]+)\]\(([^)]+)\)$", type_str)
    if m:
        ref_name = m.group(1)
        ref_file = m.group(2)
        ref_profile = parse_profile(ref_file) or "cmn"
        ref_base = parse_base_name(os.path.basename(ref_file))
        return {
            "kind": "reference",
            "name": ref_name,
            "base_name": ref_base,
            "profile": ref_profile,
        }

    # Fixed-size array: number[3], number[4], etc.
    m = re.match(r"(number|integer|string)\[(\d+)\]$", type_str)
    if m:
        return {
            "kind": "fixed_array",
            "element_type": m.group(1),
            "size": int(m.group(2)),
        }

    # Dynamic array: string[], integer[], number[]
    m = re.match(r"(string|integer|number|boolean)\[\]$", type_str)
    if m:
        return {"kind": "array", "element": {"kind": "primitive", "type": m.group(1)}}

    # Primitive: string, integer, number, boolean
    if type_str in ("string", "integer", "number", "boolean"):
        return {"kind": "primitive", "type": type_str}

    # Fallback: treat as string
    return {"kind": "primitive", "type": "string"}


def extract_enum_values(description: str) -> list[str] | None:
    """Extract enum values from HTML <li> tags in description."""
    # Pattern: <li>`value`</li> or <li>`value`: description</li>
    matches = re.findall(r"<li>`([^`]+)`", description)
    return matches if matches else None


def is_deprecated(description: str) -> bool:
    """Detect if a description marks something as deprecated."""
    return bool(
        re.search(r"\*\*Deprecated", description)
        or re.search(r"(?i)\bdeprecated\s+(?:in|with|for)\s+\d+\.\d+", description)
        or re.search(r"(?i)\bdeprecated\.?\s*$", description)
    )


def parse_properties_table(content: str) -> list[dict]:
    """Parse the ### Properties markdown table."""
    props = []

    # Find the properties section
    props_match = re.search(r"### Properties\s*\n", content)
    if not props_match:
        return props

    # Find the table after ### Properties
    table_start = props_match.end()
    rest = content[table_start:]

    # Find header row and separator
    lines = rest.split("\n")
    table_lines = []
    found_header = False
    found_separator = False

    for line in lines:
        stripped = line.strip()
        if not stripped:
            if found_separator:
                break  # End of table
            continue
        if not found_separator and stripped.startswith("|") and "---" in stripped:
            found_separator = True
            continue
        if not found_separator and stripped.startswith("|"):
            # Header row (before the separator line)
            found_header = True  # noqa: F841
            continue
        if found_separator and stripped.startswith("|"):
            table_lines.append(stripped)
        elif found_separator and not stripped.startswith("|"):
            break  # End of table

    for line in table_lines:
        cells = [c.strip() for c in line.split("|")]
        # Remove leading/trailing empty cells from pipe-delimited split
        # e.g. "| a | b | c |".split("|") -> ["", " a ", " b ", " c ", ""]
        if cells and cells[0] == "":
            cells = cells[1:]
        if cells and cells[-1] == "":
            cells = cells[:-1]
        if len(cells) < 3:
            continue

        name_cell = cells[0].strip()
        type_cell = cells[1].strip()
        desc_cell = cells[2].strip() if len(cells) > 2 else ""

        # Check if required (bold)
        required = name_cell.startswith("**") and name_cell.endswith("**")
        name = name_cell.strip("*").strip()

        # Skip empty names
        if not name:
            continue

        # Parse the type
        type_info = parse_type_column(type_cell)

        # Extract enum values from description
        enum_values = extract_enum_values(desc_cell)

        clean_desc = re.sub(r"<[^>]+>", "", desc_cell).strip()
        prop_entry = {
            "name": name,
            "type": type_info,
            "required": required,
            "description": clean_desc,
            "enum_values": enum_values,
        }
        if is_deprecated(desc_cell):
            prop_entry["deprecated"] = True
        props.append(prop_entry)

    return props


def parse_type_name(content: str) -> str:
    """Extract type name from the # heading."""
    m = re.match(r"#\s+(.+?)(?:\s+\[.*\])?\s*$", content.split("\n")[0])
    if m:
        return m.group(1).strip()
    return ""


def parse_description(content: str) -> str:
    """Extract description between heading and ### Related or ### Properties."""
    lines = content.split("\n")
    desc_lines = []
    started = False
    for line in lines[1:]:  # Skip heading
        stripped = line.strip()
        if stripped.startswith("### ") or stripped.startswith("## "):
            break
        if stripped:
            started = True
        if started:
            desc_lines.append(stripped)
    return " ".join(desc_lines).strip()


def parse_examples(content: str) -> list[dict]:
    """Extract JSON examples from the file."""
    examples = []
    example_pattern = re.compile(
        r"####\s+Example:\s*(.+?)\s*\n.*?```json\s*\n(.*?)```",
        re.DOTALL,
    )
    for m in example_pattern.finditer(content):
        examples.append({"name": m.group(1).strip(), "json": m.group(2).strip()})
    return examples


def parse_md_file(filepath: Path) -> dict | None:
    """Parse a single markdown type definition file."""
    filename = filepath.name
    if should_skip(filename):
        return None

    profile = parse_profile(filename)
    if profile is None:
        return None

    base_name = parse_base_name(filename)
    content = filepath.read_text(encoding="utf-8", errors="replace")

    type_name = parse_type_name(content)
    if not type_name:
        return None

    properties = parse_properties_table(content)
    description = parse_description(content)
    examples = parse_examples(content)
    module = get_module(base_name, profile)

    # Apply explicit name overrides, else PascalCase
    rust_name = RUST_NAME_OVERRIDES.get(base_name, to_pascal_case(base_name))

    # Handle name collisions for pcsl types
    # PCSL profile types are pointcloud-specific, so always prefix with "PointCloud"
    # to distinguish from cmn versions (if they exist)
    if profile == "pcsl":
        rust_name = "PointCloud" + rust_name

    # Handle psl profile name differentiation
    # PSL and CMN both types with same name will be in different modules (psl vs cmn),
    # so we add Psl suffix to disambiguate when the type name appears in user code
    if profile == "psl":
        key = base_name.lower().replace("_", "")
        if key in MODULE_MAP_CMN:
            # Same type name exists in cmn profile, add suffix to distinguish
            rust_name = rust_name + "Psl"

    # Handle bld profile name differentiation
    # BLD types with same name as CMN types will be in different modules (bld vs cmn),
    # so we add Building prefix when the type name would be ambiguous
    if profile == "bld":
        key = base_name.lower().replace("_", "")
        if key in MODULE_MAP_CMN:
            # Same type name exists in cmn profile, add prefix to distinguish
            rust_name = "Building" + rust_name

    type_entry = {
        "name": type_name,
        "base_name": base_name,
        "rust_name": rust_name,
        "profile": profile,
        "module": module,
        "description": description,
        "properties": properties,
        "examples": examples,
        "source_file": str(filepath.relative_to(filepath.parent.parent.parent)),
    }
    if is_deprecated(description):
        type_entry["deprecated"] = True
    return type_entry


def main():
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    spec_dir = project_root / "docs" / "i3s-spec" / "docs"
    output_dir = project_root / "generated"

    # Auto-discover all version directories (e.g. 1.6, 1.7, ..., 1.10, 2.0, 2.1)
    # Sort newest-first so the latest definition of each type wins.
    version_dirs = [
        d for d in spec_dir.iterdir() if d.is_dir() and re.match(r"\d+\.\d+$", d.name)
    ]
    version_dirs.sort(
        key=lambda d: tuple(int(x) for x in d.name.split(".")),
        reverse=True,
    )

    types_by_module: dict[str, list[dict]] = {}
    seen_types: set[str] = set()  # (base_name, profile) dedup key

    for version_dir in version_dirs:
        for md_file in sorted(version_dir.glob("*.md")):
            parsed = parse_md_file(md_file)
            if parsed is None:
                continue
            dedup_key = f"{parsed['base_name'].lower()}:{parsed['profile']}"
            if dedup_key in seen_types:
                continue
            seen_types.add(dedup_key)
            module = parsed["module"]
            types_by_module.setdefault(module, []).append(parsed)

    # Build output
    versions_found = [d.name for d in version_dirs]
    output = {
        "version": "+".join(versions_found),
        "modules": {
            mod_name: {"types": types}
            for mod_name, types in sorted(types_by_module.items())
        },
    }

    # Write output
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / "i3s_spec.json"
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(output, f, indent=2, ensure_ascii=False)

    # Print summary
    total_types = sum(len(m["types"]) for m in output["modules"].values())
    print(f"Parsed {total_types} types across {len(output['modules'])} modules:")
    for mod_name, mod_data in sorted(output["modules"].items()):
        type_names = [t["rust_name"] for t in mod_data["types"]]
        print(f"  {mod_name}: {len(type_names)} types - {', '.join(type_names)}")

    print(f"\nOutput written to: {output_path}")
    return 0


if __name__ == "__main__":
    main()
