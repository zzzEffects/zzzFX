use rayon::prelude::*;

use crate::settings::standard::{ColorPreset, ExampleEffect};
use crate::RECIP_255;

impl ExampleEffect {
    /// Apply the example effect to an RGBA buffer.
    ///
    /// Applies: brightness, tint (per-channel), invert, advanced contrast/saturation,
    /// and color presets. GPU-accelerated when available; falls back to CPU.
    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }

        // Fast path: identity
        if self.is_identity() {
            dst[..len].copy_from_slice(&src[..len]);
            return;
        }

        #[cfg(feature = "gpu")]
        {
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::gpu::standard::try_standard_effect_gpu_render(self, src, dst, width, height)
            }));
            match gpu_result {
                Ok(Ok(true)) => return,
                _ => {} // GPU unavailable — fall through to CPU
            }
        }

        self.apply_effect_cpu(src, dst, width, height);
    }

    /// Returns true when the effect is a complete no-op.
    fn is_identity(&self) -> bool {
        let brightness_ok = (self.brightness - 1.0).abs() < f32::EPSILON;
        let tint_ok = (self.tint_r - 1.0).abs() < f32::EPSILON
            && (self.tint_g - 1.0).abs() < f32::EPSILON
            && (self.tint_b - 1.0).abs() < f32::EPSILON;
        let invert_ok = !self.invert_colors;
        let advanced_ok = match &self.advanced {
            Some(adv) => {
                (adv.contrast - 1.0).abs() < f32::EPSILON
                    && (adv.saturation - 1.0).abs() < f32::EPSILON
            }
            None => true,
        };
        let preset_ok = self.color_preset == ColorPreset::None;

        brightness_ok && tint_ok && invert_ok && advanced_ok && preset_ok
    }

    fn apply_effect_cpu(&self, src: &[u8], dst: &mut [u8], width: usize, _height: usize) {
        let brightness = self.brightness.clamp(0.0, 2.0);
        let tr = self.tint_r.clamp(0.0, 2.0);
        let tg = self.tint_g.clamp(0.0, 2.0);
        let tb = self.tint_b.clamp(0.0, 2.0);
        let invert = self.invert_colors;
        let preset = self.color_preset;

        let has_advanced = self.advanced.is_some();
        let contrast = self
            .advanced
            .as_ref()
            .map(|a| a.contrast.clamp(0.0, 4.0))
            .unwrap_or(1.0);
        let saturation = self
            .advanced
            .as_ref()
            .map(|a| a.saturation.clamp(0.0, 2.0))
            .unwrap_or(1.0);

        let row_bytes = width * 4;

        dst.par_chunks_mut(row_bytes)
            .enumerate()
            .for_each(|(y, row)| {
                let src_offset = y * row_bytes;
                for x in 0..width {
                    let i = src_offset + x * 4;

                    let mut r = src[i] as f32 * RECIP_255;
                    let mut g = src[i + 1] as f32 * RECIP_255;
                    let mut b = src[i + 2] as f32 * RECIP_255;
                    let a = src[i + 3];

                    // Brightness
                    r *= brightness;
                    g *= brightness;
                    b *= brightness;

                    // Tint (per-channel)
                    r *= tr;
                    g *= tg;
                    b *= tb;

                    // Invert (luminance-preserving: invert midtones around 0.5)
                    if invert {
                        r = 1.0 - r;
                        g = 1.0 - g;
                        b = 1.0 - b;
                    }

                    // Advanced: contrast + saturation
                    if has_advanced {
                        // Contrast (power function around 0.5)
                        if (contrast - 1.0).abs() > f32::EPSILON {
                            let c = contrast;
                            r = ((r - 0.5) * c + 0.5).clamp(0.0, 1.0);
                            g = ((g - 0.5) * c + 0.5).clamp(0.0, 1.0);
                            b = ((b - 0.5) * c + 0.5).clamp(0.0, 1.0);
                        }

                        // Saturation (luminance-preserving)
                        if (saturation - 1.0).abs() > f32::EPSILON {
                            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                            r = (lum + (r - lum) * saturation).clamp(0.0, 1.0);
                            g = (lum + (g - lum) * saturation).clamp(0.0, 1.0);
                            b = (lum + (b - lum) * saturation).clamp(0.0, 1.0);
                        }
                    }

                    // Color preset
                    match preset {
                        ColorPreset::None => {}
                        ColorPreset::Warm => {
                            r = (r * 1.15).clamp(0.0, 1.0);
                            g = (g * 0.95).clamp(0.0, 1.0);
                            b = (b * 0.75).clamp(0.0, 1.0);
                        }
                        ColorPreset::Cool => {
                            r = (r * 0.85).clamp(0.0, 1.0);
                            g = (g * 0.95).clamp(0.0, 1.0);
                            b = (b * 1.15).clamp(0.0, 1.0);
                        }
                        ColorPreset::Sepia => {
                            let lr = r;
                            let lg = g;
                            let lb = b;
                            r = (lr * 0.393 + lg * 0.769 + lb * 0.189).clamp(0.0, 1.0);
                            g = (lr * 0.349 + lg * 0.686 + lb * 0.168).clamp(0.0, 1.0);
                            b = (lr * 0.272 + lg * 0.534 + lb * 0.131).clamp(0.0, 1.0);
                        }
                    }

                    let o = x * 4;
                    row[o] = (r * 255.0).round() as u8;
                    row[o + 1] = (g * 255.0).round() as u8;
                    row[o + 2] = (b * 255.0).round() as u8;
                    row[o + 3] = a;
                }
            });
    }
}
