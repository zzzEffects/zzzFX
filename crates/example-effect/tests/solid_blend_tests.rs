use example_effect::{
    SolidColorBlend, SolidColorBlendFullSettings,
    settings::Settings,
    settings::solid::BlendMode,
};

// ---------------------------------------------------------------------------
// Normal blend (should match old behavior with color_a = blend_amount)
// ---------------------------------------------------------------------------

#[test]
fn zero_alpha_is_passthrough() {
    let effect = SolidColorBlend {
        color_a: 0.0,
        color_r: 0.8,
        color_g: 0.2,
        color_b: 0.5,
        ..Default::default()
    };

    let src: Vec<u8> = (0..16).map(|i| i as u8).collect();
    let mut dst = vec![0u8; src.len()];
    effect.apply_effect(&src, &mut dst, 2, 2);
    assert_eq!(src, dst, "zero alpha should be identity");
}

#[test]
fn full_alpha_is_solid_color_normal() {
    let effect = SolidColorBlend {
        blend_mode: BlendMode::Normal,
        color_a: 1.0,
        color_r: 1.0,
        color_g: 0.0,
        color_b: 0.5,
    };

    let width = 2;
    let height = 2;
    let len = width * height * 4;
    let src = vec![50u8; len];
    let mut dst = vec![0u8; len];
    effect.apply_effect(&src, &mut dst, width, height);

    for i in (0..len).step_by(4) {
        assert_eq!(dst[i],     255, "red at pixel {}", i / 4);
        assert_eq!(dst[i + 1], 0,   "green at pixel {}", i / 4);
        assert_eq!(dst[i + 2], 128, "blue at pixel {}", i / 4);
        assert_eq!(dst[i + 3], src[i + 3], "alpha preserved");
    }
}

#[test]
fn alpha_is_preserved() {
    let effect = SolidColorBlend {
        color_a: 0.3,
        color_r: 0.5,
        color_g: 0.5,
        color_b: 0.5,
        ..Default::default()
    };
    let src: Vec<u8> = vec![0, 0, 0, 77];
    let mut dst = vec![0u8; 4];
    effect.apply_effect(&src, &mut dst, 1, 1);
    assert_eq!(dst[3], 77, "alpha must be preserved unchanged");
}

// ---------------------------------------------------------------------------
// Blend mode tests
// ---------------------------------------------------------------------------

#[test]
fn multiply_darkens() {
    // Multiply with red=0.5, green=0.5, blue=0.5, alpha=1.0 on white (255,255,255)
    // Should produce gray (127-128 range)
    let effect = SolidColorBlend {
        blend_mode: BlendMode::Multiply,
        color_a: 1.0,
        color_r: 0.5,
        color_g: 0.5,
        color_b: 0.5,
    };
    let src = vec![255u8, 255, 255, 255];
    let mut dst = vec![0u8; 4];
    effect.apply_effect(&src, &mut dst, 1, 1);

    // White * 0.5 = 127.5 → 128
    assert_eq!(dst[0], 128);
    assert_eq!(dst[1], 128);
    assert_eq!(dst[2], 128);
    assert_eq!(dst[3], 255);
}

#[test]
fn screen_lightens() {
    // Screen with white (1.0) at full alpha on black (0,0,0)
    // Should produce white
    let effect = SolidColorBlend {
        blend_mode: BlendMode::Screen,
        color_a: 1.0,
        color_r: 1.0,
        color_g: 1.0,
        color_b: 1.0,
    };
    let src = vec![0u8, 0, 0, 255];
    let mut dst = vec![0u8; 4];
    effect.apply_effect(&src, &mut dst, 1, 1);
    assert_eq!(dst[0], 255);
    assert_eq!(dst[1], 255);
    assert_eq!(dst[2], 255);
}

