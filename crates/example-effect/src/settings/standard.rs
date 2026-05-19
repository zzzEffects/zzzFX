use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsBlock, SettingsEnum};

// ---------------------------------------------------------------------------
// Parameter types
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ColorPreset {
    None = 0,
    Warm,
    Cool,
    Sepia,
}
impl SettingsEnum for ColorPreset {}

#[derive(Debug, Clone, PartialEq)]
pub struct AdvancedSettings {
    pub contrast: f32,
    pub saturation: f32,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            contrast: 1.0,
            saturation: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ExampleEffect {
    pub brightness: f32,
    pub invert_colors: bool,
    pub tint_r: f32,
    pub tint_g: f32,
    pub tint_b: f32,
    #[settings_block]
    pub advanced: Option<AdvancedSettings>,
    pub color_preset: ColorPreset,
}

impl Default for ExampleEffect {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            invert_colors: false,
            tint_r: 1.0,
            tint_g: 1.0,
            tint_b: 1.0,
            advanced: None,
            color_preset: ColorPreset::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::ExampleEffectFullSettings;
    type SID = SettingID<ExampleEffectFullSettings>;

    pub const BRIGHTNESS:     SID = setting_id!("brightness", brightness);
    pub const INVERT_COLORS:  SID = setting_id!("invert_colors", invert_colors);
    pub const TINT_R:         SID = setting_id!("tint_r", tint_r);
    pub const TINT_G:         SID = setting_id!("tint_g", tint_g);
    pub const TINT_B:         SID = setting_id!("tint_b", tint_b);
    pub const ADVANCED:       SID = setting_id!("advanced", advanced.enabled);
    pub const CONTRAST:       SID = setting_id!("contrast", advanced.settings.contrast);
    pub const SATURATION:     SID = setting_id!("saturation", advanced.settings.saturation);
    pub const COLOR_PRESET:   SID = setting_id!("color_preset", color_preset);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ExampleEffectFullSettings {
    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label: "Brightness",
                description: Some("Overall brightness multiplier."),
                kind: SettingKind::FloatRange {
                    range: 0.0..=2.0,
                    logarithmic: false,
                },
                id: setting_id::BRIGHTNESS,
            },
            SettingDescriptor {
                label: "Invert Colors",
                description: Some("Invert all colors in the image."),
                kind: SettingKind::Boolean,
                id: setting_id::INVERT_COLORS,
            },
            SettingDescriptor {
                label: "Tint Red",
                description: Some("Red channel tint multiplier."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::TINT_R,
            },
            SettingDescriptor {
                label: "Tint Green",
                description: Some("Green channel tint multiplier."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::TINT_G,
            },
            SettingDescriptor {
                label: "Tint Blue",
                description: Some("Blue channel tint multiplier."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::TINT_B,
            },
            SettingDescriptor {
                label: "Advanced",
                description: Some("Additional advanced settings."),
                kind: SettingKind::Group {
                    children: vec![
                        SettingDescriptor {
                            label: "Contrast",
                            description: Some("Contrast adjustment."),
                            kind: SettingKind::FloatRange {
                                range: 0.0..=4.0,
                                logarithmic: false,
                            },
                            id: setting_id::CONTRAST,
                        },
                        SettingDescriptor {
                            label: "Saturation",
                            description: Some("Color saturation adjustment."),
                            kind: SettingKind::Percentage { logarithmic: true },
                            id: setting_id::SATURATION,
                        },
                    ],
                },
                id: setting_id::ADVANCED,
            },
            SettingDescriptor {
                label: "Color Preset",
                description: Some("Choose a color preset."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label: "None",
                            description: Some("No color preset."),
                            index: ColorPreset::None as u32,
                        },
                        MenuItem {
                            label: "Warm",
                            description: Some("Warm color tone."),
                            index: ColorPreset::Warm as u32,
                        },
                        MenuItem {
                            label: "Cool",
                            description: Some("Cool color tone."),
                            index: ColorPreset::Cool as u32,
                        },
                        MenuItem {
                            label: "Sepia",
                            description: Some("Sepia color tone."),
                            index: ColorPreset::Sepia as u32,
                        },
                    ],
                },
                id: setting_id::COLOR_PRESET,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
