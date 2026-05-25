//! Blend modes and pixel compositing.

use rayon::prelude::*;

use crate::blend::RECIP_255;
use crate::settings::ass_subtitle::AssBlendMode;

use super::types::DirtyRect;

// ---------------------------------------------------------------------------
// Per-pixel blend
// ---------------------------------------------------------------------------

/// Blend a single source pixel onto a destination pixel.
/// Inputs and outputs are premultiplied RGBA in 0..1.
pub(crate) fn blend_pixel(mode: AssBlendMode, src: [f32; 4], dst: [f32; 4]) -> [f32; 4] {
    match mode {
        AssBlendMode::Normal => {
            let out_a = src[3] + dst[3] * (1.0 - src[3]);
            if out_a < 0.001 {
                return [0.0, 0.0, 0.0, 0.0];
            }
            let out_r = (src[0] + dst[0] * (1.0 - src[3])) / out_a;
            let out_g = (src[1] + dst[1] * (1.0 - src[3])) / out_a;
            let out_b = (src[2] + dst[2] * (1.0 - src[3])) / out_a;
            [out_r, out_g, out_b, out_a]
        }
        AssBlendMode::Add => {
            let r = (src[0] + dst[0]).min(1.0);
            let g = (src[1] + dst[1]).min(1.0);
            let b = (src[2] + dst[2]).min(1.0);
            let a = (src[3] + dst[3]).min(1.0);
            [r, g, b, a]
        }
        AssBlendMode::Screen => {
            let r = 1.0 - (1.0 - src[0]) * (1.0 - dst[0]);
            let g = 1.0 - (1.0 - src[1]) * (1.0 - dst[1]);
            let b = 1.0 - (1.0 - src[2]) * (1.0 - dst[2]);
            let a = 1.0 - (1.0 - src[3]) * (1.0 - dst[3]);
            [r, g, b, a]
        }
        AssBlendMode::Multiply => {
            [
                src[0] * dst[0],
                src[1] * dst[1],
                src[2] * dst[2],
                src[3] * dst[3],
            ]
        }
        AssBlendMode::Overlay => {
            let overlay_ch = |s: f32, d: f32| -> f32 {
                if d < 0.5 {
                    2.0 * d * s
                } else {
                    1.0 - 2.0 * (1.0 - d) * (1.0 - s)
                }
            };
            [
                overlay_ch(src[0], dst[0]),
                overlay_ch(src[1], dst[1]),
                overlay_ch(src[2], dst[2]),
                overlay_ch(src[3], dst[3]),
            ]
        }
    }
}

// ---------------------------------------------------------------------------
// Direct pixel composite (used by renderer for per-glyph pass)
// ---------------------------------------------------------------------------

/// Direct max-alpha composite for outline samples.
/// Unlike source-over, this uses max blending: if existing alpha is already
/// higher, the new sample is skipped. This prevents additive accumulation
/// when multiple outline offset stamps overlap at the same pixel.
#[inline]
pub(crate) fn direct_composite_max(output: &mut [u8], idx: usize, color: [f32; 4], coverage: f32) {
    let src_a = coverage * color[3];
    if src_a < 0.0001 {
        return;
    }

    let dst_a = output[idx + 3] as f32 * crate::blend::RECIP_255;
    if dst_a >= src_a {
        return;
    }

    // Write premultiplied color with max alpha
    output[idx] = (color[0] * src_a * 255.0 + 0.5) as u8;
    output[idx + 1] = (color[1] * src_a * 255.0 + 0.5) as u8;
    output[idx + 2] = (color[2] * src_a * 255.0 + 0.5) as u8;
    output[idx + 3] = (src_a * 255.0 + 0.5) as u8;
}

