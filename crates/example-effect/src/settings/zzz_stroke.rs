use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsBlock, SettingsEnum};

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
pub struct ZzzStroke {
    pub stroke_position: StrokePosition,
    pub fill_mode: FillMode,
    pub stroke_width: f32,
    pub stroke_color_r: f32,
    pub stroke_color_g: f32,
    pub stroke_color_b: f32,
    pub stroke_color_a: f32,
    pub alpha_threshold: f32,
    pub stroke_feathering: f32,
    pub source_opacity: f32,
    pub blend_mode: BlendMode,
    #[settings_block]
    pub gradient: Option<GradientSettings>,
    pub use_sharp_corners: bool,
}

impl Default for ZzzStroke {
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
            stroke_feathering: 0.01,
            source_opacity: 1.0,
            blend_mode: BlendMode::Normal,
            gradient: None,
            use_sharp_corners: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs (200–224 range, avoids collisions with 0–8 and 100–104)
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::ZzzStrokeFullSettings;
    type SID = SettingID<ZzzStrokeFullSettings>;

    pub const STROKE_POSITION:         SID = setting_id!(200, "stroke_position", stroke_position);
    pub const FILL_MODE:               SID = setting_id!(201, "fill_mode", fill_mode);
    pub const STROKE_WIDTH:            SID = setting_id!(202, "stroke_width", stroke_width);
    pub const STROKE_COLOR_R:          SID = setting_id!(203, "stroke_color_r", stroke_color_r);
    pub const STROKE_COLOR_G:          SID = setting_id!(204, "stroke_color_g", stroke_color_g);
    pub const STROKE_COLOR_B:          SID = setting_id!(205, "stroke_color_b", stroke_color_b);
    pub const STROKE_COLOR_A:          SID = setting_id!(206, "stroke_color_a", stroke_color_a);
    pub const ALPHA_THRESHOLD:         SID = setting_id!(207, "alpha_threshold", alpha_threshold);
    pub const STROKE_FEATHERING:       SID = setting_id!(208, "stroke_feathering", stroke_feathering);
    pub const SOURCE_OPACITY:          SID = setting_id!(209, "source_opacity", source_opacity);
    pub const BLEND_MODE:              SID = setting_id!(210, "blend_mode", blend_mode);
    pub const GRADIENT:                SID = setting_id!(211, "gradient", gradient.enabled);
    pub const USE_SHARP_CORNERS:       SID = setting_id!(212, "use_sharp_corners", use_sharp_corners);
    pub const GRADIENT_START_X:        SID = setting_id!(213, "gradient_start_x", gradient.settings.start_x);
    pub const GRADIENT_START_Y:        SID = setting_id!(214, "gradient_start_y", gradient.settings.start_y);
    pub const GRADIENT_START_COLOR_R:  SID = setting_id!(215, "gradient_start_color_r", gradient.settings.start_color_r);
    pub const GRADIENT_START_COLOR_G:  SID = setting_id!(216, "gradient_start_color_g", gradient.settings.start_color_g);
    pub const GRADIENT_START_COLOR_B:  SID = setting_id!(217, "gradient_start_color_b", gradient.settings.start_color_b);
    pub const GRADIENT_START_COLOR_A:  SID = setting_id!(218, "gradient_start_color_a", gradient.settings.start_color_a);
    pub const GRADIENT_END_X:          SID = setting_id!(219, "gradient_end_x", gradient.settings.end_x);
    pub const GRADIENT_END_Y:          SID = setting_id!(220, "gradient_end_y", gradient.settings.end_y);
    pub const GRADIENT_END_COLOR_R:    SID = setting_id!(221, "gradient_end_color_r", gradient.settings.end_color_r);
    pub const GRADIENT_END_COLOR_G:    SID = setting_id!(222, "gradient_end_color_g", gradient.settings.end_color_g);
    pub const GRADIENT_END_COLOR_B:    SID = setting_id!(223, "gradient_end_color_b", gradient.settings.end_color_b);
    pub const GRADIENT_END_COLOR_A:    SID = setting_id!(224, "gradient_end_color_a", gradient.settings.end_color_a);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ZzzStrokeFullSettings {
    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label: "Stroke Position",
                description: Some("Where the stroke is drawn relative to the alpha boundary."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label: "Outer",
                            description: Some("Stroke is drawn outside the shape."),
                            index: StrokePosition::Outer as u32,
                        },
                        MenuItem {
                            label: "Inner",
                            description: Some("Stroke is drawn inside the shape."),
                            index: StrokePosition::Inner as u32,
                        },
                        MenuItem {
                            label: "Center",
                            description: Some("Stroke is centered on the alpha boundary."),
                            index: StrokePosition::Center as u32,
                        },
                    ],
                },
                id: setting_id::STROKE_POSITION,
            },
            SettingDescriptor {
                label: "Fill Mode",
                description: Some("How the stroke color is determined."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label: "Solid Color",
                            description: Some("Uniform stroke color."),
                            index: FillMode::SolidColor as u32,
                        },
                        MenuItem {
                            label: "Distance Gradient",
                            description: Some("Gradient based on distance from start point."),
                            index: FillMode::DistanceGradient as u32,
                        },
                        MenuItem {
                            label: "Gradient",
                            description: Some("Linear gradient from start to end point."),
                            index: FillMode::Gradient as u32,
                        },
                        MenuItem {
                            label: "Source Color Extension",
                            description: Some("Stroke uses the color of the nearest edge pixel."),
                            index: FillMode::SourceColorExtension as u32,
                        },
                    ],
                },
                id: setting_id::FILL_MODE,
            },
            SettingDescriptor {
                label: "Stroke Width",
                description: Some("Width of the stroke, normalized to the larger image dimension."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_WIDTH,
            },
            SettingDescriptor {
                label: "Stroke Color Red",
                description: Some("Red component of the stroke color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_R,
            },
            SettingDescriptor {
                label: "Stroke Color Green",
                description: Some("Green component of the stroke color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_G,
            },
            SettingDescriptor {
                label: "Stroke Color Blue",
                description: Some("Blue component of the stroke color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_B,
            },
            SettingDescriptor {
                label: "Stroke Color Alpha",
                description: Some("Alpha component of the stroke color."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_COLOR_A,
            },
            SettingDescriptor {
                label: "Alpha Threshold",
                description: Some("Alpha value above which pixels are considered inside the shape."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::ALPHA_THRESHOLD,
            },
            SettingDescriptor {
                label: "Stroke Feathering",
                description: Some("Softens the stroke edges. Higher values produce softer transitions."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::STROKE_FEATHERING,
            },
            SettingDescriptor {
                label: "Source Opacity",
                description: Some("Opacity of the source image. 0 = fully transparent, 1 = fully opaque."),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SOURCE_OPACITY,
            },
            SettingDescriptor {
                label: "Blend Mode",
                description: Some("How the stroke is composited with the source image."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label: "Normal", description: Some("Standard alpha blending."), index: BlendMode::Normal as u32 },
                        MenuItem { label: "Dissolve", description: Some("Random dithering based on alpha."), index: BlendMode::Dissolve as u32 },
                        MenuItem { label: "Darken", description: Some("Keeps the darker of stroke and source."), index: BlendMode::Darken as u32 },
                        MenuItem { label: "Multiply", description: Some("Multiplies stroke and source."), index: BlendMode::Multiply as u32 },
                        MenuItem { label: "Color Burn", description: Some("Darkens source to reflect stroke."), index: BlendMode::ColorBurn as u32 },
                        MenuItem { label: "Linear Burn", description: Some("Linear darkening of source."), index: BlendMode::LinearBurn as u32 },
                        MenuItem { label: "Add", description: Some("Adds stroke and source values."), index: BlendMode::Add as u32 },
                        MenuItem { label: "Screen", description: Some("Inverse multiply, lightens."), index: BlendMode::Screen as u32 },
                        MenuItem { label: "Color Dodge", description: Some("Brightens source to reflect stroke."), index: BlendMode::ColorDodge as u32 },
                        MenuItem { label: "Linear Dodge", description: Some("Linear brightening (same as Add)."), index: BlendMode::LinearDodge as u32 },
                        MenuItem { label: "Overlay", description: Some("Combines Multiply and Screen."), index: BlendMode::Overlay as u32 },
                        MenuItem { label: "Soft Light", description: Some("Subtle contrast blend."), index: BlendMode::SoftLight as u32 },
                        MenuItem { label: "Linear Light", description: Some("Linear contrast blend."), index: BlendMode::LinearLight as u32 },
                        MenuItem { label: "Hard Mix", description: Some("High-contrast threshold blend."), index: BlendMode::HardMix as u32 },
                        MenuItem { label: "Difference", description: Some("Absolute difference between stroke and source."), index: BlendMode::Difference as u32 },
                        MenuItem { label: "Exclusion", description: Some("Lower-contrast difference."), index: BlendMode::Exclusion as u32 },
                        MenuItem { label: "Subtract", description: Some("Subtracts stroke from source."), index: BlendMode::Subtract as u32 },
                        MenuItem { label: "Divide", description: Some("Divides source by stroke."), index: BlendMode::Divide as u32 },
                        MenuItem { label: "Stencil Alpha", description: Some("Uses stroke alpha as a stencil."), index: BlendMode::StencilAlpha as u32 },
                        MenuItem { label: "Stencil Luma", description: Some("Uses stroke luminance as a stencil."), index: BlendMode::StencilLuma as u32 },
                        MenuItem { label: "Outline Alpha", description: Some("Replaces image with stroke, preserving alpha."), index: BlendMode::OutlineAlpha as u32 },
                        MenuItem { label: "Outline Luma", description: Some("Replaces image with stroke, using luminescence."), index: BlendMode::OutlineLuma as u32 },
                    ],
                },
                id: setting_id::BLEND_MODE,
            },
            SettingDescriptor {
                label: "Gradient Settings",
                description: Some("Gradient parameters used when Fill Mode is set to a gradient option."),
                kind: SettingKind::Group {
                    children: vec![
                        SettingDescriptor {
                            label: "Start Point X",
                            description: Some("X coordinate of the gradient start point."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_X,
                        },
                        SettingDescriptor {
                            label: "Start Point Y",
                            description: Some("Y coordinate of the gradient start point."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_Y,
                        },
                        SettingDescriptor {
                            label: "Start Color Red",
                            description: Some("Red component of the gradient start color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_R,
                        },
                        SettingDescriptor {
                            label: "Start Color Green",
                            description: Some("Green component of the gradient start color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_G,
                        },
                        SettingDescriptor {
                            label: "Start Color Blue",
                            description: Some("Blue component of the gradient start color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_B,
                        },
                        SettingDescriptor {
                            label: "Start Color Alpha",
                            description: Some("Alpha component of the gradient start color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_START_COLOR_A,
                        },
                        SettingDescriptor {
                            label: "End Point X",
                            description: Some("X coordinate of the gradient end point."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_X,
                        },
                        SettingDescriptor {
                            label: "End Point Y",
                            description: Some("Y coordinate of the gradient end point."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_Y,
                        },
                        SettingDescriptor {
                            label: "End Color Red",
                            description: Some("Red component of the gradient end color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_R,
                        },
                        SettingDescriptor {
                            label: "End Color Green",
                            description: Some("Green component of the gradient end color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_G,
                        },
                        SettingDescriptor {
                            label: "End Color Blue",
                            description: Some("Blue component of the gradient end color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_B,
                        },
                        SettingDescriptor {
                            label: "End Color Alpha",
                            description: Some("Alpha component of the gradient end color."),
                            kind: SettingKind::Percentage { logarithmic: false },
                            id: setting_id::GRADIENT_END_COLOR_A,
                        },
                    ],
                },
                id: setting_id::GRADIENT,
            },
            SettingDescriptor {
                label: "Use Sharp Corners",
                description: Some("When enabled, stroke corners are sharp (square) instead of rounded."),
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
