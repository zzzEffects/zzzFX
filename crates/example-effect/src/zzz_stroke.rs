use std::cell::RefCell;

use crate::blend::{self, blend_channel, is_stencil_or_outline};
use crate::settings::zzz_stroke::{
    BlendMode, FillMode, StrokePosition, ZzzStroke,
};

// Per-thread reusable buffers to avoid allocation churn across frames.
struct StrokeBufs {
    mask: Vec<bool>,
    dists: Vec<f32>,
    nearest_cols: Vec<u32>,
}

impl Default for StrokeBufs {
    fn default() -> Self {
        Self {
            mask: Vec::new(),
            dists: Vec::new(),
            nearest_cols: Vec::new(),
        }
    }
}

thread_local! {
    static BUFS: RefCell<StrokeBufs> = RefCell::new(StrokeBufs::default());
}

impl ZzzStroke {
    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        assert!(src.len() >= len, "source buffer too small");
        assert!(dst.len() >= len, "destination buffer too small");

        if width == 0 || height == 0 {
            return;
        }

        let total_pixels = width * height;
        let sw = self.stroke_width.clamp(0.0, 1.0);
        let threshold = self.alpha_threshold.clamp(0.0, 1.0);
        let feather = self.stroke_feathering.clamp(0.0, 1.0);
        let src_opacity = self.source_opacity.clamp(0.0, 1.0);
        let stroke_a = self.stroke_color_a.clamp(0.0, 1.0);
        let max_dim = width.max(height) as f32;
        let w_px = sw * max_dim;
        let feather_px = feather * w_px;

        // Fast path: no stroke
        if w_px <= 0.0 || stroke_a <= 0.0 {
            if (src_opacity - 1.0).abs() < f32::EPSILON {
                dst[..len].copy_from_slice(&src[..len]);
            } else {
                dst[..len].copy_from_slice(&src[..len]);
                for p in dst.chunks_mut(4) {
                    p[3] = (p[3] as f32 * src_opacity).round() as u8;
                }
            }
            return;
        }

