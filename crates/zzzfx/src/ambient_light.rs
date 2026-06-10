use std::cell::RefCell;

use rayon::prelude::*;

use crate::blend::RECIP_255;
use crate::settings::ambient_light::AmbientLight;

// ---------------------------------------------------------------------------
// Thread-local buffers
// ---------------------------------------------------------------------------

struct AmbientBufs {
    /// Small-radius blur: local ambient color right behind each edge pixel
    ambient_local: Vec<[f32; 3]>,
    /// Large-radius blur: global ambient color temperature for interior tint
    ambient_global: Vec<[f32; 3]>,
    /// Reusable intermediate for separable blur passes
    blur_h: Vec<[f32; 3]>,
    /// Edge distance transform: pixel distance to nearest transparent pixel
    edge_dist: Vec<f32>,
    /// Smoothstepped edge factor: 0 = at alpha boundary, 1 = far inside
    edge_factor: Vec<f32>,
}

impl Default for AmbientBufs {
    fn default() -> Self {
        Self {
            ambient_local: Vec::new(),
            ambient_global: Vec::new(),
            blur_h: Vec::new(),
            edge_dist: Vec::new(),
            edge_factor: Vec::new(),
        }
    }
}

thread_local! {
    static BUFS: RefCell<AmbientBufs> = RefCell::new(AmbientBufs::default());
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl AmbientLight {
    /// Returns true when the effect is a complete no-op (identity composite).
    pub fn is_identity(&self) -> bool {
        let intensity = self.intensity.clamp(0.0, 1.0);
        let light_wrap = self.light_wrap.clamp(0.0, 1.0);
        let ambient_tint = self.ambient_tint.clamp(0.0, 1.0);
        let fg_op = self.fg_opacity.clamp(0.0, 1.0);
        let bg_op = self.bg_opacity.clamp(0.0, 1.0);
        (intensity <= 0.0 || (light_wrap <= 0.0 && ambient_tint <= 0.0))
            && fg_op >= 1.0
            && bg_op >= 1.0
            && !self.swap_fg_bg
    }

    pub fn apply_effect(
        &self,
        fg: &[u8],
        bg: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) {
        let total = width * height;
        let len = total * 4;
        if fg.len() < len || bg.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }

        // --- Handle swap ---
        let (fg, bg) = if self.swap_fg_bg {
            (bg, fg)
        } else {
            (fg, bg)
        };

        if self.is_identity() {
            simple_over(fg, bg, dst, width, height, 1.0, 1.0);
            return;
        }

        let intensity = self.intensity.clamp(0.0, 1.0);
        let edge_width = self.edge_width.clamp(0.0, 1.0);
        let light_wrap = self.light_wrap.clamp(0.0, 1.0);
        let ambient_tint = self.ambient_tint.clamp(0.0, 1.0);
        let blur_radius = (self.blur_radius.clamp(0.0, 200.0).round() as usize).min(200);
        let brightness = self.brightness.clamp(0.0, 2.0);
        let fg_opacity = self.fg_opacity.clamp(0.0, 1.0);
        let bg_opacity = self.bg_opacity.clamp(0.0, 1.0);

        let mut bufs = BUFS.with(|cell| cell.take());

        if bufs.ambient_local.len() != total {
            bufs.ambient_local.resize(total, [0.0; 3]);
            bufs.ambient_global.resize(total, [0.0; 3]);
            bufs.blur_h.resize(total, [0.0; 3]);
            bufs.edge_dist.resize(total, 0.0);
            bufs.edge_factor.resize(total, 0.0);
        }

        // Stage 1: Edge proximity + edge factor (CPU)
        compute_edge_proximity(fg, &mut bufs.edge_dist, width, height);
        let max_dim = width.max(height) as f32;
        let edge_px = edge_width * max_dim;
        compute_edge_factors(&bufs.edge_dist, &mut bufs.edge_factor, edge_px);

        // Stage 2: Dual-scale Gaussian blur on background (CPU)
        let local_r = if blur_radius > 0 { (blur_radius / 4).max(1) } else { 0 };
        let global_r = blur_radius;

        if local_r > 0 {
            gaussian_blur_bg(bg, &mut bufs.ambient_local, &mut bufs.blur_h, width, height, local_r);
        } else {
            copy_bg_rgb(bg, &mut bufs.ambient_local, width);
        }

        if global_r > 0 {
            gaussian_blur_bg(bg, &mut bufs.ambient_global, &mut bufs.blur_h, width, height, global_r);
        } else {
            copy_bg_rgb(bg, &mut bufs.ambient_global, width);
        }

        // Stage 3: Try GPU composite
        #[cfg(feature = "gpu")]
        {
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::gpu::ambient_light::try_ambient_light_gpu_render(
                    &bufs.ambient_local, &bufs.ambient_global,
                    fg, bg, &bufs.edge_factor,
                    self, dst, width, height,
                )
            }));
            if let Ok(Ok(true)) = gpu_result {
                BUFS.with(|cell| cell.replace(bufs));
                return;
            }
        }

        // Stage 4: CPU composite fallback
        composite_ambient(
            fg, bg, dst,
            &bufs.ambient_local, &bufs.ambient_global, &bufs.edge_factor,
            width, height,
            intensity, light_wrap, ambient_tint, brightness,
            fg_opacity, bg_opacity,
        );

        BUFS.with(|cell| cell.replace(bufs));
    }
}

