use std::cell::RefCell;
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
    pub(crate) font_size: i32,
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
        font_size: 0,
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

fn build_glyph_cache(font_name: &str, font_size: i32, charset: &str) -> Option<GlyphCache> {
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

fn ensure_cache(font_name: &str, font_size: i32, charset: &str) {
    let mut guard = cache_lock().lock().unwrap_or_else(|e| e.into_inner());
    let needs_rebuild = guard.font_name != font_name || guard.font_size != font_size || guard.charset != charset;
    if needs_rebuild {
        if let Some(new_cache) = build_glyph_cache(font_name, font_size, charset) {
            *guard = Arc::new(new_cache);
        }
    }
}

// ---------------------------------------------------------------------------
// Per-thread reusable cell buffers (fixes B6 — avoids frame allocation)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CellMeta {
    char_idx: usize,
    avg_r: f32,
    avg_g: f32,
    avg_b: f32,
}

/// Cells binned by output strip for O(N) lookup (fixes B5).
struct BinnedCells {
    /// `bins[s]` contains all cells whose rows overlap strip `s`.
    bins: Vec<Vec<CellMeta>>,
}

struct RenderBufs {
    binned: BinnedCells,
}

impl Default for RenderBufs {
    fn default() -> Self {
        Self { binned: BinnedCells { bins: Vec::new() } }
    }
}

thread_local! {
    static RENDER_BUFS: RefCell<RenderBufs> = RefCell::new(RenderBufs::default());
}

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

impl ZzzAsciiArt {
    pub fn is_identity(&self) -> bool {
        false
    }

