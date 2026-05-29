use rayon::prelude::*;

use crate::settings::solid::{BlendMode, SolidColorBlend};
use crate::RECIP_255;

impl SolidColorBlend {
    /// Blend the entire image with a solid color using the selected blend mode.
    ///
    /// The buffer is RGBA interleaved, 1 byte per channel. Alpha is copied through unchanged.
    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }

        let a = self.color_a.clamp(0.0, 1.0);

        // Fast path: no blend
        if a == 0.0 {
            dst[..len].copy_from_slice(&src[..len]);
            return;
        }

        #[cfg(feature = "gpu")]
        {
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::gpu::solid_blend::try_solid_blend_gpu_render(self, src, dst, width, height)
            }));
            match gpu_result {
                Ok(Ok(true)) => return,
                _ => {} // GPU unavailable — fall through to CPU
            }
        }

        self.apply_effect_cpu(src, dst, width, height);
    }

    fn apply_effect_cpu(&self, src: &[u8], dst: &mut [u8], width: usize, _height: usize) {
        let a = self.color_a.clamp(0.0, 1.0);
        let cr = self.color_r.clamp(0.0, 1.0);
        let cg = self.color_g.clamp(0.0, 1.0);
        let cb = self.color_b.clamp(0.0, 1.0);

        let sr = cr;
        let sg = cg;
        let sb = cb;
        let af = a;
        let inv = 1.0 - af;
        let row_bytes = width * 4;

        match self.blend_mode {
            BlendMode::Normal => {
                dst.par_chunks_mut(row_bytes)
                    .enumerate()
                    .for_each(|(y, row)| {
                        let src_offset = y * row_bytes;
                        for x in 0..width {
                            let i = src_offset + x * 4;
                            let o = x * 4;
                            row[o] = (src[i] as f32 * RECIP_255 * inv + sr * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 1] = (src[i + 1] as f32 * RECIP_255 * inv + sg * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 2] = (src[i + 2] as f32 * RECIP_255 * inv + sb * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 3] = src[i + 3];
                        }
                    });
            }
            BlendMode::Multiply => {
                dst.par_chunks_mut(row_bytes)
                    .enumerate()
                    .for_each(|(y, row)| {
                        let src_offset = y * row_bytes;
                        for x in 0..width {
                            let i = src_offset + x * 4;
                            let o = x * 4;
                            let ir = src[i] as f32 * RECIP_255;
                            let ig = src[i + 1] as f32 * RECIP_255;
                            let ib = src[i + 2] as f32 * RECIP_255;
                            row[o] = (ir * inv + ir * sr * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 1] = (ig * inv + ig * sg * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 2] = (ib * inv + ib * sb * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 3] = src[i + 3];
                        }
                    });
            }
            BlendMode::Screen => {
                dst.par_chunks_mut(row_bytes)
                    .enumerate()
                    .for_each(|(y, row)| {
                        let src_offset = y * row_bytes;
                        for x in 0..width {
                            let i = src_offset + x * 4;
                            let o = x * 4;
                            let ir = src[i] as f32 * RECIP_255;
                            let ig = src[i + 1] as f32 * RECIP_255;
                            let ib = src[i + 2] as f32 * RECIP_255;
                            row[o] = (ir * inv + (1.0 - (1.0 - ir) * (1.0 - sr)) * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 1] = (ig * inv + (1.0 - (1.0 - ig) * (1.0 - sg)) * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 2] = (ib * inv + (1.0 - (1.0 - ib) * (1.0 - sb)) * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 3] = src[i + 3];
                        }
                    });
            }
            BlendMode::Overlay => {
                dst.par_chunks_mut(row_bytes)
                    .enumerate()
                    .for_each(|(y, row)| {
                        let src_offset = y * row_bytes;
                        for x in 0..width {
                            let i = src_offset + x * 4;
                            let o = x * 4;
                            let ir = src[i] as f32 * RECIP_255;
                            let ig = src[i + 1] as f32 * RECIP_255;
                            let ib = src[i + 2] as f32 * RECIP_255;
                            row[o] = (ir * inv + overlay(ir, sr) * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 1] = (ig * inv + overlay(ig, sg) * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 2] = (ib * inv + overlay(ib, sb) * af)
                                .clamp(0.0, 1.0)
                                .mul_add(255.0, 0.5) as u8;
                            row[o + 3] = src[i + 3];
                        }
                    });
            }
        }
    }
}

/// Overlay blend: uses Multiply on dark areas and Screen on light areas.
#[inline]
fn overlay(base: f32, blend: f32) -> f32 {
    if base < 0.5 {
        2.0 * base * blend
    } else {
        1.0 - 2.0 * (1.0 - base) * (1.0 - blend)
    }
}
