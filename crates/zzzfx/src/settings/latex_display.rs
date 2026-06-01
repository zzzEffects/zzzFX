use zzzfx_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// Math Style enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum MathStyle {
    Display = 0,
    Inline = 1,
}

impl SettingsEnum for MathStyle {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct LaTeXDisplay {
    pub font_size: f32,
    pub scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub dpi: f32,
    pub math_style: MathStyle,
}

impl Default for LaTeXDisplay {
    fn default() -> Self {
        Self {
            font_size: 8.0,
            scale: 1.0,
            position_x: 0.5,
            position_y: 0.5,
            rotation: 0.0,
            opacity: 1.0,
            dpi: 96.0,
            math_style: MathStyle::Display,
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
    use super::LaTeXDisplayFullSettings;
    type SID = SettingID<LaTeXDisplayFullSettings>;

    pub const FONT_SIZE:   SID = setting_id!("font_size", font_size);
    pub const SCALE:       SID = setting_id!("scale", scale);
    pub const POSITION_X:  SID = setting_id!("position_x", position_x);
    pub const POSITION_Y:  SID = setting_id!("position_y", position_y);
    pub const ROTATION:    SID = setting_id!("rotation", rotation);
    pub const OPACITY:     SID = setting_id!("opacity", opacity);
    pub const DPI:         SID = setting_id!("dpi", dpi);
    pub const MATH_STYLE:  SID = setting_id!("math_style", math_style);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for LaTeXDisplayFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXFontSize,
                description_key: Some(TrKey::ParamLaTeXFontSizeDesc),
                kind: SettingKind::FloatRange { range: 1.0..=512.0, logarithmic: false },
                id: setting_id::FONT_SIZE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXScale,
                description_key: Some(TrKey::ParamLaTeXScaleDesc),
                kind: SettingKind::FloatRange { range: 0.01..=10.0, logarithmic: false },
                id: setting_id::SCALE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXPositionX,
                description_key: Some(TrKey::ParamLaTeXPositionXDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXPositionY,
                description_key: Some(TrKey::ParamLaTeXPositionYDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXRotation,
                description_key: Some(TrKey::ParamLaTeXRotationDesc),
                kind: SettingKind::FloatRange { range: 0.0..=360.0, logarithmic: false },
                id: setting_id::ROTATION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXOpacity,
                description_key: Some(TrKey::ParamLaTeXOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXDpi,
                description_key: Some(TrKey::ParamLaTeXDpiDesc),
                kind: SettingKind::FloatRange { range: 72.0..=600.0, logarithmic: false },
                id: setting_id::DPI,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXMathStyle,
                description_key: Some(TrKey::ParamLaTeXMathStyleDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuLaTeXDisplay, description_key: Some(TrKey::MenuLaTeXDisplayDesc), index: MathStyle::Display as u32 },
                        MenuItem { label_key: TrKey::MenuLaTeXInline, description_key: Some(TrKey::MenuLaTeXInlineDesc), index: MathStyle::Inline as u32 },
                    ],
                },
                id: setting_id::MATH_STYLE,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
