use std::cell::RefCell;

use rayon::prelude::*;

use crate::blend::RECIP_255;
use crate::settings::chroma_key::ZzzChromaKey;

// ---------------------------------------------------------------------------
// BT.601 YCbCr conversion helpers
// ---------------------------------------------------------------------------

#[inline]
fn rgb_to_ycbcr(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    let cb = -0.168736 * r - 0.331264 * g + 0.5 * b + 0.5;
    let cr = 0.5 * r - 0.418688 * g - 0.081312 * b + 0.5;
    (y, cb, cr)
}

// ---------------------------------------------------------------------------
// Thread-local alpha blur buffers (only used when edge_blur > 0)
// ---------------------------------------------------------------------------

struct AlphaBufs {
    alpha: Vec<f32>,
    blur_h: Vec<f32>,
}

impl Default for AlphaBufs {
    fn default() -> Self {
        Self { alpha: Vec::new(), blur_h: Vec::new() }
    }
}

thread_local! {
    static ALPHA_BUFS: RefCell<AlphaBufs> = RefCell::new(AlphaBufs::default());
}

// ---------------------------------------------------------------------------
// Separable box blur for single-channel f32 alpha
// ---------------------------------------------------------------------------

fn box_blur_f32_h(src: &[f32], dst: &mut [f32], width: usize, _height: usize, r: usize) {
    dst.par_iter_mut().enumerate().for_each(|(i, out)| {
        let y = i / width;
        let x = i % width;
        let x0 = x.saturating_sub(r);
        let x1 = (x + r + 1).min(width);
        let actual = (x1 - x0) as f64;
        let mut sum: f64 = 0.0;
        for sx in x0..x1 {
            sum += src[y * width + sx] as f64;
        }
        *out = (sum / actual) as f32;
    });
}

fn box_blur_f32_v(src: &[f32], dst: &mut [f32], width: usize, height: usize, r: usize) {
    dst.par_iter_mut().enumerate().for_each(|(i, out)| {
        let y = i / width;
        let x = i % width;
        let y0 = y.saturating_sub(r);
        let y1 = (y + r + 1).min(height);
        let actual = (y1 - y0) as f64;
        let mut sum: f64 = 0.0;
        for sy in y0..y1 {
            sum += src[sy * width + x] as f64;
        }
        *out = (sum / actual) as f32;
    });
}

