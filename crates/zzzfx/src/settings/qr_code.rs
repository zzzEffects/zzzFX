use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// ECL enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum Ecl {
    L = 0,
    M = 1,
    Q = 2,
    H = 3,
}

impl SettingsEnum for Ecl {}

// ---------------------------------------------------------------------------
// ModuleShape enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ModuleShape {
    Square = 0,
    Circle = 1,
    RoundedSquare = 2,
    Vertical = 3,
    Horizontal = 4,
    Diamond = 5,
}

impl SettingsEnum for ModuleShape {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct QrCode {
    pub content: String,
    pub scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub margin: f32,
    pub ecl: Ecl,
    pub module_shape: ModuleShape,
    pub module_color_r: f32,
    pub module_color_g: f32,
    pub module_color_b: f32,
    pub module_color_a: f32,
    pub light_module_color_r: f32,
    pub light_module_color_g: f32,
    pub light_module_color_b: f32,
    pub light_module_color_a: f32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
}

impl Default for QrCode {
    fn default() -> Self {
        Self {
            content: String::new(),
            scale: 1.0,
            position_x: 0.5,
            position_y: 0.5,
            rotation: 0.0,
            opacity: 1.0,
            margin: 4.0,
            ecl: Ecl::M,
            module_shape: ModuleShape::Square,
            module_color_r: 0.0,
            module_color_g: 0.0,
            module_color_b: 0.0,
            module_color_a: 1.0,
            light_module_color_r: 1.0,
            light_module_color_g: 1.0,
            light_module_color_b: 1.0,
            light_module_color_a: 1.0,
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
pub struct QrCodeFullSettings {
    pub content: String,
    pub scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub margin: f32,
    pub ecl: Ecl,
    pub module_shape: ModuleShape,
    pub module_color_r: f32,
    pub module_color_g: f32,
    pub module_color_b: f32,
    pub module_color_a: f32,
    pub light_module_color_r: f32,
    pub light_module_color_g: f32,
    pub light_module_color_b: f32,
    pub light_module_color_a: f32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
}

impl Default for QrCodeFullSettings {
    fn default() -> Self {
        Self::from(QrCode::default())
    }
}

impl From<&QrCode> for QrCodeFullSettings {
    fn from(value: &QrCode) -> Self {
        Self {
            content: value.content.clone(),
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            margin: value.margin,
            ecl: value.ecl,
            module_shape: value.module_shape,
            module_color_r: value.module_color_r,
            module_color_g: value.module_color_g,
            module_color_b: value.module_color_b,
            module_color_a: value.module_color_a,
            light_module_color_r: value.light_module_color_r,
            light_module_color_g: value.light_module_color_g,
            light_module_color_b: value.light_module_color_b,
            light_module_color_a: value.light_module_color_a,
            background_color_r: value.background_color_r,
            background_color_g: value.background_color_g,
            background_color_b: value.background_color_b,
            background_color_a: value.background_color_a,
        }
    }
}

impl From<QrCode> for QrCodeFullSettings {
    fn from(value: QrCode) -> Self {
        Self {
            content: value.content,
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            margin: value.margin,
            ecl: value.ecl,
            module_shape: value.module_shape,
            module_color_r: value.module_color_r,
            module_color_g: value.module_color_g,
            module_color_b: value.module_color_b,
            module_color_a: value.module_color_a,
            light_module_color_r: value.light_module_color_r,
            light_module_color_g: value.light_module_color_g,
            light_module_color_b: value.light_module_color_b,
            light_module_color_a: value.light_module_color_a,
            background_color_r: value.background_color_r,
            background_color_g: value.background_color_g,
            background_color_b: value.background_color_b,
            background_color_a: value.background_color_a,
        }
    }
}

impl From<&QrCodeFullSettings> for QrCode {
    fn from(value: &QrCodeFullSettings) -> Self {
        Self {
            content: value.content.clone(),
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            margin: value.margin,
            ecl: value.ecl,
            module_shape: value.module_shape,
            module_color_r: value.module_color_r,
            module_color_g: value.module_color_g,
            module_color_b: value.module_color_b,
            module_color_a: value.module_color_a,
            light_module_color_r: value.light_module_color_r,
            light_module_color_g: value.light_module_color_g,
            light_module_color_b: value.light_module_color_b,
            light_module_color_a: value.light_module_color_a,
            background_color_r: value.background_color_r,
            background_color_g: value.background_color_g,
            background_color_b: value.background_color_b,
            background_color_a: value.background_color_a,
        }
    }
}

impl From<QrCodeFullSettings> for QrCode {
    fn from(value: QrCodeFullSettings) -> Self {
        Self {
            content: value.content,
            scale: value.scale,
            position_x: value.position_x,
            position_y: value.position_y,
            rotation: value.rotation,
            opacity: value.opacity,
            margin: value.margin,
            ecl: value.ecl,
            module_shape: value.module_shape,
            module_color_r: value.module_color_r,
            module_color_g: value.module_color_g,
            module_color_b: value.module_color_b,
            module_color_a: value.module_color_a,
            light_module_color_r: value.light_module_color_r,
            light_module_color_g: value.light_module_color_g,
            light_module_color_b: value.light_module_color_b,
            light_module_color_a: value.light_module_color_a,
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
    use super::QrCodeFullSettings;
    type SID = SettingID<QrCodeFullSettings>;

    pub const CONTENT:      SID = setting_id!("content", content);
    pub const SCALE:        SID = setting_id!("scale", scale);
    pub const POSITION_X:   SID = setting_id!("position_x", position_x);
    pub const POSITION_Y:   SID = setting_id!("position_y", position_y);
    pub const ROTATION:     SID = setting_id!("rotation", rotation);
    pub const OPACITY:      SID = setting_id!("opacity", opacity);
    pub const MARGIN:       SID = setting_id!("margin", margin);
    pub const ECL:          SID = setting_id!("ecl", ecl);
    pub const MODULE_SHAPE: SID = setting_id!("module_shape", module_shape);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for QrCodeFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::NativeQrCodeContent,
                description_key: Some(TrKey::NativeQrCodeContentHint),
                kind: SettingKind::String { secret: false, multiline: true, animates: false },
                id: setting_id::CONTENT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodeScale,
                description_key: Some(TrKey::ParamQrCodeScaleDesc),
                kind: SettingKind::FloatRange { range: 0.01..=10.0, logarithmic: false },
                id: setting_id::SCALE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodePositionX,
                description_key: Some(TrKey::ParamQrCodePositionXDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodePositionY,
                description_key: Some(TrKey::ParamQrCodePositionYDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodeRotation,
                description_key: Some(TrKey::ParamQrCodeRotationDesc),
                kind: SettingKind::FloatRange { range: 0.0..=360.0, logarithmic: false },
                id: setting_id::ROTATION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodeOpacity,
                description_key: Some(TrKey::ParamQrCodeOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodeMargin,
                description_key: Some(TrKey::ParamQrCodeMarginDesc),
                kind: SettingKind::FloatRange { range: 0.0..=10.0, logarithmic: false },
                id: setting_id::MARGIN,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodeEcl,
                description_key: Some(TrKey::ParamQrCodeEclDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuQrEclL, description_key: Some(TrKey::MenuQrEclLDesc), index: Ecl::L as u32 },
                        MenuItem { label_key: TrKey::MenuQrEclM, description_key: Some(TrKey::MenuQrEclMDesc), index: Ecl::M as u32 },
                        MenuItem { label_key: TrKey::MenuQrEclQ, description_key: Some(TrKey::MenuQrEclQDesc), index: Ecl::Q as u32 },
                        MenuItem { label_key: TrKey::MenuQrEclH, description_key: Some(TrKey::MenuQrEclHDesc), index: Ecl::H as u32 },
                    ],
                },
                id: setting_id::ECL,
            },
            SettingDescriptor {
                label_key: TrKey::ParamQrCodeModuleShape,
                description_key: Some(TrKey::ParamQrCodeModuleShapeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuQrShapeSquare, description_key: Some(TrKey::MenuQrShapeSquareDesc), index: ModuleShape::Square as u32 },
                        MenuItem { label_key: TrKey::MenuQrShapeCircle, description_key: Some(TrKey::MenuQrShapeCircleDesc), index: ModuleShape::Circle as u32 },
                        MenuItem { label_key: TrKey::MenuQrShapeRoundedSquare, description_key: Some(TrKey::MenuQrShapeRoundedSquareDesc), index: ModuleShape::RoundedSquare as u32 },
                        MenuItem { label_key: TrKey::MenuQrShapeVertical, description_key: Some(TrKey::MenuQrShapeVerticalDesc), index: ModuleShape::Vertical as u32 },
                        MenuItem { label_key: TrKey::MenuQrShapeHorizontal, description_key: Some(TrKey::MenuQrShapeHorizontalDesc), index: ModuleShape::Horizontal as u32 },
                        MenuItem { label_key: TrKey::MenuQrShapeDiamond, description_key: Some(TrKey::MenuQrShapeDiamondDesc), index: ModuleShape::Diamond as u32 },
                    ],
                },
                id: setting_id::MODULE_SHAPE,
            },
        ]
        .into_boxed_slice()
    }

}
