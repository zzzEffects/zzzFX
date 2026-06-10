use rayon::prelude::*;
use std::sync::OnceLock;

use crate::settings::halftone::{ChannelMode, DotShape, HalfTone};

const RCP_255: f32 = 1.0 / 255.0;

// ── GPU / CPU dispatch ────────────────────────────────────────────────

trait HalfToneProcessor: Send + Sync {
    fn process(
        &self, settings: &HalfTone, src: &[u8], dst: &mut [u8],
        width: usize, height: usize,
    ) -> Result<(), String>;
}

struct CpuProcessor;
impl HalfToneProcessor for CpuProcessor {
    fn process(
        &self, s: &HalfTone, src: &[u8], dst: &mut [u8], w: usize, h: usize,
    ) -> Result<(), String> {
        render_cpu(s, src, dst, w, h);
        Ok(())
    }
}

#[cfg(feature = "gpu")]
struct GpuProcessor;
#[cfg(feature = "gpu")]
impl HalfToneProcessor for GpuProcessor {
    fn process(
        &self, s: &HalfTone, src: &[u8], dst: &mut [u8], w: usize, h: usize,
    ) -> Result<(), String> {
        match crate::gpu::halftone::try_render(s, src, dst, w, h) {
            Ok(true) => Ok(()),
            Ok(false) => Err("GPU unavailable".into()),
            Err(e) => Err(e),
        }
    }
}

struct FallbackProcessor {
    #[cfg(feature = "gpu")]
    gpu: GpuProcessor,
    cpu: CpuProcessor,
}
impl FallbackProcessor {
    fn new() -> Self {
        Self {
            #[cfg(feature = "gpu")]
            gpu: GpuProcessor,
            cpu: CpuProcessor,
        }
    }
}
impl HalfToneProcessor for FallbackProcessor {
    fn process(
        &self, s: &HalfTone, src: &[u8], dst: &mut [u8], w: usize, h: usize,
    ) -> Result<(), String> {
        #[cfg(feature = "gpu")]
        if self.gpu.process(s, src, dst, w, h).is_ok() {
            return Ok(());
        }
        self.cpu.process(s, src, dst, w, h)
    }
}

static PROCESSOR: OnceLock<FallbackProcessor> = OnceLock::new();

// ── Public API ────────────────────────────────────────────────────────

impl HalfTone {
    pub fn is_identity(&self) -> bool {
        self.dot_size <= 0.001 && self.blend_with_original >= 0.999
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
        let processor = PROCESSOR.get_or_init(FallbackProcessor::new);
        if let Err(e) = processor.process(self, src, dst, width, height) {
            eprintln!("[zzzfx] halftone render failed: {e}");
        }
    }
}

// ── CPU render ────────────────────────────────────────────────────────

