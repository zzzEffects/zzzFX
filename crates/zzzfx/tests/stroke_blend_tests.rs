use zzzfx::blend::{blend_channel, luminance};
use zzzfx::settings::stroke::BlendMode as StrokeBlendMode;

#[test]
fn normal_returns_blend_color() {
    assert_eq!(blend_channel(StrokeBlendMode::Normal, 0.3, 0.8, 1.0, 0.5), 0.8);
}

#[test]
fn dissolve_with_low_rng_returns_blend() {
    assert_eq!(
        blend_channel(StrokeBlendMode::Dissolve, 0.3, 0.8, 0.5, 0.1),
        0.8
    );
}

#[test]
fn dissolve_with_high_rng_returns_base() {
    assert_eq!(
        blend_channel(StrokeBlendMode::Dissolve, 0.3, 0.8, 0.5, 0.9),
        0.3
    );
}

#[test]
fn darken_is_min() {
    assert_eq!(blend_channel(StrokeBlendMode::Darken, 0.3, 0.8, 1.0, 0.0), 0.3);
    assert_eq!(blend_channel(StrokeBlendMode::Darken, 0.8, 0.3, 1.0, 0.0), 0.3);
}

#[test]
fn multiply() {
    assert!((blend_channel(StrokeBlendMode::Multiply, 0.5, 0.5, 1.0, 0.0) - 0.25).abs() < 1e-6);
}

#[test]
fn screen() {
    let result = blend_channel(StrokeBlendMode::Screen, 0.0, 1.0, 1.0, 0.0);
    assert!((result - 1.0).abs() < 1e-6);
}

#[test]
fn overlay() {
    // Dark base
    let result = blend_channel(StrokeBlendMode::Overlay, 0.25, 0.5, 1.0, 0.0);
    assert!((result - 0.25).abs() < 1e-6); // 2*0.25*0.5 = 0.25

    // Light base
    let result = blend_channel(StrokeBlendMode::Overlay, 0.75, 0.5, 1.0, 0.0);
    assert!((result - 0.75).abs() < 1e-6); // 1-2*0.25*0.5 = 0.75
}

#[test]
fn difference() {
    assert!((blend_channel(StrokeBlendMode::Difference, 0.8, 0.3, 1.0, 0.0) - 0.5).abs() < 1e-6);
}

#[test]
fn add_clamps() {
    assert_eq!(blend_channel(StrokeBlendMode::Add, 0.6, 0.6, 1.0, 0.0), 1.0);
}

#[test]
fn subtract() {
    assert!((blend_channel(StrokeBlendMode::Subtract, 0.8, 0.3, 1.0, 0.0) - 0.5).abs() < 1e-6);
}

#[test]
fn divide() {
    assert!((blend_channel(StrokeBlendMode::Divide, 0.5, 0.5, 1.0, 0.0) - 1.0).abs() < 1e-6);
}

#[test]
fn color_burn() {
    let result = blend_channel(StrokeBlendMode::ColorBurn, 0.5, 0.5, 1.0, 0.0);
    assert!(!result.is_nan());
    assert!(result >= 0.0);
    assert!(result <= 1.0);
}

#[test]
fn color_dodge() {
    let result = blend_channel(StrokeBlendMode::ColorDodge, 0.5, 0.5, 1.0, 0.0);
    assert!(!result.is_nan());
    assert!(result >= 0.0);
    assert!(result <= 1.0);
}

#[test]
fn luminance_values() {
    // White
    assert!((luminance(1.0, 1.0, 1.0) - 1.0).abs() < 1e-4);
    // Black
    assert!(luminance(0.0, 0.0, 0.0).abs() < 1e-4);
    // Green is brightest
    let lg = luminance(0.0, 1.0, 0.0);
    assert!((lg - 0.7152).abs() < 1e-4);
}
