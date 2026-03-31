//! Binary tile format decoders for all 3D Tiles 1.x container formats.
//!
//! Each `decode_*` function accepts raw file bytes and returns a [`GltfModel`]
//! that the GPU-upload phase can consume directly, implementing the same
//! conversion logic as cesium-native's `*ToGltfConverter` family.
//!
//! # Supported formats
//!
//! | Format | Magic | Notes |
//! |--------|-------|-------|
//! | GLB    | `glTF`| Passed through as-is. |
//! | b3dm   | `b3dm`| All three header variants (modern, legacy 1, legacy 2). RTC_CENTER injected as CESIUM_RTC. |
//! | i3dm   | `i3dm`| Embedded-GLB mode only (`gltfFormat == 1`). Instance transforms applied via `EXT_mesh_gpu_instancing`. |
//! | cmpt   | `cmpt`| Recursively decodes inner tiles and merges them. |
//! | pnts   | `pnts`| All position/colour/normal encodings; result is a POINTS primitive. |

use moderu::{AccessorType, GltfModel, GltfModelBuilder, Material, Node, PrimitiveMode, Scene};
use moderu_io::{GltfOk, GltfReader};
use serde_json::Value as Json;

const MAX_SANE_TILE_COUNT: usize = 65_536;

/// Decode any supported tile format from raw bytes.
///
/// Returns `None` for unsupported formats (pnts metadata-only, unrecognised),
/// or when the tile header is malformed.
pub fn decode_tile(data: &[u8], format: &crate::TileFormat) -> Option<GltfModel> {
    use crate::TileFormat::*;
    match format {
        Glb => decode_glb(data),
        B3dm => decode_b3dm(data),
        I3dm => decode_i3dm(data),
        Cmpt => decode_cmpt(data),
        Pnts => decode_pnts(data),
        Json | Unknown => None,
    }
}

fn decode_glb(data: &[u8]) -> Option<GltfModel> {
    let GltfOk { model, .. } = GltfReader::default().parse(data).ok()?;
    Some(model)
}

/// Threshold for detecting legacy b3dm header variants.
///
/// When the field that would be `batchTableJsonByteLength` in the modern header
/// (bytes `[20..24]`) reads ≥ this value, the bytes there are actually the
/// start of JSON or GLB data, indicating a Legacy-1 header.  The same check
/// on `[24..28]` identifies Legacy-2.  (Values &gt;= 0x22000000 are impossible
/// as byte-length fields for any real payload.)
const B3DM_LEGACY_THRESHOLD: usize = 0x2200_0000;

/// All parsings of a b3dm (modern, legacy-1, legacy-2) reduce to these offsets.
struct B3dmOffsets {
    /// Number of bytes before the feature-table JSON.
    header_len: usize,
    ft_json_len: usize,
    ft_bin_len: usize,
    bt_json_len: usize,
    bt_bin_len: usize,
}

impl B3dmOffsets {
    /// Parse from raw bytes, detecting all three header variants.
    fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 28 {
            return None;
        }
        let ft_json = le32(data, 12) as usize;
        let ft_bin = le32(data, 16) as usize;
        let bt_json = le32(data, 20) as usize;
        let bt_bin = le32(data, 24) as usize;

        if bt_json >= B3DM_LEGACY_THRESHOLD {
            // Legacy-1: 20-byte header — `ft_bin` holds batchTableByteLength.
            Some(Self {
                header_len: 20,
                ft_json_len: 0,
                ft_bin_len: 0,
                bt_json_len: ft_bin,
                bt_bin_len: 0,
            })
        } else if bt_bin >= B3DM_LEGACY_THRESHOLD {
            // Legacy-2: 24-byte header — `ft_json`/`ft_bin` are btJSON/btBin.
            Some(Self {
                header_len: 24,
                ft_json_len: 0,
                ft_bin_len: 0,
                bt_json_len: ft_json,
                bt_bin_len: ft_bin,
            })
        } else {
            // Modern 28-byte header.
            Some(Self {
                header_len: 28,
                ft_json_len: ft_json,
                ft_bin_len: ft_bin,
                bt_json_len: bt_json,
                bt_bin_len: bt_bin,
            })
        }
    }

    fn glb_start(&self) -> usize {
        self.header_len + self.ft_json_len + self.ft_bin_len + self.bt_json_len + self.bt_bin_len
    }

    fn ft_json_range(&self) -> std::ops::Range<usize> {
        self.header_len..self.header_len + self.ft_json_len
    }
}

