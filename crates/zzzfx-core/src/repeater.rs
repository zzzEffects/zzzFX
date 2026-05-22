use rayon::prelude::*;

use crate::blend::{self, blend_channel, IS_STENCIL_OR_OUTLINE, RECIP_255};
use crate::settings::repeater::LayerOrder;
use crate::settings::stroke::BlendMode;

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

        let bmode = self.blend_mode;
        let center_x = width as f32 * 0.5;
        let center_y = height as f32 * 0.5;
        let w = width as f32;
        let h = height as f32;

        // Precompute per-layer transform parameters (H2: merged center offset)
        let layer_params: Vec<((f32, f32), (f32, f32))> = layers
            .iter()
            .map(|layer| {
                let offset_x = (layer.position_x - 0.5) * w;
                let offset_y = (layer.position_y - 0.5) * h;
                let angle_rad = (-layer.rotation_deg).to_radians();
                // H5: Use sin_cos() for simultaneous computation (single FPU instruction)
                let (sin_a, cos_a) = angle_rad.sin_cos();
                // H2: Precompute inversed center offset to save 2 subtractions per pixel per layer
                let pre_x = center_x - offset_x;
                let pre_y = center_y - offset_y;
                ((pre_x, pre_y), (cos_a, sin_a))
            })
            .collect();

        let stencil = IS_STENCIL_OR_OUTLINE[bmode as usize];
        let is_dissolve = bmode == BlendMode::Dissolve;
        let is_below = matches!(self.layer_order, LayerOrder::Below);
        let num_layers = layers.len();

        dst.par_chunks_mut(width * 4)
            .enumerate()
            .for_each(|(oy, row)| {
                for ox in 0..width {
                    let pixel_idx = oy * width + ox;

                    // Only compute RNG when using Dissolve mode (B2)
                    let (rng1, rng2, rng3) = if is_dissolve {
                        let h = (pixel_idx as u32).wrapping_mul(0x45d9f3b);
                        let h = (h ^ (h >> 16)).wrapping_mul(0x85ebca6b);
                        let h = h ^ (h >> 13);
                        let rng_base = fast_u32_to_f32(h);
                        let rng2 = fast_u32_to_f32(h ^ 0xDEADBEEF);
                        let rng3 = fast_u32_to_f32(h ^ 0xCAFEBABE);
                        (rng_base, rng2, rng3)
                    } else {
                        (0.0, 0.0, 0.0)
                    };

                    // Accumulate from transparent black
                    let mut acc = [0.0f32; 4];

                    // H1: Direct iteration order instead of Box<dyn Iterator>
                    if is_below {
                        for li in (0..num_layers).rev() {
                            process_layer(
                                layers,
                                &layer_params,
                                li,
                                ox,
                                oy,
                                center_x,
                                center_y,
                                width,
                                height,
                                bmode,
                                stencil,
                                rng1,
                                rng2,
                                rng3,
                                &mut acc,
                            );
                        }
                    } else {
                        for li in 0..num_layers {
                            process_layer(
                                layers,
                                &layer_params,
                                li,
                                ox,
                                oy,
                                center_x,
                                center_y,
                                width,
                                height,
                                bmode,
                                stencil,
                                rng1,
                                rng2,
                                rng3,
                                &mut acc,
                            );
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

/// Process one layer for one pixel. Extracted to avoid code duplication between
/// the Above and Below iteration orders.
#[inline]
fn process_layer(
    layers: &[CompositorLayer],
    layer_params: &[((f32, f32), (f32, f32))],
    li: usize,
    ox: usize,
    oy: usize,
    center_x: f32,
    center_y: f32,
    width: usize,
    height: usize,
    bmode: BlendMode,
    stencil: bool,
    rng1: f32,
    rng2: f32,
    rng3: f32,
    acc: &mut [f32; 4],
) {
    let layer = &layers[li];
    let ((pre_x, pre_y), (cos_a, sin_a)) = layer_params[li];

    // Inverse transform: output pixel → source pixel
    let cx = ox as f32 - center_x;
    let cy = oy as f32 - center_y;

    // Inverse rotation
    let rx = cx * cos_a - cy * sin_a;
    let ry = cx * sin_a + cy * cos_a;

    // H2: Use precomputed offset (merged with center_x/center_y)
    let sx_f = rx + pre_x;
    let sy_f = ry + pre_y;

    // Nearest-neighbor sample with clamp
    let sx = (sx_f.round() as isize).clamp(0, width as isize - 1) as usize;
    let sy = (sy_f.round() as isize).clamp(0, height as isize - 1) as usize;

    let src_idx = (sy * width + sx) * 4;
    let sr = layer.rgba[src_idx] as f32 * RECIP_255;
    let sg = layer.rgba[src_idx + 1] as f32 * RECIP_255;
    let sb = layer.rgba[src_idx + 2] as f32 * RECIP_255;
    let sa = layer.rgba[src_idx + 3] as f32 * RECIP_255;

    if sa <= 0.0 {
        return;
    }

    if acc[3] <= 0.0 {
        // First visible layer
        *acc = [sr, sg, sb, sa];
        return;
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
            let blended_r = blend_channel(bmode, acc[0], sr, sa, rng1);
            let blended_g = blend_channel(bmode, acc[1], sg, sa, rng2);
            let blended_b = blend_channel(bmode, acc[2], sb, sa, rng3);
            acc[0] = blended_r;
            acc[1] = blended_g;
            acc[2] = blended_b;
            acc[3] = stencil_a * acc[3];
        }
    } else {
        let blended_r = blend_channel(bmode, acc[0], sr, sa, rng1);
        let blended_g = blend_channel(bmode, acc[1], sg, sa, rng2);
        let blended_b = blend_channel(bmode, acc[2], sb, sa, rng3);

        let inv = 1.0 - sa;
        acc[0] = (blended_r * sa + acc[0] * inv).clamp(0.0, 1.0);
        acc[1] = (blended_g * sa + acc[1] * inv).clamp(0.0, 1.0);
        acc[2] = (blended_b * sa + acc[2] * inv).clamp(0.0, 1.0);
        acc[3] = (sa + acc[3] * inv).clamp(0.0, 1.0);
    }
}

/// Fast f32 conversion from u32 for RNG values in [0, 1).
/// Uses bit manipulation instead of float division (ntsc-rs technique).
#[inline]
fn fast_u32_to_f32(input: u32) -> f32 {
    f32::from_bits((input >> 9) | 0x3F800000) - 1.0
}