/// Direct source-over composite with fast transparent-dst path.
/// `coverage` is alpha coverage in 0..=1.
#[inline]
pub(crate) fn direct_composite(output: &mut [u8], idx: usize, color: [f32; 4], coverage: f32) {
    let alpha = coverage;
    let src_a = alpha * color[3];
    if src_a < 0.0001 {
        return;
    }

    // Fast path: destination pixel is transparent — just write src
    let dst_a = output[idx + 3];
    if dst_a == 0 {
        output[idx] = (color[0] * src_a * 255.0 + 0.5) as u8;
        output[idx + 1] = (color[1] * src_a * 255.0 + 0.5) as u8;
        output[idx + 2] = (color[2] * src_a * 255.0 + 0.5) as u8;
        output[idx + 3] = (src_a * 255.0 + 0.5) as u8;
        return;
    }

    let dst_r = output[idx] as f32 * RECIP_255;
    let dst_g = output[idx + 1] as f32 * RECIP_255;
    let dst_b = output[idx + 2] as f32 * RECIP_255;
    let dst_a_f = dst_a as f32 * RECIP_255;

    let out_a = src_a + dst_a_f * (1.0 - src_a);
    let inv_out_a = 1.0 / out_a;
    // dst_r is already premultiplied (R*A), so we only need (1 - src_a)
    let out_r = (color[0] * src_a + dst_r * (1.0 - src_a)) * inv_out_a;
    let out_g = (color[1] * src_a + dst_g * (1.0 - src_a)) * inv_out_a;
    let out_b = (color[2] * src_a + dst_b * (1.0 - src_a)) * inv_out_a;

    output[idx] = (out_r * 255.0 + 0.5) as u8;
    output[idx + 1] = (out_g * 255.0 + 0.5) as u8;
    output[idx + 2] = (out_b * 255.0 + 0.5) as u8;
    output[idx + 3] = (out_a * 255.0 + 0.5) as u8;
}

// ---------------------------------------------------------------------------
// Dirty-rect compositing onto output buffer
// ---------------------------------------------------------------------------

/// Blend source buffer onto destination within a dirty rectangle.
/// Parallelized by row via rayon.
pub(crate) fn cpu_composite_dirty_rect(
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    dirty: &DirtyRect,
    blend_mode: AssBlendMode,
) {
    assert_eq!(dst.len() % (width * 4), 0);
    let row_stride = width * 4;
    let min_y = dirty.min_y.max(0) as usize;
    let max_y = (dirty.max_y as usize)
        .min(dst.len() / row_stride)
        .saturating_sub(1);
    if min_y > max_y {
        return;
    }

    dst[min_y * row_stride..=(max_y * row_stride + row_stride - 1)]
        .par_chunks_mut(row_stride)
        .enumerate()
        .for_each(|(rel_y, dst_row)| {
            let y = (min_y + rel_y) as i32;
            let row_start = y as usize * width * 4;
            for x in dirty.min_x..=dirty.max_x {
                let col_offset = x as usize * 4;
                let idx = row_start + col_offset;
                let sa = src[idx + 3];
                if sa == 0 {
                    continue;
                }

                // Fast path: opaque src pixel
                if sa == 255 {
                    dst_row[col_offset] = src[idx];
                    dst_row[col_offset + 1] = src[idx + 1];
                    dst_row[col_offset + 2] = src[idx + 2];
                    dst_row[col_offset + 3] = 255;
                    continue;
                }

                // Fast path: transparent dst
                if dst_row[col_offset + 3] == 0 {
                    dst_row[col_offset] = src[idx];
                    dst_row[col_offset + 1] = src[idx + 1];
                    dst_row[col_offset + 2] = src[idx + 2];
                    dst_row[col_offset + 3] = sa;
                    continue;
                }

                let sa_f = sa as f32 * RECIP_255;
                let sr = src[idx] as f32 * RECIP_255;
                let sg = src[idx + 1] as f32 * RECIP_255;
                let sb = src[idx + 2] as f32 * RECIP_255;

                let da = dst_row[col_offset + 3] as f32 * RECIP_255;
                let dr = dst_row[col_offset] as f32 * RECIP_255;
                let dg = dst_row[col_offset + 1] as f32 * RECIP_255;
                let db = dst_row[col_offset + 2] as f32 * RECIP_255;

                // sr/dr are already premultiplied in the temp_buf
                let src_px = [sr, sg, sb, sa_f];
                let dst_px = [dr, dg, db, da];

                let blended = blend_pixel(blend_mode, src_px, dst_px);

                dst_row[col_offset] = (blended[0] * 255.0 + 0.5) as u8;
                dst_row[col_offset + 1] = (blended[1] * 255.0 + 0.5) as u8;
                dst_row[col_offset + 2] = (blended[2] * 255.0 + 0.5) as u8;
                dst_row[col_offset + 3] = (blended[3] * 255.0 + 0.5) as u8;
            }
        });
}

