//! Attribute compression and decompression utilities.
//! Pluggable geometry-buffer decoding.
//!
//! [`GeometryDecoder`] is the trait used to convert a raw binary blob into
//! [`GeometryData`].  The default implementation ([`PassthroughDecoder`]) calls
//! the built-in I3S binary layout parser.  Feature-gated alternatives include
//! [`crate::draco::DracoGeometryDecoder`] and (with the `lepcc` feature)
//! [`crate::lepcc::LepccGeometryDecoder`].

use i3s_util::Result;

use crate::geometry::{GeometryData, GeometryLayout, parse_geometry_buffer};

/// Decode a raw geometry-buffer blob into [`GeometryData`].
///
/// Implementors must be `Send + Sync` so they can be shared across the
/// worker-thread pool behind an `Arc<dyn GeometryDecoder>`.
pub trait GeometryDecoder: Send + Sync {
    fn decode(&self, data: &[u8], layout: &GeometryLayout) -> Result<GeometryData>;
}

/// Passthrough decoder — delegates to the built-in I3S binary layout parser.
///
/// Use this for uncompressed geometry buffers (the common case).
pub struct PassthroughDecoder;

impl GeometryDecoder for PassthroughDecoder {
    #[inline]
    fn decode(&self, data: &[u8], layout: &GeometryLayout) -> Result<GeometryData> {
        parse_geometry_buffer(data, layout)
    }
}