fn render_cpu(
    settings: &HalfTone,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) {
    let dot_size = settings.dot_size.clamp(0.0, 100.0);
    let angle = settings.angle.clamp(0.0, 360.0);
    let invert = settings.invert;
    let contrast = settings.contrast.clamp(0.0, 1.0);
    let smoothness = (settings.smoothness.clamp(0.0, 1.0) * 0.5).max(0.001);
    let blend = settings.blend_with_original.clamp(0.0, 1.0);
    let w = width as f32;
    let h = height as f32;

    let diagonal = (w * w + h * h).sqrt();
    let cell_spacing = (dot_size / 100.0 * diagonal).max(2.0);

    // Grid anchor in screen space — rotation pivots around this point
    let ax = settings.position_x.clamp(0.0, 1.0) * w;
    let ay = settings.position_y.clamp(0.0, 1.0) * h;

    let rad = angle.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let contrast_factor = 1.0 + (contrast - 0.5) * 2.0;
    let half_cell = cell_spacing * 0.5;

    let rgb_cos_sin: [(f32, f32); 3] = [
        { let a = rad + 15f32.to_radians(); (a.cos(), a.sin()) },
        { let a = rad + 45f32.to_radians(); (a.cos(), a.sin()) },
        { let a = rad + 75f32.to_radians(); (a.cos(), a.sin()) },
    ];

    dst.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let yf = y as f32;
            for x in 0..width {
                let xf = x as f32;
                let si = (y * width + x) * 4;
                let o = x * 4;

                let sr = src[si] as f32 * RCP_255;
                let sg = src[si + 1] as f32 * RCP_255;
                let sb = src[si + 2] as f32 * RCP_255;
                let sa = src[si + 3] as f32 * RCP_255;

                let (hr, hg, hb) = match settings.channel_mode {
                    ChannelMode::Luminance => {
                        let lum = 0.2126 * sr + 0.7152 * sg + 0.0722 * sb;
                        let brightness = apply_contrast(lum, contrast_factor);
                        let b = if invert { 1.0 - brightness } else { brightness };
                        let coverage = dot_coverage(
                            xf, yf, b, cell_spacing, half_cell,
                            cos_a, sin_a, ax, ay, settings.dot_shape, smoothness,
                        );
                        let dot = 1.0 - coverage;
                        (dot, dot, dot)
                    }
                    ChannelMode::RGB => {
                        let br = apply_contrast(sr, contrast_factor);
                        let bg = apply_contrast(sg, contrast_factor);
                        let bb = apply_contrast(sb, contrast_factor);
                        let ch = [
                            if invert { 1.0 - br } else { br },
                            if invert { 1.0 - bg } else { bg },
                            if invert { 1.0 - bb } else { bb },
                        ];
                        let cr = dot_coverage(
                            xf, yf, ch[0], cell_spacing, half_cell,
                            rgb_cos_sin[0].0, rgb_cos_sin[0].1,
                            ax, ay, settings.dot_shape, smoothness,
                        );
                        let cg = dot_coverage(
                            xf, yf, ch[1], cell_spacing, half_cell,
                            rgb_cos_sin[1].0, rgb_cos_sin[1].1,
                            ax, ay, settings.dot_shape, smoothness,
                        );
                        let cb = dot_coverage(
                            xf, yf, ch[2], cell_spacing, half_cell,
                            rgb_cos_sin[2].0, rgb_cos_sin[2].1,
                            ax, ay, settings.dot_shape, smoothness,
                        );
                        (1.0 - cr, 1.0 - cg, 1.0 - cb)
                    }
                };

                if blend <= 0.001 {
                    row[o] = (hr * 255.0).round() as u8;
                    row[o + 1] = (hg * 255.0).round() as u8;
                    row[o + 2] = (hb * 255.0).round() as u8;
                } else {
                    row[o] = (hr + (sr - hr) * blend).mul_add(255.0, 0.5).round() as u8;
                    row[o + 1] = (hg + (sg - hg) * blend).mul_add(255.0, 0.5).round() as u8;
                    row[o + 2] = (hb + (sb - hb) * blend).mul_add(255.0, 0.5).round() as u8;
                }
                row[o + 3] = (sa * 255.0).round() as u8;
            }
        });
}

fn apply_contrast(v: f32, factor: f32) -> f32 {
    ((v - 0.5) * factor + 0.5).clamp(0.0, 1.0)
}

/// Returns 1.0 when pixel is fully inside the dot, 0.0 when outside.
/// Anti-aliased via smoothstep.
fn dot_coverage(
    px: f32, py: f32,
    brightness: f32,
    cell_spacing: f32,
    half_cell: f32,
    cos_a: f32, sin_a: f32,
    ax: f32, ay: f32,
    shape: DotShape,
    smoothness: f32,
) -> f32 {
    // Transform to anchor-relative coords — rotation pivots around (ax, ay)
    let sx = px - ax;
    let sy = py - ay;
    let rx = sx * cos_a + sy * sin_a;
    let ry = -sx * sin_a + sy * cos_a;

    let cx = (rx / cell_spacing).round() * cell_spacing;
    let cy = (ry / cell_spacing).round() * cell_spacing;

    let dx = rx - cx;
    let dy = ry - cy;

    let dot_radius = (1.0 - brightness) * half_cell;

    let dist = match shape {
        DotShape::Circle => (dx * dx + dy * dy).sqrt(),
        DotShape::Square => dx.abs().max(dy.abs()),
        DotShape::Diamond => dx.abs() + dy.abs(),
    };

    let soft = smoothness * cell_spacing;
    let inner = dot_radius - soft;
    let outer = dot_radius + soft;
    if dist <= inner {
        1.0
    } else if dist >= outer {
        0.0
    } else {
        let t = (dist - inner) / (outer - inner);
        1.0 - t * t * (3.0 - 2.0 * t)
    }
}
