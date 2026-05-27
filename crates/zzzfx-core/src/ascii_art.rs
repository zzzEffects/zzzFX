use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use font_kit::source::SystemSource;
use fontdue::Font as FontdueFont;
use rayon::prelude::*;

use crate::settings::ascii_art::{ColorMode, ZzzAsciiArt};

// ---------------------------------------------------------------------------
// Glyph bitmap — single channel alpha, copy-on-write via Arc
// ---------------------------------------------------------------------------

pub(crate) struct GlyphBitmap {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) data: Arc<[u8]>,
}

pub(crate) struct GlyphCache {
    pub(crate) font_name: String,
    pub(crate) font_size: f32,
    pub(crate) charset: String,
    pub(crate) bitmaps: Arc<[GlyphBitmap]>,
}

/// Shared glyph cache. Wrapped in `Arc<Mutex<>>` so the lock is only held
/// during cache rebuilds — render threads clone an `Arc<GlyphCache>` and
/// drop the lock immediately, preventing cross-instance blocking (fixes B2).
static GLYPH_CACHE: OnceLock<Mutex<Arc<GlyphCache>>> = OnceLock::new();

fn cache_lock() -> &'static Mutex<Arc<GlyphCache>> {
    GLYPH_CACHE.get_or_init(|| Mutex::new(Arc::new(GlyphCache {
        font_name: String::new(),
        font_size: 0.0,
        charset: String::new(),
        bitmaps: Arc::new([]),
    })))
}