        BUFS.with(|bufs_cell| {
            let bufs = &mut *bufs_cell.borrow_mut();

            // Resize reusable buffers (only grows, never shrinks)
            bufs.mask.resize(total_pixels, false);
            bufs.dists.resize(total_pixels, 0.0f32);
            bufs.nearest_cols.resize(total_pixels, 0u32);

            let mask = &mut bufs.mask;
            let dists = &mut bufs.dists;
            let nearest_cols = &mut bufs.nearest_cols;

            // Stage 1: Build binary mask
            for (i, chunk) in src.chunks(4).enumerate() {
                mask[i] = (chunk[3] as f32 / 255.0) >= threshold;
            }

            // Stage 2+3: Initialize distance transform (inline edge detection)
            for y in 0..height {
                for x in 0..width {
                    let idx = y * width + x;
                    if !mask[idx] {
                        dists[idx] = 1e10;
                        nearest_cols[idx] = 0;
                        continue;
                    }
                    // Check if edge pixel (mask pixel with a non-mask 4-neighbor)
                    let is_edge = (x > 0 && !mask[idx - 1])
                        || (x + 1 < width && !mask[idx + 1])
                        || (y > 0 && !mask[idx - width])
                        || (y + 1 < height && !mask[idx + width]);
                    if is_edge {
                        dists[idx] = 0.0;
                        nearest_cols[idx] = x as u32;
                    } else {
                        dists[idx] = 1e10;
                        nearest_cols[idx] = 0;
                    }
                }
            }

            let (cardinal, diagonal): (f32, f32) = if self.use_sharp_corners {
                (1.0, 1.0)
            } else {
                (3.0, 4.0)
            };

            // Forward pass: top → bottom, left → right
            for y in 0..height {
                let row_offset = y * width;
                for x in 0..width {
                    let idx = row_offset + x;
                    if y > 0 && x > 0 {
                        try_update(dists, nearest_cols, idx, idx - width - 1, diagonal);
                    }
                    if y > 0 {
                        try_update(dists, nearest_cols, idx, idx - width, cardinal);
                    }
                    if y > 0 && x + 1 < width {
                        try_update(dists, nearest_cols, idx, idx - width + 1, diagonal);
                    }
                    if x > 0 {
                        try_update(dists, nearest_cols, idx, idx - 1, cardinal);
                    }
                }
            }

            // Backward pass: bottom → top, right → left
            for y in (0..height).rev() {
                let row_offset = y * width;
                for x in (0..width).rev() {
                    let idx = row_offset + x;
                    if x + 1 < width {
                        try_update(dists, nearest_cols, idx, idx + 1, cardinal);
                    }
                    if y + 1 < height && x > 0 {
                        try_update(dists, nearest_cols, idx, idx + width - 1, diagonal);
                    }
                    if y + 1 < height {
                        try_update(dists, nearest_cols, idx, idx + width, cardinal);
                    }
                    if y + 1 < height && x + 1 < width {
                        try_update(dists, nearest_cols, idx, idx + width + 1, diagonal);
                    }
                }
            }

            // Scale distances
            let scale = if self.use_sharp_corners { 1.0 } else { 3.0 };
            for d in dists.iter_mut() {
                *d /= scale;
            }

            // Stage 4: Compute stroke per pixel
            let stroke_r = self.stroke_color_r.clamp(0.0, 1.0);
            let stroke_g = self.stroke_color_g.clamp(0.0, 1.0);
            let stroke_b = self.stroke_color_b.clamp(0.0, 1.0);
            let pos = self.stroke_position;
            let fmode = self.fill_mode;
            let gradient = self.gradient.clone();
            let bmode = self.blend_mode;

            for pixel_idx in 0..total_pixels {
                let x = pixel_idx % width;
                let y = pixel_idx / width;
                let idx = pixel_idx;
                let inside = mask[idx];
                let d = dists[idx];

                let stroke_alpha_local = match pos {
                    StrokePosition::Outer => {
                        if inside {
                            0.0
                        } else {
                            smoothstep_edge(w_px + feather_px, w_px - feather_px, d)
                        }
                    }
                    StrokePosition::Inner => {
                        if !inside {
                            0.0
                        } else {
                            smoothstep_edge(w_px + feather_px, w_px - feather_px, d)
                        }
                    }
                    StrokePosition::Center => {
                        let half_w = w_px * 0.5;
                        smoothstep_edge(half_w + feather_px, half_w - feather_px, d)
                    }
                };

                let sa = stroke_alpha_local * stroke_a;
                let is_stroke = sa > 0.0;

                let (sr, sg, sb) = if is_stroke {
                    match fmode {
                        FillMode::SolidColor => (stroke_r, stroke_g, stroke_b),
                        FillMode::DistanceGradient => {
                            if let Some(ref g) = gradient {
                                let gx = g.start_x * width as f32;
                                let gy = g.start_y * height as f32;
                                let dx = x as f32 - gx;
                                let dy = y as f32 - gy;
                                let dist = (dx * dx + dy * dy).sqrt();
                                let max_dist = ((width as f32).powi(2) + (height as f32).powi(2)).sqrt();
                                let t = (dist / max_dist).clamp(0.0, 1.0);
                                (
                                    g.start_color_r + t * (g.end_color_r - g.start_color_r),
                                    g.start_color_g + t * (g.end_color_g - g.start_color_g),
                                    g.start_color_b + t * (g.end_color_b - g.start_color_b),
                                )
                            } else {
                                (stroke_r, stroke_g, stroke_b)
                            }
                        }
                        FillMode::Gradient => {
                            if let Some(ref g) = gradient {
                                let dx = g.end_x - g.start_x;
                                let dy = g.end_y - g.start_y;
                                let len_sq = dx * dx + dy * dy;
                                let gx = g.start_x * width as f32;
                                let gy = g.start_y * height as f32;
                                let px = x as f32 - gx;
                                let py = y as f32 - gy;
                                let t = if len_sq > 0.0 {
                                    ((px * dx + py * dy) / len_sq).clamp(0.0, 1.0)
                                } else {
                                    0.0
                                };
                                (
                                    g.start_color_r + t * (g.end_color_r - g.start_color_r),
                                    g.start_color_g + t * (g.end_color_g - g.start_color_g),
                                    g.start_color_b + t * (g.end_color_b - g.start_color_b),
                                )
                            } else {
                                (stroke_r, stroke_g, stroke_b)
                            }
                        }
                        FillMode::SourceColorExtension => {
                            let nc = nearest_cols[idx] as usize;
                            let src_idx = (y * width + nc) * 4;
                            if src_idx + 2 < src.len() {
                                (
                                    src[src_idx] as f32 / 255.0,
                                    src[src_idx + 1] as f32 / 255.0,
                                    src[src_idx + 2] as f32 / 255.0,
                                )
                            } else {
                                (stroke_r, stroke_g, stroke_b)
                            }
                        }
                    }
                } else {
                    (0.0, 0.0, 0.0)
                };

                let out_idx = pixel_idx * 4;
                let src_r = src[out_idx] as f32 / 255.0;
                let src_g = src[out_idx + 1] as f32 / 255.0;
                let src_b = src[out_idx + 2] as f32 / 255.0;

                if sa > 0.0 {
                    // RNG for dissolve — simple hash per pixel
                    let mut rng_call = || {
                        let h = (pixel_idx as u32).wrapping_mul(0x45d9f3b);
                        let h = (h ^ (h >> 16)).wrapping_mul(0x85ebca6b);
                        let h = h ^ (h >> 13);
                        h as f32 / u32::MAX as f32
                    };

                    let blended_r = blend_channel(bmode, src_r, sr, sa, &mut rng_call);
                    let blended_g = blend_channel(bmode, src_g, sg, sa, &mut rng_call);
                    let blended_b = blend_channel(bmode, src_b, sb, sa, &mut rng_call);

                    if is_stencil_or_outline(bmode) {
                        let stencil_a = match bmode {
                            BlendMode::StencilAlpha => sa,
                            BlendMode::StencilLuma => sa * blend::luminance(sr, sg, sb),
                            BlendMode::OutlineAlpha => sa,
                            BlendMode::OutlineLuma => sa * blend::luminance(sr, sg, sb),
                            _ => sa,
                        };

                        if matches!(bmode, BlendMode::OutlineAlpha | BlendMode::OutlineLuma) {
                            dst[out_idx] = (sr * 255.0).round() as u8;
                            dst[out_idx + 1] = (sg * 255.0).round() as u8;
                            dst[out_idx + 2] = (sb * 255.0).round() as u8;
                            dst[out_idx + 3] = (stencil_a * 255.0).round() as u8;
                        } else {
                            dst[out_idx] = (blended_r * 255.0).round() as u8;
                            dst[out_idx + 1] = (blended_g * 255.0).round() as u8;
                            dst[out_idx + 2] = (blended_b * 255.0).round() as u8;
                            dst[out_idx + 3] = (stencil_a * 255.0).round() as u8;
                        }
                    } else {
                        let inv = 1.0 - sa;
                        dst[out_idx] = ((blended_r * sa + src_r * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
                        dst[out_idx + 1] = ((blended_g * sa + src_g * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
                        dst[out_idx + 2] = ((blended_b * sa + src_b * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
                        dst[out_idx + 3] = src[out_idx + 3];
                    }
                } else {
                    dst[out_idx] = (src_r * 255.0).round() as u8;
                    dst[out_idx + 1] = (src_g * 255.0).round() as u8;
                    dst[out_idx + 2] = (src_b * 255.0).round() as u8;
                    dst[out_idx + 3] = src[out_idx + 3];
                }

                if (src_opacity - 1.0).abs() > f32::EPSILON {
                    dst[out_idx + 3] = (dst[out_idx + 3] as f32 * src_opacity).round() as u8;
                }
            }
        });
    }
}

#[inline]
fn try_update(
    dists: &mut [f32],
    nearest_cols: &mut [u32],
    idx: usize,
    neighbor: usize,
    weight: f32,
) {
    let new_dist = dists[neighbor] + weight;
    if new_dist < dists[idx] {
        dists[idx] = new_dist;
        nearest_cols[idx] = nearest_cols[neighbor];
    }
}

#[inline]
fn smoothstep_edge(edge0: f32, edge1: f32, x: f32) -> f32 {
    if x >= edge0 {
        0.0
    } else if x <= edge1 {
        1.0
    } else {
        let t = (x - edge1) / (edge0 - edge1);
        t * t * (3.0 - 2.0 * t)
    }
}
