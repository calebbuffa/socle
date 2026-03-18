//! LEPCC-compressed point-cloud geometry decoding for I3S.
//!
//! I3S Point Cloud layers use LEPCC (`lepcc-xyz`) to compress the `f64`
//! position buffer.  Each call to [`LepccGeometryDecoder::decode`] creates a
//! fresh [`lepcc_ffi::Context`] — the C context is single-threaded by design
//! and must not be shared across calls.
//!
//! Requires the `lepcc` feature flag.

use i3s_util::{I3sError, Result};

use crate::codec::GeometryDecoder;
use crate::geometry::{GeometryData, GeometryLayout};

/// Geometry decoder for LEPCC-compressed I3S point-cloud layers (`lepcc-xyz`).
pub struct LepccGeometryDecoder;

impl GeometryDecoder for LepccGeometryDecoder {
    fn decode(&self, data: &[u8], _layout: &GeometryLayout) -> Result<GeometryData> {
        let ctx = lepcc_ffi::Context::new();
        let points = ctx
            .decode_xyz(data)
            .map_err(|e| I3sError::Lepcc(e.to_string()))?;
        let vertex_count = points.len() as u32;
        let positions = points
            .into_iter()
            .map(|[x, y, z]| [x as f32, y as f32, z as f32])
            .collect();
        Ok(GeometryData {
            vertex_count,
            positions,
            ..Default::default()
        })
    }
}