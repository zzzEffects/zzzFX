use std::sync::OnceLock;

use rayon::prelude::*;

use crate::settings::pixel_art::{Dithering, ZzzPixelArt};

/// Reciprocal of 255 for u8→f32 conversion.
const RCP_255: f64 = 1.0 / 255.0;

// 4×4 Bayer matrix normalized to 0..1 for ordered dithering.
const BAYER_4X4: [[f32; 4]; 4] = [
    [ 0.0/16.0,  8.0/16.0,  2.0/16.0, 10.0/16.0],
    [12.0/16.0,  4.0/16.0, 14.0/16.0,  6.0/16.0],
    [ 3.0/16.0, 11.0/16.0,  1.0/16.0,  9.0/16.0],
    [15.0/16.0,  7.0/16.0, 13.0/16.0,  5.0/16.0],
];

// Floyd-Steinberg error distribution weights to right/bottom neighbors.
const FS_WEIGHT_RIGHT: f32 = 7.0 / 16.0;
const FS_WEIGHT_DOWN_LEFT: f32 = 3.0 / 16.0;
const FS_WEIGHT_DOWN: f32 = 5.0 / 16.0;
const FS_WEIGHT_DOWN_RIGHT: f32 = 1.0 / 16.0;

// ---------------------------------------------------------------------------
// PixelArtProcessor trait — allows swapping CPU / GPU / fallback strategies
// ---------------------------------------------------------------------------

