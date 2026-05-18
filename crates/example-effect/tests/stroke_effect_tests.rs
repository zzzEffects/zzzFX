use example_effect::{FillMode, StrokePosition, ZzzStroke};

fn make_square_with_alpha(width: usize, height: usize) -> Vec<u8> {
    let len = width * height * 4;
    let mut buf = vec![0u8; len];
    let cx = width / 2;
    let cy = height / 2;
    let r = (width.min(height) / 4) as i32;
    for y in 0..height {
        for x in 0..width {
            let dx = x as i32 - cx as i32;
            let dy = y as i32 - cy as i32;
            let inside = dx * dx + dy * dy <= r * r;
            let idx = (y * width + x) * 4;
            buf[idx] = 255;
            buf[idx + 1] = 255;
            buf[idx + 2] = 255;
            buf[idx + 3] = if inside { 255 } else { 0 };
        }
    }
    buf
}

#[test]
fn zero_width_is_passthrough() {
    let effect = ZzzStroke {
        stroke_width: 0.0,
        ..Default::default()
    };
    let w = 16;
    let h = 16;
    let src = make_square_with_alpha(w, h);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, w, h);
    assert_eq!(src, dst, "zero stroke width should be passthrough");
}

#[test]
fn zero_alpha_stroke_is_passthrough() {
    let effect = ZzzStroke {
        stroke_width: 0.1,
        stroke_color_a: 0.0,
        ..Default::default()
    };
    let w = 16;
    let h = 16;
    let src = make_square_with_alpha(w, h);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, w, h);
    assert_eq!(src, dst, "zero stroke alpha should be passthrough");
}

#[test]
fn outer_stroke_expands() {
    let effect = ZzzStroke {
        stroke_position: StrokePosition::Outer,
        stroke_width: 0.5,
        stroke_color_r: 1.0,
        stroke_color_g: 0.0,
        stroke_color_b: 0.0,
        stroke_color_a: 1.0,
        stroke_feathering: 0.0,
        ..Default::default()
    };
    let w = 32;
    let h = 32;
    let src = make_square_with_alpha(w, h);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, w, h);

    // Check that stroke was applied: some pixels should be modified outside the shape.
    // GPU (JFA Euclidean) and CPU (4SSED) may produce slightly different stroke boundaries,
    // so check for any red-tinted pixels rather than exact R-only values.
    let mut stroke_pixels = 0;
    for i in (0..dst.len()).step_by(4) {
        let is_stroke = dst[i] > 200 && dst[i + 1] < dst[i] && dst[i + 2] < dst[i];
        if is_stroke {
            stroke_pixels += 1;
        }
    }
    assert!(
        stroke_pixels > 0,
        "outer stroke should produce red-tinted pixels outside the shape"
    );
}

#[test]
fn inner_stroke_is_inside() {
    let effect = ZzzStroke {
        stroke_position: StrokePosition::Inner,
        stroke_width: 0.15,
        stroke_color_r: 1.0,
        stroke_color_g: 0.0,
        stroke_color_b: 0.0,
        stroke_color_a: 1.0,
        stroke_feathering: 0.0,
        ..Default::default()
    };
    let w = 32;
    let h = 32;
    let src = make_square_with_alpha(w, h);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, w, h);

    // The center pixel should still be white (inner stroke doesn't reach center)
    let cx = w / 2;
    let cy = h / 2;
    let center_idx = (cy * w + cx) * 4;
    assert_eq!(dst[center_idx], 255);
    assert_eq!(dst[center_idx + 1], 255);
    assert_eq!(dst[center_idx + 2], 255);
}

#[test]
fn source_opacity_reduces_output_alpha() {
    let effect = ZzzStroke {
        source_opacity: 0.5,
        stroke_width: 0.0, // no stroke, only source opacity
        ..Default::default()
    };
    let w = 8;
    let h = 8;
    let src = make_square_with_alpha(w, h);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, w, h);

    for i in (0..dst.len()).step_by(4) {
        let expected = (src[i + 3] as f32 * 0.5).round() as u8;
        assert_eq!(dst[i + 3], expected);
    }
}

#[test]
fn different_dimensions_work() {
    for (w, h) in [(1, 1), (4, 4), (16, 9), (32, 8)] {
        let len = w * h * 4;
        let src: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0u8; len];
        let effect = ZzzStroke {
            stroke_width: 0.05,
            stroke_color_a: 0.5,
            ..Default::default()
        };
        effect.apply_effect(&src, &mut dst, w, h);
    }
}

fn make_rect_with_alpha(width: usize, height: usize) -> Vec<u8> {
    let len = width * height * 4;
    let mut buf = vec![0u8; len];
    let margin = width / 4;
    for y in 0..height {
        for x in 0..width {
            let inside = x >= margin && x < width - margin && y >= margin && y < height - margin;
            let idx = (y * width + x) * 4;
            buf[idx] = 255;
            buf[idx + 1] = 255;
            buf[idx + 2] = 255;
            buf[idx + 3] = if inside { 255 } else { 0 };
        }
    }
    buf
}

#[test]
fn sharp_corners_vs_rounded_produce_different_output() {
    let w = 64;
    let h = 64;
    let src = make_rect_with_alpha(w, h);

    // Use a wider stroke to make corner differences more visible
    // (both GPU Euclidean JFA and CPU 4SSED)
    let effect_rounded = ZzzStroke {
        stroke_position: StrokePosition::Outer,
        stroke_width: 1.0,
        stroke_color_r: 1.0,
        stroke_color_g: 0.0,
        stroke_color_b: 0.0,
        stroke_color_a: 1.0,
        stroke_feathering: 0.0,
        use_sharp_corners: false,
        ..Default::default()
    };
    let effect_sharp = ZzzStroke {
        use_sharp_corners: true,
        ..effect_rounded.clone()
    };

    let mut dst_rounded = vec![0u8; src.len()];
    let mut dst_sharp = vec![0u8; src.len()];
    effect_rounded.apply_effect(&src, &mut dst_rounded, w, h);
    effect_sharp.apply_effect(&src, &mut dst_sharp, w, h);

    // The two should differ when sharp corners are enabled
    let diff = dst_rounded
        .iter()
        .zip(dst_sharp.iter())
        .any(|(a, b)| a != b);
    assert!(diff, "sharp vs rounded should produce different outputs");
}

#[test]
fn solid_color_fill_is_uniform() {
    let effect = ZzzStroke {
        stroke_position: StrokePosition::Outer,
        fill_mode: FillMode::SolidColor,
        stroke_width: 0.1,
        stroke_color_r: 0.0,
        stroke_color_g: 1.0,
        stroke_color_b: 0.5,
        stroke_color_a: 1.0,
        stroke_feathering: 0.0,
        ..Default::default()
    };
    let w = 32;
    let h = 32;
    let src = make_square_with_alpha(w, h);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, w, h);

    // All stroke pixels should have the same color
    for i in (0..dst.len()).step_by(4) {
        if dst[i + 3] > 0 && src[i + 3] == 0 {
            assert_eq!(dst[i], 0);
            assert_eq!(dst[i + 1], 255);
            assert_eq!(dst[i + 2], 128);
        }
    }
}

#[test]
fn default_settings_produce_output() {
    let effect = ZzzStroke::default();
    let w = 16;
    let h = 16;
    let src = make_square_with_alpha(w, h);
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, w, h);
    // Should not panic and produce some output (may be mostly passthrough with tiny stroke)
}
