use resvg::tiny_skia::{self, Pixmap};
use resvg::usvg;

use fast_qr::convert::svg::SvgBuilder;
use fast_qr::convert::{Builder, Shape};
use fast_qr::qr::QRBuilder;

use crate::settings::qr_code::{Ecl, ModuleShape, QrCode};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a QR code to the destination RGBA8888 buffer.
///
/// Returns `true` if the QR code was encoded and rendered successfully,
/// `false` if the content could not be encoded (e.g. too large).
pub fn render_qr(
    content: &str,
    settings: &QrCode,
    module_color: [f32; 4],
    light_module_color: [f32; 4],
    bg_color: [f32; 4],
    pos_x: f32,
    pos_y: f32,
    dst_buf: &mut [u8],
    output_w: usize,
    output_h: usize,
) -> bool {
    // --- Build QR code ---
    let ecl = match settings.ecl {
        Ecl::L => fast_qr::ECL::L,
        Ecl::M => fast_qr::ECL::M,
        Ecl::Q => fast_qr::ECL::Q,
        Ecl::H => fast_qr::ECL::H,
    };

    let shape = match settings.module_shape {
        ModuleShape::Square => Shape::Square,
        ModuleShape::Circle => Shape::Circle,
        ModuleShape::RoundedSquare => Shape::RoundedSquare,
        ModuleShape::Vertical => Shape::Vertical,
        ModuleShape::Horizontal => Shape::Horizontal,
        ModuleShape::Diamond => Shape::Diamond,
    };

    let qr = match QRBuilder::new(content).ecl(ecl).build() {
        Ok(qr) => qr,
        Err(_) => return false,
    };

    // --- Build SVG string (no margin — we draw it ourselves) ---
    let mc = color_to_u8_4(module_color);
    let lmc = color_to_u8_4(light_module_color);
    let qr_size = qr.size as f32;

    let svg_str = SvgBuilder::default()
        .module_color(mc)
        .background_color(lmc)
        .shape(shape)
        .margin(0)
        .to_str(&qr);

    // --- Parse SVG ---
    let opt = usvg::Options::default();
    let tree = match usvg::Tree::from_str(&svg_str, &opt) {
        Ok(t) => t,
        Err(_) => return false,
    };

    let svg_size = tree.size();
    let svg_w = svg_size.width();
    let svg_h = svg_size.height();
    if svg_w <= 0.0 || svg_h <= 0.0 {
        return false;
    }

    // --- Compute scale from total area including smooth margin ---
    let total_svg_size = qr_size + 2.0 * settings.margin;
    let fit_scale = (output_w as f32 / total_svg_size).min(output_h as f32 / total_svg_size);
    let sx = fit_scale * settings.scale;
    let sy = fit_scale * settings.scale;

    // --- Transforms: bg_rect fills margin area, qr_transform places the QR code centered within it ---
    let bg_transform = build_transform(total_svg_size, total_svg_size, sx, sy, settings.rotation, pos_x, pos_y, output_w, output_h);
    let qr_transform = build_transform(svg_w, svg_h, sx, sy, settings.rotation, pos_x, pos_y, output_w, output_h);

    // --- Rasterize: first fill light_module_color rect (smooth margin), then QR code on top ---
    let mut svg_pixmap = match Pixmap::new(output_w as u32, output_h as u32) {
        Some(p) => p,
        None => return false,
    };

    // Pass 1: background rect with smooth margin
    {
        let mut pb = tiny_skia::PathBuilder::new();
        pb.push_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, total_svg_size, total_svg_size).unwrap());
        if let Some(path) = pb.finish() {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(lmc[0], lmc[1], lmc[2], lmc[3]);
            paint.anti_alias = true;
            svg_pixmap.as_mut().fill_path(&path, &paint, tiny_skia::FillRule::Winding, bg_transform, None);
        }
    }

    // Pass 2: QR code modules on top
    resvg::render(&tree, qr_transform, &mut svg_pixmap.as_mut());

    // --- Composite over background ---
    composite_qr_over_bg(
        svg_pixmap.data(),
        dst_buf,
        settings.opacity,
        bg_color,
        output_w,
        output_h,
    );

    true
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn color_to_u8_4(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

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
    t = t.pre_concat(tiny_skia::Transform::from_rotate(angle_deg));
    t = t.pre_concat(tiny_skia::Transform::from_scale(sx, sy));
    t = t.pre_concat(tiny_skia::Transform::from_translate(-svg_w / 2.0, -svg_h / 2.0));
    t
}

fn composite_qr_over_bg(
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

    // Initialize dst with background color first
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
