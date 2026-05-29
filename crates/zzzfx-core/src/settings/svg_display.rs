use example_effect_macros::FullSettings;

use effect_settings::{SettingDescriptor, SettingKind, Settings, TrKey};

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ZzzSvgDisplay {
    pub scale: f32,
    pub fit_to_output: bool,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub preserve_aspect_ratio: bool,
    pub dpi: f32,
}

impl Default for ZzzSvgDisplay {
    fn default() -> Self {
        Self {
            scale: 1.0,
            fit_to_output: true,
            position_x: 0.5,
            position_y: 0.5,
            rotation: 0.0,
            opacity: 1.0,
            preserve_aspect_ratio: true,
            dpi: 96.0,
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
    use super::ZzzSvgDisplayFullSettings;
    type SID = SettingID<ZzzSvgDisplayFullSettings>;

    pub const SCALE:                 SID = setting_id!("scale", scale);
    pub const FIT_TO_OUTPUT:         SID = setting_id!("fit_to_output", fit_to_output);
    pub const POSITION_X:            SID = setting_id!("position_x", position_x);
    pub const POSITION_Y:            SID = setting_id!("position_y", position_y);
    pub const ROTATION:              SID = setting_id!("rotation", rotation);
    pub const OPACITY:               SID = setting_id!("opacity", opacity);
    pub const PRESERVE_ASPECT_RATIO: SID = setting_id!("preserve_aspect_ratio", preserve_aspect_ratio);
    pub const DPI:                   SID = setting_id!("dpi", dpi);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ZzzSvgDisplayFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamSvgScale,
                description_key: Some(TrKey::ParamSvgScaleDesc),
                kind: SettingKind::FloatRange { range: 0.01..=10.0, logarithmic: false },
                id: setting_id::SCALE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgFitToOutput,
                description_key: Some(TrKey::ParamSvgFitToOutputDesc),
                kind: SettingKind::Boolean,
                id: setting_id::FIT_TO_OUTPUT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgPositionX,
                description_key: Some(TrKey::ParamSvgPositionXDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgPositionY,
                description_key: Some(TrKey::ParamSvgPositionYDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgRotation,
                description_key: Some(TrKey::ParamSvgRotationDesc),
                kind: SettingKind::FloatRange { range: 0.0..=360.0, logarithmic: false },
                id: setting_id::ROTATION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgOpacity,
                description_key: Some(TrKey::ParamSvgOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgPreserveAspectRatio,
                description_key: Some(TrKey::ParamSvgPreserveAspectRatioDesc),
                kind: SettingKind::Boolean,
                id: setting_id::PRESERVE_ASPECT_RATIO,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgDpi,
                description_key: Some(TrKey::ParamSvgDpiDesc),
                kind: SettingKind::FloatRange { range: 72.0..=600.0, logarithmic: false },
                id: setting_id::DPI,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
