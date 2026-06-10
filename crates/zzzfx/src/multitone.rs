use rayon::prelude::*;
use std::sync::OnceLock;

use crate::settings::multitone::{ColorMappingSettings, MultiTone, ToneDithering, ToneMode};

const RCP_255: f32 = 1.0 / 255.0;

const BAYER_4X4: [[f32; 4]; 4] = [
    [ 0.0/16.0,  8.0/16.0,  2.0/16.0, 10.0/16.0],
    [12.0/16.0,  4.0/16.0, 14.0/16.0,  6.0/16.0],
    [ 3.0/16.0, 11.0/16.0,  1.0/16.0,  9.0/16.0],
    [15.0/16.0,  7.0/16.0, 13.0/16.0,  5.0/16.0],
];

const FS_W_RIGHT: f32 = 7.0 / 16.0;
const FS_W_DOWN_LEFT: f32 = 3.0 / 16.0;
const FS_W_DOWN: f32 = 5.0 / 16.0;
const FS_W_DOWN_RIGHT: f32 = 1.0 / 16.0;

// ── GPU / CPU dispatch ────────────────────────────────────────────────

trait MultiToneProcessor: Send + Sync {
    fn process(
        &self, settings: &MultiTone, src: &[u8], dst: &mut [u8],
        width: usize, height: usize,
    ) -> Result<(), String>;
}

struct CpuProcessor;
impl MultiToneProcessor for CpuProcessor {
    fn process(
        &self, s: &MultiTone, src: &[u8], dst: &mut [u8], w: usize, h: usize,
    ) -> Result<(), String> {
        render_cpu(s, src, dst, w, h);
        Ok(())
    }
}

