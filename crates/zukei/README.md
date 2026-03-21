# zukei

Low-level math and FFI foundation for the runtime stack.

*zukei is Japanese for "geometry" / "shape" — the mathematical primitives underlying everything else.*

## Overview

zukei owns the FFI-safe vector, matrix, and spatial bounds types used across the crate ecosystem.
It sits below traversal, selection, loading, and format adapters.

## Types

- `Vec2`, `Vec3`, `Vec4` — double-precision vectors
- `Mat3`, `Mat4` — double-precision matrices
- `SpatialBounds` — generic spatial bounds (OBB, MBS, etc.)

All types are `#[repr(C)]` for FFI safety.

## Features

**Default** — FFI-safe core types only, no external dependencies.

**`glam`** — enables the `zukei::glam` module with conversion impls to/from `glam` double-precision types.

```toml
[dependencies]
zukei = { version = "0.1", features = ["glam"] }
```

## License

Apache-2.0
