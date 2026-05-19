use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};

// ---------------------------------------------------------------------------
// Blend mode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum BlendMode {
    Normal = 0,
    Multiply,
    Screen,
    Overlay,
}
impl SettingsEnum for BlendMode {}

// ---------------------------------------------------------------------------
// Settings struct
// ---------------------------------------------------------------------------

/// Settings for the solid-color-blend effect.
///
/// The solid color is stored as RGBA where:
/// - `color_r`, `color_g`, `color_b` are the solid color channels (0–1)
/// - `color_a` doubles as the blend amount (0 = full original, 1 = full solid color)
#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct SolidColorBlend {
    /// Red component of the solid color, in [0.0, 1.0].
    pub color_r: f32,
    /// Green component of the solid color, in [0.0, 1.0].
    pub color_g: f32,
    /// Blue component of the solid color, in [0.0, 1.0].
    pub color_b: f32,
    /// Alpha component = blend amount. 0.0 = full original image, 1.0 = full solid color.
    pub color_a: f32,
    /// The blend mode.
    pub blend_mode: BlendMode,
}

impl Default for SolidColorBlend {
    fn default() -> Self {
        Self {
            color_r: 0.0,
            color_g: 0.0,
            color_b: 0.0,
            color_a: 0.0,
            blend_mode: BlendMode::Normal,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::SolidColorBlendFullSettings;
    type SID = SettingID<SolidColorBlendFullSettings>;

    pub const COLOR_R:    SID = setting_id!("color_r", color_r);
    pub const COLOR_G:    SID = setting_id!("color_g", color_g);
    pub const COLOR_B:    SID = setting_id!("color_b", color_b);
    pub const COLOR_A:    SID = setting_id!("color_a", color_a);
    pub const BLEND_MODE: SID = setting_id!("blend_mode", blend_mode);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for SolidColorBlendFullSettings {
    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label: "Color Red",
                description: Some("Red component of the solid color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::COLOR_R,
            },
            SettingDescriptor {
                label: "Color Green",
                description: Some("Green component of the solid color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::COLOR_G,
            },
            SettingDescriptor {
                label: "Color Blue",
                description: Some("Blue component of the solid color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::COLOR_B,
            },
            SettingDescriptor {
                label: "Blend Amount",
                description: Some("Alpha channel blending. 0% = original image, 100% = solid color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::COLOR_A,
            },
            SettingDescriptor {
                label: "Blend Mode",
                description: Some("How the solid color is blended with the image."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label: "Normal",
                            description: Some("Linear interpolation between image and solid color."),
                            index: BlendMode::Normal as u32,
                        },
                        MenuItem {
                            label: "Multiply",
                            description: Some("Multiplies the image by the solid color."),
                            index: BlendMode::Multiply as u32,
                        },
                        MenuItem {
                            label: "Screen",
                            description: Some("Screens the image with the solid color (inverse multiply)."),
                            index: BlendMode::Screen as u32,
                        },
                        MenuItem {
                            label: "Overlay",
                            description: Some("Combines Multiply and Screen based on image brightness."),
                            index: BlendMode::Overlay as u32,
                        },
                    ],
                },
                id: setting_id::BLEND_MODE,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
