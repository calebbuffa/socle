//! Trait for content types that can receive raster overlay textures.
//!
//! Modelled after cesium-native's `IPrepareRendererResources`: the framework
//! computes overlay texture coordinates and UV transforms, then hands the
//! image pixels plus translation/scale to the renderer. The renderer is free
//! to upload a GPU texture, bake into a GLB, or do nothing at all.

use crate::overlay::{OverlayId, RasterOverlayTile};

/// Content that can receive raster overlay tiles.
///
/// The framework computes the UV `translation` and `scale` from the geometry
/// and overlay rectangles, then calls [`attach_raster`](Self::attach_raster)
/// with the image data and transform. The implementor decides how to apply
/// them (GPU texture upload, model modification, etc.).
///
/// # Example
///
/// ```ignore
/// impl OverlayTarget for MyGpuTile {
///     fn attach_raster(&mut self, tile: &RasterOverlayTile,
///                      translation: [f64; 2], scale: [f64; 2]) {
///         self.upload_texture(tile.pixels, tile.width, tile.height);
///         self.set_uv_transform(translation, scale);
///     }
///     fn detach_raster(&mut self, _id: OverlayId) {
///         self.remove_overlay_texture();
///     }
/// }
/// ```
pub trait OverlayTarget {
    /// An overlay tile is being attached to this content.
    ///
    /// `tile` contains the RGBA pixel data, dimensions, and geographic rectangle.
    /// `translation` and `scale` are the UV offset/scale for KHR_texture_transform
    /// (or equivalent), computed by the framework from the geometry and overlay
    /// rectangles.
    fn attach_raster(&mut self, tile: &RasterOverlayTile, translation: [f64; 2], scale: [f64; 2]);

    /// An overlay was detached from this content.
    fn detach_raster(&mut self, overlay_id: OverlayId);
}