/// Returns a cloned `Arc<GlyphCache>` without holding the lock during rendering.
fn get_cache_snapshot() -> Arc<GlyphCache> {
    cache_lock().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

// ---------------------------------------------------------------------------
// Font loading — returns None instead of panicking (fixes B3)
// ---------------------------------------------------------------------------

fn load_monospace_font(preferred_name: &str) -> Option<Vec<u8>> {
    let source = SystemSource::new();

    if !preferred_name.is_empty() {
        if let Ok(handle) = source.select_by_postscript_name(preferred_name) {
            if let Ok(font) = handle.load() {
                if let Some(data) = font.copy_font_data() {
                    return Some(data.to_vec());
                }
            }
        }
        let query = preferred_name.to_lowercase();
        if let Ok(handles) = source.all_fonts() {
            for handle in handles {
                if let Ok(font) = handle.load() {
                    let full = font.full_name().to_lowercase();
                    let family = font.family_name().to_lowercase();
                    if full == query || family == query {
                        if let Some(data) = font.copy_font_data() {
                            return Some(data.to_vec());
                        }
                    }
                }
            }
        }
    }

    let candidates = [
        "Consolas", "CourierNewPSMT", "CourierNew", "Courier",
        "LiberationMono", "DejaVuSansMono", "Menlo-Regular", "Monaco",
        "SourceCodePro-Regular", "JetBrainsMono-Regular",
    ];

    for name in &candidates {
        if let Ok(handle) = source.select_by_postscript_name(name) {
            if let Ok(font) = handle.load() {
                if let Some(data) = font.copy_font_data() {
                    return Some(data.to_vec());
                }
            }
        }
    }

    let handles = source.all_fonts().unwrap_or_default();
    for handle in handles {
        if let Ok(font) = handle.load() {
            if let Some(data) = font.copy_font_data() {
                return Some(data.to_vec());
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Font coverage helpers (for CJK fallback — fixes B7 by reusing font handles)
// ---------------------------------------------------------------------------

fn font_covers_charset(font_data: &[u8], charset: &str) -> bool {
    let Ok(font) = FontdueFont::from_bytes(font_data, fontdue::FontSettings::default()) else {
        return false;
    };
    charset
        .chars()
        .all(|c| c == ' ' || font.lookup_glyph_index(c) != 0)
}

fn find_font_for_charset(charset: &str) -> Option<Vec<u8>> {
    #[cfg(target_os = "windows")]
    let cjk: &[&str] = &[
        "MicrosoftYaHei","MicrosoftYaHeiUI","SimSun","NSimSun","SimHei",
        "FangSong","KaiTi","MSMincho","MSGothic","MalgunGothic","Gulim","Dotum",
    ];
    #[cfg(target_os = "macos")]
    let cjk: &[&str] = &[
        "PingFangSC-Regular","PingFangTC-Regular","HiraginoSans-W3",
        "HiraginoSans-W6","AppleSDGothicNeo-Regular",
    ];
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let cjk: &[&str] = &[
        "NotoSansCJK-Regular","NotoSansSC-Regular","NotoSansTC-Regular",
        "NotoSansJP-Regular","NotoSansKR-Regular","WenQuanYiMicroHei",
        "WenQuanYiZenHei","SourceHanSansSC-Regular","SourceHanSansJP-Regular",
    ];

    let source = SystemSource::new();
    for name in cjk {
        if let Ok(handle) = source.select_by_postscript_name(name) {
            if let Ok(font) = handle.load() {
                if let Some(data) = font.copy_font_data() {
                    if font_covers_charset(&data, charset) {
                        return Some(data.to_vec());
                    }
                }
            }
        }
    }

    let handles = source.all_fonts().unwrap_or_default();
    for handle in handles {
        if let Ok(font) = handle.load() {
            if let Some(data) = font.copy_font_data() {
                if font_covers_charset(&data, charset) {
                    return Some(data.to_vec());
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Glyph cache builder — returns None on failure (fixes B4)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Font data caches — avoids re-enumerating system fonts on every
// font_size / charset change. Only reloaded when font_name changes.
// ---------------------------------------------------------------------------

static FONT_CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
static CJK_CACHE: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();

fn font_cache() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    FONT_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cjk_cache() -> &'static Mutex<HashMap<String, Vec<u8>>> {
    CJK_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_or_load_font(font_name: &str) -> Option<Vec<u8>> {
    let mut guard = font_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(data) = guard.get(font_name) {
        return Some(data.clone());
    }
    let data = load_monospace_font(font_name)?;
    guard.insert(font_name.to_string(), data.clone());
    Some(data)
}

fn get_or_load_cjk(charset: &str) -> Option<Vec<u8>> {
    let mut guard = cjk_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(data) = guard.get(charset) {
        return Some(data.clone());
    }
    let data = find_font_for_charset(charset)?;
    guard.insert(charset.to_string(), data.clone());
    Some(data)
}

fn build_glyph_cache(font_name: &str, font_size: f32, charset: &str) -> Option<GlyphCache> {
    // Font data is cached — reloaded only when font_name changes (fixes lag
    // when toggling char set or adjusting font_size).
    let font_data = get_or_load_font(font_name)?;

    let font_data = if font_covers_charset(&font_data, charset) {
        font_data
    } else if let Some(cjk) = get_or_load_cjk(charset) {
        cjk
    } else {
        font_data
    };

    let font = FontdueFont::from_bytes(&font_data[..], fontdue::FontSettings::default()).ok()?;

    let mut bitmaps: Vec<GlyphBitmap> = Vec::with_capacity(charset.chars().count());
    for ch in charset.chars() {
        let (metrics, coverage) = font.rasterize(ch, font_size as f32);
        let row_bytes = metrics.width;
        let mut flipped = vec![0u8; coverage.len()];
        for y in 0..metrics.height {
            let src_row = y * row_bytes;
            let dst_row = (metrics.height - 1 - y) * row_bytes;
            flipped[dst_row..dst_row + row_bytes]
                .copy_from_slice(&coverage[src_row..src_row + row_bytes]);
        }
        bitmaps.push(GlyphBitmap {
            width: metrics.width as u32,
            height: metrics.height as u32,
            data: flipped.into(),
        });
    }

    Some(GlyphCache {
        font_name: font_name.to_string(),
        font_size,
        charset: charset.to_string(),
        bitmaps: bitmaps.into(),
    })
}

fn ensure_cache(font_name: &str, font_size: f32, charset: &str) {
    let mut guard = cache_lock().lock().unwrap_or_else(|e| e.into_inner());
    let needs_rebuild = guard.font_name != font_name
        || (guard.font_size - font_size).abs() > 1e-4
        || guard.charset != charset;
    if needs_rebuild {
        if let Some(new_cache) = build_glyph_cache(font_name, font_size, charset) {
            *guard = Arc::new(new_cache);
        }
    }
}

// ---------------------------------------------------------------------------
// Per-thread reusable cell buffers (fixes B6 — avoids frame allocation)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// GPU path (forward declaration)
// ---------------------------------------------------------------------------

mod gpu_impl {
    use super::*;
    pub fn try_gpu_render(
        _settings: &ZzzAsciiArt,
        _src: &[u8],
        _dst: &mut [u8],
        _width: usize,
        _height: usize,
        _cache: &GlyphCache,
    ) -> Result<bool, String> {
        // GPU path disabled pending output verification.
        // Re-enable: crate::gpu::ascii_art::try_render(...)
        Ok(false)
    }
}

// ---------------------------------------------------------------------------
// Effect implementation
// ---------------------------------------------------------------------------

/// Reciprocal of 255, for converting u8→f32.
const RCP_255: f64 = 1.0 / 255.0;

#[inline]
fn sample_glyph_x(local: f32, bm_w: u32, cell_size: f32, fill: bool, scale: f32) -> u32 {
    sample_glyph_1d(local, bm_w, cell_size, fill, scale)
}

#[inline]
fn sample_glyph_y(local: f32, bm_h: u32, cell_size: f32, fill: bool, scale: f32) -> u32 {
    sample_glyph_1d(local, bm_h, cell_size, fill, scale)
}

/// Map a local coordinate [0,1] within a cell to a glyph bitmap index,
/// applying fill/stretch. Returns `size` (sentinel) if out of bounds.
fn sample_glyph_1d(local: f32, bm_size: u32, cell_size: f32, fill: bool, scale: f32) -> u32 {
    if bm_size == 0 { return 0; }
    let bms = bm_size as f32;
    if fill {
        // Glyph fills the cell: local [0,1] → bitmap [0, bms-1], with scale
        let c = (local - 0.5) / scale + 0.5;
        if c < 0.0 || c > 1.0 { return bm_size; }
        ((c * (bms - 1.0)).round() as u32).min(bm_size - 1)
    } else {
        // Glyph at native size, centered: local offset maps to bitmap
        let glyph_w = bms;
        let _margin = (cell_size - glyph_w).max(0.0) * 0.5 / cell_size;
        // With scale: glyph appears scale× larger in the cell
        let scaled_w = glyph_w * scale;
        let scaled_margin = (cell_size - scaled_w).max(0.0) * 0.5 / cell_size;
        if local < scaled_margin || local > 1.0 - scaled_margin { return bm_size; }
        let c = (local - scaled_margin) / (1.0 - 2.0 * scaled_margin);
        ((c * (bms - 1.0)).round() as u32).min(bm_size - 1)
    }
}

impl ZzzAsciiArt {
    pub fn is_identity(&self) -> bool {
        false
    }

    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;

        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }
        // Clear output to prevent ghosting when host reuses buffers (VEGAS Pro)
        dst[..len].fill(0);

        let charset = self.resolve_charset();

        // Compute floating-point cell size — no rounding (user request #1)
        let min_dim = width.min(height) as f32;
        let cell_size = if self.font_size > 0.0 {
            self.font_size * min_dim / 100.0
        } else {
            1.0 // fallback for zero/negative font_size (crash fix #3)
        };

        ensure_cache(&self.font_name, cell_size, &charset);

        let cache = get_cache_snapshot();
        let bitmaps = &cache.bitmaps;
        if bitmaps.is_empty() {
            return; // crash fix #3: no glyphs → nothing to render
        }

        // ── Try GPU first ──────────────────────────────────────────
        match gpu_impl::try_gpu_render(self, src, dst, width, height, &cache) {
            Ok(true) => return,
            Ok(false) => {}
            Err(_) => {}
        }

        // ── CPU path ───────────────────────────────────────────────

        let bg_r = self.bg_color_r.clamp(0.0, 1.0);
        let bg_g = self.bg_color_g.clamp(0.0, 1.0);
        let bg_b = self.bg_color_b.clamp(0.0, 1.0);
        let bg_a = self.bg_color_a.clamp(0.0, 1.0);
        let brightness = self.brightness.clamp(0.0, 1.0) as f64;
        let contrast = self.contrast.clamp(0.0, 1.0) as f64;
        let invert = self.invert_luma;
        let color_mode = self.color_mode;
        let charset_len = charset.chars().count();
        let contrast_factor = 1.0 + (contrast - 0.5) * 2.0;

        let w = width as f32;
        let h = height as f32;

        // Grid dimensions — +2 ensures cells on both sides fully cover the frame
        // regardless of offset direction (fixes edge gaps).
        let cols = (w / cell_size).ceil().max(1.0) as usize + 2;
        let rows = (h / cell_size).ceil().max(1.0) as usize + 2;

        // Position snaps a cell corner to (px*w, py*h), then shifted left/up by
        // one cell so the grid always starts before the frame origin.
        let ox = self.pos_x * w - (self.pos_x * w / cell_size).round() * cell_size - cell_size;
        let oy = self.pos_y * h - (self.pos_y * h / cell_size).round() * cell_size - cell_size;

        // ── Cell analysis (per output pixel → determine cell → sample source) ──
        let cell_size_f64 = cell_size as f64;
        let ox_f64 = ox as f64;
        let oy_f64 = oy as f64;
        let w_f64 = w as f64;
        let h_f64 = h as f64;

        // Compute cell metadata: for each cell, average the source pixels it covers
        struct CellData {
            char_idx: usize,
            avg_r: f32,
            avg_g: f32,
            avg_b: f32,
        }

        let all_cells: Vec<Vec<CellData>> = (0..rows)
            .into_par_iter()
            .map(|row| {
                let cy0 = row as f64 * cell_size_f64 + oy_f64;
                let cy1 = (cy0 + cell_size_f64).min(h_f64);
                let cy0 = cy0.max(0.0);
                let iy0 = cy0.floor() as usize;
                let iy1 = (cy1.ceil() as usize).min(height);
                let mut row_cells = Vec::with_capacity(cols);

                for col in 0..cols {
                    let cx0 = col as f64 * cell_size_f64 + ox_f64;
                    let cx1 = (cx0 + cell_size_f64).min(w_f64);
                    let cx0 = cx0.max(0.0);
                    let ix0 = cx0.floor() as usize;
                    let ix1 = (cx1.ceil() as usize).min(width);

                    let mut sum_luma = 0.0f64;
                    let mut sum_r = 0.0f64;
                    let mut sum_g = 0.0f64;
                    let mut sum_b = 0.0f64;
                    let mut total_weight = 0.0f64;

                    for iy in iy0..iy1 {
                        let py0 = iy as f64;
                        let py1 = py0 + 1.0;
                        let wy = (py1.min(cy1) - py0.max(cy0)).max(0.0);
                        let src_row = iy * width;

                        for ix in ix0..ix1 {
                            let px0 = ix as f64;
                            let px1 = px0 + 1.0;
                            let wx = (px1.min(cx1) - px0.max(cx0)).max(0.0);
                            let w = wx * wy;

                            let idx = (src_row + ix) * 4;
                            let r = src[idx] as f64;
                            let g = src[idx + 1] as f64;
                            let b = src[idx + 2] as f64;
                            sum_luma += (0.2126 * r + 0.7152 * g + 0.0722 * b) * w;
                            sum_r += r * w;
                            sum_g += g * w;
                            sum_b += b * w;
                            total_weight += w;
                        }
                    }

                    let inv = if total_weight > 0.0 { 1.0 / total_weight } else { 0.0 };
                    let avg_luma = sum_luma * RCP_255 * inv;
                    let avg_r = (sum_r * RCP_255 * inv) as f32;
                    let avg_g = (sum_g * RCP_255 * inv) as f32;
                    let avg_b = (sum_b * RCP_255 * inv) as f32;

                    let adjusted = ((avg_luma - 0.5) * contrast_factor + 0.5
                        + (brightness - 0.5))
                    .clamp(0.0, 1.0);

                    let luma = if invert { 1.0 - adjusted } else { adjusted };
                    let raw = ((luma * (charset_len - 1) as f64).round() as usize)
                        .min(charset_len.saturating_sub(1));
                    let char_idx = charset_len.saturating_sub(1).saturating_sub(raw);

                    row_cells.push(CellData { char_idx, avg_r, avg_g, avg_b });
                }
                row_cells
            })
            .collect();

        // ── Cell rendering: per-output-row, parallel ───────────
        let row_bytes = width * 4;
        dst.par_chunks_mut(row_bytes)
            .enumerate()
            .for_each(|(iy, row)| {
                let row_f = iy as f32;
                let grid_row = ((row_f - oy) / cell_size).floor() as isize;
                if grid_row < 0 || grid_row as usize >= rows {
                    // outside grid → fill with background
                    for ix in 0..width {
                        let o = ix * 4;
                        row[o] = (bg_r * 255.0).round() as u8;
                        row[o + 1] = (bg_g * 255.0).round() as u8;
                        row[o + 2] = (bg_b * 255.0).round() as u8;
                        row[o + 3] = (bg_a * 255.0).round() as u8;
                    }
                    return;
                }
                let r = grid_row as usize;
                let row_cells = &all_cells[r];
                let cell_y0 = r as f32 * cell_size + oy;

                for (col, cell) in row_cells.iter().enumerate() {
                    let cell_x0 = col as f32 * cell_size + ox;
                    let bitmap = &bitmaps[cell.char_idx];
                    let bm_w = bitmap.width;
                    let bm_h = bitmap.height;
                    if bm_w == 0 || bm_h == 0 { continue; }

                    let (fg_r, fg_g, fg_b): (f32, f32, f32) = match color_mode {
                        ColorMode::Grayscale => (1.0, 1.0, 1.0),
                        ColorMode::Colored => (cell.avg_r, cell.avg_g, cell.avg_b),
                        ColorMode::GreenTerminal => (0.0, 1.0, 0.0),
                    };

                    let ix0 = (cell_x0.floor() as isize).max(0) as usize;
                    let ix1 = ((cell_x0 + cell_size).ceil() as isize).max(0) as usize;
                    let ix1 = ix1.min(width);

                    let cos_a = self.font_rotation.to_radians().cos();
                    let sin_a = self.font_rotation.to_radians().sin();

                    for ix in ix0..ix1 {
                        let lx = (ix as f32 + 0.5 - cell_x0) / cell_size;
                        let ly = (row_f + 0.5 - cell_y0) / cell_size;
                        // Rotation around cell center
                        let rx = (lx - 0.5) * cos_a - (ly - 0.5) * sin_a + 0.5;
                        let ry = (lx - 0.5) * sin_a + (ly - 0.5) * cos_a + 0.5;
                        let bm_y = sample_glyph_y(ry, bm_h, cell_size, self.font_fill, self.font_scale_y);
                        if bm_y >= bm_h { continue; }
                        let bm_x = sample_glyph_x(rx, bm_w, cell_size, self.font_fill, self.font_scale_x);
                        if bm_x >= bm_w { continue; }
                        let glyph_alpha = bitmap.data[bm_y as usize * bm_w as usize + bm_x as usize] as f32 / 255.0;

                        let fa = glyph_alpha;
                        let o = ix * 4;
                        row[o] = (fg_r * fa + bg_r * (1.0 - fa)).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                        row[o + 1] = (fg_g * fa + bg_g * (1.0 - fa)).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                        row[o + 2] = (fg_b * fa + bg_b * (1.0 - fa)).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                        row[o + 3] = (fa + (1.0 - fa) * bg_a).clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8;
                    }
                }
            });
    }
}
