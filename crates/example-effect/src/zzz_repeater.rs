use rayon::prelude::*;

use crate::blend::{self, blend_channel, is_stencil_or_outline};
use crate::settings::zzz_repeater::LayerOrder;
use crate::settings::zzz_stroke::BlendMode;

pub struct CompositorLayer<'a> {
    pub rgba: &'a [u8],
    pub position_x: f32,
    pub position_y: f32,
    pub rotation_deg: f32,
    pub blend_mode: BlendMode,
}

impl super::ZzzRepeater {
    /// Returns true when the effect can be an identity (no-op) pass-through.
    /// Note: the OFX wrapper must also check for zero keyframes.
    pub fn is_identity(&self) -> bool {
        self.time_offset == 0.0
    }

    pub fn composite_layers(
        &self,
        layers: &[CompositorLayer],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) {
        if layers.is_empty() {
            dst.fill(0);
            return;
        }

        // GPU first — try wgpu compute; if unavailable or panics, fall through to CPU.
        // catch_unwind prevents Rust panics from unwinding into the host's C FFI.
        const MAX_GPU_LAYERS: usize = 32;
        if layers.len() <= MAX_GPU_LAYERS {
            let gpu_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::gpu::repeater::try_repeater_gpu_render(self, layers, dst, width, height)
            }));
            match gpu_result {
                Ok(Ok(true)) => return,
                _ => {} // GPU unavailable, errored, or panicked — fall through to CPU
            }
        }

        // Precompute layer ordering: whether to iterate oldest→newest or newest→oldest.
        // Above = old at bottom, new on top → process bottom-to-top (index 0..n)
        // Below = new at bottom, old on top → process bottom-to-top (index n-1..0)
        let bmode = self.blend_mode;

        let center_x = width as f32 * 0.5;
        let center_y = height as f32 * 0.5;
        let w = width as f32;
        let h = height as f32;

        let offsets_and_angles: Vec<((f32, f32), (f32, f32))> = layers
            .iter()
            .map(|layer| {
                let offset_x = (layer.position_x - 0.5) * w;
                let offset_y = (layer.position_y - 0.5) * h;
                let angle_rad = (-layer.rotation_deg).to_radians();
                let cos_a = angle_rad.cos();
                let sin_a = angle_rad.sin();
                ((offset_x, offset_y), (cos_a, sin_a))
            })
            .collect();

        let stencil = is_stencil_or_outline(bmode);

        dst.par_chunks_mut(width * 4)
            .enumerate()
            .for_each(|(oy, row)| {
                for ox in 0..width {
                    let pixel_idx = oy * width + ox;

                    // Hash-based RNG for Dissolve mode
                    let h = (pixel_idx as u32).wrapping_mul(0x45d9f3b);
                    let h = (h ^ (h >> 16)).wrapping_mul(0x85ebca6b);
                    let h = h ^ (h >> 13);
                    let rng_base = h as f32 / u32::MAX as f32;
                    let rng1 = rng_base;
                    let rng2 = {
                        let h2 = (h ^ 0xDEADBEEF) as f32 / u32::MAX as f32;
                        h2
                    };
                    let rng3 = {
                        let h3 = (h ^ 0xCAFEBABE) as f32 / u32::MAX as f32;
                        h3
                    };

                    // Accumulate from transparent black
                    let mut acc = [0.0f32; 4];

                    // Determine iteration order based on LayerOrder
                    let iter: Box<dyn Iterator<Item = usize>> = match self.layer_order {
                        LayerOrder::Above => Box::new(0..layers.len()),
                        LayerOrder::Below => Box::new((0..layers.len()).rev()),
                    };

                    for li in iter {
                        let layer = &layers[li];
                        let ((off_x, off_y), (cos_a, sin_a)) = offsets_and_angles[li];

                        // Inverse transform: output pixel → source pixel
                        let cx = ox as f32 - center_x;
                        let cy = oy as f32 - center_y;

                        // Inverse rotation
                        let rx = cx * cos_a - cy * sin_a;
                        let ry = cx * sin_a + cy * cos_a;

                        // Inverse position offset
                        let sx_f = rx - off_x + center_x;
                        let sy_f = ry - off_y + center_y;

                        // Nearest-neighbor sample with clamp
                        let sx = (sx_f.round() as isize).clamp(0, width as isize - 1) as usize;
                        let sy = (sy_f.round() as isize).clamp(0, height as isize - 1) as usize;

                        let src_idx = (sy * width + sx) * 4;
                        let sr = layer.rgba[src_idx] as f32 / 255.0;
                        let sg = layer.rgba[src_idx + 1] as f32 / 255.0;
                        let sb = layer.rgba[src_idx + 2] as f32 / 255.0;
                        let sa = layer.rgba[src_idx + 3] as f32 / 255.0;

                        if sa <= 0.0 {
                            continue;
                        }

                        if acc[3] <= 0.0 {
                            // First visible layer
                            acc = [sr, sg, sb, sa];
                            continue;
                        }

                        if stencil {
                            let stencil_a = match bmode {
                                BlendMode::StencilAlpha => sa,
                                BlendMode::StencilLuma => sa * blend::luminance(sr, sg, sb),
                                BlendMode::OutlineAlpha => sa,
                                BlendMode::OutlineLuma => sa * blend::luminance(sr, sg, sb),
                                _ => sa,
                            };

                            if matches!(
                                bmode,
                                BlendMode::OutlineAlpha | BlendMode::OutlineLuma
                            ) {
                                // Outline modes: replace color, multiply alpha by stencil
                                acc[0] = sr;
                                acc[1] = sg;
                                acc[2] = sb;
                                acc[3] = stencil_a * acc[3];
                            } else {
                                // Stencil modes: keep color, multiply alpha by stencil
                                let blended_r = blend_channel(bmode, acc[0], sr, sa, &mut || rng1);
                                let blended_g = blend_channel(bmode, acc[1], sg, sa, &mut || rng2);
                                let blended_b = blend_channel(bmode, acc[2], sb, sa, &mut || rng3);
                                acc[0] = blended_r;
                                acc[1] = blended_g;
                                acc[2] = blended_b;
                                acc[3] = stencil_a * acc[3];
                            }
                        } else {
                            let blended_r = blend_channel(bmode, acc[0], sr, sa, &mut || rng1);
                            let blended_g = blend_channel(bmode, acc[1], sg, sa, &mut || rng2);
                            let blended_b = blend_channel(bmode, acc[2], sb, sa, &mut || rng3);

                            let inv = 1.0 - sa;
                            acc[0] = (blended_r * sa + acc[0] * inv).clamp(0.0, 1.0);
                            acc[1] = (blended_g * sa + acc[1] * inv).clamp(0.0, 1.0);
                            acc[2] = (blended_b * sa + acc[2] * inv).clamp(0.0, 1.0);
                            acc[3] = (sa + acc[3] * inv).clamp(0.0, 1.0);
                        }
                    }

                    let out = &mut row[ox * 4..ox * 4 + 4];
                    out[0] = (acc[0] * 255.0).round() as u8;
                    out[1] = (acc[1] * 255.0).round() as u8;
                    out[2] = (acc[2] * 255.0).round() as u8;
                    out[3] = (acc[3] * 255.0).round() as u8;
                }
            });
    }
}
