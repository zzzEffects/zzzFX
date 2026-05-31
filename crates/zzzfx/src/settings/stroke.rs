use zzzfx_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsBlock, SettingsEnum};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// Stroke position enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum StrokePosition {
    Outer = 0,
    Inner,
    Center,
}
impl SettingsEnum for StrokePosition {}

// ---------------------------------------------------------------------------
// Fill mode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum FillMode {
    SolidColor = 0,
    DistanceGradient,
    Gradient,
    SourceColorExtension,
}
impl SettingsEnum for FillMode {}

// ---------------------------------------------------------------------------
// Blend mode enum (22 modes)
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum BlendMode {
    Normal = 0,
    Dissolve,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    Add,
    Screen,
    ColorDodge,
    LinearDodge,
    Overlay,
    SoftLight,
    LinearLight,
    HardMix,
    Difference,
    Exclusion,
    Subtract,
    Divide,
    StencilAlpha,
    StencilLuma,
    OutlineAlpha,
    OutlineLuma,
}
impl SettingsEnum for BlendMode {}

// ---------------------------------------------------------------------------
// Gradient settings (nested settings block)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct GradientSettings {
    pub start_x: f32,
    pub start_y: f32,
    pub start_color_r: f32,
    pub start_color_g: f32,
    pub start_color_b: f32,
    pub start_color_a: f32,
    pub end_x: f32,
    pub end_y: f32,
    pub end_color_r: f32,
    pub end_color_g: f32,
    pub end_color_b: f32,
    pub end_color_a: f32,
}