fn decode_b3dm(data: &[u8]) -> Option<GltfModel> {
    let offsets = B3dmOffsets::parse(data)?;
    let glb_start = offsets.glb_start();
    let glb = data.get(glb_start..)?;

    let GltfOk { mut model, .. } = GltfReader::default().parse(glb).ok()?;

    // Inject RTC_CENTER from the feature-table JSON if present.
    if offsets.ft_json_len > 0 {
        let ft_json_bytes = data.get(offsets.ft_json_range())?;
        if let Ok(ft) = serde_json::from_slice::<Json>(ft_json_bytes) {
            if let Some(center) = parse_vec3(&ft, "RTC_CENTER") {
                model.extensions.insert(
                    "CESIUM_RTC".to_owned(),
                    serde_json::json!({ "center": center }),
                );
                model.extensions_used.push("CESIUM_RTC".to_owned());
            }
        }
    }

    Some(model)
}

fn i3dm_read_positions(ft: &Json, ft_bin: &[u8], count: usize) -> Option<Vec<[f32; 3]>> {
    if let Some(off) = parse_byte_offset(ft, "POSITION") {
        return Some(read_vec3f32(ft_bin, off, count));
    }
    if let Some(off) = parse_byte_offset(ft, "POSITION_QUANTIZED") {
        let vo = parse_vec3(ft, "QUANTIZED_VOLUME_OFFSET").unwrap_or([0.0; 3]);
        let vs = parse_vec3(ft, "QUANTIZED_VOLUME_SCALE").unwrap_or([1.0; 3]);
        return Some(read_quantized_vec3(ft_bin, off, count, vo, vs));
    }
    None
}

fn i3dm_read_rotations(
    ft: &Json,
    ft_bin: &[u8],
    positions: &[[f32; 3]],
    count: usize,
) -> Vec<[f32; 4]> {
    let east_north_up = ft
        .get("EAST_NORTH_UP")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    if east_north_up {
        return positions.iter().map(|&p| enu_quaternion(p)).collect();
    }
    if let (Some(up_off), Some(right_off)) = (
        parse_byte_offset(ft, "NORMAL_UP"),
        parse_byte_offset(ft, "NORMAL_RIGHT"),
    ) {
        let ups = read_vec3f32(ft_bin, up_off, count);
        let rights = read_vec3f32(ft_bin, right_off, count);
        return ups
            .iter()
            .zip(rights.iter())
            .map(|(u, r)| rotation_from_up_right(*u, *r))
            .collect();
    }
    if let (Some(up_off), Some(right_off)) = (
        parse_byte_offset(ft, "NORMAL_UP_OCT32P"),
        parse_byte_offset(ft, "NORMAL_RIGHT_OCT32P"),
    ) {
        let ups = read_oct_normals(ft_bin, up_off, count);
        let rights = read_oct_normals(ft_bin, right_off, count);
        return ups
            .iter()
            .zip(rights.iter())
            .map(|(u, r)| rotation_from_up_right(*u, *r))
            .collect();
    }
    vec![[0.0, 0.0, 0.0, 1.0]; count]
}

fn i3dm_read_scales(ft: &Json, ft_bin: &[u8], count: usize) -> Vec<[f32; 3]> {
    if let Some(off) = parse_byte_offset(ft, "SCALE_NON_UNIFORM") {
        return read_vec3f32(ft_bin, off, count);
    }
    if let Some(off) = parse_byte_offset(ft, "SCALE") {
        return (0..count)
            .map(|i| {
                let s = read_f32(ft_bin, off + i * 4).unwrap_or(1.0);
                [s, s, s]
            })
            .collect();
    }
    vec![[1.0, 1.0, 1.0]; count]
}

fn i3dm_apply_instancing(
    mut model: GltfModel,
    positions: Vec<[f32; 3]>,
    rotations: Vec<[f32; 4]>,
    scales: Vec<[f32; 3]>,
    ft: &Json,
) -> GltfModel {
    let mut inst_builder = GltfModelBuilder::new();
    let trans_flat: Vec<f32> = positions.iter().flat_map(|p| p.iter().copied()).collect();
    let rot_flat: Vec<f32> = rotations.iter().flat_map(|q| q.iter().copied()).collect();
    let scale_flat: Vec<f32> = scales.iter().flat_map(|s| s.iter().copied()).collect();
    let trans_acc = inst_builder.push_accessor(&trans_flat, AccessorType::Vec3);
    let rot_acc = inst_builder.push_accessor(&rot_flat, AccessorType::Vec4);
    let scale_acc = inst_builder.push_accessor(&scale_flat, AccessorType::Vec3);
    let inst_model = inst_builder.finish();
    let acc_base = model.accessors.len();
    model = model.merge(inst_model);
    let instancing_ext = serde_json::json!({
        "attributes": {
            "TRANSLATION": acc_base + trans_acc.0,
            "ROTATION":    acc_base + rot_acc.0,
            "SCALE":       acc_base + scale_acc.0,
        }
    });
    let target_node = model
        .nodes
        .iter()
        .position(|n| n.mesh.is_some())
        .or_else(|| {
            if model.nodes.is_empty() {
                None
            } else {
                Some(0)
            }
        });
    if let Some(idx) = target_node {
        model.nodes[idx]
            .extensions
            .insert("EXT_mesh_gpu_instancing".to_owned(), instancing_ext);
    }
    model
        .extensions_used
        .push("EXT_mesh_gpu_instancing".to_owned());
    model.extensions_used.sort();
    model.extensions_used.dedup();
    if let Some(center) = parse_vec3(ft, "RTC_CENTER") {
        model.extensions.insert(
            "CESIUM_RTC".to_owned(),
            serde_json::json!({ "center": center }),
        );
        model.extensions_used.push("CESIUM_RTC".to_owned());
        model.extensions_used.sort();
        model.extensions_used.dedup();
    }
    model
}

