# moderu

*moderu* is Japanese for "model" — a Rust crate for working with [glTF 2.0](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html) assets in memory.

## Features

- **Generated types** — auto-generated Rust structs covering the full glTF 2.0 JSON schema.
- **Accessor utilities** — type-safe views and iterators over buffer data (`AccessorView`, `resolve_accessor`, …).
- **Builder** — construct models programmatically via `GltfModelBuilder` without managing buffer and index bookkeeping by hand.
- **Scene graph** — traverse node hierarchies and compute world transforms (`SceneGraph`, `TransformCache`).
- **Merge** — combine two `GltfModel` values with automatic index remapping (`GltfModel::merge`).
- **Compaction** — collapse buffers and remove unused accessors, materials, and meshes (`GltfModel::compact`).
- **Metadata** — typed property-table and property-texture views for `EXT_structural_metadata`.
- **Extensions** — pluggable typed extension registry (`ExtensionRegistry`, `GltfExtension`).

## Quick start

```rust
use moderu::GltfModelBuilder;

let mut b = GltfModelBuilder::new();

let pos  = b.push_accessor(&positions_vec3);
let norm = b.push_accessor(&normals_vec3);
let idxs = b.push_indices(&indices_u32);

let prim = b.primitive()
    .indices(idxs)
    .attribute("POSITION", pos)
    .attribute("NORMAL",   norm)
    .build();

b.mesh().primitive(prim).build();
let model = b.finish();
```

## License

See the repository root `LICENSE` file.
