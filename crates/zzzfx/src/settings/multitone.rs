use zzzfx_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{
    MenuItem, SettingDescriptor, SettingKind, Settings, SettingsBlock, SettingsEnum,
};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// ToneMode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ToneMode {
    PerChannel = 0,
    Luminance = 1,
}
impl SettingsEnum for ToneMode {}

// ---------------------------------------------------------------------------
// ToneDithering enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ToneDithering {
    None = 0,
    Ordered = 1,
    FloydSteinberg = 2,
}
impl SettingsEnum for ToneDithering {}

// ---------------------------------------------------------------------------
// ColorMappingSettings — nested sub-settings for gradient-map colorization
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ColorMappingSettings {
    pub shadow_color_r: f32,
    pub shadow_color_g: f32,
    pub shadow_color_b: f32,
    pub shadow_color_a: f32,
    pub midtone_color_r: f32,
    pub midtone_color_g: f32,
    pub midtone_color_b: f32,
    pub midtone_color_a: f32,
    pub highlight_color_r: f32,
    pub highlight_color_g: f32,
    pub highlight_color_b: f32,
    pub highlight_color_a: f32,
    pub midtone_position: f32,
    pub blend_with_original: f32,
}

impl Default for ColorMappingSettings {
    fn default() -> Self {
        Self {
            shadow_color_r: 0.0,
            shadow_color_g: 0.0,
            shadow_color_b: 0.0,
            shadow_color_a: 1.0,
            midtone_color_r: 0.5,
            midtone_color_g: 0.5,
            midtone_color_b: 0.5,
            midtone_color_a: 1.0,
            highlight_color_r: 1.0,
            highlight_color_g: 1.0,
            highlight_color_b: 1.0,
            highlight_color_a: 1.0,
            midtone_position: 0.5,
            blend_with_original: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct MultiTone {
    pub tone_levels: f32,
    pub mode: ToneMode,
    pub dithering: ToneDithering,
    pub dithering_amount: f32,
    pub edge_softness: f32,
    pub preserve_luminosity: bool,
    #[settings_block]
    pub color_mapping: Option<ColorMappingSettings>,
}

impl Default for MultiTone {
    fn default() -> Self {
        Self {
            tone_levels: 8.0,
            mode: ToneMode::PerChannel,
            dithering: ToneDithering::None,
            dithering_amount: 0.5,
            edge_softness: 0.0,
            preserve_luminosity: true,
            color_mapping: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::MultiToneFullSettings;
    type SID = SettingID<MultiToneFullSettings>;

    pub const TONE_LEVELS:         SID = setting_id!("tone_levels", tone_levels);
    pub const MODE:                SID = setting_id!("mode", mode);
    pub const DITHERING:           SID = setting_id!("dithering", dithering);
    pub const DITHERING_AMOUNT:    SID = setting_id!("dithering_amount", dithering_amount);
    pub const EDGE_SOFTNESS:       SID = setting_id!("edge_softness", edge_softness);
    pub const PRESERVE_LUMINOSITY: SID = setting_id!("preserve_luminosity", preserve_luminosity);

    // Color Mapping group (enabled flag)
    pub const COLOR_MAPPING:               SID = setting_id!("color_mapping", color_mapping.enabled);

    // Color Mapping children
    pub const SHADOW_COLOR:                SID = setting_id!("shadow_color_r", color_mapping.settings.shadow_color_r);
    pub const SHADOW_COLOR_R:              SID = setting_id!("shadow_color_r", color_mapping.settings.shadow_color_r);
    pub const SHADOW_COLOR_G:              SID = setting_id!("shadow_color_g", color_mapping.settings.shadow_color_g);
    pub const SHADOW_COLOR_B:              SID = setting_id!("shadow_color_b", color_mapping.settings.shadow_color_b);
    pub const SHADOW_COLOR_A:              SID = setting_id!("shadow_color_a", color_mapping.settings.shadow_color_a);
    pub const MIDTONE_COLOR:               SID = setting_id!("midtone_color_r", color_mapping.settings.midtone_color_r);
    pub const MIDTONE_COLOR_R:             SID = setting_id!("midtone_color_r", color_mapping.settings.midtone_color_r);
    pub const MIDTONE_COLOR_G:             SID = setting_id!("midtone_color_g", color_mapping.settings.midtone_color_g);
    pub const MIDTONE_COLOR_B:             SID = setting_id!("midtone_color_b", color_mapping.settings.midtone_color_b);
    pub const MIDTONE_COLOR_A:             SID = setting_id!("midtone_color_a", color_mapping.settings.midtone_color_a);
    pub const HIGHLIGHT_COLOR:             SID = setting_id!("highlight_color_r", color_mapping.settings.highlight_color_r);
    pub const HIGHLIGHT_COLOR_R:           SID = setting_id!("highlight_color_r", color_mapping.settings.highlight_color_r);
    pub const HIGHLIGHT_COLOR_G:           SID = setting_id!("highlight_color_g", color_mapping.settings.highlight_color_g);
    pub const HIGHLIGHT_COLOR_B:           SID = setting_id!("highlight_color_b", color_mapping.settings.highlight_color_b);
    pub const HIGHLIGHT_COLOR_A:           SID = setting_id!("highlight_color_a", color_mapping.settings.highlight_color_a);
    pub const MIDTONE_POSITION:            SID = setting_id!("midtone_position", color_mapping.settings.midtone_position);
    pub const BLEND_WITH_ORIGINAL:         SID = setting_id!("blend_with_original", color_mapping.settings.blend_with_original);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for MultiToneFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamMultiToneLevels,
                description_key: Some(TrKey::ParamMultiToneLevelsDesc),
                kind: SettingKind::FloatRange {
                    range: 2.0..=32.0,
                    logarithmic: false,
                },
                id: setting_id::TONE_LEVELS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMultiToneMode,
                description_key: Some(TrKey::ParamMultiToneModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuTonePerChannel,
                            description_key: Some(TrKey::MenuTonePerChannelDesc),
                            index: ToneMode::PerChannel as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuToneLuminance,
                            description_key: Some(TrKey::MenuToneLuminanceDesc),
                            index: ToneMode::Luminance as u32,
                        },
                    ],
                },
                id: setting_id::MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMultiToneDithering,
                description_key: Some(TrKey::ParamMultiToneDitheringDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuDitherNone,
                            description_key: Some(TrKey::MenuDitherNoneDesc),
                            index: ToneDithering::None as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuDitherOrdered,
                            description_key: Some(TrKey::MenuDitherOrderedDesc),
                            index: ToneDithering::Ordered as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuDitherFloydSteinberg,
                            description_key: Some(TrKey::MenuDitherFloydSteinbergDesc),
                            index: ToneDithering::FloydSteinberg as u32,
                        },
                    ],
                },
                id: setting_id::DITHERING,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMultiToneDitheringAmount,
                description_key: Some(TrKey::ParamMultiToneDitheringAmountDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::DITHERING_AMOUNT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMultiToneEdgeSoftness,
                description_key: Some(TrKey::ParamMultiToneEdgeSoftnessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::EDGE_SOFTNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMultiTonePreserveLuminosity,
                description_key: Some(TrKey::ParamMultiTonePreserveLuminosityDesc),
                kind: SettingKind::Boolean,
                id: setting_id::PRESERVE_LUMINOSITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMultiToneColorMapping,
                description_key: Some(TrKey::ParamMultiToneColorMappingDesc),
                kind: SettingKind::Group {
                    children: vec![
                        SettingDescriptor {
                            label_key: TrKey::ParamMultiToneShadowColor,
                            description_key: Some(TrKey::ParamMultiToneShadowColorDesc),
                            kind: SettingKind::ColorRGBA {
                                r_id: setting_id::SHADOW_COLOR_R,
                                g_id: setting_id::SHADOW_COLOR_G,
                                b_id: setting_id::SHADOW_COLOR_B,
                                a_id: setting_id::SHADOW_COLOR_A,
                            },
                            id: setting_id::SHADOW_COLOR,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamMultiToneMidtoneColor,
                            description_key: Some(TrKey::ParamMultiToneMidtoneColorDesc),
                            kind: SettingKind::ColorRGBA {
                                r_id: setting_id::MIDTONE_COLOR_R,
                                g_id: setting_id::MIDTONE_COLOR_G,
                                b_id: setting_id::MIDTONE_COLOR_B,
                                a_id: setting_id::MIDTONE_COLOR_A,
                            },
                            id: setting_id::MIDTONE_COLOR,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamMultiToneHighlightColor,
                            description_key: Some(TrKey::ParamMultiToneHighlightColorDesc),
                            kind: SettingKind::ColorRGBA {
                                r_id: setting_id::HIGHLIGHT_COLOR_R,
                                g_id: setting_id::HIGHLIGHT_COLOR_G,
                                b_id: setting_id::HIGHLIGHT_COLOR_B,
                                a_id: setting_id::HIGHLIGHT_COLOR_A,
                            },
                            id: setting_id::HIGHLIGHT_COLOR,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamMultiToneMidtonePosition,
                            description_key: Some(TrKey::ParamMultiToneMidtonePositionDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::MIDTONE_POSITION,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamMultiToneBlendWithOriginal,
                            description_key: Some(TrKey::ParamMultiToneBlendWithOriginalDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::BLEND_WITH_ORIGINAL,
                        },
                    ],
                },
                id: setting_id::COLOR_MAPPING,
            },
        ]
        .into_boxed_slice()
    }
}
