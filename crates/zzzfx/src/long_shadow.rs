use rayon::prelude::*;

use crate::settings::long_shadow::LongShadow;

const RCP_255: f32 = 1.0 / 255.0;

impl LongShadow {
    pub fn is_identity(&self) -> bool {
        let len = self.length.clamp(0.0, 1.0);
        let opacity = self.opacity.clamp(0.0, 1.0);
        let shadow_a = self.shadow_color_a.clamp(0.0, 1.0);
        len <= 0.0 || opacity <= 0.0 || shadow_a <= 0.0
    }

    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }
        if self.is_identity() {
            dst[..len].copy_from_slice(&src[..len]);
            return;
        }
        self.render_cpu(src, dst, width, height);
    }

    fn render_cpu(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let total = width * height;

        let angle = self.angle.clamp(0.0, 360.0);
        let length = self.length.clamp(0.0, 1.0);
        let softness = self.softness.clamp(0.0, 1.0);
        let fade = self.fade.clamp(0.0, 1.0);
        let opacity = self.opacity.clamp(0.0, 1.0);
        let threshold = self.alpha_threshold.clamp(0.0, 1.0);
        let source_opacity = self.source_opacity.clamp(0.0, 1.0);

        let sr = self.shadow_color_r.clamp(0.0, 1.0);
        let sg = self.shadow_color_g.clamp(0.0, 1.0);
        let sb = self.shadow_color_b.clamp(0.0, 1.0);
        let sa = self.shadow_color_a.clamp(0.0, 1.0);

        // Direction vector (shadow casts FROM this direction)
        let rad = angle.to_radians();
        let dx = rad.cos();
        let dy = rad.sin();

        // Length in pixels based on frame diagonal
        let diagonal = ((width * width + height * height) as f32).sqrt();
        let length_px = (length * diagonal).round() as usize;
        if length_px == 0 {
            dst[..total * 4].copy_from_slice(&src[..total * 4]);
            return;
        }

        // Origin offset in pixels (0.5 = center, no net offset)
        let ox = ((self.offset_x.clamp(0.0, 1.0) - 0.5) * width as f32).round() as isize;
        let oy = ((self.offset_y.clamp(0.0, 1.0) - 0.5) * height as f32).round() as isize;

        // Step 1: Build binary mask from source alpha
        let mut mask = vec![false; total];
        mask.par_iter_mut().enumerate().for_each(|(i, m)| {
            let a = src[i * 4 + 3] as f32 * RCP_255;
            *m = a >= threshold;
        });

        // Step 2: Per-pixel ray march — find nearest opaque source pixel
        //   for each output pixel along the reverse shadow direction.
        let mut shadow_accum = vec![0.0f32; total];
        let length_px_f = length_px as f32;
        let w = width as isize;
        let h = height as isize;

        shadow_accum
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, out)| {
                let px = (i % width) as isize;
                let py = (i / width) as isize;

                for d in 1..=length_px {
                    let sx = px - (dx * d as f32).round() as isize - ox;
                    let sy = py - (dy * d as f32).round() as isize - oy;

                    if sx >= 0 && sx < w && sy >= 0 && sy < h {
                        if mask[(sy as usize) * width + (sx as usize)] {
                            let t = d as f32 / length_px_f;
                            *out = 1.0 - fade * t;
                            break;
                        }
                    }
                }
            });

        // Step 3: Apply softness (box blur of shadow mask)
        if softness > 0.0 {
            let blur_radius = (softness * length_px_f * 0.3).ceil() as isize;
            if blur_radius > 0 {
                let radius = blur_radius as usize;
                let mut blurred = vec![0.0f32; total];
                blurred
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(idx, out_blur)| {
                        let x = idx % width;
                        let y = idx / width;
                        let x0 = if x >= radius { x - radius } else { 0 };
                        let x1 = (x + radius + 1).min(width);
                        let y0 = if y >= radius { y - radius } else { 0 };
                        let y1 = (y + radius + 1).min(height);
                        let mut sum = 0.0f64;
                        let mut count = 0u32;
                        for j in y0..y1 {
                            for i in x0..x1 {
                                sum += shadow_accum[j * width + i] as f64;
                                count += 1;
                            }
                        }
                        *out_blur = (sum / count as f64) as f32;
                    });
                shadow_accum = blurred;
            }
        }

        // Step 4: Composite — source OVER shadow (shadow behind source)
        dst.par_chunks_mut(width * 4)
            .enumerate()
            .for_each(|(y, row)| {
                for x in 0..width {
                    let i = y * width + x;
                    let o = x * 4;

                    let shadow_alpha = shadow_accum[i] * sa * opacity;

                    let src_r = src[i * 4] as f32 * RCP_255;
                    let src_g = src[i * 4 + 1] as f32 * RCP_255;
                    let src_b = src[i * 4 + 2] as f32 * RCP_255;
                    let src_a = src[i * 4 + 3] as f32 * RCP_255 * source_opacity;

                    // source OVER shadow (premultiplied):
                    // C = src_rgb*src_a + shadow_rgb*shadow_a*(1-src_a)
                    let inv = 1.0 - src_a;
                    row[o] =
                        ((src_r * src_a + sr * shadow_alpha * inv).clamp(0.0, 1.0) * 255.0)
                            .round() as u8;
                    row[o + 1] =
                        ((src_g * src_a + sg * shadow_alpha * inv).clamp(0.0, 1.0) * 255.0)
                            .round() as u8;
                    row[o + 2] =
                        ((src_b * src_a + sb * shadow_alpha * inv).clamp(0.0, 1.0) * 255.0)
                            .round() as u8;
                    row[o + 3] =
                        ((src_a + shadow_alpha * inv).clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            });
    }
}
