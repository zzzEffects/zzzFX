use example_effect::settings::standard::{setting_id, ExampleEffectFullSettings};
use example_effect::settings::{
    EnumValue, GetSetFieldError, SettingKind, Settings, SettingsBlock, SettingsList,
};

#[test]
fn settings_list_has_all_descriptors() {
    let list = SettingsList::<ExampleEffectFullSettings>::new();
    // 8 top-level descriptors: brightness, invert_colors, tint_r, tint_g, tint_b,
    // advanced (group), color_preset — plus 2 children inside "advanced"
    let all: Vec<_> = list.all_descriptors().collect();
    // 9 descriptors: brightness, invert_colors, tint_r, tint_g, tint_b,
    // advanced (group), contrast (child), saturation (child), color_preset
    assert_eq!(all.len(), 9);
}

#[test]
fn get_and_set_fields() {
    let mut settings = ExampleEffectFullSettings::default();

    // Float field
    assert_eq!(settings.get_field::<f32>(&setting_id::BRIGHTNESS).unwrap(), 1.0);
    settings.set_field::<f32>(&setting_id::BRIGHTNESS, 0.5).unwrap();
    assert_eq!(settings.get_field::<f32>(&setting_id::BRIGHTNESS).unwrap(), 0.5);

    // Boolean field
    assert!(!settings.get_field::<bool>(&setting_id::INVERT_COLORS).unwrap());
    settings.set_field::<bool>(&setting_id::INVERT_COLORS, true).unwrap();
    assert!(settings.get_field::<bool>(&setting_id::INVERT_COLORS).unwrap());

    // Enum field
    assert_eq!(
        settings.get_field::<EnumValue>(&setting_id::COLOR_PRESET).unwrap().0,
        0 // None
    );
    settings.set_field::<EnumValue>(&setting_id::COLOR_PRESET, EnumValue(1)).unwrap(); // Warm
    assert_eq!(
        settings.get_field::<EnumValue>(&setting_id::COLOR_PRESET).unwrap().0,
        1
    );
}

#[test]
fn json_round_trip() {
    let list = SettingsList::<ExampleEffectFullSettings>::new();

    let mut settings = ExampleEffectFullSettings::default();
    settings.brightness = 0.75;
    settings.invert_colors = true;
    settings.color_preset =
        example_effect::settings::standard::ColorPreset::Warm;

    let json = list.to_json_string(&settings).unwrap();
    let parsed = list.from_json_generic(&json).unwrap();

    assert_eq!(parsed.brightness, 0.75);
    assert!(parsed.invert_colors);
    assert_eq!(parsed.color_preset, example_effect::settings::standard::ColorPreset::Warm);
}

#[test]
fn settings_block_round_trip() {
    // When settings block is disabled
    let block = SettingsBlock {
        enabled: false,
        settings: 42i32,
    };
    let opt: Option<i32> = Option::from(&block);
    assert!(opt.is_none());

    // When settings block is enabled
    let block = SettingsBlock {
        enabled: true,
        settings: 42i32,
    };
    let opt: Option<i32> = Option::from(&block);
    assert_eq!(opt, Some(42));
}

#[test]
fn type_mismatch_error() {
    let settings = ExampleEffectFullSettings::default();
    // Try to get a float field as a bool
    let result = settings.get_field::<bool>(&setting_id::BRIGHTNESS);
    assert!(result.is_err());
    match result {
        Err(GetSetFieldError::TypeMismatch { .. }) => {}
        _ => panic!("expected TypeMismatch"),
    }
}

#[test]
fn legacy_value_is_default() {
    let legacy = ExampleEffectFullSettings::legacy_value();
    let default = ExampleEffectFullSettings::default();
    assert_eq!(legacy.brightness, default.brightness);
}

#[test]
fn descriptor_labels() {
    let descriptors = ExampleEffectFullSettings::setting_descriptors();
    let labels: Vec<&str> = descriptors.iter().map(|d| d.label_key.en()).collect();
    assert!(labels.contains(&"Brightness"));
    assert!(labels.contains(&"Invert Colors"));
    assert!(labels.contains(&"Color Preset"));
}

#[test]
fn descriptor_kinds() {
    let descriptors = ExampleEffectFullSettings::setting_descriptors();
    let brightness = descriptors.iter().find(|d| d.label_key.en() == "Brightness").unwrap();
    assert!(matches!(brightness.kind, SettingKind::FloatRange { .. }));

    let invert = descriptors.iter().find(|d| d.label_key.en() == "Invert Colors").unwrap();
    assert!(matches!(invert.kind, SettingKind::Boolean));

    let preset = descriptors.iter().find(|d| d.label_key.en() == "Color Preset").unwrap();
    assert!(matches!(preset.kind, SettingKind::Enumeration { .. }));
}
