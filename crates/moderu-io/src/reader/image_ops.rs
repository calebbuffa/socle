//! Image pixel manipulation utilities.
//!
//! Data structures (`Image`, `Rectangle`) are defined in `moderu`.
//! Processing operations like blitting live here in `moderu-reader`.

use moderu::{ImageData, Rectangle};

fn rect_in_bounds(img: &ImageData, r: Rectangle) -> bool {
    r.x >= 0
        && r.y >= 0
        && r.width > 0
        && r.height > 0
        && r.x_end() <= img.width as i32
        && r.y_end() <= img.height as i32
}

/// Copy (and nearest-neighbour scale) `source_rect` from `source` into
/// `target_rect` in `target`. Returns `false` if the rects are out of
/// bounds or the images have a different channel count.
pub fn blit_image(
    target: &mut ImageData,
    target_rect: Rectangle,
    source: &ImageData,
    source_rect: Rectangle,
) -> bool {
    if source.channels != target.channels || source.channels == 0 {
        return false;
    }
    if !rect_in_bounds(target, target_rect) || !rect_in_bounds(source, source_rect) {
        return false;
    }
    let bpp = source.channels as usize;
    let src_stride = source.width as usize * bpp;
    let dst_stride = target.width as usize * bpp;
    for dr in 0..target_rect.height as usize {
        let sr = dr * source_rect.height as usize / target_rect.height as usize;
        let src_row_start = (source_rect.y as usize + sr) * src_stride;
        let dst_row_start = (target_rect.y as usize + dr) * dst_stride;
        for dc in 0..target_rect.width as usize {
            let sc = dc * source_rect.width as usize / target_rect.width as usize;
            let src_off = src_row_start + (source_rect.x as usize + sc) * bpp;
            let dst_off = dst_row_start + (target_rect.x as usize + dc) * bpp;
            let Some(src_px) = source.data.get(src_off..src_off + bpp) else {
                return false;
            };
            let Some(dst_px) = target.data.get_mut(dst_off..dst_off + bpp) else {
                return false;
            };
            dst_px.copy_from_slice(src_px);
        }
    }
    true
}