#[cfg(feature = "gpu")]
struct GpuProcessor;
#[cfg(feature = "gpu")]
impl MultiToneProcessor for GpuProcessor {
    fn process(
        &self, s: &MultiTone, src: &[u8], dst: &mut [u8], w: usize, h: usize,
    ) -> Result<(), String> {
        match crate::gpu::multitone::try_render(s, src, dst, w, h) {
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
impl MultiToneProcessor for FallbackProcessor {
    fn process(
        &self, s: &MultiTone, src: &[u8], dst: &mut [u8], w: usize, h: usize,
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

impl MultiTone {
    pub fn is_identity(&self) -> bool {
        let no_color_map = self.color_mapping.is_none()
            || self.color_mapping.as_ref().is_some_and(|cm| cm.blend_with_original >= 0.999);
        self.tone_levels >= 32.0
            && matches!(self.dithering, ToneDithering::None)
            && self.edge_softness <= 0.001
            && no_color_map
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
            eprintln!("[zzzfx] multitone render failed: {e}");
        }
    }
}

// ── CPU render ────────────────────────────────────────────────────────

fn render_cpu(
    settings: &MultiTone,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) {
    let tone_levels_f = (settings.tone_levels.clamp(2.0, 32.0).floor() as u32).max(2);
    let levels_f = (tone_levels_f - 1) as f32;
    let dither_amount = settings.dithering_amount.clamp(0.0, 1.0);
    let edge_softness = settings.edge_softness.clamp(0.0, 1.0);
    let preserve_lum = settings.preserve_luminosity;
    let color_map = &settings.color_mapping;

    match settings.dithering {
        ToneDithering::None | ToneDithering::Ordered => {
            dst.par_chunks_mut(width * 4)
                .enumerate()
                .for_each(|(y, row)| {
                    for x in 0..width {
                        let si = (y * width + x) * 4;
                        let o = x * 4;

                        let mut r = src[si] as f32 * RCP_255;
                        let mut g = src[si + 1] as f32 * RCP_255;
                        let mut b = src[si + 2] as f32 * RCP_255;
                        let a = src[si + 3] as f32 * RCP_255;

                        if matches!(settings.dithering, ToneDithering::Ordered) {
                            let bayer = BAYER_4X4[y % 4][x % 4];
                            let noise = (bayer - 0.5) * dither_amount;
                            r = (r + noise).clamp(0.0, 1.0);
                            g = (g + noise).clamp(0.0, 1.0);
                            b = (b + noise).clamp(0.0, 1.0);
                        }

                        let (qr, qg, qb) = quantize_pixel(
                            r, g, b, levels_f, settings.mode, edge_softness, preserve_lum,
                        );
                        let (fr, fg, fb) = apply_color_map(qr, qg, qb, color_map);

                        row[o] = (fr * 255.0).round() as u8;
                        row[o + 1] = (fg * 255.0).round() as u8;
                        row[o + 2] = (fb * 255.0).round() as u8;
                        row[o + 3] = (a * 255.0).round() as u8;
                    }
                });
        }
        ToneDithering::FloydSteinberg => {
            let n = width * height;
            let mut buf: Vec<[f32; 3]> = Vec::with_capacity(n);
            for p in src.chunks_exact(4).take(n) {
                buf.push([p[0] as f32 * RCP_255, p[1] as f32 * RCP_255, p[2] as f32 * RCP_255]);
            }

            for y in 0..height {
                let row_off = y * width;
                for x in 0..width {
                    let idx = row_off + x;
                    let old_r = buf[idx][0];
                    let old_g = buf[idx][1];
                    let old_b = buf[idx][2];

                    let (qr, qg, qb) = quantize_pixel(
                        old_r, old_g, old_b, levels_f, settings.mode,
                        edge_softness, preserve_lum,
                    );
                    let err_r = (old_r - qr) * dither_amount;
                    let err_g = (old_g - qg) * dither_amount;
                    let err_b = (old_b - qb) * dither_amount;
                    buf[idx] = [qr, qg, qb];

                    let (fr, fg, fb) = apply_color_map(qr, qg, qb, color_map);
                    let o = idx * 4;
                    dst[o] = (fr * 255.0).round() as u8;
                    dst[o + 1] = (fg * 255.0).round() as u8;
                    dst[o + 2] = (fb * 255.0).round() as u8;
                    dst[o + 3] = src[o + 3];

                    if x + 1 < width {
                        let n = &mut buf[idx + 1];
                        n[0] += err_r * FS_W_RIGHT;
                        n[1] += err_g * FS_W_RIGHT;
                        n[2] += err_b * FS_W_RIGHT;
                    }
                    if y + 1 < height {
                        let nr = (y + 1) * width;
                        if x > 0 {
                            let n = &mut buf[nr + x - 1];
                            n[0] += err_r * FS_W_DOWN_LEFT;
                            n[1] += err_g * FS_W_DOWN_LEFT;
                            n[2] += err_b * FS_W_DOWN_LEFT;
                        }
                        {
                            let n = &mut buf[nr + x];
                            n[0] += err_r * FS_W_DOWN;
                            n[1] += err_g * FS_W_DOWN;
                            n[2] += err_b * FS_W_DOWN;
                        }
                        if x + 1 < width {
                            let n = &mut buf[nr + x + 1];
                            n[0] += err_r * FS_W_DOWN_RIGHT;
                            n[1] += err_g * FS_W_DOWN_RIGHT;
                            n[2] += err_b * FS_W_DOWN_RIGHT;
                        }
                    }
                }
                for x in 0..width {
                    let idx = row_off + x;
                    buf[idx][0] = buf[idx][0].clamp(0.0, 1.0);
                    buf[idx][1] = buf[idx][1].clamp(0.0, 1.0);
                    buf[idx][2] = buf[idx][2].clamp(0.0, 1.0);
                }
            }
        }
    }
}

// ── Color mapping ─────────────────────────────────────────────────────

fn apply_color_map(
    qr: f32, qg: f32, qb: f32,
    color_map: &Option<ColorMappingSettings>,
) -> (f32, f32, f32) {
    let Some(cm) = color_map else { return (qr, qg, qb); };
    let blend = cm.blend_with_original.clamp(0.0, 1.0);
    if blend >= 0.999 { return (qr, qg, qb); }

    let lum = 0.2126 * qr + 0.7152 * qg + 0.0722 * qb;
    let mp = cm.midtone_position.clamp(0.001, 0.999);

    let (cr, cg, cb) = if lum <= mp {
        let t = lum / mp;
        lerp3(
            cm.shadow_color_r.clamp(0.0, 1.0), cm.shadow_color_g.clamp(0.0, 1.0), cm.shadow_color_b.clamp(0.0, 1.0),
            cm.midtone_color_r.clamp(0.0, 1.0), cm.midtone_color_g.clamp(0.0, 1.0), cm.midtone_color_b.clamp(0.0, 1.0),
            t,
        )
    } else {
        let t = (lum - mp) / (1.0 - mp);
        lerp3(
            cm.midtone_color_r.clamp(0.0, 1.0), cm.midtone_color_g.clamp(0.0, 1.0), cm.midtone_color_b.clamp(0.0, 1.0),
            cm.highlight_color_r.clamp(0.0, 1.0), cm.highlight_color_g.clamp(0.0, 1.0), cm.highlight_color_b.clamp(0.0, 1.0),
            t,
        )
    };

    if blend <= 0.001 {
        (cr, cg, cb)
    } else {
        (cr + (qr - cr) * blend, cg + (qg - cg) * blend, cb + (qb - cb) * blend)
    }
}

fn lerp3(r0: f32, g0: f32, b0: f32, r1: f32, g1: f32, b1: f32, t: f32) -> (f32, f32, f32) {
    (r0 + (r1 - r0) * t, g0 + (g1 - g0) * t, b0 + (b1 - b0) * t)
}

// ── Quantization ──────────────────────────────────────────────────────

fn quantize_pixel(
    r: f32, g: f32, b: f32,
    levels_f: f32,
    mode: ToneMode,
    edge_softness: f32,
    preserve_lum: bool,
) -> (f32, f32, f32) {
    match mode {
        ToneMode::PerChannel => {
            let qr = quantize_channel(r, levels_f, edge_softness);
            let qg = quantize_channel(g, levels_f, edge_softness);
            let qb = quantize_channel(b, levels_f, edge_softness);
            if preserve_lum {
                preserve_luminosity(r, g, b, qr, qg, qb)
            } else {
                (qr, qg, qb)
            }
        }
        ToneMode::Luminance => {
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            let q_lum = quantize_channel(lum, levels_f, edge_softness);
            if lum > 0.001 {
                let ratio = q_lum / lum;
                ( (r * ratio).clamp(0.0, 1.0), (g * ratio).clamp(0.0, 1.0), (b * ratio).clamp(0.0, 1.0) )
            } else {
                (q_lum, q_lum, q_lum)
            }
        }
    }
}

fn quantize_channel(v: f32, levels_f: f32, edge_softness: f32) -> f32 {
    if edge_softness <= 0.001 {
        (v * levels_f + 0.5).floor() / levels_f
    } else {
        let scaled = v * levels_f;
        let lower = scaled.floor();
        let frac = scaled - lower;
        let sw = edge_softness * 0.5;
        let t = if frac < sw {
            0.0
        } else if frac > 1.0 - sw {
            1.0
        } else {
            let s = (frac - sw) / (1.0 - 2.0 * sw).max(0.001);
            s * s * (3.0 - 2.0 * s)
        };
        (lower + t) / levels_f
    }
}

fn preserve_luminosity(
    orig_r: f32, orig_g: f32, orig_b: f32,
    qr: f32, qg: f32, qb: f32,
) -> (f32, f32, f32) {
    let orig_lum = 0.2126 * orig_r + 0.7152 * orig_g + 0.0722 * orig_b;
    let q_lum = 0.2126 * qr + 0.7152 * qg + 0.0722 * qb;
    if q_lum > 0.001 {
        let ratio = orig_lum / q_lum;
        ( (qr * ratio).clamp(0.0, 1.0), (qg * ratio).clamp(0.0, 1.0), (qb * ratio).clamp(0.0, 1.0) )
    } else {
        (qr, qg, qb)
    }
}
