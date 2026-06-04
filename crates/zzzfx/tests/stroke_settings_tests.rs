use zzzfx::settings::stroke::setting_id;
use zzzfx::settings::{
    EnumValue, GetSetFieldError, SettingKind, Settings, SettingsBlock, SettingsList,
};
use zzzfx::{StrokePosition, StrokeFullSettings};

#[test]
fn zzz_stroke_descriptors_count() {
    let list = SettingsList::<StrokeFullSettings>::new();
    let all: Vec<_> = list.all_descriptors().collect();
    // 11 top-level + gradient group children = 17
    assert_eq!(all.len(), 17);
}

#[test]
fn get_and_set_fields() {
    let mut settings = StrokeFullSettings::default();

    // Float field
    assert_eq!(
        settings.get_field::<f32>(&setting_id::STROKE_WIDTH).unwrap(),
        5.0
    );
    settings
        .set_field::<f32>(&setting_id::STROKE_WIDTH, 50.0)
        .unwrap();
    assert_eq!(
        settings.get_field::<f32>(&setting_id::STROKE_WIDTH).unwrap(),
        50.0
    );

    // Enum field
    assert_eq!(
        settings
            .get_field::<EnumValue>(&setting_id::STROKE_POSITION)
            .unwrap()
            .0,
        StrokePosition::Outer as u32
    );
    settings
        .set_field::<EnumValue>(
            &setting_id::STROKE_POSITION,
            EnumValue(StrokePosition::Inner as u32),
        )
        .unwrap();
    assert_eq!(
        settings
            .get_field::<EnumValue>(&setting_id::STROKE_POSITION)
            .unwrap()
            .0,
        StrokePosition::Inner as u32
    );

    // Boolean field
    assert!(!settings
        .get_field::<bool>(&setting_id::USE_SHARP_CORNERS)
        .unwrap());
    settings
        .set_field::<bool>(&setting_id::USE_SHARP_CORNERS, true)
        .unwrap();
    assert!(settings
        .get_field::<bool>(&setting_id::USE_SHARP_CORNERS)
        .unwrap());
}

#[test]
fn json_round_trip() {
    let list = SettingsList::<StrokeFullSettings>::new();

    let mut settings = StrokeFullSettings::default();
    settings.stroke_width = 50.0;
    settings.stroke_color_r = 0.8;
    settings.stroke_color_g = 0.2;
    settings.stroke_color_b = 0.4;
    settings.edge_blend = 0.5;
    settings.use_sharp_corners = true;

    let json = list.to_json_string(&settings).unwrap();
    let parsed = list.from_json_generic(&json).unwrap();

    assert_eq!(parsed.stroke_width, 50.0);
    assert_eq!(parsed.stroke_color_r, 0.8);
    assert_eq!(parsed.stroke_color_g, 0.2);
    assert_eq!(parsed.stroke_color_b, 0.4);
    assert_eq!(parsed.edge_blend, 0.5);
    assert!(parsed.use_sharp_corners);
}

#[test]
fn settings_block_round_trip() {
    // When disabled
    let block = SettingsBlock {
        enabled: false,
        settings: 42i32,
    };
    let opt: Option<i32> = Option::from(&block);
    assert!(opt.is_none());

    // When enabled
    let block = SettingsBlock {
        enabled: true,
        settings: 42i32,
    };
    let opt: Option<i32> = Option::from(&block);
    assert_eq!(opt, Some(42));
}

#[test]
fn type_mismatch_error() {
    let settings = StrokeFullSettings::default();
    let result = settings.get_field::<bool>(&setting_id::STROKE_WIDTH);
    assert!(result.is_err());
    match result {
        Err(GetSetFieldError::TypeMismatch { .. }) => {}
        _ => panic!("expected TypeMismatch"),
    }
}

#[test]
fn legacy_value_is_default() {
    let legacy = StrokeFullSettings::legacy_value();
    let default = StrokeFullSettings::default();
    assert_eq!(legacy.stroke_width, default.stroke_width);
    assert_eq!(legacy.stroke_position, default.stroke_position);
}

#[test]
fn descriptor_labels() {
    let descriptors = StrokeFullSettings::setting_descriptors();
    let labels: Vec<&str> = descriptors.iter().map(|d| d.label_key.en()).collect();
    assert!(labels.contains(&"Stroke Position"));
    assert!(labels.contains(&"Fill Mode"));
    assert!(labels.contains(&"Stroke Width"));
    assert!(labels.contains(&"Alpha Threshold"));
    assert!(labels.contains(&"Edge Blend"));
    assert!(labels.contains(&"Blend Mode"));
    assert!(labels.contains(&"Gradient Settings"));
    assert!(labels.contains(&"Use Sharp Corners"));
}

#[test]
fn descriptor_kinds() {
    let descriptors = StrokeFullSettings::setting_descriptors();

    let stroke_pos = descriptors
        .iter()
        .find(|d| d.label_key.en() == "Stroke Position")
        .unwrap();
    assert!(matches!(stroke_pos.kind, SettingKind::Enumeration { .. }));

    let sw = descriptors
        .iter()
        .find(|d| d.label_key.en() == "Stroke Width")
        .unwrap();
    assert!(matches!(sw.kind, SettingKind::FloatRange { .. }));

    let sharp = descriptors
        .iter()
        .find(|d| d.label_key.en() == "Use Sharp Corners")
        .unwrap();
    assert!(matches!(sharp.kind, SettingKind::Boolean));

    let group = descriptors
        .iter()
        .find(|d| d.label_key.en() == "Gradient Settings")
        .unwrap();
    assert!(matches!(group.kind, SettingKind::Group { .. }));
}