// ---------------------------------------------------------------------------
// Identity-path OVER composite
// ---------------------------------------------------------------------------

fn simple_over(
    fg: &[u8], bg: &[u8], dst: &mut [u8],
    width: usize, _height: usize,
    fg_opacity: f32, bg_opacity: f32,
) {
    dst.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..width {
                let i = y * width + x;
                let o = x * 4;

                let fa = fg[i * 4 + 3] as f32 * RECIP_255 * fg_opacity;
                let fr = fg[i * 4] as f32 * RECIP_255;
                let fg_g = fg[i * 4 + 1] as f32 * RECIP_255;
                let fb = fg[i * 4 + 2] as f32 * RECIP_255;

                let ba = bg[i * 4 + 3] as f32 * RECIP_255 * bg_opacity;
                let br = bg[i * 4] as f32 * RECIP_255;
                let bg_g = bg[i * 4 + 1] as f32 * RECIP_255;
                let bb = bg[i * 4 + 2] as f32 * RECIP_255;

                let inv_fa = 1.0 - fa;
                row[o] = ((fr * fa + br * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
                row[o + 1] = ((fg_g * fa + bg_g * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
                row[o + 2] = ((fb * fa + bb * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
                row[o + 3] = ((fa + ba * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });
}

// ---------------------------------------------------------------------------
// Stage 1: Gaussian-approximating blur (3 box blur iterations)
// ---------------------------------------------------------------------------

fn copy_bg_rgb(bg: &[u8], dst: &mut [[f32; 3]], _width: usize) {
    dst.par_iter_mut().enumerate().for_each(|(i, out)| {
        let o = i * 4;
        out[0] = bg[o] as f32 * RECIP_255;
        out[1] = bg[o + 1] as f32 * RECIP_255;
        out[2] = bg[o + 2] as f32 * RECIP_255;
    });
}

fn gaussian_blur_bg(
    bg: &[u8],
    dst: &mut [[f32; 3]],
    blur_h: &mut [[f32; 3]],
    width: usize,
    height: usize,
    radius: usize,
) {
    let r = radius.min(width.max(height) / 2).max(1);

    // Iteration 1: bg → blur_h (H) → dst (V)
    box_blur_h(bg, blur_h, width, height, r);
    box_blur_v(blur_h, dst, width, height, r);

    // Iteration 2
    box_blur_h_float(dst, blur_h, width, height, r);
    box_blur_v(blur_h, dst, width, height, r);

    // Iteration 3
    box_blur_h_float(dst, blur_h, width, height, r);
    box_blur_v(blur_h, dst, width, height, r);
}

fn box_blur_h(
    src: &[u8],
    dst: &mut [[f32; 3]],
    width: usize,
    _height: usize,
    r: usize,
) {
    let window = (2 * r + 1) as f32;
    let scale = RECIP_255 as f64 / window as f64;

    dst.par_iter_mut().enumerate().for_each(|(i, out)| {
        let y = i / width;
        let x = i % width;
        let x0 = x.saturating_sub(r);
        let x1 = (x + r + 1).min(width);
        let mut sum = [0.0f64; 3];
        for sx in x0..x1 {
            let o = (y * width + sx) * 4;
            sum[0] += src[o] as f64;
            sum[1] += src[o + 1] as f64;
            sum[2] += src[o + 2] as f64;
        }
        out[0] = (sum[0] * scale) as f32;
        out[1] = (sum[1] * scale) as f32;
        out[2] = (sum[2] * scale) as f32;
    });
}

fn box_blur_h_float(
    src: &[[f32; 3]],
    dst: &mut [[f32; 3]],
    width: usize,
    _height: usize,
    r: usize,
) {
    dst.par_iter_mut().enumerate().for_each(|(i, out)| {
        let y = i / width;
        let x = i % width;
        let x0 = x.saturating_sub(r);
        let x1 = (x + r + 1).min(width);
        let actual = (x1 - x0) as f32;
        let mut sum = [0.0f64; 3];
        for sx in x0..x1 {
            let idx = y * width + sx;
            sum[0] += src[idx][0] as f64;
            sum[1] += src[idx][1] as f64;
            sum[2] += src[idx][2] as f64;
        }
        out[0] = (sum[0] / actual as f64) as f32;
        out[1] = (sum[1] / actual as f64) as f32;
        out[2] = (sum[2] / actual as f64) as f32;
    });
}

fn box_blur_v(
    src: &[[f32; 3]],
    dst: &mut [[f32; 3]],
    width: usize,
    height: usize,
    r: usize,
) {
    dst.par_iter_mut().enumerate().for_each(|(i, out)| {
        let y = i / width;
        let x = i % width;
        let y0 = y.saturating_sub(r);
        let y1 = (y + r + 1).min(height);
        let actual = (y1 - y0) as f32;
        let mut sum = [0.0f64; 3];
        for sy in y0..y1 {
            let idx = sy * width + x;
            sum[0] += src[idx][0] as f64;
            sum[1] += src[idx][1] as f64;
            sum[2] += src[idx][2] as f64;
        }
        out[0] = (sum[0] / actual as f64) as f32;
        out[1] = (sum[1] / actual as f64) as f32;
        out[2] = (sum[2] / actual as f64) as f32;
    });
}

// ---------------------------------------------------------------------------
// Stage 2: Edge proximity via distance transform on alpha
// ---------------------------------------------------------------------------

fn compute_edge_proximity(
    fg: &[u8],
    dist: &mut [f32],
    width: usize,
    height: usize,
) {
    let w = width as isize;
    let h = height as isize;

    dist.par_iter_mut().enumerate().for_each(|(i, d)| {
        *d = if fg[i * 4 + 3] == 0 { 0.0 } else { 1e10 };
    });

    // First pass: top-left → bottom-right
    for y in 0..h {
        let row_base = (y * w) as usize;
        for x in 0..w {
            let idx = row_base + x as usize;
            if dist[idx] == 0.0 { continue; }
            let mut best = dist[idx];
            if x > 0 { best = best.min(dist[idx - 1] + 1.0); }
            if y > 0 { best = best.min(dist[idx - width] + 1.0); }
            if x > 0 && y > 0 { best = best.min(dist[idx - width - 1] + 1.414); }
            if x + 1 < w && y > 0 { best = best.min(dist[idx - width + 1] + 1.414); }
            dist[idx] = best;
        }
    }

    // Second pass: bottom-right → top-left
    for y in (0..h).rev() {
        let row_base = (y * w) as usize;
        for x in (0..w).rev() {
            let idx = row_base + x as usize;
            if dist[idx] == 0.0 { continue; }
            let mut best = dist[idx];
            if x + 1 < w { best = best.min(dist[idx + 1] + 1.0); }
            if y + 1 < h { best = best.min(dist[idx + width] + 1.0); }
            if x > 0 && y + 1 < h { best = best.min(dist[idx + width - 1] + 1.414); }
            if x + 1 < w && y + 1 < h { best = best.min(dist[idx + width + 1] + 1.414); }
            dist[idx] = best;
        }
    }
}

// ---------------------------------------------------------------------------
// Stage 3: Edge factor — smoothstep from edge (0) to interior (1)
//   ef = 0   at alpha boundary (d=0)
//   ef = 0.5 at d = edge_px/2
//   ef = 1   at d >= edge_px
//   When edge_px=0: ef remains at the initial 0.0 fill → all pixels treated as "at edge"
// ---------------------------------------------------------------------------

fn compute_edge_factors(
    edge_dist: &[f32],
    edge_factor: &mut [f32],
    edge_px: f32,
) {
    if edge_px <= 0.0 {
        // Zero edge width: all pixels are beyond the edge zone → ef=1 (no light)
        edge_factor.fill(1.0);
        return;
    }
    edge_factor
        .par_iter_mut()
        .enumerate()
        .for_each(|(i, ef)| {
            let d = edge_dist[i];
            if d <= 0.0 {
                // Transparent pixels: at the boundary
                *ef = 0.0;
            } else {
                let t = (d / edge_px).clamp(0.0, 1.0);
                *ef = t * t * (3.0 - 2.0 * t); // smoothstep: 0→edge, 1→deep interior
            }
        });
}

// ---------------------------------------------------------------------------
// Stage 4: Composite with localized light wrap + ambient tint
// ---------------------------------------------------------------------------
// light_amount = 1.0 - ef  (1.0 at edge, 0.0 beyond edge_px)
// - Light wrap: scales with light_amount * light_wrap * intensity
// - Ambient tint: scales with light_amount * ambient_tint * intensity
// Both are purely spatial — no alpha-dependent gating.
// fg_opacity and bg_opacity modulate the final alpha channels.

fn composite_ambient(
    fg: &[u8],
    bg: &[u8],
    dst: &mut [u8],
    ambient_local: &[[f32; 3]],
    ambient_global: &[[f32; 3]],
    edge_factor: &[f32],
    width: usize,
    _height: usize,
    intensity: f32,
    light_wrap: f32,
    ambient_tint: f32,
    brightness: f32,
    fg_opacity: f32,
    bg_opacity: f32,
) {
    let use_opacity = fg_opacity < 1.0 || bg_opacity < 1.0;

    dst.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, row)| {
            for x in 0..width {
                let i = y * width + x;
                let o = x * 4;

                let fg_r = fg[i * 4] as f32 * RECIP_255;
                let fg_g = fg[i * 4 + 1] as f32 * RECIP_255;
                let fg_b = fg[i * 4 + 2] as f32 * RECIP_255;
                let fg_a = if use_opacity {
                    fg[i * 4 + 3] as f32 * RECIP_255 * fg_opacity
                } else {
                    fg[i * 4 + 3] as f32 * RECIP_255
                };

                let bg_r = bg[i * 4] as f32 * RECIP_255;
                let bg_g = bg[i * 4 + 1] as f32 * RECIP_255;
                let bg_b = bg[i * 4 + 2] as f32 * RECIP_255;
                let bg_a = if use_opacity {
                    bg[i * 4 + 3] as f32 * RECIP_255 * bg_opacity
                } else {
                    bg[i * 4 + 3] as f32 * RECIP_255
                };

                if fg_a <= 0.0 {
                    if bg_opacity < 1.0 {
                        row[o] = (bg_r * bg_a * 255.0).round() as u8;
                        row[o + 1] = (bg_g * bg_a * 255.0).round() as u8;
                        row[o + 2] = (bg_b * bg_a * 255.0).round() as u8;
                        row[o + 3] = (bg_a * 255.0).round() as u8;
                    } else {
                        row[o] = bg[i * 4];
                        row[o + 1] = bg[i * 4 + 1];
                        row[o + 2] = bg[i * 4 + 2];
                        row[o + 3] = bg[i * 4 + 3];
                    }
                    continue;
                }

                let ef = edge_factor[i];
                // light_amount: 1.0 at alpha edge (ef=0), 0.0 beyond edge_px (ef=1)
                let light_amount = 1.0 - ef;

                // --- Local ambient (small blur): light wrap color ---
                let local_r = ambient_local[i][0] * brightness;
                let local_g = ambient_local[i][1] * brightness;
                let local_b = ambient_local[i][2] * brightness;
                let local_lum = 0.2126 * local_r + 0.7152 * local_g + 0.0722 * local_b;

                // --- Global ambient (large blur): tint color ---
                let glob_r = ambient_global[i][0] * brightness;
                let glob_g = ambient_global[i][1] * brightness;
                let glob_b = ambient_global[i][2] * brightness;

                // === 1. Ambient tint: chroma shift toward global ambient color ===
                let tint = light_amount * ambient_tint * intensity;
                let fg_lum = 0.2126 * fg_r + 0.7152 * fg_g + 0.0722 * fg_b;
                let glob_lum = 0.2126 * glob_r + 0.7152 * glob_g + 0.0722 * glob_b;
                let glob_lum = if glob_lum < 0.001 { 0.001 } else { glob_lum };
                let tint_target_r = fg_lum * (glob_r / glob_lum);
                let tint_target_g = fg_lum * (glob_g / glob_lum);
                let tint_target_b = fg_lum * (glob_b / glob_lum);

                let mut mod_r = fg_r + (tint_target_r - fg_r) * tint;
                let mut mod_g = fg_g + (tint_target_g - fg_g) * tint;
                let mut mod_b = fg_b + (tint_target_b - fg_b) * tint;

                // === 2. Light wrap: localized, brightness-gated ===
                let wrap = light_amount * light_wrap * intensity;
                // Brightness gate: wrap stronger where local background is brighter
                let brightness_gate = (local_lum * 2.0).clamp(0.0, 1.0);
                let wrap_strength = wrap * brightness_gate;
                mod_r = (mod_r + local_r * wrap_strength).clamp(0.0, 1.0);
                mod_g = (mod_g + local_g * wrap_strength).clamp(0.0, 1.0);
                mod_b = (mod_b + local_b * wrap_strength).clamp(0.0, 1.0);

                // === 3. Standard OVER: modified fg over background ===
                let inv_fa = 1.0 - fg_a;
                row[o] = ((mod_r * fg_a + bg_r * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
                row[o + 1] = ((mod_g * fg_a + bg_g * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
                row[o + 2] = ((mod_b * fg_a + bg_b * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
                row[o + 3] = ((fg_a + bg_a * inv_fa).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });
}
