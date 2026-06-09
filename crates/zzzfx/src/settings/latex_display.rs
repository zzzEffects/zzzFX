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

#[derive(Clone, Debug, PartialEq)]
pub struct LaTeXDisplay {
    pub formula: String,
    pub font_name: String,
    pub font_size: f32,
    pub scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub dpi: f32,
    pub math_style: MathStyle,
    pub text_color_r: f32,
    pub text_color_g: f32,
    pub text_color_b: f32,
    pub text_color_a: f32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
}

impl Default for LaTeXDisplay {
    fn default() -> Self {
        Self {
            formula: String::new(),
            font_name: String::new(),
            font_size: 10.0,
            scale: 1.0,
            position_x: 0.5,
            position_y: 0.5,
            rotation: 0.0,
            opacity: 1.0,
            dpi: 96.0,
            math_style: MathStyle::Display,
            text_color_r: 1.0,
            text_color_g: 1.0,
            text_color_b: 1.0,
            text_color_a: 1.0,
            background_color_r: 0.0,
            background_color_g: 0.0,
            background_color_b: 0.0,
            background_color_a: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// FullSettings struct (manual — derive macro doesn't support String)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct LaTeXDisplayFullSettings {
    pub formula: String,
    pub font_name: String,
    pub font_size: f32,
    pub scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub dpi: f32,
    pub math_style: MathStyle,
    pub text_color_r: f32,
    pub text_color_g: f32,
    pub text_color_b: f32,
    pub text_color_a: f32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
}

impl Default for LaTeXDisplayFullSettings {
    fn default() -> Self {
        Self::from(LaTeXDisplay::default())
    }
}

impl From<&LaTeXDisplay> for LaTeXDisplayFullSettings {
    fn from(value: &LaTeXDisplay) -> Self {
        Self {
            formula: value.formula.clone(),
            font_name: value.font_name.clone(),
            font_size: value.font_size,
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            dpi: value.dpi,
            math_style: value.math_style,
            text_color_r: value.text_color_r,
            text_color_g: value.text_color_g,
            text_color_b: value.text_color_b,
            text_color_a: value.text_color_a,
            background_color_r: value.background_color_r,
            background_color_g: value.background_color_g,
            background_color_b: value.background_color_b,
            background_color_a: value.background_color_a,
        }
    }
}

impl From<LaTeXDisplay> for LaTeXDisplayFullSettings {
    fn from(value: LaTeXDisplay) -> Self {
        Self {
            formula: value.formula,
            font_name: value.font_name,
            font_size: value.font_size,
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            dpi: value.dpi,
            math_style: value.math_style,
            text_color_r: value.text_color_r,
            text_color_g: value.text_color_g,
            text_color_b: value.text_color_b,
            text_color_a: value.text_color_a,
            background_color_r: value.background_color_r,
            background_color_g: value.background_color_g,
            background_color_b: value.background_color_b,
            background_color_a: value.background_color_a,
        }
    }
}

impl From<&LaTeXDisplayFullSettings> for LaTeXDisplay {
    fn from(value: &LaTeXDisplayFullSettings) -> Self {
        Self {
            formula: value.formula.clone(),
            font_name: value.font_name.clone(),
            font_size: value.font_size,
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            dpi: value.dpi,
            math_style: value.math_style,
            text_color_r: value.text_color_r,
            text_color_g: value.text_color_g,
            text_color_b: value.text_color_b,
            text_color_a: value.text_color_a,
            background_color_r: value.background_color_r,
            background_color_g: value.background_color_g,
            background_color_b: value.background_color_b,
            background_color_a: value.background_color_a,
        }
    }
}

impl From<LaTeXDisplayFullSettings> for LaTeXDisplay {
    fn from(value: LaTeXDisplayFullSettings) -> Self {
        Self {
            formula: value.formula,
            font_name: value.font_name,
            font_size: value.font_size,
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            dpi: value.dpi,
            math_style: value.math_style,
            text_color_r: value.text_color_r,
            text_color_g: value.text_color_g,
            text_color_b: value.text_color_b,
            text_color_a: value.text_color_a,
            background_color_r: value.background_color_r,
            background_color_g: value.background_color_g,
            background_color_b: value.background_color_b,
            background_color_a: value.background_color_a,
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

    pub const FORMULA:     SID = setting_id!("formula", formula);
    pub const FONT_NAME:   SID = setting_id!("font_name", font_name);
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
                label_key: TrKey::NativeLaTeXFormula,
                description_key: Some(TrKey::NativeLaTeXFormulaHint),
                kind: SettingKind::String { secret: false, multiline: true, animates: false },
                id: setting_id::FORMULA,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLaTeXFontSize,
                description_key: Some(TrKey::ParamLaTeXFontSizeDesc),
                kind: SettingKind::FloatRange { range: 1.0..=512.0, logarithmic: false },
                id: setting_id::FONT_SIZE,
            },
            SettingDescriptor {
                label_key: TrKey::NativeLaTeXFontChoice,
                description_key: None,
                kind: SettingKind::String { secret: true, multiline: false, animates: false },
                id: setting_id::FONT_NAME,
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

}
