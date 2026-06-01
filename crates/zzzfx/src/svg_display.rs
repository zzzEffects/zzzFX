use std::hash::{DefaultHasher, Hash, Hasher};

use resvg::tiny_skia;
use resvg::usvg;
use usvg::fontdb;

use crate::settings::svg_display::SvgDisplay;

// ---------------------------------------------------------------------------
// Lazy fontdb — shared with latex_display via crate::get_fontdb()
// ---------------------------------------------------------------------------

fn get_fontdb() -> &'static fontdb::Database {
    crate::get_fontdb()
}

// ---------------------------------------------------------------------------
// Cached SVG state — stores the parsed vector tree (not rasterized pixels)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct CachedSvg {
    pub tree: usvg::Tree,
    pub native_w: f32,
    pub native_h: f32,
    pub dpi: f32,
    byte_hash: u64,
}

impl CachedSvg {
    fn hash_bytes(data: &[u8]) -> u64 {
        let mut h = DefaultHasher::new();
        data.len().hash(&mut h);
        if data.len() <= 128 {
            data.hash(&mut h);
        } else {
            data[..64].hash(&mut h);
            data[data.len() - 64..].hash(&mut h);
        }
        h.finish()
    }

    pub fn is_valid(&self, svg_bytes: &[u8], dpi: f32) -> bool {
        (self.dpi - dpi).abs() < f32::EPSILON && self.byte_hash == Self::hash_bytes(svg_bytes)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse SVG bytes at a given DPI. Returns None if parsing fails.
pub fn build_cache(svg_bytes: &[u8], dpi: f32) -> Option<CachedSvg> {
    let tree = parse_svg(svg_bytes, dpi)?;
    let size = tree.size();
    let w = size.width();
    let h = size.height();
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some(CachedSvg {
        tree,
        native_w: w,
        native_h: h,
        dpi,
        byte_hash: CachedSvg::hash_bytes(svg_bytes),
    })
}

/// Render the cached SVG tree to the destination RGBA8888 buffer.
///
/// Rasterizes the vector data directly at output resolution — no pre-rasterize +
/// upscale, so the result is sharp at any scale.
///
/// Returns a new `CachedSvg` on cache miss, or `None` if the existing cache was used.
pub fn render_svg(
    svg_bytes: &[u8],
    cache: Option<&CachedSvg>,
    settings: &SvgDisplay,
    pos_x: f32,
    pos_y: f32,
    dst_buf: &mut [u8],
    output_w: usize,
    output_h: usize,
    bg: [f32; 4],
) -> Option<CachedSvg> {
    let cached = if let Some(c) = cache {
        if c.is_valid(svg_bytes, settings.dpi) {
            c.clone()
        } else {
            build_cache(svg_bytes, settings.dpi)?
        }
    } else {
        build_cache(svg_bytes, settings.dpi)?
    };

    let is_new_cache = cache.map_or(true, |c| !std::ptr::eq(c, &cached));

    // Compute effective scale (fit_scale * manual_scale)
    let scale = compute_effective_scale(settings, cached.native_w, cached.native_h, output_w, output_h);

    // Build forward transform: T(pos) * R(angle) * S(sx,sy) * T(-svg_center)
    let transform = build_transform(
        cached.native_w, cached.native_h,
        scale.0, scale.1,
        settings.rotation,
        pos_x, pos_y,
        output_w, output_h,
    );

    // Rasterize SVG at output resolution → temp pixmap (transparent background)
    let mut svg_pixmap = match tiny_skia::Pixmap::new(output_w as u32, output_h as u32) {
        Some(p) => p,
        None => return if is_new_cache { Some(cached) } else { None },
    };
    resvg::render(&cached.tree, transform, &mut svg_pixmap.as_mut());

    // Composite SVG over background with opacity
    composite_svg_over_bg(
        svg_pixmap.data(),
        dst_buf,
        settings.opacity,
        bg,
        output_w,
        output_h,
    );

    if is_new_cache { Some(cached) } else { None }
}

// ---------------------------------------------------------------------------
// Compute effective scale — fit_scale * manual_scale
// ---------------------------------------------------------------------------

fn compute_effective_scale(
    settings: &SvgDisplay,
    svg_w: f32,
    svg_h: f32,
    output_w: usize,
    output_h: usize,
) -> (f32, f32) {
    let fit_sx = if settings.fit_to_output {
        let sx = output_w as f32 / svg_w;
        let sy = output_h as f32 / svg_h;
        if settings.preserve_aspect_ratio {
            let s = sx.min(sy);
            (s, s)
        } else {
            (sx, sy)
        }
    } else {
        (1.0, 1.0)
    };

    (fit_sx.0 * settings.scale, fit_sx.1 * settings.scale)
}

// ---------------------------------------------------------------------------
// Build affine transform
// ---------------------------------------------------------------------------

fn build_transform(
    svg_w: f32,
    svg_h: f32,
    sx: f32,
    sy: f32,
    angle_deg: f32,
    pos_x: f32,
    pos_y: f32,
    output_w: usize,
    output_h: usize,
) -> tiny_skia::Transform {
    let tgt_x = pos_x * output_w as f32;
    let tgt_y = pos_y * output_h as f32;

    let mut t = tiny_skia::Transform::from_translate(tgt_x, tgt_y);
    // from_rotate expects degrees, not radians
    t = t.pre_concat(tiny_skia::Transform::from_rotate(angle_deg));
    t = t.pre_concat(tiny_skia::Transform::from_scale(sx, sy));
    t = t.pre_concat(tiny_skia::Transform::from_translate(-svg_w / 2.0, -svg_h / 2.0));
    t
}

// ---------------------------------------------------------------------------
// Composite SVG pixels over background with opacity
// ---------------------------------------------------------------------------

fn composite_svg_over_bg(
    svg_pixels: &[u8],
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

    // Initialize dst with background color first, then blend SVG on top
    for chunk in dst.chunks_exact_mut(4) {
        chunk[0] = br;
        chunk[1] = bbg;
        chunk[2] = bb;
        chunk[3] = (ba_f * 255.0).round() as u8;
    }

    let n = (output_w * output_h * 4).min(svg_pixels.len());
    let overlap = &svg_pixels[..n];

    for (dst_chunk, svg_chunk) in dst[..n].chunks_exact_mut(4).zip(overlap.chunks_exact(4)) {
        let sr = svg_chunk[0] as f32 / 255.0;
        let sg = svg_chunk[1] as f32 / 255.0;
        let sb = svg_chunk[2] as f32 / 255.0;
        let sa = (svg_chunk[3] as f32 / 255.0) * opacity;

        if sa <= 0.0 {
            continue;
        }

        let out_a = sa + ba_f * (1.0 - sa);
        let inv_a = 1.0 / out_a;
        dst_chunk[0] = ((sr * sa + br as f32 / 255.0 * ba_f * (1.0 - sa)) * inv_a * 255.0).round() as u8;
        dst_chunk[1] = ((sg * sa + bbg as f32 / 255.0 * ba_f * (1.0 - sa)) * inv_a * 255.0).round() as u8;
        dst_chunk[2] = ((sb * sa + bb as f32 / 255.0 * ba_f * (1.0 - sa)) * inv_a * 255.0).round() as u8;
        dst_chunk[3] = (out_a * 255.0).round() as u8;
    }
}

// ---------------------------------------------------------------------------
// Parse SVG
// ---------------------------------------------------------------------------

fn parse_svg(svg_bytes: &[u8], dpi: f32) -> Option<usvg::Tree> {
    let mut opt = usvg::Options::default();
    opt.fontdb = get_fontdb().clone().into();
    opt.dpi = dpi;
    usvg::Tree::from_data(svg_bytes, &opt).ok()
}