impl Default for GradientSettings {
    fn default() -> Self {
        Self {
            start_x: 0.0,
            start_y: 0.0,
            start_color_r: 0.0,
            start_color_g: 0.0,
            start_color_b: 0.0,
            start_color_a: 1.0,
            end_x: 1.0,
            end_y: 1.0,
            end_color_r: 1.0,
            end_color_g: 1.0,
            end_color_b: 1.0,
            end_color_a: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct Stroke {
    pub stroke_position: StrokePosition,
    pub fill_mode: FillMode,
    pub stroke_width: f32,
    pub stroke_color_r: f32,
    pub stroke_color_g: f32,
    pub stroke_color_b: f32,
    pub stroke_color_a: f32,
    pub alpha_threshold: f32,
    pub edge_blend: f32,
    pub stroke_feathering: f32,
    pub source_opacity: f32,
    pub blend_mode: BlendMode,
    #[settings_block]
    pub gradient: Option<GradientSettings>,
    pub use_sharp_corners: bool,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            stroke_position: StrokePosition::Outer,
            fill_mode: FillMode::SolidColor,
            stroke_width: 0.05,
            stroke_color_r: 1.0,
            stroke_color_g: 1.0,
            stroke_color_b: 1.0,
            stroke_color_a: 1.0,
            alpha_threshold: 0.5,
            edge_blend: 1.0,
            stroke_feathering: 0.01,
            source_opacity: 1.0,
            blend_mode: BlendMode::Normal,
            gradient: None,
            use_sharp_corners: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::StrokeFullSettings;
    type SID = SettingID<StrokeFullSettings>;

    pub const STROKE_POSITION:         SID = setting_id!("stroke_position", stroke_position);
    pub const FILL_MODE:               SID = setting_id!("fill_mode", fill_mode);
    pub const STROKE_WIDTH:            SID = setting_id!("stroke_width", stroke_width);
    pub const STROKE_COLOR_R:          SID = setting_id!("stroke_color_r", stroke_color_r);
    pub const STROKE_COLOR_G:          SID = setting_id!("stroke_color_g", stroke_color_g);
    pub const STROKE_COLOR_B:          SID = setting_id!("stroke_color_b", stroke_color_b);
    pub const STROKE_COLOR_A:          SID = setting_id!("stroke_color_a", stroke_color_a);
    pub const ALPHA_THRESHOLD:         SID = setting_id!("alpha_threshold", alpha_threshold);
    pub const EDGE_BLEND:              SID = setting_id!("edge_blend", edge_blend);
    pub const STROKE_FEATHERING:       SID = setting_id!("stroke_feathering", stroke_feathering);
    pub const SOURCE_OPACITY:          SID = setting_id!("source_opacity", source_opacity);
    pub const BLEND_MODE:              SID = setting_id!("blend_mode", blend_mode);
    pub const GRADIENT:                SID = setting_id!("gradient", gradient.enabled);
    pub const USE_SHARP_CORNERS:       SID = setting_id!("use_sharp_corners", use_sharp_corners);
    pub const GRADIENT_START_X:        SID = setting_id!("gradient_start_x", gradient.settings.start_x);
    pub const GRADIENT_START_Y:        SID = setting_id!("gradient_start_y", gradient.settings.start_y);
    pub const GRADIENT_START_COLOR_R:  SID = setting_id!("gradient_start_color_r", gradient.settings.start_color_r);
    pub const GRADIENT_START_COLOR_G:  SID = setting_id!("gradient_start_color_g", gradient.settings.start_color_g);
    pub const GRADIENT_START_COLOR_B:  SID = setting_id!("gradient_start_color_b", gradient.settings.start_color_b);
    pub const GRADIENT_START_COLOR_A:  SID = setting_id!("gradient_start_color_a", gradient.settings.start_color_a);
    pub const GRADIENT_END_X:          SID = setting_id!("gradient_end_x", gradient.settings.end_x);
    pub const GRADIENT_END_Y:          SID = setting_id!("gradient_end_y", gradient.settings.end_y);
    pub const GRADIENT_END_COLOR_R:    SID = setting_id!("gradient_end_color_r", gradient.settings.end_color_r);
    pub const GRADIENT_END_COLOR_G:    SID = setting_id!("gradient_end_color_g", gradient.settings.end_color_g);
    pub const GRADIENT_END_COLOR_B:    SID = setting_id!("gradient_end_color_b", gradient.settings.end_color_b);
    pub const GRADIENT_END_COLOR_A:    SID = setting_id!("gradient_end_color_a", gradient.settings.end_color_a);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for StrokeFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamStrokePosition,
                description_key: Some(TrKey::ParamStrokePositionDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuStrokeOuter,
                            description_key: Some(TrKey::MenuStrokeOuterDesc),
                            index: StrokePosition::Outer as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuStrokeInner,
                            description_key: Some(TrKey::MenuStrokeInnerDesc),
                            index: StrokePosition::Inner as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuStrokeCenter,
                            description_key: Some(TrKey::MenuStrokeCenterDesc),
                            index: StrokePosition::Center as u32,
                        },
                    ],
                },
                id: setting_id::STROKE_POSITION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamFillMode,
                description_key: Some(TrKey::ParamFillModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuSolidColor,
                            description_key: Some(TrKey::MenuSolidColorDesc),
                            index: FillMode::SolidColor as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuDistanceGradient,
                            description_key: Some(TrKey::MenuDistanceGradientDesc),
                            index: FillMode::DistanceGradient as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuGradient,
                            description_key: Some(TrKey::MenuGradientDesc),
                            index: FillMode::Gradient as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuSourceColorExtension,
                            description_key: Some(TrKey::MenuSourceColorExtensionDesc),
                            index: FillMode::SourceColorExtension as u32,
                        },
                    ],
                },
                id: setting_id::FILL_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamStrokeWidth,
                description_key: Some(TrKey::ParamStrokeWidthDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_WIDTH,
            },
            SettingDescriptor {
                label_key: TrKey::ParamStrokeColorRed,
                description_key: Some(TrKey::ParamStrokeColorRedDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_R,
            },
            SettingDescriptor {
                label_key: TrKey::ParamStrokeColorGreen,
                description_key: Some(TrKey::ParamStrokeColorGreenDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_G,
            },
            SettingDescriptor {
                label_key: TrKey::ParamStrokeColorBlue,
                description_key: Some(TrKey::ParamStrokeColorBlueDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_B,
            },
            SettingDescriptor {
                label_key: TrKey::ParamStrokeColorAlpha,
                description_key: Some(TrKey::ParamStrokeColorAlphaDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_A,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAlphaThreshold,
                description_key: Some(TrKey::ParamAlphaThresholdDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::ALPHA_THRESHOLD,
            },
            SettingDescriptor {
                label_key: TrKey::ParamEdgeBlend,
                description_key: Some(TrKey::ParamEdgeBlendDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::EDGE_BLEND,
            },
            SettingDescriptor {
                label_key: TrKey::ParamStrokeFeathering,
                description_key: Some(TrKey::ParamStrokeFeatheringDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_FEATHERING,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSourceOpacity,
                description_key: Some(TrKey::ParamSourceOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SOURCE_OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamBlendMode,
                description_key: Some(TrKey::ParamBlendModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuNormal, description_key: Some(TrKey::MenuNormalDesc), index: BlendMode::Normal as u32 },
                        MenuItem { label_key: TrKey::MenuDissolve, description_key: Some(TrKey::MenuDissolveDesc), index: BlendMode::Dissolve as u32 },
                        MenuItem { label_key: TrKey::MenuDarken, description_key: Some(TrKey::MenuDarkenDesc), index: BlendMode::Darken as u32 },
                        MenuItem { label_key: TrKey::MenuMultiply, description_key: Some(TrKey::MenuMultiplyDesc), index: BlendMode::Multiply as u32 },
                        MenuItem { label_key: TrKey::MenuColorBurn, description_key: Some(TrKey::MenuColorBurnDesc), index: BlendMode::ColorBurn as u32 },
                        MenuItem { label_key: TrKey::MenuLinearBurn, description_key: Some(TrKey::MenuLinearBurnDesc), index: BlendMode::LinearBurn as u32 },
                        MenuItem { label_key: TrKey::MenuAdd, description_key: Some(TrKey::MenuAddDesc), index: BlendMode::Add as u32 },
                        MenuItem { label_key: TrKey::MenuScreen, description_key: Some(TrKey::MenuScreenDesc), index: BlendMode::Screen as u32 },
                        MenuItem { label_key: TrKey::MenuColorDodge, description_key: Some(TrKey::MenuColorDodgeDesc), index: BlendMode::ColorDodge as u32 },
                        MenuItem { label_key: TrKey::MenuLinearDodge, description_key: Some(TrKey::MenuLinearDodgeDesc), index: BlendMode::LinearDodge as u32 },
                        MenuItem { label_key: TrKey::MenuOverlay, description_key: Some(TrKey::MenuOverlayDesc), index: BlendMode::Overlay as u32 },
                        MenuItem { label_key: TrKey::MenuSoftLight, description_key: Some(TrKey::MenuSoftLightDesc), index: BlendMode::SoftLight as u32 },
                        MenuItem { label_key: TrKey::MenuLinearLight, description_key: Some(TrKey::MenuLinearLightDesc), index: BlendMode::LinearLight as u32 },
                        MenuItem { label_key: TrKey::MenuHardMix, description_key: Some(TrKey::MenuHardMixDesc), index: BlendMode::HardMix as u32 },
                        MenuItem { label_key: TrKey::MenuDifference, description_key: Some(TrKey::MenuDifferenceDesc), index: BlendMode::Difference as u32 },
                        MenuItem { label_key: TrKey::MenuExclusion, description_key: Some(TrKey::MenuExclusionDesc), index: BlendMode::Exclusion as u32 },
                        MenuItem { label_key: TrKey::MenuSubtract, description_key: Some(TrKey::MenuSubtractDesc), index: BlendMode::Subtract as u32 },
                        MenuItem { label_key: TrKey::MenuDivide, description_key: Some(TrKey::MenuDivideDesc), index: BlendMode::Divide as u32 },
                        MenuItem { label_key: TrKey::MenuStencilAlpha, description_key: Some(TrKey::MenuStencilAlphaDesc), index: BlendMode::StencilAlpha as u32 },
                        MenuItem { label_key: TrKey::MenuStencilLuma, description_key: Some(TrKey::MenuStencilLumaDesc), index: BlendMode::StencilLuma as u32 },
                        MenuItem { label_key: TrKey::MenuOutlineAlpha, description_key: Some(TrKey::MenuOutlineAlphaDesc), index: BlendMode::OutlineAlpha as u32 },
                        MenuItem { label_key: TrKey::MenuOutlineLuma, description_key: Some(TrKey::MenuOutlineLumaDesc), index: BlendMode::OutlineLuma as u32 },
                    ],
                },
                id: setting_id::BLEND_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamGradientSettings,
                description_key: Some(TrKey::ParamGradientSettingsDesc),
                kind: SettingKind::Group {
                    children: vec![
                        SettingDescriptor {
                            label_key: TrKey::ParamStartPointX,
                            description_key: Some(TrKey::ParamStartPointXDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_X,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamStartPointY,
                            description_key: Some(TrKey::ParamStartPointYDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_Y,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamStartColorRed,
                            description_key: Some(TrKey::ParamStartColorRedDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_R,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamStartColorGreen,
                            description_key: Some(TrKey::ParamStartColorGreenDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_G,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamStartColorBlue,
                            description_key: Some(TrKey::ParamStartColorBlueDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_B,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamStartColorAlpha,
                            description_key: Some(TrKey::ParamStartColorAlphaDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_A,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamEndPointX,
                            description_key: Some(TrKey::ParamEndPointXDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_X,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamEndPointY,
                            description_key: Some(TrKey::ParamEndPointYDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_Y,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamEndColorRed,
                            description_key: Some(TrKey::ParamEndColorRedDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_R,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamEndColorGreen,
                            description_key: Some(TrKey::ParamEndColorGreenDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_G,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamEndColorBlue,
                            description_key: Some(TrKey::ParamEndColorBlueDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_B,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamEndColorAlpha,
                            description_key: Some(TrKey::ParamEndColorAlphaDesc),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_A,
                        },
                    ],
                },
                id: setting_id::GRADIENT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamUseSharpCorners,
                description_key: Some(TrKey::ParamUseSharpCornersDesc),
                kind: SettingKind::Boolean,
                id: setting_id::USE_SHARP_CORNERS,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
