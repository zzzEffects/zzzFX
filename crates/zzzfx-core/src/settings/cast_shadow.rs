use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use effect_settings::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum, TrKey};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum PivotMode {
    AutoSingle = 0,
    AutoMulti = 1,
    ManualSingle = 2,
}

impl SettingsEnum for PivotMode {}

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ZzzCastShadow {
    pub shadow_color_r: f32,
    pub shadow_color_g: f32,
    pub shadow_color_b: f32,
    pub shadow_color_a: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub pivot_angle: f32,
    pub pivot_mode: PivotMode,
    pub manual_center_x: f32,
    pub manual_center_y: f32,
    pub shear_angle: f32,
    pub shear_amount: f32,
    pub scale: f32,
    pub softness: f32,
    pub alpha_threshold: f32,
    pub source_opacity: f32,
    pub fade: f32,
}

impl Default for ZzzCastShadow {
    fn default() -> Self {
        Self {
            shadow_color_r: 0.0,
            shadow_color_g: 0.0,
            shadow_color_b: 0.0,
            shadow_color_a: 0.8,
            offset_x: 0.5,
            offset_y: 0.5,
            pivot_angle: 0.0,
            pivot_mode: PivotMode::AutoSingle,
            manual_center_x: 0.5,
            manual_center_y: 0.5,
            shear_angle: 0.0,
            shear_amount: 0.0,
            scale: 1.0,
            softness: 0.15,
            alpha_threshold: 0.01,
            source_opacity: 1.0,
            fade: 0.0,
        }
    }
}

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::ZzzCastShadowFullSettings;
    type SID = SettingID<ZzzCastShadowFullSettings>;

    pub const PIVOT_ANGLE:       SID = setting_id!("pivot_angle", pivot_angle);
    pub const PIVOT_MODE:        SID = setting_id!("pivot_mode", pivot_mode);
    pub const SHEAR_ANGLE:       SID = setting_id!("shear_angle", shear_angle);
    pub const SHEAR_AMOUNT:      SID = setting_id!("shear_amount", shear_amount);
    pub const SCALE:             SID = setting_id!("scale", scale);
    pub const SOFTNESS:          SID = setting_id!("softness", softness);
    pub const ALPHA_THRESHOLD:   SID = setting_id!("alpha_threshold", alpha_threshold);
    pub const SOURCE_OPACITY:    SID = setting_id!("source_opacity", source_opacity);
    pub const FADE:              SID = setting_id!("fade", fade);
}

impl Settings for ZzzCastShadowFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowPivotAngle,
                description_key: Some(TrKey::ParamCastShadowPivotAngleDesc),
                kind: SettingKind::FloatRange { range: 0.0..=360.0, logarithmic: false },
                id: setting_id::PIVOT_ANGLE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowPivotMode,
                description_key: Some(TrKey::ParamCastShadowPivotModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuPivotAutoSingle, description_key: Some(TrKey::MenuPivotAutoSingleDesc), index: PivotMode::AutoSingle as u32 },
                        MenuItem { label_key: TrKey::MenuPivotAutoMulti, description_key: Some(TrKey::MenuPivotAutoMultiDesc), index: PivotMode::AutoMulti as u32 },
                        MenuItem { label_key: TrKey::MenuPivotManualSingle, description_key: Some(TrKey::MenuPivotManualSingleDesc), index: PivotMode::ManualSingle as u32 },
                    ],
                },
                id: setting_id::PIVOT_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowShearAngle,
                description_key: Some(TrKey::ParamCastShadowShearAngleDesc),
                kind: SettingKind::FloatRange { range: 0.0..=360.0, logarithmic: false },
                id: setting_id::SHEAR_ANGLE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowShearAmount,
                description_key: Some(TrKey::ParamCastShadowShearAmountDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SHEAR_AMOUNT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowScale,
                description_key: Some(TrKey::ParamCastShadowScaleDesc),
                kind: SettingKind::FloatRange { range: 0.1..=3.0, logarithmic: false },
                id: setting_id::SCALE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowSoftness,
                description_key: Some(TrKey::ParamCastShadowSoftnessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SOFTNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowAlphaThreshold,
                description_key: Some(TrKey::ParamCastShadowAlphaThresholdDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::ALPHA_THRESHOLD,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowSourceOpacity,
                description_key: Some(TrKey::ParamCastShadowSourceOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SOURCE_OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamCastShadowFade,
                description_key: Some(TrKey::ParamCastShadowFadeDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::FADE,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