fn decode_i3dm(data: &[u8]) -> Option<GltfModel> {
    // 32-byte header (no legacy variants).
    if data.len() < 32 {
        return None;
    }
    if le32(data, 28) != 1 {
        return None;
    } // gltfFormat == 0: URI mode not supported.
    let ft_json_len = le32(data, 12) as usize;
    let ft_bin_len = le32(data, 16) as usize;
    let bt_json_len = le32(data, 20) as usize;
    let bt_bin_len = le32(data, 24) as usize;
    let ft_json_start = 32;
    let ft_bin_start = ft_json_start + ft_json_len;
    let glb_start = ft_bin_start + ft_bin_len + bt_json_len + bt_bin_len;
    let glb = data.get(glb_start..)?;
    let GltfOk { mut model, .. } = GltfReader::default().parse(glb).ok()?;
    let ft: Json = if ft_json_len > 0 {
        let bytes = data.get(ft_json_start..ft_json_start + ft_json_len)?;
        serde_json::from_slice(bytes).ok()?
    } else {
        Json::Object(Default::default())
    };
    let count = ft
        .get("INSTANCES_LENGTH")
        .and_then(Json::as_u64)
        .unwrap_or(0) as usize;
    if count == 0 {
        return Some(model);
    }
    let ft_bin = data
        .get(ft_bin_start..ft_bin_start + ft_bin_len)
        .unwrap_or(&[]);
    let Some(positions) = i3dm_read_positions(&ft, ft_bin, count) else {
        return Some(model);
    };
    let rotations = i3dm_read_rotations(&ft, ft_bin, &positions, count);
    let scales = i3dm_read_scales(&ft, ft_bin, count);
    model = i3dm_apply_instancing(model, positions, rotations, scales, &ft);
    Some(model)
}

fn decode_cmpt(data: &[u8]) -> Option<GltfModel> {
    // 16-byte outer header: magic(4) version(4) byteLength(4) tilesLength(4).
    if data.len() < 16 {
        return None;
    }
    let version = le32(data, 4);
    let byte_len = le32(data, 8) as usize;
    let tile_count = le32(data, 12) as usize;

    if version != 1 || byte_len > data.len() {
        return None;
    }
    debug_assert!(
        tile_count < MAX_SANE_TILE_COUNT,
        "cmpt tile_count={tile_count} exceeds sane maximum — likely corrupt header"
    );
    let tile_count = tile_count.min(MAX_SANE_TILE_COUNT);
    let mut merged: Option<GltfModel> = None;
    let mut pos = 16usize;

    for _ in 0..tile_count {
        // Each inner tile starts with: magic(4) version(4) byteLength(4).
        if pos + 12 > byte_len {
            break;
        }
        let inner_len = le32(data, pos + 8) as usize;
        if inner_len < 12 || pos + inner_len > byte_len {
            break;
        }

        let inner_data = &data[pos..pos + inner_len];
        let inner_format = super::TileFormat::detect("", inner_data);
        if let Some(inner_model) = decode_tile(inner_data, &inner_format) {
            merged = Some(match merged.take() {
                None => inner_model,
                Some(m) => m.merge(inner_model),
            });
        }
        pos += inner_len;
    }

    merged
}

/// Per-point or constant colour from a pnts feature table.
enum PntsColorData {
    /// Per-point RGBA [0,255] → linear f32 [0,1].
    Rgba(Vec<[f32; 4]>),
    /// Per-point RGB [0,255] → linear f32 [0,1] (alpha = 1).
    Rgb(Vec<[f32; 3]>),
    /// Constant RGBA for all points, pre-converted to linear f32.
    Constant([f32; 4]),
}

fn pnts_read_positions(ft: &Json, ft_bin: &[u8], count: usize) -> Option<Vec<[f32; 3]>> {
    if let Some(off) = parse_byte_offset(ft, "POSITION") {
        return Some(read_vec3f32(ft_bin, off, count));
    }
    if let Some(off) = parse_byte_offset(ft, "POSITION_QUANTIZED") {
        let vo = parse_vec3(ft, "QUANTIZED_VOLUME_OFFSET").unwrap_or([0.0; 3]);
        let vs = parse_vec3(ft, "QUANTIZED_VOLUME_SCALE").unwrap_or([1.0; 3]);
        return Some(read_quantized_vec3(ft_bin, off, count, vo, vs));
    }
    None
}

