# selekt

Format-agnostic tile selection engine for spatial data.

*selekt is for deciding what to render from hierarchical spatial datasets.*

## Overview

selekt is a runtime-agnostic selection and load-orchestration core for tiled spatial formats.
It handles LOD traversal, content loading, residency management, and async load scheduling —
with no dependency on any specific tile format (3D Tiles, I3S, etc).

**Core types:**

- **`SelectionEngine<C>`** — the main engine, generic over content type `C`
- **`SpatialHierarchy`** — trait for providing the spatial node tree
- **`ContentLoader<C>`** — trait for async content fetching and decoding
- **`LodEvaluator`** — trait for LOD metric evaluation
- **`ViewState`** — camera/view parameters for selection
- **`ViewUpdateResult`** — per-frame selection output (selected nodes, load requests)

## Design Principles

- **Format-agnostic**: no I3S, 3D Tiles, or glTF concepts in core
- **Content-generic**: `SelectionEngine<C>` stores loaded content of any type `C`
- **No GPU coupling**: selekt decides *what* to render, not *how* — GPU preparation lives in `belag`
- **Deterministic**: synchronous traversal and load orchestration with explicit main-thread pumping
- **Async-internal**: uses `orkester` futures internally, but the public API is synchronous

## Architecture

WIP

## License

Apache-2.0