trait PixelArtProcessor: Send + Sync {
    fn process(
        &self,
        settings: &ZzzPixelArt,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// CpuProcessor — the pure-CPU path (rayon-parallel)
// ---------------------------------------------------------------------------

struct CpuProcessor;

impl PixelArtProcessor for CpuProcessor {
    fn process(
        &self,
        settings: &ZzzPixelArt,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) -> Result<(), String> {
        cpu_render(settings, src, dst, width, height);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// GpuProcessor — delegates to the cached-GPU wgpu compute pipeline
// ---------------------------------------------------------------------------

#[cfg(feature = "gpu")]
struct GpuProcessor;

#[cfg(feature = "gpu")]
impl PixelArtProcessor for GpuProcessor {
    fn process(
        &self,
        settings: &ZzzPixelArt,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) -> Result<(), String> {
        match crate::gpu::pixel_art::try_render(settings, src, dst, width, height) {
            Ok(true) => Ok(()),
            Ok(false) => Err("GPU unavailable".into()),
            Err(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// FallbackProcessor — tries GPU first, falls back to CPU
// ---------------------------------------------------------------------------

#[cfg(feature = "gpu")]
struct FallbackProcessor {
    gpu: GpuProcessor,
    cpu: CpuProcessor,
}

#[cfg(feature = "gpu")]
impl FallbackProcessor {
    fn new() -> Self {
        Self {
            gpu: GpuProcessor,
            cpu: CpuProcessor,
        }
    }
}

#[cfg(feature = "gpu")]
impl PixelArtProcessor for FallbackProcessor {
    fn process(
        &self,
        settings: &ZzzPixelArt,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) -> Result<(), String> {
        match self.gpu.process(settings, src, dst, width, height) {
            Ok(()) => Ok(()),
            Err(_) => self.cpu.process(settings, src, dst, width, height),
        }
    }
}

#[cfg(not(feature = "gpu"))]
struct FallbackProcessor {
    cpu: CpuProcessor,
}

#[cfg(not(feature = "gpu"))]
impl FallbackProcessor {
    fn new() -> Self {
        Self { cpu: CpuProcessor }
    }
}

#[cfg(not(feature = "gpu"))]
impl PixelArtProcessor for FallbackProcessor {
    fn process(
        &self,
        settings: &ZzzPixelArt,
        src: &[u8],
        dst: &mut [u8],
        width: usize,
        height: usize,
    ) -> Result<(), String> {
        self.cpu.process(settings, src, dst, width, height)
    }
}

// ---------------------------------------------------------------------------
// Static processor (initialized once)
// ---------------------------------------------------------------------------

static PROCESSOR: OnceLock<FallbackProcessor> = OnceLock::new();

fn get_processor() -> &'static FallbackProcessor {
    PROCESSOR.get_or_init(FallbackProcessor::new)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl ZzzPixelArt {
    /// Returns true if the effect is an identity for ALL frame dimensions.
    /// This covers the pixel_size=0 case which always produces 1×1 blocks.
    pub fn is_identity(&self) -> bool {
        self.pixel_size_h <= 0.0
            && self.pixel_size_v <= 0.0
            && !self.show_grid
            && self.color_levels >= 256.0
            && matches!(self.dithering, Dithering::None)
            && (self.contrast - 0.5).abs() < 0.001
            && (self.saturation - 0.5).abs() < 0.001
    }

    /// Returns true when the effect would produce no visible change for the given frame size.
    pub fn is_identity_for(&self, width: u32, height: u32) -> bool {
        let pw = ((width as f32 * (self.pixel_size_h.clamp(0.0, 100.0) / 100.0)).round() as u32).clamp(1, width);
        let ph = if self.square {
            pw
        } else {
            ((height as f32 * (self.pixel_size_v.clamp(0.0, 100.0) / 100.0)).round() as u32).clamp(1, height)
        };
        pw == 1
            && ph == 1
            && !self.show_grid
            && self.color_levels >= 256.0
            && matches!(self.dithering, Dithering::None)
            && (self.contrast - 0.5).abs() < 0.001
            && (self.saturation - 0.5).abs() < 0.001
    }

    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;
        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }
        if let Err(e) = get_processor().process(self, src, dst, width, height) {
            eprintln!("[zzzfx] pixel art render failed: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// CPU render path (rayon-parallel)
// ---------------------------------------------------------------------------

fn cpu_render(
    settings: &ZzzPixelArt,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) {
    // ── Compute pixel dimensions from percentages ─────────────────
    let pixel_w = ((width as f32 * (settings.pixel_size_h.clamp(0.0, 100.0) / 100.0)).round() as usize)
        .clamp(1, width);
    let pixel_h = if settings.square {
        pixel_w
    } else {
        ((height as f32 * (settings.pixel_size_v.clamp(0.0, 100.0) / 100.0)).round() as usize)
            .clamp(1, height)
    };

    let color_levels = (settings.color_levels.clamp(2.0, 256.0).floor() as usize).max(2);
    let dither_amount = settings.dithering_amount.clamp(0.0, 1.0);
    let show_grid = settings.show_grid;
    let grid_thickness = settings.grid_thickness.clamp(0.0, 1.0);
    let grid_r = settings.grid_color_r.clamp(0.0, 1.0);
    let grid_g = settings.grid_color_g.clamp(0.0, 1.0);
    let grid_b = settings.grid_color_b.clamp(0.0, 1.0);
    let grid_a = settings.grid_color_a.clamp(0.0, 1.0);
    let contrast = settings.contrast.clamp(0.0, 1.0) as f64;
    let saturation = settings.saturation.clamp(0.0, 1.0) as f64;

    let contrast_factor = 1.0 + (contrast - 0.5) * 2.0;
    let saturation_factor = 1.0 + (saturation - 0.5) * 2.0;

    let cols = width.div_ceil(pixel_w);
    let rows = height.div_ceil(pixel_h);

    // ── Variable-cell path: per-column/row widths via Bresenham rounding ──
    if !settings.use_same_integer {
        cpu_render_variable(settings, src, dst, width, height);
        return;
    }

    // Stage 1: Parallel cell analysis — compute average color per cell,
    // apply contrast/saturation/ordered-dithering, then quantize
    // (skip quantization for Floyd-Steinberg — it handles it in pass 1b).
    let is_fs = matches!(settings.dithering, Dithering::FloydSteinberg);

    let total_cells = rows * cols;
    let mut cells: Vec<[f32; 4]> = (0..total_cells)
        .into_par_iter()
        .map(|cell_idx| {
            let row = cell_idx / cols;
            let col = cell_idx % cols;
            let cell_y = row * pixel_h;
            let cell_h = pixel_h.min(height - cell_y);
            let cell_x = col * pixel_w;
            let cell_w = pixel_w.min(width - cell_x);

            let mut sum_r = 0.0f64;
            let mut sum_g = 0.0f64;
            let mut sum_b = 0.0f64;
            let mut sum_a = 0.0f64;
            let mut count = 0u64;

            for dy in 0..cell_h {
                let src_row = (cell_y + dy) * width;
                for dx in 0..cell_w {
                    let i = (src_row + cell_x + dx) * 4;
                    sum_r += src[i] as f64;
                    sum_g += src[i + 1] as f64;
                    sum_b += src[i + 2] as f64;
                    sum_a += src[i + 3] as f64;
                    count += 1;
                }
            }

            let np = count.max(1) as f64;
            let mut r = (sum_r * RCP_255 / np) as f32;
            let mut g = (sum_g * RCP_255 / np) as f32;
            let mut b = (sum_b * RCP_255 / np) as f32;
            let a = (sum_a * RCP_255 / np) as f32;

            // Apply contrast: (v - 0.5) * factor + 0.5
            r = (((r as f64 - 0.5) * contrast_factor + 0.5).clamp(0.0, 1.0)) as f32;
            g = (((g as f64 - 0.5) * contrast_factor + 0.5).clamp(0.0, 1.0)) as f32;
            b = (((b as f64 - 0.5) * contrast_factor + 0.5).clamp(0.0, 1.0)) as f32;

            // Apply saturation: (v - lum) * factor + lum (Rec.709)
            let lum = 0.2126_f64 * r as f64 + 0.7152_f64 * g as f64 + 0.0722_f64 * b as f64;
            r = (((r as f64 - lum) * saturation_factor + lum).clamp(0.0, 1.0)) as f32;
            g = (((g as f64 - lum) * saturation_factor + lum).clamp(0.0, 1.0)) as f32;
            b = (((b as f64 - lum) * saturation_factor + lum).clamp(0.0, 1.0)) as f32;

            // Apply ordered dithering before quantization
            if matches!(settings.dithering, Dithering::Ordered) {
                let bayer_val = BAYER_4X4[row % 4][col % 4];
                let noise = (bayer_val - 0.5) * dither_amount;
                r = (r + noise).clamp(0.0, 1.0);
                g = (g + noise).clamp(0.0, 1.0);
                b = (b + noise).clamp(0.0, 1.0);
            }

            // Quantize — skip for Floyd-Steinberg (handled in pass 1b)
            if !is_fs {
                let levels_f = (color_levels - 1) as f32;
                r = (r * levels_f + 0.5).floor() / levels_f;
                g = (g * levels_f + 0.5).floor() / levels_f;
                b = (b * levels_f + 0.5).floor() / levels_f;
            }

            [r, g, b, a]
        })
        .collect();

    // Stage 1b: Floyd-Steinberg error diffusion (serial, scan-line)
    if is_fs {
        floyd_steinberg_diffuse(&mut cells, cols, rows, color_levels, dither_amount);
    }

    // Stage 2: Fill destination pixels with cell colors + grid overlay
    // Hoist grid pixel constants outside the loop
    let grid_px_h = if show_grid {
        (grid_thickness * pixel_w as f32).round() as usize
    } else {
        0
    };
    let grid_px_v = if show_grid {
        (grid_thickness * pixel_h as f32).round() as usize
    } else {
        0
    };

    let strip_height = pixel_h;
    let strip_bytes = width * strip_height * 4;

    dst.par_chunks_mut(strip_bytes)
        .enumerate()
        .for_each(|(strip_idx, strip)| {
            let strip_start_y = strip_idx * strip_height;
            let strip_end_y = (strip_start_y + strip_height).min(height);

            for row in 0..rows {
                let cell_y = row * pixel_h;
                if cell_y >= strip_end_y || (cell_y + pixel_h) <= strip_start_y {
                    continue;
                }
                for col in 0..cols {
                    let cell_x = col * pixel_w;
                    let cell_w = pixel_w.min(width - cell_x);
                    let [cr, cg, cb, ca] = cells[row * cols + col];

                    for dy in 0..pixel_h {
                        let abs_y = cell_y + dy;
                        if abs_y < strip_start_y || abs_y >= strip_end_y {
                            continue;
                        }
                        let strip_row = abs_y - strip_start_y;
                        let out_row_base = strip_row * width;

                        let is_grid_row =
                            show_grid && dy >= pixel_h - grid_px_v && dy < pixel_h;

                        for dx in 0..cell_w {
                            let out_idx = (out_row_base + cell_x + dx) * 4;

                            let is_grid_col =
                                show_grid && dx >= cell_w - grid_px_h && dx < cell_w;
                            let is_grid = is_grid_row || is_grid_col;

                            let (out_r, out_g, out_b) = if is_grid {
                                (
                                    cr * (1.0 - grid_a) + grid_r * grid_a,
                                    cg * (1.0 - grid_a) + grid_g * grid_a,
                                    cb * (1.0 - grid_a) + grid_b * grid_a,
                                )
                            } else {
                                (cr, cg, cb)
                            };

                            strip[out_idx] = (out_r * 255.0).round() as u8;
                            strip[out_idx + 1] = (out_g * 255.0).round() as u8;
                            strip[out_idx + 2] = (out_b * 255.0).round() as u8;
                            strip[out_idx + 3] = (ca * 255.0).round() as u8;
                        }
                    }
                }
            }
        });
}

// ---------------------------------------------------------------------------
// Variable-cell CPU render path (use_same_integer = false)
// ---------------------------------------------------------------------------

/// Compute per-column (or per-row) widths via Bresenham cumulative rounding.
/// Returns (count, widths) where widths sum to `total` and each width is
/// either floor(target) or ceil(target), except possibly the last.
fn compute_variable_sizes(target: f32, total: usize) -> (usize, Vec<usize>) {
    if target <= 0.5 {
        return (1, vec![total]);
    }
    let mut starts = vec![0usize];
    let mut i = 1;
    loop {
        let s = (i as f32 * target).round() as usize;
        if s >= total {
            break;
        }
        starts.push(s);
        i += 1;
    }
    starts.push(total);
    let count = starts.len() - 1;
    let widths: Vec<usize> = starts.windows(2).map(|w| (w[1] - w[0]).max(1)).collect();
    (count, widths)
}

fn cpu_render_variable(
    settings: &ZzzPixelArt,
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
) {
    let target_w = width as f32 * (settings.pixel_size_h.clamp(0.0, 100.0) / 100.0);
    let target_h = if settings.square {
        target_w
    } else {
        height as f32 * (settings.pixel_size_v.clamp(0.0, 100.0) / 100.0)
    };

    let (cols, col_widths) = compute_variable_sizes(target_w, width);
    let (rows, row_heights) = compute_variable_sizes(target_h, height);

    // Cumulative start positions
    let col_starts: Vec<usize> = std::iter::once(0)
        .chain(col_widths.iter().scan(0, |acc, w| {
            *acc += w;
            Some(*acc)
        }))
        .take(cols)
        .collect();
    let row_starts: Vec<usize> = std::iter::once(0)
        .chain(row_heights.iter().scan(0, |acc, h| {
            *acc += h;
            Some(*acc)
        }))
        .take(rows)
        .collect();

    let color_levels = (settings.color_levels.clamp(2.0, 256.0).floor() as usize).max(2);
    let dither_amount = settings.dithering_amount.clamp(0.0, 1.0);
    let show_grid = settings.show_grid;
    let grid_thickness = settings.grid_thickness.clamp(0.0, 1.0);
    let grid_r = settings.grid_color_r.clamp(0.0, 1.0);
    let grid_g = settings.grid_color_g.clamp(0.0, 1.0);
    let grid_b = settings.grid_color_b.clamp(0.0, 1.0);
    let grid_a = settings.grid_color_a.clamp(0.0, 1.0);
    let contrast = settings.contrast.clamp(0.0, 1.0) as f64;
    let saturation = settings.saturation.clamp(0.0, 1.0) as f64;

    let contrast_factor = 1.0 + (contrast - 0.5) * 2.0;
    let saturation_factor = 1.0 + (saturation - 0.5) * 2.0;
    let is_fs = matches!(settings.dithering, Dithering::FloydSteinberg);

    // Stage 1: Parallel cell analysis with per-cell dimensions
    let total_cells = rows * cols;
    let mut cells: Vec<[f32; 4]> = (0..total_cells)
        .into_par_iter()
        .map(|cell_idx| {
            let row = cell_idx / cols;
            let col = cell_idx % cols;
            let cell_h = row_heights[row];
            let cell_w = col_widths[col];
            let cell_y = row_starts[row];
            let cell_x = col_starts[col];

            let mut sum_r = 0.0f64;
            let mut sum_g = 0.0f64;
            let mut sum_b = 0.0f64;
            let mut sum_a = 0.0f64;
            let mut count = 0u64;

            for dy in 0..cell_h {
                let src_row = (cell_y + dy) * width;
                for dx in 0..cell_w {
                    let i = (src_row + cell_x + dx) * 4;
                    sum_r += src[i] as f64;
                    sum_g += src[i + 1] as f64;
                    sum_b += src[i + 2] as f64;
                    sum_a += src[i + 3] as f64;
                    count += 1;
                }
            }

            let np = count.max(1) as f64;
            let mut r = (sum_r * RCP_255 / np) as f32;
            let mut g = (sum_g * RCP_255 / np) as f32;
            let mut b = (sum_b * RCP_255 / np) as f32;
            let a = (sum_a * RCP_255 / np) as f32;

            r = (((r as f64 - 0.5) * contrast_factor + 0.5).clamp(0.0, 1.0)) as f32;
            g = (((g as f64 - 0.5) * contrast_factor + 0.5).clamp(0.0, 1.0)) as f32;
            b = (((b as f64 - 0.5) * contrast_factor + 0.5).clamp(0.0, 1.0)) as f32;

            let lum = 0.2126_f64 * r as f64 + 0.7152_f64 * g as f64 + 0.0722_f64 * b as f64;
            r = (((r as f64 - lum) * saturation_factor + lum).clamp(0.0, 1.0)) as f32;
            g = (((g as f64 - lum) * saturation_factor + lum).clamp(0.0, 1.0)) as f32;
            b = (((b as f64 - lum) * saturation_factor + lum).clamp(0.0, 1.0)) as f32;

            if matches!(settings.dithering, Dithering::Ordered) {
                let bayer_val = BAYER_4X4[row % 4][col % 4];
                let noise = (bayer_val - 0.5) * dither_amount;
                r = (r + noise).clamp(0.0, 1.0);
                g = (g + noise).clamp(0.0, 1.0);
                b = (b + noise).clamp(0.0, 1.0);
            }

            if !is_fs {
                let levels_f = (color_levels - 1) as f32;
                r = (r * levels_f + 0.5).floor() / levels_f;
                g = (g * levels_f + 0.5).floor() / levels_f;
                b = (b * levels_f + 0.5).floor() / levels_f;
            }

            [r, g, b, a]
        })
        .collect();

    // Stage 1b: Floyd-Steinberg error diffusion
    if is_fs {
        floyd_steinberg_diffuse(&mut cells, cols, rows, color_levels, dither_amount);
    }

    // Stage 2: Fill destination pixels. Process cells in parallel via output rows.
    // Each output row belongs to exactly one cell row, so we can parallelise by row.
    dst.par_chunks_mut(width * 4)
        .enumerate()
        .for_each(|(y, out_row)| {
            // Find which row this pixel y belongs to
            let cell_row = match row_starts.binary_search(&y) {
                Ok(r) => r,
                Err(r) => r.saturating_sub(1),
            };
            if cell_row >= rows || y >= row_starts[cell_row] + row_heights[cell_row] {
                return; // shouldn't happen
            }

            let cell_h = row_heights[cell_row];
            let cell_y = row_starts[cell_row];
            let dy = y - cell_y;

            let grid_px_v = (grid_thickness * target_h).round() as usize;
            let is_grid_row = show_grid && dy >= cell_h.saturating_sub(grid_px_v) && dy < cell_h;

            for col in 0..cols {
                let cell_w = col_widths[col];
                let cell_x = col_starts[col];
                let cell_end_x = cell_x + cell_w;
                let [cr, cg, cb, ca] = cells[cell_row * cols + col];

                let grid_px_h = (grid_thickness * target_w).round() as usize;
                let grid_start_x = cell_end_x.saturating_sub(grid_px_h);

                for x in cell_x..cell_end_x.min(width) {
                    let out_idx = (x) * 4;
                    let is_grid_col =
                        show_grid && x >= grid_start_x && x < cell_end_x;
                    let is_grid = is_grid_row || is_grid_col;

                    let (out_r, out_g, out_b) = if is_grid {
                        (
                            cr * (1.0 - grid_a) + grid_r * grid_a,
                            cg * (1.0 - grid_a) + grid_g * grid_a,
                            cb * (1.0 - grid_a) + grid_b * grid_a,
                        )
                    } else {
                        (cr, cg, cb)
                    };

                    out_row[out_idx] = (out_r * 255.0).round() as u8;
                    out_row[out_idx + 1] = (out_g * 255.0).round() as u8;
                    out_row[out_idx + 2] = (out_b * 255.0).round() as u8;
                    out_row[out_idx + 3] = (ca * 255.0).round() as u8;
                }
            }
        });
}

// ---------------------------------------------------------------------------
// Floyd-Steinberg error diffusion
// ---------------------------------------------------------------------------

pub(crate) fn floyd_steinberg_diffuse(
    cells: &mut [[f32; 4]],
    cols: usize,
    rows: usize,
    color_levels: usize,
    dither_amount: f32,
) {
    let levels_f = (color_levels - 1) as f32;

    // Save original unquantized RGB values for blending (skip when dither_amount == 1.0)
    let originals: Option<Vec<[f32; 3]>> = if dither_amount < 1.0 {
        Some(cells.iter().map(|c| [c[0], c[1], c[2]]).collect())
    } else {
        None
    };

    for row in 0..rows {
        for col in 0..cols {
            let idx = row * cols + col;
            let old_r = cells[idx][0];
            let old_g = cells[idx][1];
            let old_b = cells[idx][2];

            // Quantize (full error diffusion)
            let new_r = (old_r * levels_f + 0.5).floor() / levels_f;
            let new_g = (old_g * levels_f + 0.5).floor() / levels_f;
            let new_b = (old_b * levels_f + 0.5).floor() / levels_f;

            // Blend with original using dither_amount
            if let Some(ref orig) = originals {
                let o = orig[idx];
                cells[idx][0] = o[0] + (new_r - o[0]) * dither_amount;
                cells[idx][1] = o[1] + (new_g - o[1]) * dither_amount;
                cells[idx][2] = o[2] + (new_b - o[2]) * dither_amount;
            } else {
                cells[idx] = [new_r, new_g, new_b, cells[idx][3]];
            }

            // Compute error relative to the blended result, then diffuse
            let err_r = (old_r - cells[idx][0]) * dither_amount;
            let err_g = (old_g - cells[idx][1]) * dither_amount;
            let err_b = (old_b - cells[idx][2]) * dither_amount;

            // Diffuse error to neighbors (no per-write clamping — bounded error)
            if col + 1 < cols {
                let n = &mut cells[idx + 1];
                n[0] += err_r * FS_WEIGHT_RIGHT;
                n[1] += err_g * FS_WEIGHT_RIGHT;
                n[2] += err_b * FS_WEIGHT_RIGHT;
            }
            if row + 1 < rows {
                if col > 0 {
                    let n = &mut cells[idx + cols - 1];
                    n[0] += err_r * FS_WEIGHT_DOWN_LEFT;
                    n[1] += err_g * FS_WEIGHT_DOWN_LEFT;
                    n[2] += err_b * FS_WEIGHT_DOWN_LEFT;
                }
                {
                    let n = &mut cells[idx + cols];
                    n[0] += err_r * FS_WEIGHT_DOWN;
                    n[1] += err_g * FS_WEIGHT_DOWN;
                    n[2] += err_b * FS_WEIGHT_DOWN;
                }
                if col + 1 < cols {
                    let n = &mut cells[idx + cols + 1];
                    n[0] += err_r * FS_WEIGHT_DOWN_RIGHT;
                    n[1] += err_g * FS_WEIGHT_DOWN_RIGHT;
                    n[2] += err_b * FS_WEIGHT_DOWN_RIGHT;
                }
            }
        }

        // Per-row final clamp to keep values in [0, 1]
        for col in 0..cols {
            let idx = row * cols + col;
            cells[idx][0] = cells[idx][0].clamp(0.0, 1.0);
            cells[idx][1] = cells[idx][1].clamp(0.0, 1.0);
            cells[idx][2] = cells[idx][2].clamp(0.0, 1.0);
        }
    }
}