fn pnts_read_color(ft: &Json, ft_bin: &[u8], count: usize) -> Option<PntsColorData> {
    if let Some(off) = parse_byte_offset(ft, "RGBA") {
        let raw = read_u8_vec(ft_bin, off, count * 4);
        let rgba = raw
            .chunks_exact(4)
            .map(|c| {
                [
                    srgb_u8_to_linear(c[0]),
                    srgb_u8_to_linear(c[1]),
                    srgb_u8_to_linear(c[2]),
                    c[3] as f32 / 255.0,
                ]
            })
            .collect();
        return Some(PntsColorData::Rgba(rgba));
    }
    if let Some(off) = parse_byte_offset(ft, "RGB") {
        let raw = read_u8_vec(ft_bin, off, count * 3);
        let rgb = raw
            .chunks_exact(3)
            .map(|c| {
                [
                    srgb_u8_to_linear(c[0]),
                    srgb_u8_to_linear(c[1]),
                    srgb_u8_to_linear(c[2]),
                ]
            })
            .collect();
        return Some(PntsColorData::Rgb(rgb));
    }
    if let Some(off) = parse_byte_offset(ft, "RGB565") {
        return Some(PntsColorData::Rgb(read_rgb565(ft_bin, off, count)));
    }
    if let Some(arr) = ft.get("CONSTANT_RGBA").and_then(Json::as_array) {
        if arr.len() >= 4 {
            let c: Vec<f32> = arr
                .iter()
                .take(4)
                .map(|v| {
                    debug_assert!(
                        v.as_u64().is_some(),
                        "CONSTANT_RGBA element is not a valid u8 integer"
                    );
                    v.as_u64().unwrap_or(255) as f32 / 255.0
                })
                .collect();
            let rgba = [
                srgb_linear_to_linear(c[0]),
                srgb_linear_to_linear(c[1]),
                srgb_linear_to_linear(c[2]),
                c[3],
            ];
            return Some(PntsColorData::Constant(rgba));
        }
    }
    None
}

