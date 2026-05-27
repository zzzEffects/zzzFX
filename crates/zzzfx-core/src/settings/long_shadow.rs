use example_effect_macros::FullSettings;

use effect_settings::{SettingDescriptor, SettingKind, Settings, TrKey};

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ZzzLongShadow {
    pub shadow_color_r: f32,
    pub shadow_color_g: f32,
    pub shadow_color_b: f32,
    pub shadow_color_a: f32,
    pub angle: f32,
    pub length: f32,
    pub softness: f32,
    pub fade: f32,
    pub opacity: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub alpha_threshold: f32,
    pub source_opacity: f32,
}

impl Default for ZzzLongShadow {
    fn default() -> Self {
        Self {
            shadow_color_r: 0.0,
            shadow_color_g: 0.0,
            shadow_color_b: 0.0,
            shadow_color_a: 0.8,
            angle: 45.0,
            length: 0.3,
            softness: 0.0,
            fade: 0.0,
            opacity: 1.0,
            offset_x: 0.5,
            offset_y: 0.5,
            alpha_threshold: 0.01,
            source_opacity: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::ZzzLongShadowFullSettings;
    type SID = SettingID<ZzzLongShadowFullSettings>;

    pub const ANGLE:            SID = setting_id!("angle", angle);
    pub const LENGTH:           SID = setting_id!("length", length);
    pub const SOFTNESS:         SID = setting_id!("softness", softness);
    pub const FADE:             SID = setting_id!("fade", fade);
    pub const OPACITY:          SID = setting_id!("opacity", opacity);
    pub const ALPHA_THRESHOLD:  SID = setting_id!("alpha_threshold", alpha_threshold);
    pub const SOURCE_OPACITY:   SID = setting_id!("source_opacity", source_opacity);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ZzzLongShadowFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamShadowAngle,
                description_key: Some(TrKey::ParamShadowAngleDesc),
                kind: SettingKind::FloatRange { range: 0.0..=360.0, logarithmic: false },
                id: setting_id::ANGLE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamShadowLength,
                description_key: Some(TrKey::ParamShadowLengthDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::LENGTH,
            },
            SettingDescriptor {
                label_key: TrKey::ParamShadowSoftness,
                description_key: Some(TrKey::ParamShadowSoftnessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SOFTNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamShadowFade,
                description_key: Some(TrKey::ParamShadowFadeDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::FADE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamShadowOpacity,
                description_key: Some(TrKey::ParamShadowOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamShadowAlphaThreshold,
                description_key: Some(TrKey::ParamShadowAlphaThresholdDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::ALPHA_THRESHOLD,
            },
            SettingDescriptor {
                label_key: TrKey::ParamShadowSourceOpacity,
                description_key: Some(TrKey::ParamShadowSourceOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SOURCE_OPACITY,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