fn blur_alpha(alpha: &mut [f32], blur_h: &mut [f32], width: usize, height: usize, radius: usize) {
    let r = radius.min(width.max(height) / 2).max(1);
    box_blur_f32_h(alpha, blur_h, width, height, r);
    box_blur_f32_v(blur_h, alpha, width, height, r);
    box_blur_f32_h(alpha, blur_h, width, height, r);
    box_blur_f32_v(blur_h, alpha, width, height, r);
    box_blur_f32_h(alpha, blur_h, width, height, r);
    box_blur_f32_v(blur_h, alpha, width, height, r);
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl ZzzChromaKey {
    pub fn is_identity(&self) -> bool {
        !self.show_matte && !self.invert && self.threshold <= 0.0
    }

    pub fn apply_effect(
        &self,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) {
        let total = width * height;
        let len = total * 4;
        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }

        let threshold = self.threshold.clamp(0.0, 1.0);
        if !self.show_matte && !self.invert && threshold <= 0.0 {
            dst.copy_from_slice(src);
            return;
        }

        // ── Try GPU first ──
        let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::gpu::chroma_key::try_chroma_key_gpu_render(self, src, dst, width, height)
        }));
        match gpu_result {
            Ok(Ok(true)) => return,
            _ => {} // fall through to CPU path
        }

        // ── CPU path ──
        let edge_softness = self.edge_softness.clamp(0.0, 1.0);
        let spill_suppression = self.spill_suppression.clamp(0.0, 1.0);
        let edge_blur = self.edge_blur.clamp(0.0, 20.0);
        let show_matte = self.show_matte;
        let invert = self.invert;

        let key_r = self.key_color_r.clamp(0.0, 1.0);
        let key_g = self.key_color_g.clamp(0.0, 1.0);
        let key_b = self.key_color_b.clamp(0.0, 1.0);
        let (_, key_cb, key_cr) = rgb_to_ycbcr(key_r, key_g, key_b);

        let threshold_sq = threshold * threshold;
        let soft_end = (threshold + edge_softness).clamp(0.0, 1.0);
        let soft_end_sq = soft_end * soft_end;
        let range_sq = soft_end_sq - threshold_sq;
        let blur_radius = edge_blur.round() as usize;
        let need_blur = blur_radius > 0;

        if need_blur {
            // ── 2-pass CPU: compute alpha → blur → composite ──
            let mut bufs = ALPHA_BUFS.with(|cell| cell.take());
            if bufs.alpha.len() != total {
                bufs.alpha.resize(total, 0.0);
                bufs.blur_h.resize(total, 0.0);
            }

            bufs.alpha.par_iter_mut().enumerate().for_each(|(i, alpha)| {
                let o = i * 4;
                let r = src[o] as f32 * RECIP_255;
                let g = src[o + 1] as f32 * RECIP_255;
                let b = src[o + 2] as f32 * RECIP_255;
                let (_, cb, cr) = rgb_to_ycbcr(r, g, b);
                let dc = cb - key_cb;
                let dr = cr - key_cr;
                let dist_sq = (dc * dc + dr * dr) * 0.5;
                *alpha = compute_key_alpha(dist_sq, threshold_sq, soft_end_sq, range_sq, edge_softness);
            });

            blur_alpha(&mut bufs.alpha, &mut bufs.blur_h, width, height, blur_radius);

            dst.par_chunks_mut(width * 4).enumerate().for_each(|(y, row)| {
                let src_offset = y * width * 4;
                for x in 0..width {
                    let i = src_offset + x * 4;
                    let o = x * 4;
                    let idx = y * width + x;
                    let r = src[i] as f32 * RECIP_255;
                    let g = src[i + 1] as f32 * RECIP_255;
                    let b = src[i + 2] as f32 * RECIP_255;
                    let a = src[i + 3] as f32 * RECIP_255;
                    write_pixel(row, o, r, g, b, a, bufs.alpha[idx], spill_suppression, show_matte, invert);
                }
            });

            ALPHA_BUFS.with(|cell| cell.replace(bufs));
        } else {
            // ── Fused single-pass CPU: compute + composite in one traversal ──
            dst.par_chunks_mut(width * 4).enumerate().for_each(|(y, row)| {
                let src_offset = y * width * 4;
                for x in 0..width {
                    let i = src_offset + x * 4;
                    let o = x * 4;
                    let r = src[i] as f32 * RECIP_255;
                    let g = src[i + 1] as f32 * RECIP_255;
                    let b = src[i + 2] as f32 * RECIP_255;
                    let a = src[i + 3] as f32 * RECIP_255;
                    let (_, cb, cr) = rgb_to_ycbcr(r, g, b);
                    let dc = cb - key_cb;
                    let dr = cr - key_cr;
                    let dist_sq = (dc * dc + dr * dr) * 0.5;
                    let key_alpha = compute_key_alpha(dist_sq, threshold_sq, soft_end_sq, range_sq, edge_softness);
                    write_pixel(row, o, r, g, b, a, key_alpha, spill_suppression, show_matte, invert);
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers (used by both CPU paths)
// ---------------------------------------------------------------------------

#[inline]
fn compute_key_alpha(dist_sq: f32, threshold_sq: f32, soft_end_sq: f32, range_sq: f32, edge_softness: f32) -> f32 {
    if dist_sq <= threshold_sq {
        0.0
    } else if edge_softness <= 0.0 || dist_sq >= soft_end_sq {
        1.0
    } else {
        let t = (dist_sq - threshold_sq) / range_sq;
        t * t * (3.0 - 2.0 * t)
    }
}

#[inline]
fn write_pixel(row: &mut [u8], o: usize, r: f32, g: f32, b: f32, a: f32, mut key_alpha: f32, spill_suppression: f32, show_matte: bool, invert: bool) {
    if invert {
        key_alpha = 1.0 - key_alpha;
    }
    if show_matte {
        let v = (key_alpha * 255.0).round() as u8;
        row[o] = v;
        row[o + 1] = v;
        row[o + 2] = v;
        row[o + 3] = 255;
        return;
    }

    let (out_r, out_g, out_b) = if spill_suppression > 0.0 && key_alpha < 1.0 {
        let spill = spill_suppression * (1.0 - key_alpha).sqrt();
        let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        (
            (r + (lum - r) * spill).clamp(0.0, 1.0),
            (g + (lum - g) * spill).clamp(0.0, 1.0),
            (b + (lum - b) * spill).clamp(0.0, 1.0),
        )
    } else {
        (r, g, b)
    };

    let out_a = a * key_alpha;
    row[o] = (out_r * out_a * 255.0).round() as u8;
    row[o + 1] = (out_g * out_a * 255.0).round() as u8;
    row[o + 2] = (out_b * out_a * 255.0).round() as u8;
    row[o + 3] = (out_a * 255.0).round() as u8;
}