fn pnts_build_model(
    positions: Vec<[f32; 3]>,
    color: Option<PntsColorData>,
    normals: Option<Vec<[f32; 3]>>,
) -> GltfModel {
    let mut builder = GltfModelBuilder::new();
    let pos_flat: Vec<f32> = positions.iter().flat_map(|p| p.iter().copied()).collect();
    let pos_acc = builder.push_accessor(&pos_flat, AccessorType::Vec3);
    let mut prim = builder
        .primitive()
        .mode(PrimitiveMode::Points)
        .attribute("POSITION", pos_acc);
    if let Some(norm_data) = normals {
        let norm_flat: Vec<f32> = norm_data.iter().flat_map(|n| n.iter().copied()).collect();
        let norm_acc = builder.push_accessor(&norm_flat, AccessorType::Vec3);
        prim = prim.attribute("NORMAL", norm_acc);
    }
    match &color {
        Some(PntsColorData::Rgba(vals)) => {
            let flat: Vec<f32> = vals.iter().flat_map(|c| c.iter().copied()).collect();
            let acc = builder.push_accessor(&flat, AccessorType::Vec4);
            prim = prim.attribute("COLOR_0", acc);
        }
        Some(PntsColorData::Rgb(vals)) => {
            let flat: Vec<f32> = vals.iter().flat_map(|c| c.iter().copied()).collect();
            let acc = builder.push_accessor(&flat, AccessorType::Vec3);
            prim = prim.attribute("COLOR_0", acc);
        }
        Some(PntsColorData::Constant(_)) | None => {}
    }
    builder.push_mesh(prim.build());
    let mut model = builder.finish();
    if let Some(PntsColorData::Constant([r, g, b, a])) = color {
        let mat = Material {
            pbr_metallic_roughness: Some(moderu::MaterialPbrMetallicRoughness {
                base_color_factor: vec![r as f64, g as f64, b as f64, a as f64],
                metallic_factor: 0.0,
                roughness_factor: 1.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        model.materials.push(mat);
        if let Some(mesh) = model.meshes.first_mut() {
            if let Some(prim) = mesh.primitives.first_mut() {
                prim.material = Some(0);
            }
        }
    }
    model.nodes.push(Node {
        mesh: Some(0),
        ..Default::default()
    });
    model.scenes.push(Scene {
        nodes: Some(vec![0]),
        ..Default::default()
    });
    model.scene = Some(0);
    model
}

fn decode_pnts(data: &[u8]) -> Option<GltfModel> {
    if data.len() < 28 {
        return None;
    }
    if le32(data, 4) != 1 {
        return None;
    }
    let ft_json_len = le32(data, 12) as usize;
    let ft_bin_len = le32(data, 16) as usize;
    let ft_json_start = 28;
    let ft_bin_start = ft_json_start + ft_json_len;
    let ft_json_bytes = data
        .get(ft_json_start..ft_json_start + ft_json_len)
        .unwrap_or(&[]);
    let ft_bin = data
        .get(ft_bin_start..ft_bin_start + ft_bin_len)
        .unwrap_or(&[]);
    let ft: Json = if ft_json_bytes.is_empty() {
        Json::Object(Default::default())
    } else {
        serde_json::from_slice(ft_json_bytes).ok()?
    };
    let count = ft.get("POINTS_LENGTH").and_then(Json::as_u64).unwrap_or(0) as usize;
    if count == 0 {
        return None;
    }
    let positions = pnts_read_positions(&ft, ft_bin, count)?;
    let color = pnts_read_color(&ft, ft_bin, count);
    let normals = if let Some(off) = parse_byte_offset(&ft, "NORMAL") {
        Some(read_vec3f32(ft_bin, off, count))
    } else if let Some(off) = parse_byte_offset(&ft, "NORMAL_OCT16P") {
        Some(read_oct_normals(ft_bin, off, count))
    } else {
        None
    };
    let mut model = pnts_build_model(positions, color, normals);
    if let Some(center) = parse_vec3(&ft, "RTC_CENTER") {
        model.extensions.insert(
            "CESIUM_RTC".to_owned(),
            serde_json::json!({ "center": center }),
        );
        model.extensions_used.push("CESIUM_RTC".to_owned());
    }
    Some(model)
}

// (GltfModel::merge lives in moderu — see moderu::merge)

// ── Instance math helpers ─────────────────────────────────────────────────────

/// Build a quaternion [x,y,z,w] from a world-space up and right vector.
///
/// Matches `rotationFromUpRight` in cesium-native's `I3dmToGltfConverter.cpp`.
fn rotation_from_up_right(up: [f32; 3], right: [f32; 3]) -> [f32; 4] {
    // forward = cross(right, up)
    let forward = [
        right[1] * up[2] - right[2] * up[1],
        right[2] * up[0] - right[0] * up[2],
        right[0] * up[1] - right[1] * up[0],
    ];
    // Rotation matrix columns: right, up, forward.
    mat3_to_quat([right, up, forward])
}

/// Compute the ENU-aligned quaternion for an instance at ECEF position `p`.
///
/// East = normalize(cross([0,0,1], radial_up))
/// North = cross(radial_up, east)
/// Up = radial_up
fn enu_quaternion(p: [f32; 3]) -> [f32; 4] {
    let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    if len < 1e-6 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let up_vec = [p[0] / len, p[1] / len, p[2] / len];
    let z = [0.0f32, 0.0, 1.0];
    // east = cross(z, up_vec)
    let mut east = [
        z[1] * up_vec[2] - z[2] * up_vec[1],
        z[2] * up_vec[0] - z[0] * up_vec[2],
        z[0] * up_vec[1] - z[1] * up_vec[0],
    ];
    let elen = (east[0] * east[0] + east[1] * east[1] + east[2] * east[2]).sqrt();
    if elen < 1e-6 {
        // Near poles: fall back to identity.
        return [0.0, 0.0, 0.0, 1.0];
    }
    east = [east[0] / elen, east[1] / elen, east[2] / elen];
    // north = cross(up_vec, east)
    let north = [
        up_vec[1] * east[2] - up_vec[2] * east[1],
        up_vec[2] * east[0] - up_vec[0] * east[2],
        up_vec[0] * east[1] - up_vec[1] * east[0],
    ];
    mat3_to_quat([east, north, up_vec])
}

/// Convert a 3×3 rotation matrix (column-major: `[col0, col1, col2]`) to a
/// unit quaternion `[x, y, z, w]`.
///
/// Uses the Shepperd method.
fn mat3_to_quat(cols: [[f32; 3]; 3]) -> [f32; 4] {
    // Row-major indexing: m[row][col]
    let m = |r: usize, c: usize| cols[c][r];

    let trace = m(0, 0) + m(1, 1) + m(2, 2);
    if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        [
            (m(2, 1) - m(1, 2)) * s,
            (m(0, 2) - m(2, 0)) * s,
            (m(1, 0) - m(0, 1)) * s,
            0.25 / s,
        ]
    } else if m(0, 0) > m(1, 1) && m(0, 0) > m(2, 2) {
        let s = 2.0 * (1.0 + m(0, 0) - m(1, 1) - m(2, 2)).sqrt();
        [
            0.25 * s,
            (m(0, 1) + m(1, 0)) / s,
            (m(0, 2) + m(2, 0)) / s,
            (m(2, 1) - m(1, 2)) / s,
        ]
    } else if m(1, 1) > m(2, 2) {
        let s = 2.0 * (1.0 + m(1, 1) - m(0, 0) - m(2, 2)).sqrt();
        [
            (m(0, 1) + m(1, 0)) / s,
            0.25 * s,
            (m(1, 2) + m(2, 1)) / s,
            (m(0, 2) - m(2, 0)) / s,
        ]
    } else {
        let s = 2.0 * (1.0 + m(2, 2) - m(0, 0) - m(1, 1)).sqrt();
        [
            (m(0, 2) + m(2, 0)) / s,
            (m(1, 2) + m(2, 1)) / s,
            0.25 * s,
            (m(1, 0) - m(0, 1)) / s,
        ]
    }
}

// Binary-read helpers

/// Read a little-endian u32 from `data` at byte offset `off`.
#[inline]
pub(crate) fn le32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(data[off..off + 4].try_into().unwrap_or([0; 4]))
}

#[inline]
fn read_f32(data: &[u8], off: usize) -> Option<f32> {
    data.get(off..off + 4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
}

/// Read `count` Vec3-f32 values (12 bytes each) starting at binary offset `off`.
fn read_vec3f32(data: &[u8], off: usize, count: usize) -> Vec<[f32; 3]> {
    (0..count)
        .filter_map(|i| {
            let base = off + i * 12;
            let x = read_f32(data, base)?;
            let y = read_f32(data, base + 4)?;
            let z = read_f32(data, base + 8)?;
            Some([x, y, z])
        })
        .collect()
}

/// Read `count` Vec3 u16-quantized values and dequantize them.
fn read_quantized_vec3(
    data: &[u8],
    off: usize,
    count: usize,
    volume_offset: [f64; 3],
    volume_scale: [f64; 3],
) -> Vec<[f32; 3]> {
    (0..count)
        .filter_map(|i| {
            let base = off + i * 6;
            if base + 6 > data.len() {
                return None;
            }
            let xq = u16::from_le_bytes(data[base..base + 2].try_into().ok()?) as f64;
            let yq = u16::from_le_bytes(data[base + 2..base + 4].try_into().ok()?) as f64;
            let zq = u16::from_le_bytes(data[base + 4..base + 6].try_into().ok()?) as f64;
            let x = (xq / 65535.0 * volume_scale[0] + volume_offset[0]) as f32;
            let y = (yq / 65535.0 * volume_scale[1] + volume_offset[1]) as f32;
            let z = (zq / 65535.0 * volume_scale[2] + volume_offset[2]) as f32;
            Some([x, y, z])
        })
        .collect()
}

/// Read `count` oct-encoded normals (2×u16 each, 4 bytes/normal).
fn read_oct_normals(data: &[u8], off: usize, count: usize) -> Vec<[f32; 3]> {
    (0..count)
        .filter_map(|i| {
            let base = off + i * 4;
            if base + 4 > data.len() {
                return None;
            }
            let ox = u16::from_le_bytes(data[base..base + 2].try_into().ok()?);
            let oy = u16::from_le_bytes(data[base + 2..base + 4].try_into().ok()?);
            Some(oct_decode_16p(ox, oy))
        })
        .collect()
}

/// Decode a 16-bit oct-encoded normal (2×u16 in [0, 65535]) to a unit f32 vec3.
///
/// Matches `AttributeCompression::octDecodeInRange(ox, oy, 65535)` from cesium-native.
fn oct_decode_16p(ox: u16, oy: u16) -> [f32; 3] {
    let mut x = ox as f32 / 32767.5 - 1.0;
    let mut y = oy as f32 / 32767.5 - 1.0;
    let z = 1.0 - x.abs() - y.abs();
    if z < 0.0 {
        let old_x = x;
        x = (1.0 - y.abs()) * x.signum();
        y = (1.0 - old_x.abs()) * y.signum();
    }
    let len = (x * x + y * y + z * z).sqrt();
    if len < 1e-6 {
        [0.0, 0.0, 1.0]
    } else {
        [x / len, y / len, z / len]
    }
}

/// Read a contiguous slice of raw bytes from `data` starting at `off`.
fn read_u8_vec(data: &[u8], off: usize, len: usize) -> &[u8] {
    data.get(off..off + len).unwrap_or(&[])
}

/// Read `count` RGB565-encoded colours and expand to linear f32 vec3.
fn read_rgb565(data: &[u8], off: usize, count: usize) -> Vec<[f32; 3]> {
    (0..count)
        .filter_map(|i| {
            let base = off + i * 2;
            if base + 2 > data.len() {
                return None;
            }
            let raw = u16::from_le_bytes(data[base..base + 2].try_into().ok()?);
            let r5 = ((raw >> 11) & 0x1F) as f32 / 31.0;
            let g6 = ((raw >> 5) & 0x3F) as f32 / 63.0;
            let b5 = (raw & 0x1F) as f32 / 31.0;
            // RGB565 is sRGB; convert to linear.
            Some([
                srgb_linear_to_linear(r5),
                srgb_linear_to_linear(g6),
                srgb_linear_to_linear(b5),
            ])
        })
        .collect()
}

/// sRGB u8 → linear f32.
#[inline]
fn srgb_u8_to_linear(u: u8) -> f32 {
    srgb_linear_to_linear(u as f32 / 255.0)
}

/// sRGB normalized [0,1] → linear [0,1].  (IEC 61966-2-1 exact piecewise.)
fn srgb_linear_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

// Feature-table JSON helpers

/// Read a `{ "byteOffset": N }` field from the feature-table JSON.
fn parse_byte_offset(ft: &Json, key: &str) -> Option<usize> {
    ft.get(key)?.get("byteOffset")?.as_u64().map(|n| n as usize)
}

/// Read a `[x, y, z]` array (f64) from the feature-table JSON.
fn parse_vec3(ft: &Json, key: &str) -> Option<[f64; 3]> {
    let arr = ft.get(key)?.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    Some([arr[0].as_f64()?, arr[1].as_f64()?, arr[2].as_f64()?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use moderu::Asset;

    fn le32_bytes(n: u32) -> [u8; 4] {
        n.to_le_bytes()
    }

    #[test]
    fn oct_decode_x_axis() {
        // (ox=65535, oy=32767) should decode to approximately [1, 0, 0].
        let n = oct_decode_16p(65535, 32767);
        assert!((n[0] - 1.0).abs() < 0.01, "x≈1 got {:?}", n);
        assert!(n[1].abs() < 0.01, "y≈0 got {:?}", n);
    }

    #[test]
    fn oct_decode_z_axis() {
        // (32767, 32767) should decode to approximately [0, 0, 1] (top of sphere).
        let n = oct_decode_16p(32767, 32767);
        assert!(n[2] > 0.9, "z should be positive, got {:?}", n);
    }

    #[test]
    fn oct_decode_is_unit() {
        let test_cases = [(0u16, 0u16), (65535, 65535), (32767, 0), (0, 32767)];
        for (ox, oy) in test_cases {
            let n = oct_decode_16p(ox, oy);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "({ox},{oy}) len={len:.6} n={n:?}");
        }
    }

    #[test]
    fn identity_matrix_gives_identity_quat() {
        let q = mat3_to_quat([[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]);
        // Identity quaternion: [0,0,0,1]
        assert!((q[3] - 1.0).abs() < 1e-5, "w should be 1, got {:?}", q);
        assert!(q[0].abs() < 1e-5);
        assert!(q[1].abs() < 1e-5);
        assert!(q[2].abs() < 1e-5);
    }

    #[test]
    fn quat_is_unit_length() {
        let q = mat3_to_quat([[0., 1., 0.], [0., 0., 1.], [1., 0., 0.]]);
        let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
        assert!((len - 1.0).abs() < 1e-5, "len={len:.6}");
    }

    // ── sRGB conversion ───────────────────────────────────────────────────────

    #[test]
    fn srgb_black_and_white() {
        assert!((srgb_u8_to_linear(0) - 0.0).abs() < 1e-6);
        assert!((srgb_u8_to_linear(255) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn srgb_midpoint_is_darker_in_linear() {
        let mid = srgb_u8_to_linear(128);
        assert!(mid < 0.5, "linear(128) should be < 0.5, got {mid}");
        assert!(mid > 0.2, "linear(128) should be > 0.2, got {mid}");
    }

    // ── GltfModel::merge ──────────────────────────────────────────────────────

    fn minimal_model() -> GltfModel {
        let mut m = GltfModel {
            asset: Asset {
                version: "2.0".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        m.buffers.push(moderu::Buffer {
            data: vec![0u8, 1, 2, 3],
            byte_length: 4,
            ..Default::default()
        });
        m.buffer_views.push(moderu::BufferView {
            buffer: 0,
            byte_offset: 0,
            byte_length: 4,
            ..Default::default()
        });
        m.accessors.push(moderu::Accessor {
            buffer_view: Some(0),
            count: 1,
            component_type: moderu::AccessorComponentType::Float,
            r#type: AccessorType::Scalar,
            ..Default::default()
        });
        m
    }

    #[test]
    fn merge_two_minimal_models_remaps_indices() {
        let a = minimal_model();
        let b = minimal_model();
        let merged = a.merge(b);
        assert_eq!(merged.buffers.len(), 2);
        assert_eq!(merged.buffer_views.len(), 2);
        assert_eq!(merged.accessors.len(), 2);
        // Second buffer_view must point to buffer 1.
        assert_eq!(merged.buffer_views[1].buffer, 1);
        // Second accessor must point to buffer_view 1.
        assert_eq!(merged.accessors[1].buffer_view, Some(1));
    }

    // ── b3dm header variants ──────────────────────────────────────────────────

    fn build_b3dm(
        header_extra: &[u8],
        ft_json: &[u8],
        ft_bin: &[u8],
        bt_json: &[u8],
        bt_bin: &[u8],
        glb: &[u8],
    ) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(b"b3dm");
        data.extend_from_slice(&le32_bytes(1)); // version
        let total = 28 + ft_json.len() + ft_bin.len() + bt_json.len() + bt_bin.len() + glb.len();
        data.extend_from_slice(&le32_bytes(total as u32));
        data.extend_from_slice(&le32_bytes(ft_json.len() as u32));
        data.extend_from_slice(&le32_bytes(ft_bin.len() as u32));
        data.extend_from_slice(&le32_bytes(bt_json.len() as u32));
        data.extend_from_slice(&le32_bytes(bt_bin.len() as u32));
        let _ = header_extra;
        data.extend_from_slice(ft_json);
        data.extend_from_slice(ft_bin);
        data.extend_from_slice(bt_json);
        data.extend_from_slice(bt_bin);
        data.extend_from_slice(glb);
        data
    }

    /// Minimal valid GLB (12-byte header only, no JSON chunk — not useful for
    /// GltfReader but sufficient to test offset arithmetic).
    fn tiny_glb() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"glTF");
        v.extend_from_slice(&le32_bytes(2));
        v.extend_from_slice(&le32_bytes(12));
        v
    }

    #[test]
    fn b3dm_offsets_modern_no_tables() {
        let offsets =
            B3dmOffsets::parse(&build_b3dm(&[], &[], &[], &[], &[], &tiny_glb())).unwrap();
        assert_eq!(offsets.glb_start(), 28);
    }

    #[test]
    fn b3dm_offsets_modern_with_tables() {
        let ft = b"{}";
        let bt = b"{\"a\":1}";
        let data = build_b3dm(&[], ft, &[], bt, &[], &tiny_glb());
        let offsets = B3dmOffsets::parse(&data).unwrap();
        assert_eq!(offsets.glb_start(), 28 + ft.len() + bt.len());
    }

    #[test]
    fn b3dm_legacy1_offsets() {
        // manually construct legacy-1 layout
        let glb = tiny_glb();
        let bt_data = b"[]"; // 2 bytes — will appear at [20..24] as '[', ']', then GLB start
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"b3dm");
        data.extend_from_slice(&le32_bytes(1));
        data.extend_from_slice(&le32_bytes((20 + bt_data.len() + glb.len()) as u32));
        data.extend_from_slice(&le32_bytes(0x1000)); // batchLength — read as ft_json
        data.extend_from_slice(&le32_bytes(bt_data.len() as u32)); // batchTableByteLength — read as ft_bin
        data.extend_from_slice(bt_data);
        data.extend_from_slice(&glb);
        // [20..24] should be first 4 bytes of bt_data ('['=0x5B) + first 3 bytes of glb
        let bt_json_field = u32::from_le_bytes(data[20..24].try_into().unwrap());
        assert!(
            bt_json_field >= 0x2200_0000,
            "legacy-1 detection should fire"
        );
        let offsets = B3dmOffsets::parse(&data).unwrap();
        assert_eq!(offsets.glb_start(), 20 + bt_data.len());
    }

    // ── cmpt ──────────────────────────────────────────────────────────────────

    #[test]
    fn cmpt_too_short_returns_none() {
        assert!(decode_cmpt(b"cmpt\x01\x00\x00\x00").is_none());
    }

    // ── quantized positions ───────────────────────────────────────────────────

    #[test]
    fn quantized_vec3_dequantizes_correctly() {
        // A u16 of 65535 at scale 10 and offset 0 should give 10.0.
        let mut data = [0u8; 6];
        data[0..2].copy_from_slice(&65535u16.to_le_bytes());
        data[2..4].copy_from_slice(&0u16.to_le_bytes());
        data[4..6].copy_from_slice(&32767u16.to_le_bytes());
        let pts = read_quantized_vec3(&data, 0, 1, [0.0; 3], [10.0, 10.0, 10.0]);
        assert_eq!(pts.len(), 1);
        assert!((pts[0][0] - 10.0).abs() < 1e-3, "x={}", pts[0][0]);
        assert!((pts[0][1]).abs() < 1e-3, "y={}", pts[0][1]);
        assert!((pts[0][2] - 4.999).abs() < 0.01, "z≈5, got {}", pts[0][2]);
    }
}
