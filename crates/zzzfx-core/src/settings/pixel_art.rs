use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use effect_settings::{
    MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum, TrKey,
};

// ---------------------------------------------------------------------------
// Dithering enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum Dithering {
    None = 0,
    Ordered,
    FloydSteinberg,
}
impl SettingsEnum for Dithering {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ZzzPixelArt {
    pub pixel_size_h: f32,
    pub pixel_size_v: f32,
    pub square: bool,
    pub color_levels: f32,
    pub dithering: Dithering,
    pub dithering_amount: f32,
    pub show_grid: bool,
    pub grid_thickness: f32,
    pub grid_color_r: f32,
    pub grid_color_g: f32,
    pub grid_color_b: f32,
    pub grid_color_a: f32,
    pub contrast: f32,
    pub saturation: f32,
}

impl Default for ZzzPixelArt {
    fn default() -> Self {
        Self {
            pixel_size_h: 0.001,
            pixel_size_v: 0.001,
            square: true,
            color_levels: 16.0,
            dithering: Dithering::None,
            dithering_amount: 0.5,
            show_grid: false,
            grid_thickness: 0.1,
            grid_color_r: 0.0,
            grid_color_g: 0.0,
            grid_color_b: 0.0,
            grid_color_a: 0.5,
            contrast: 0.5,
            saturation: 0.5,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::ZzzPixelArtFullSettings;
    type SID = SettingID<ZzzPixelArtFullSettings>;

    pub const PIXEL_SIZE_H:     SID = setting_id!("pixel_size_h", pixel_size_h);
    pub const PIXEL_SIZE_V:     SID = setting_id!("pixel_size_v", pixel_size_v);
    pub const SQUARE:           SID = setting_id!("square", square);
    pub const COLOR_LEVELS:     SID = setting_id!("color_levels", color_levels);
    pub const DITHERING:        SID = setting_id!("dithering", dithering);
    pub const DITHERING_AMOUNT: SID = setting_id!("dithering_amount", dithering_amount);
    pub const SHOW_GRID:        SID = setting_id!("show_grid", show_grid);
    pub const GRID_THICKNESS:   SID = setting_id!("grid_thickness", grid_thickness);
    pub const GRID_COLOR_R:     SID = setting_id!("grid_color_r", grid_color_r);
    pub const GRID_COLOR_G:     SID = setting_id!("grid_color_g", grid_color_g);
    pub const GRID_COLOR_B:     SID = setting_id!("grid_color_b", grid_color_b);
    pub const GRID_COLOR_A:     SID = setting_id!("grid_color_a", grid_color_a);
    pub const CONTRAST:         SID = setting_id!("contrast", contrast);
    pub const SATURATION:       SID = setting_id!("saturation", saturation);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ZzzPixelArtFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamPixelSizeH,
                description_key: Some(TrKey::ParamPixelSizeHDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::PIXEL_SIZE_H,
            },
            SettingDescriptor {
                label_key: TrKey::ParamPixelSizeV,
                description_key: Some(TrKey::ParamPixelSizeVDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::PIXEL_SIZE_V,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSquare,
                description_key: Some(TrKey::ParamSquareDesc),
                kind: SettingKind::Boolean,
                id: setting_id::SQUARE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamColorLevels,
                description_key: Some(TrKey::ParamColorLevelsDesc),
                kind: SettingKind::FloatRange {
                    range: 2.0..=256.0,
                    logarithmic: false,
                },
                id: setting_id::COLOR_LEVELS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamDithering,
                description_key: Some(TrKey::ParamDitheringDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuDitherNone,
                            description_key: Some(TrKey::MenuDitherNoneDesc),
                            index: Dithering::None as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuDitherOrdered,
                            description_key: Some(TrKey::MenuDitherOrderedDesc),
                            index: Dithering::Ordered as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuDitherFloydSteinberg,
                            description_key: Some(TrKey::MenuDitherFloydSteinbergDesc),
                            index: Dithering::FloydSteinberg as u32,
                        },
                    ],
                },
                id: setting_id::DITHERING,
            },
            SettingDescriptor {
                label_key: TrKey::ParamDitheringAmount,
                description_key: Some(TrKey::ParamDitheringAmountDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::DITHERING_AMOUNT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamShowGrid,
                description_key: Some(TrKey::ParamShowGridDesc),
                kind: SettingKind::Boolean,
                id: setting_id::SHOW_GRID,
            },
            SettingDescriptor {
                label_key: TrKey::ParamGridThickness,
                description_key: Some(TrKey::ParamGridThicknessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::GRID_THICKNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamGridColorRed,
                description_key: Some(TrKey::ParamGridColorRedDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::GRID_COLOR_R,
            },
            SettingDescriptor {
                label_key: TrKey::ParamGridColorGreen,
                description_key: Some(TrKey::ParamGridColorGreenDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::GRID_COLOR_G,
            },
            SettingDescriptor {
                label_key: TrKey::ParamGridColorBlue,
                description_key: Some(TrKey::ParamGridColorBlueDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::GRID_COLOR_B,
            },
            SettingDescriptor {
                label_key: TrKey::ParamGridColorAlpha,
                description_key: Some(TrKey::ParamGridColorAlphaDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::GRID_COLOR_A,
            },
            SettingDescriptor {
                label_key: TrKey::ParamPixelContrast,
                description_key: Some(TrKey::ParamPixelContrastDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::CONTRAST,
            },
            SettingDescriptor {
                label_key: TrKey::ParamPixelSaturation,
                description_key: Some(TrKey::ParamPixelSaturationDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SATURATION,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
