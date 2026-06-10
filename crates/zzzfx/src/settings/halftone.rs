use zzzfx_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{
    MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum,
};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// DotShape enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum DotShape {
    Circle = 0,
    Square = 1,
    Diamond = 2,
}
impl SettingsEnum for DotShape {}

// ---------------------------------------------------------------------------
// ChannelMode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ChannelMode {
    Luminance = 0,
    RGB = 1,
}
impl SettingsEnum for ChannelMode {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct HalfTone {
    pub dot_size: f32,
    pub angle: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub dot_shape: DotShape,
    pub channel_mode: ChannelMode,
    pub invert: bool,
    pub contrast: f32,
    pub smoothness: f32,
    pub blend_with_original: f32,
}

impl Default for HalfTone {
    fn default() -> Self {
        Self {
            dot_size: 0.1,
            angle: 45.0,
            position_x: 0.5,
            position_y: 0.5,
            dot_shape: DotShape::Circle,
            channel_mode: ChannelMode::Luminance,
            invert: false,
            contrast: 0.5,
            smoothness: 0.1,
            blend_with_original: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::HalfToneFullSettings;
    type SID = SettingID<HalfToneFullSettings>;

    pub const DOT_SIZE:               SID = setting_id!("dot_size", dot_size);
    pub const ANGLE:                  SID = setting_id!("angle", angle);
    pub const POSITION_X:             SID = setting_id!("position_x", position_x);
    pub const POSITION_Y:             SID = setting_id!("position_y", position_y);
    pub const DOT_SHAPE:              SID = setting_id!("dot_shape", dot_shape);
    pub const CHANNEL_MODE:           SID = setting_id!("channel_mode", channel_mode);
    pub const INVERT:                 SID = setting_id!("invert", invert);
    pub const CONTRAST:               SID = setting_id!("contrast", contrast);
    pub const SMOOTHNESS:             SID = setting_id!("smoothness", smoothness);
    pub const BLEND_WITH_ORIGINAL:    SID = setting_id!("blend_with_original", blend_with_original);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for HalfToneFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamHalfTonePositionX,
                description_key: Some(TrKey::ParamHalfTonePositionXDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfTonePositionY,
                description_key: Some(TrKey::ParamHalfTonePositionYDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneDotSize,
                description_key: Some(TrKey::ParamHalfToneDotSizeDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=100.0,
                    logarithmic: false,
                },
                id: setting_id::DOT_SIZE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneAngle,
                description_key: Some(TrKey::ParamHalfToneAngleDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=360.0,
                    logarithmic: false,
                },
                id: setting_id::ANGLE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneDotShape,
                description_key: Some(TrKey::ParamHalfToneDotShapeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuDotShapeCircle,
                            description_key: Some(TrKey::MenuDotShapeCircleDesc),
                            index: DotShape::Circle as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuDotShapeSquare,
                            description_key: Some(TrKey::MenuDotShapeSquareDesc),
                            index: DotShape::Square as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuDotShapeDiamond,
                            description_key: Some(TrKey::MenuDotShapeDiamondDesc),
                            index: DotShape::Diamond as u32,
                        },
                    ],
                },
                id: setting_id::DOT_SHAPE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneChannelMode,
                description_key: Some(TrKey::ParamHalfToneChannelModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuChannelLuminance,
                            description_key: Some(TrKey::MenuChannelLuminanceDesc),
                            index: ChannelMode::Luminance as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuChannelRGB,
                            description_key: Some(TrKey::MenuChannelRGBDesc),
                            index: ChannelMode::RGB as u32,
                        },
                    ],
                },
                id: setting_id::CHANNEL_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneInvert,
                description_key: Some(TrKey::ParamHalfToneInvertDesc),
                kind: SettingKind::Boolean,
                id: setting_id::INVERT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneContrast,
                description_key: Some(TrKey::ParamHalfToneContrastDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::CONTRAST,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneSmoothness,
                description_key: Some(TrKey::ParamHalfToneSmoothnessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SMOOTHNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamHalfToneBlendWithOriginal,
                description_key: Some(TrKey::ParamHalfToneBlendWithOriginalDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::BLEND_WITH_ORIGINAL,
            },
        ]
        .into_boxed_slice()
    }
}
