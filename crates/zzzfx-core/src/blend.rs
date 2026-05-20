use crate::settings::stroke::BlendMode;

/// Blend two color channels (both in 0..1 range) using the specified mode.
/// `base` is the source image channel, `blend` is the stroke color channel.
pub fn blend_channel(mode: BlendMode, base: f32, blend: f32, stroke_alpha: f32, rng: &mut impl FnMut() -> f32) -> f32 {
    match mode {
        BlendMode::Normal => blend,
        BlendMode::Dissolve => {
            if rng() < stroke_alpha { blend } else { base }
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
        BlendMode::LinearLight => {
            (base + 2.0 * blend - 1.0).clamp(0.0, 1.0)
        }
        BlendMode::HardMix => {
            if base + blend < 1.0 { 0.0 } else { 1.0 }
        }
        BlendMode::Difference => (base - blend).abs(),
        BlendMode::Exclusion => base + blend - 2.0 * base * blend,
        BlendMode::Subtract => (base - blend).max(0.0),
        BlendMode::Divide => {
            if blend <= 0.0 { 1.0 } else { (base / blend).min(1.0) }
        }
        BlendMode::StencilAlpha | BlendMode::OutlineAlpha | BlendMode::StencilLuma | BlendMode::OutlineLuma => {
            blend
        }
    }
}

/// Returns whether this blend mode is a stencil/outline type that needs special alpha handling.
pub fn is_stencil_or_outline(mode: BlendMode) -> bool {
    matches!(
        mode,
        BlendMode::StencilAlpha
            | BlendMode::StencilLuma
            | BlendMode::OutlineAlpha
            | BlendMode::OutlineLuma
    )
}

/// Luminance of an RGB color (Rec. 709 coefficients).
pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}
