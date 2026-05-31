use zzzfx_macros::FullSettings;

use super::{SettingDescriptor, SettingKind, Settings};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct AmbientLight {
    pub intensity: f32,
    pub edge_width: f32,
    pub light_wrap: f32,
    pub ambient_tint: f32,
    pub blur_radius: f32,
    pub brightness: f32,
    pub fg_opacity: f32,
    pub bg_opacity: f32,
    pub swap_fg_bg: bool,
}

impl Default for AmbientLight {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            edge_width: 0.1,
            light_wrap: 0.5,
            ambient_tint: 0.3,
            blur_radius: 40.0,
            brightness: 1.0,
            fg_opacity: 1.0,
            bg_opacity: 1.0,
            swap_fg_bg: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::setting_id;
    use crate::settings::SettingID;
    use super::AmbientLightFullSettings;
    type SID = SettingID<AmbientLightFullSettings>;

    pub const INTENSITY:     SID = setting_id!("intensity", intensity);
    pub const EDGE_WIDTH:    SID = setting_id!("edge_width", edge_width);
    pub const LIGHT_WRAP:    SID = setting_id!("light_wrap", light_wrap);
    pub const AMBIENT_TINT:  SID = setting_id!("ambient_tint", ambient_tint);
    pub const BLUR_RADIUS:   SID = setting_id!("blur_radius", blur_radius);
    pub const BRIGHTNESS:    SID = setting_id!("brightness", brightness);
    pub const FG_OPACITY:    SID = setting_id!("fg_opacity", fg_opacity);
    pub const BG_OPACITY:    SID = setting_id!("bg_opacity", bg_opacity);
    pub const SWAP_FG_BG:    SID = setting_id!("swap_fg_bg", swap_fg_bg);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for AmbientLightFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightIntensity,
                description_key: Some(TrKey::ParamAmbientLightIntensityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::INTENSITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightEdgeWidth,
                description_key: Some(TrKey::ParamAmbientLightEdgeWidthDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::EDGE_WIDTH,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightLightWrap,
                description_key: Some(TrKey::ParamAmbientLightLightWrapDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::LIGHT_WRAP,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightAmbientTint,
                description_key: Some(TrKey::ParamAmbientLightAmbientTintDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::AMBIENT_TINT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightBlurRadius,
                description_key: Some(TrKey::ParamAmbientLightBlurRadiusDesc),
                kind: SettingKind::FloatRange { range: 0.0..=200.0, logarithmic: false },
                id: setting_id::BLUR_RADIUS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightBrightness,
                description_key: Some(TrKey::ParamAmbientLightBrightnessDesc),
                kind: SettingKind::FloatRange { range: 0.0..=2.0, logarithmic: false },
                id: setting_id::BRIGHTNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightFgOpacity,
                description_key: Some(TrKey::ParamAmbientLightFgOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::FG_OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightBgOpacity,
                description_key: Some(TrKey::ParamAmbientLightBgOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::BG_OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAmbientLightSwapFgBg,
                description_key: Some(TrKey::ParamAmbientLightSwapFgBgDesc),
                kind: SettingKind::Boolean,
                id: setting_id::SWAP_FG_BG,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
