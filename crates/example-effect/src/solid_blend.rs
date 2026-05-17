use crate::settings::solid::{BlendMode, SolidColorBlend};

impl SolidColorBlend {
    /// Blend the entire image with a solid color using the selected blend mode.
    ///
    /// The buffer is RGBA interleaved, 1 byte per channel. Alpha is copied through unchanged.
    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        assert!(src.len() >= len, "source buffer too small");
        assert!(dst.len() >= len, "destination buffer too small");

        let a = self.color_a.clamp(0.0, 1.0);
        let cr = self.color_r.clamp(0.0, 1.0);
        let cg = self.color_g.clamp(0.0, 1.0);
        let cb = self.color_b.clamp(0.0, 1.0);

        // Fast path: no blend
        if a == 0.0 {
            dst[..len].copy_from_slice(&src[..len]);
            return;
        }

        match self.blend_mode {
            BlendMode::Normal => {
                let inv = 1.0 - a;
                let sr = (cr * 255.0).round() as u8;
                let sg = (cg * 255.0).round() as u8;
                let sb = (cb * 255.0).round() as u8;
                for i in (0..len).step_by(4) {
                    dst[i]     = (src[i] as f32 * inv + sr as f32 * a).round() as u8;
                    dst[i + 1] = (src[i + 1] as f32 * inv + sg as f32 * a).round() as u8;
                    dst[i + 2] = (src[i + 2] as f32 * inv + sb as f32 * a).round() as u8;
                    dst[i + 3] = src[i + 3];
                }
            }
            BlendMode::Multiply => {
                let sr = cr as f64;
                let sg = cg as f64;
                let sb = cb as f64;
                let af = a as f64;
                let inv = 1.0 - af;
                for i in (0..len).step_by(4) {
                    let ir = src[i] as f64 / 255.0;
                    let ig = src[i + 1] as f64 / 255.0;
                    let ib = src[i + 2] as f64 / 255.0;
                    dst[i]     = (ir * inv + ir * sr * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 1] = (ig * inv + ig * sg * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 2] = (ib * inv + ib * sb * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 3] = src[i + 3];
                }
            }
            BlendMode::Screen => {
                let sr = cr as f64;
                let sg = cg as f64;
                let sb = cb as f64;
                let af = a as f64;
                let inv = 1.0 - af;
                for i in (0..len).step_by(4) {
                    let ir = src[i] as f64 / 255.0;
                    let ig = src[i + 1] as f64 / 255.0;
                    let ib = src[i + 2] as f64 / 255.0;
                    dst[i]     = (ir * inv + (1.0 - (1.0 - ir) * (1.0 - sr)) * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 1] = (ig * inv + (1.0 - (1.0 - ig) * (1.0 - sg)) * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 2] = (ib * inv + (1.0 - (1.0 - ib) * (1.0 - sb)) * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 3] = src[i + 3];
                }
            }
            BlendMode::Overlay => {
                let sr = cr as f64;
                let sg = cg as f64;
                let sb = cb as f64;
                let af = a as f64;
                let inv = 1.0 - af;
                for i in (0..len).step_by(4) {
                    let ir = src[i] as f64 / 255.0;
                    let ig = src[i + 1] as f64 / 255.0;
                    let ib = src[i + 2] as f64 / 255.0;
                    dst[i]     = (ir * inv + overlay(ir, sr) * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 1] = (ig * inv + overlay(ig, sg) * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 2] = (ib * inv + overlay(ib, sb) * af).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    dst[i + 3] = src[i + 3];
                }
            }
        }
    }
}

/// Overlay blend: uses Multiply on dark areas and Screen on light areas.
fn overlay(base: f64, blend: f64) -> f64 {
    if base < 0.5 {
        2.0 * base * blend
    } else {
        1.0 - 2.0 * (1.0 - base) * (1.0 - blend)
    }
}
