//! Overlay tile compositing — combining multiple quadtree-level tiles into a
//! single image that covers a geometry tile's geographic extent.

use std::sync::Arc;

use moderu::{ImageData, Rectangle};

use crate::overlay::RasterOverlayTile;

/// Composite multiple overlay tiles into a single image covering `target_width × target_height`.
///
/// Each source tile has a geographic `rectangle` describing the extent it covers.
/// `target_rectangle` is the geographic extent the output image should cover.
/// This function blits each source tile into the correct region of the output.
///
/// Returns the composited tile whose `rectangle` equals `target_rectangle`.
pub fn composite_overlay_tiles(
    source_tiles: &[RasterOverlayTile],
    target_width: u32,
    target_height: u32,
    target_rectangle: terra::GlobeRectangle,
) -> RasterOverlayTile {
    let mut target = ImageData::solid_rgba(target_width, target_height, 0, 0, 0, 0);

    let tr_w = (target_rectangle.east - target_rectangle.west).max(f64::EPSILON);
    let tr_h = (target_rectangle.north - target_rectangle.south).max(f64::EPSILON);

    for tile in source_tiles {
        // Map the source tile's rectangle into pixel coordinates in the target.
        // Pixel row 0 is north (top of image), so Y is measured from the north edge.
        let frac_x = (tile.rectangle.west - target_rectangle.west) / tr_w;
        let frac_y = (target_rectangle.north - tile.rectangle.north) / tr_h;
        let frac_w = (tile.rectangle.east - tile.rectangle.west) / tr_w;
        let frac_h = (tile.rectangle.north - tile.rectangle.south) / tr_h;

        let dst_x = (frac_x * target_width as f64).round() as i32;
        let dst_y = (frac_y * target_height as f64).round() as i32;
        let dst_w = (frac_w * target_width as f64).round() as i32;
        let dst_h = (frac_h * target_height as f64).round() as i32;

        if dst_w <= 0 || dst_h <= 0 {
            continue;
        }

        let src = ImageData {
            data: tile.pixels.to_vec(),
            width: tile.width,
            height: tile.height,
            channels: 4,
            bytes_per_channel: 1,
            compressed_pixel_format: moderu::GpuCompressedPixelFormat::None,
            mip_positions: Vec::new(),
        };

        let src_rect = Rectangle::new(0, 0, src.width as i32, src.height as i32);
        let dst_rect = Rectangle::new(dst_x, dst_y, dst_w, dst_h);

        // blit handles resampling if src/dst sizes differ.
        let _ = target.blit(&src, src_rect, dst_rect);
    }

    RasterOverlayTile {
        pixels: Arc::from(target.data),
        width: target_width,
        height: target_height,
        rectangle: target_rectangle,
    }
}