    pub fn apply_effect(&self, src: &[u8], dst: &mut [u8], width: usize, height: usize) {
        let len = width * height * 4;

        // Fix B1: early return instead of panic
        if src.len() < len || dst.len() < len || width == 0 || height == 0 {
            return;
        }

        let font_size = self.font_size.clamp(4, 64) as usize;
        let charset = self.resolve_charset();
        ensure_cache(&self.font_name, self.font_size, &charset);

        // Fix B2: clone Arc, drop lock before rendering
        let cache = get_cache_snapshot();
        let bitmaps = &cache.bitmaps;
        let cache_font_size = cache.font_size as usize;

        // ── Try GPU first ──────────────────────────────────────────
        match gpu_impl::try_gpu_render(self, src, dst, width, height, &cache) {
            Ok(true) => return,
            Ok(false) => {} // GPU unavailable → fall through to CPU
            Err(_) => {}    // GPU error → fall through to CPU
        }

        // ── CPU path (rayon parallel, pre-binned, reusable buffers) ─

        let bg_alpha = self.background_alpha.clamp(0.0, 1.0);
        let brightness = self.brightness.clamp(0.0, 1.0) as f64;
        let contrast = self.contrast.clamp(0.0, 1.0) as f64;
        let invert = self.invert_luma;
        let color_mode = self.color_mode;
        let charset_len = charset.chars().count();

        let cols = (width + font_size - 1) / font_size;
        let rows = (height + font_size - 1) / font_size;
        let strips = rows; // one strip per cell-row for simplicity
        let contrast_factor = 1.0 + (contrast - 0.5) * 2.0;

        // ── Cell analysis + binning (O1: pre-bin during analysis) ──
        RENDER_BUFS.with(|bufs_cell| {
            let bufs = &mut *bufs_cell.borrow_mut();
            let binned = &mut bufs.binned;
            binned.bins.clear();
            binned.bins.resize(strips, Vec::new());

            let row_indices: Vec<usize> = (0..rows).collect();

            // Parallel cell analysis with manual 4-pixel unrolling (O4)
            let all_bins: Vec<Vec<CellMeta>> = row_indices
                .par_iter()
                .map(|&row| {
                    let cell_y = row * font_size;
                    let cell_h = font_size.min(height - cell_y);
                    let _strip_idx = row; // one strip per row
                    let mut strip_cells = Vec::with_capacity(cols);

                    for col in 0..cols {
                        let cell_x = col * font_size;
                        let cell_w = font_size.min(width - cell_x);

                        let mut sum_luma = 0.0f64;
                        let mut sum_r = 0.0f64;
                        let mut sum_g = 0.0f64;
                        let mut sum_b = 0.0f64;
                        let mut pixel_count = 0u64;

                        // 4-pixel unrolled inner loop for better ILP
                        for dy in 0..cell_h {
                            let src_row = (cell_y + dy) * width;
                            let mut dx = 0usize;
                            let end4 = cell_w - (cell_w % 4);
                            while dx < end4 {
                                let i0 = (src_row + cell_x + dx) * 4;
                                let i1 = i0 + 4;
                                let i2 = i1 + 4;
                                let i3 = i2 + 4;
                                let r0 = src[i0] as f64; let g0 = src[i0+1] as f64; let b0 = src[i0+2] as f64;
                                let r1 = src[i1] as f64; let g1 = src[i1+1] as f64; let b1 = src[i1+2] as f64;
                                let r2 = src[i2] as f64; let g2 = src[i2+1] as f64; let b2 = src[i2+2] as f64;
                                let r3 = src[i3] as f64; let g3 = src[i3+1] as f64; let b3 = src[i3+2] as f64;
                                sum_luma += 0.299*(r0+r1+r2+r3) + 0.587*(g0+g1+g2+g3) + 0.114*(b0+b1+b2+b3);
                                sum_r += r0 + r1 + r2 + r3;
                                sum_g += g0 + g1 + g2 + g3;
                                sum_b += b0 + b1 + b2 + b3;
                                pixel_count += 4;
                                dx += 4;
                            }
                            for dx in end4..cell_w {
                                let idx = (src_row + cell_x + dx) * 4;
                                let r = src[idx] as f64;
                                let g = src[idx + 1] as f64;
                                let b = src[idx + 2] as f64;
                                sum_luma += 0.299 * r + 0.587 * g + 0.114 * b;
                                sum_r += r;
                                sum_g += g;
                                sum_b += b;
                                pixel_count += 1;
                            }
                        }

                        let np = pixel_count.max(1) as f64;
                        let avg_luma = sum_luma * RCP_255 / np;
                        let avg_r = (sum_r * RCP_255 / np) as f32;
                        let avg_g = (sum_g * RCP_255 / np) as f32;
                        let avg_b = (sum_b * RCP_255 / np) as f32;

                        let adjusted = ((avg_luma - 0.5) * contrast_factor + 0.5
                            + (brightness - 0.5))
                        .clamp(0.0, 1.0);

                        let luma = if invert { 1.0 - adjusted } else { adjusted };
                        let raw = ((luma * (charset_len - 1) as f64).round() as usize)
                            .min(charset_len - 1);
                        let char_idx = charset_len - 1 - raw;

                        strip_cells.push(CellMeta { char_idx, avg_r, avg_g, avg_b });
                    }
                    strip_cells
                })
                .collect();

            // Merge parallel results into bins
            for (i, strip_cells) in all_bins.into_iter().enumerate() {
                binned.bins[i] = strip_cells;
            }

            // ── Cell rendering (O(N) strip lookup, no filtering) ──
            let strip_height = font_size;
            let strip_bytes = width * strip_height * 4;

            dst.par_chunks_mut(strip_bytes)
                .enumerate()
                .for_each(|(strip_idx, strip)| {
                    let strip_start_row = strip_idx * strip_height;
                    let strip_end_row = (strip_start_row + strip_height).min(height);

                    let strip_cells = &binned.bins[strip_idx];

                    for (col, cell) in strip_cells.iter().enumerate() {
                        let cell_y = strip_idx * font_size;
                        let cell_x = col * font_size;
                        let cell_w = font_size.min(width - cell_x);
                        let cell_h = font_size.min(height - cell_y);

                        let bitmap = &bitmaps[cell.char_idx];
                        let bm_w = bitmap.width as usize;
                        let bm_h = bitmap.height as usize;

                        let offset_x = (cache_font_size.saturating_sub(bm_w)) / 2;
                        let offset_y = (cache_font_size.saturating_sub(bm_h)) / 2;

                        let (fg_r, fg_g, fg_b): (f32, f32, f32) = match color_mode {
                            ColorMode::Grayscale => (1.0, 1.0, 1.0),
                            ColorMode::Colored => (cell.avg_r, cell.avg_g, cell.avg_b),
                            ColorMode::GreenTerminal => (0.0, 1.0, 0.0),
                        };

                        for dy in 0..cell_h {
                            let abs_y = cell_y + dy;
                            if abs_y < strip_start_row || abs_y >= strip_end_row {
                                continue;
                            }
                            let strip_row = abs_y - strip_start_row;
                            let out_row = strip_row * width;
                            let gy_base = dy as isize - offset_y as isize;

                            for dx in 0..cell_w {
                                let out_idx = (out_row + cell_x + dx) * 4;
                                let gx = dx as isize - offset_x as isize;

                                let glyph_alpha: f32 = if gx >= 0
                                    && (gx as usize) < bm_w
                                    && gy_base >= 0
                                    && (gy_base as usize) < bm_h
                                {
                                    bitmap.data[(gy_base as usize) * bm_w + gx as usize]
                                        as f32 / 255.0
                                } else {
                                    0.0
                                };

                                let fa = glyph_alpha;
                                let out_r = (fg_r * fa).clamp(0.0, 1.0);
                                let out_g = (fg_g * fa).clamp(0.0, 1.0);
                                let out_b = (fg_b * fa).clamp(0.0, 1.0);
                                let out_a = (fa + (1.0 - fa) * bg_alpha).clamp(0.0, 1.0);

                                strip[out_idx] = (out_r * 255.0).round() as u8;
                                strip[out_idx + 1] = (out_g * 255.0).round() as u8;
                                strip[out_idx + 2] = (out_b * 255.0).round() as u8;
                                strip[out_idx + 3] = (out_a * 255.0).round() as u8;
                            }
                        }
                    }
                });
        });
    }
}