#[test]
fn overlay_on_gray_is_unchanged_color() {
    // Overlay with gray (0.5) on gray (128) → should stay around 128
    let effect = SolidColorBlend {
        blend_mode: BlendMode::Overlay,
        color_a: 1.0,
        color_r: 0.5,
        color_g: 0.5,
        color_b: 0.5,
    };
    let src = vec![128u8, 128, 128, 255];
    let mut dst = vec![0u8; 4];
    effect.apply_effect(&src, &mut dst, 1, 1);

    // Overlay on 0.5 base with 0.5 blend: 2 * 0.5 * 0.5 = 0.5 → 128
    assert!((dst[0] as i32 - 128).abs() <= 1, "should be ~128, got {}", dst[0]);
    assert!((dst[1] as i32 - 128).abs() <= 1, "should be ~128, got {}", dst[1]);
    assert!((dst[2] as i32 - 128).abs() <= 1, "should be ~128, got {}", dst[2]);
}

#[test]
fn different_dimensions_work() {
    for &mode in &[BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen, BlendMode::Overlay] {
        let effect = SolidColorBlend {
            blend_mode: mode,
            color_a: 0.25,
            color_r: 1.0,
            color_g: 1.0,
            color_b: 1.0,
        };
        for (w, h) in [(1, 1), (3, 2)] {
            let len = w * h * 4;
            let src: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let mut dst = vec![0u8; len];
            effect.apply_effect(&src, &mut dst, w, h);
            // Alpha should be untouched
            for p in 0..len / 4 {
                assert_eq!(dst[p * 4 + 3], src[p * 4 + 3], "alpha for mode {mode:?} at pixel {p}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Settings tests
// ---------------------------------------------------------------------------

#[test]
fn solid_blend_descriptors_count() {
    let list = example_effect::settings::SettingsList::<SolidColorBlendFullSettings>::new();
    let all: Vec<_> = list.all_descriptors().collect();
    assert_eq!(all.len(), 5, "should have 5 descriptors: color_r, color_g, color_b, color_a, blend_mode");
}

#[test]
fn solid_blend_default_is_passthrough() {
    let default = SolidColorBlend::default();
    assert_eq!(default.color_a, 0.0);
    assert_eq!(default.blend_mode, BlendMode::Normal);
    let src: Vec<u8> = (0..16).map(|i| i as u8).collect();
    let mut dst = vec![0u8; 16];
    default.apply_effect(&src, &mut dst, 2, 2);
    assert_eq!(src, dst);
}

#[test]
fn solid_blend_json_round_trip() {
    use example_effect::settings::SettingsList;
    use example_effect::settings::solid::setting_id;

    let list = SettingsList::<SolidColorBlendFullSettings>::new();

    let mut settings = SolidColorBlendFullSettings::default();
    settings.set_field::<f32>(&setting_id::COLOR_R, 0.5).unwrap();
    settings.set_field::<f32>(&setting_id::COLOR_G, 0.2).unwrap();
    settings.set_field::<f32>(&setting_id::COLOR_B, 0.9).unwrap();
    settings.set_field::<f32>(&setting_id::COLOR_A, 0.75).unwrap();
    settings.set_field::<example_effect::settings::EnumValue>(
        &setting_id::BLEND_MODE,
        example_effect::settings::EnumValue(BlendMode::Screen as u32),
    ).unwrap();

    let json = list.to_json_string(&settings).unwrap();
    let parsed = list.from_json_generic(&json).unwrap();

    assert_eq!(parsed.get_field::<f32>(&setting_id::COLOR_R).unwrap(), 0.5);
    assert_eq!(parsed.get_field::<f32>(&setting_id::COLOR_G).unwrap(), 0.2);
    assert_eq!(parsed.get_field::<f32>(&setting_id::COLOR_B).unwrap(), 0.9);
    assert_eq!(parsed.get_field::<f32>(&setting_id::COLOR_A).unwrap(), 0.75);
    assert_eq!(
        parsed.get_field::<example_effect::settings::EnumValue>(&setting_id::BLEND_MODE).unwrap().0,
        BlendMode::Screen as u32
    );
}
