use example_effect::ExampleEffect;

#[test]
fn passthrough_preserves_pixels() {
    let effect = ExampleEffect::default();

    let width = 4;
    let height = 2;
    let len = width * height * 4;

    // Create source buffer with known pattern
    let src: Vec<u8> = (0..len).map(|i| i as u8).collect();
    let mut dst = vec![0u8; len];

    effect.apply_effect(&src, &mut dst, width, height);

    assert_eq!(src, dst, "passthrough should preserve all pixels");
}

#[test]
fn passthrough_with_parameters_does_nothing() {
    let mut effect = ExampleEffect::default();
    effect.brightness = 0.5;
    effect.invert_colors = true;
    effect.color_preset = example_effect::settings::standard::ColorPreset::Sepia;
    effect.advanced = Some(example_effect::settings::standard::AdvancedSettings {
        contrast: 2.0,
        saturation: 0.5,
    });

    let width = 2;
    let height = 2;
    let len = width * height * 4;

    let src: Vec<u8> = (0..len).map(|i| (i * 17) as u8).collect();
    let mut dst = vec![0u8; len];

    effect.apply_effect(&src, &mut dst, width, height);

    // Even with non-default parameters, the passthrough should not modify pixels
    assert_eq!(src, dst, "passthrough should not modify pixels regardless of parameters");
}

#[test]
fn different_dimensions() {
    let effect = ExampleEffect::default();

    for (w, h) in [(1, 1), (16, 9), (64, 64), (1920, 1080)] {
        let len = w * h * 4;
        let src: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let mut dst = vec![0u8; len];
        effect.apply_effect(&src, &mut dst, w, h);
        assert_eq!(src, dst, "passthrough failed for {w}x{h}");
    }
}
