use crate::settings::stroke::BlendMode;

/// Reciprocal of 255.0 — multiply by this instead of dividing by 255.0.
/// Uses f32 division to match LLVM's optimization of `x / 255.0_f32`.
pub const RECIP_255: f32 = 1.0_f32 / 255.0_f32;

/// Precomputed lookup table for `is_stencil_or_outline()`.
pub static IS_STENCIL_OR_OUTLINE: [bool; 22] = [
    false, // Normal
    false, // Dissolve
    false, // Darken
    false, // Multiply
    false, // ColorBurn
    false, // LinearBurn
    false, // Add
    false, // Screen
    false, // ColorDodge
    false, // LinearDodge
    false, // Overlay
    false, // SoftLight
    false, // LinearLight
    false, // HardMix
    false, // Difference
    false, // Exclusion
    false, // Subtract
    false, // Divide
    true,  // StencilAlpha
    true,  // OutlineAlpha
    true,  // StencilLuma
    true,  // OutlineLuma
];

/// Blend two color channels (both in 0..1 range) using the specified mode.
/// `base` is the source image channel, `blend` is the stroke color channel.
/// `rng_value` is only used by Dissolve mode; pass 0.0 for all other modes.
#[inline(always)]
pub fn blend_channel(
    mode: BlendMode,
    base: f32,
    blend: f32,
    stroke_alpha: f32,
    rng_value: f32,
) -> f32 {
    match mode {
        BlendMode::Normal => blend,
        BlendMode::Dissolve => {
            if rng_value < stroke_alpha {
                blend
            } else {
                base
            }
        }
        BlendMode::Darken => base.min(blend),
        BlendMode::Multiply => base * blend,
        BlendMode::ColorBurn => {
            if blend <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - base) / blend).min(1.0)
            }
        }
        BlendMode::LinearBurn => (base + blend - 1.0).max(0.0),
        BlendMode::Add => (base + blend).min(1.0),
        BlendMode::Screen => 1.0 - (1.0 - base) * (1.0 - blend),
        BlendMode::ColorDodge => {
            if blend >= 1.0 {
                1.0
            } else {
                (base / (1.0 - blend)).min(1.0)
            }
        }
        BlendMode::LinearDodge => (base + blend).min(1.0),
        BlendMode::Overlay => {
            if base < 0.5 {
                2.0 * base * blend
            } else {
                1.0 - 2.0 * (1.0 - base) * (1.0 - blend)
            }
        }
        BlendMode::SoftLight => {
            if blend < 0.5 {
                base - (1.0 - 2.0 * blend) * base * (1.0 - base)
            } else {
                let d = if base < 0.25 {
                    ((16.0 * base - 12.0) * base + 4.0) * base
                } else {
                    base.sqrt()
                };
                base + (2.0 * blend - 1.0) * (d - base)
            }
        }
        BlendMode::LinearLight => (base + 2.0 * blend - 1.0).clamp(0.0, 1.0),
        BlendMode::HardMix => {
            if base + blend < 1.0 {
                0.0
            } else {
                1.0
            }
        }
        BlendMode::Difference => (base - blend).abs(),
        BlendMode::Exclusion => base + blend - 2.0 * base * blend,
        BlendMode::Subtract => (base - blend).max(0.0),
        BlendMode::Divide => {
            if blend <= 0.0 {
                1.0
            } else {
                (base / blend).min(1.0)
            }
        }
        BlendMode::StencilAlpha
        | BlendMode::OutlineAlpha
        | BlendMode::StencilLuma
        | BlendMode::OutlineLuma => blend,
    }
}

/// Returns whether this blend mode is a stencil/outline type that needs special alpha handling.
#[inline(always)]
pub fn is_stencil_or_outline(mode: BlendMode) -> bool {
    IS_STENCIL_OR_OUTLINE[mode as usize]
}

/// Luminance of an RGB color (Rec. 709 coefficients).
pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Fast f32 conversion from u32 for RNG values in [0, 1).
/// Uses bit manipulation instead of float division.
#[inline]
pub fn fast_u32_to_f32(input: u32) -> f32 {
    // Produces a value in [0, 1). The subtraction may round to exactly 1.0
    // under non-default rounding modes, so clamp defensively.
    (f32::from_bits((input >> 9) | 0x3F800000) - 1.0).min(1.0 - f32::EPSILON)
}

/// Composite RGBA8888 SVG/overlay pixels over a solid background color with opacity.
/// Shared by SVG display, QR code, and LaTeX display effects.
pub fn composite_over_bg(
    overlay_pixels: &[u8],
    dst: &mut [u8],
    opacity: f32,
    bg: [f32; 4],
    output_w: usize,
    output_h: usize,
) {
    let br = (bg[0] * 255.0).round() as u8;
    let bbg = (bg[1] * 255.0).round() as u8;
    let bb = (bg[2] * 255.0).round() as u8;
    let ba_f = bg[3];

    for chunk in dst.chunks_exact_mut(4) {
        chunk[0] = br;
        chunk[1] = bbg;
        chunk[2] = bb;
        chunk[3] = (ba_f * 255.0).round() as u8;
    }

    let n = (output_w * output_h * 4).min(overlay_pixels.len());
    let overlap = &overlay_pixels[..n];

    for (dst_chunk, ov_chunk) in dst[..n].chunks_exact_mut(4).zip(overlap.chunks_exact(4)) {
        let sr = ov_chunk[0] as f32 / 255.0;
        let sg = ov_chunk[1] as f32 / 255.0;
        let sb = ov_chunk[2] as f32 / 255.0;
        let sa = (ov_chunk[3] as f32 / 255.0) * opacity;

        if sa <= 0.0 {
            continue;
        }

        let out_a = sa + ba_f * (1.0 - sa);
        let inv_a = 1.0 / out_a;
        dst_chunk[0] = ((sr * sa + br as f32 / 255.0 * ba_f * (1.0 - sa)) * inv_a * 255.0)
            .round() as u8;
        dst_chunk[1] = ((sg * sa + bbg as f32 / 255.0 * ba_f * (1.0 - sa)) * inv_a * 255.0)
            .round() as u8;
        dst_chunk[2] = ((sb * sa + bb as f32 / 255.0 * ba_f * (1.0 - sa)) * inv_a * 255.0)
            .round() as u8;
        dst_chunk[3] = (out_a * 255.0).round() as u8;
    }
}
