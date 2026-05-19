use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::zzz_stroke::BlendMode;
use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};

// ---------------------------------------------------------------------------
// Layer order enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum LayerOrder {
    Above = 0,
    Below,
}
impl SettingsEnum for LayerOrder {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ZzzRepeater {
    pub time_offset: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub layer_order: LayerOrder,
    pub max_layers: f32,
    pub blend_mode: BlendMode,
}

impl Default for ZzzRepeater {
    fn default() -> Self {
        Self {
            time_offset: 0.0,
            position_x: 0.5,
            position_y: 0.5,
            rotation: 0.0,
            layer_order: LayerOrder::Above,
            max_layers: 0.0,
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
    use super::ZzzRepeaterFullSettings;
    type SID = SettingID<ZzzRepeaterFullSettings>;

    pub const TIME_OFFSET: SID = setting_id!("time_offset", time_offset);
    pub const POSITION_X:  SID = setting_id!("position_x", position_x);
    pub const POSITION_Y:  SID = setting_id!("position_y", position_y);
    pub const ROTATION:    SID = setting_id!("rotation", rotation);
    pub const LAYER_ORDER: SID = setting_id!("layer_order", layer_order);
    pub const MAX_LAYERS:  SID = setting_id!("max_layers", max_layers);
    pub const BLEND_MODE:  SID = setting_id!("blend_mode", blend_mode);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ZzzRepeaterFullSettings {
    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label: "Time Offset",
                description: Some("Time offset in seconds. Keyframes on this parameter trigger repeat layers. Output time = max(0, currentTime - value)."),
                kind: SettingKind::FloatRange {
                    range: 0.0..=20.0,
                    logarithmic: false,
                },
                id: setting_id::TIME_OFFSET,
            },
            SettingDescriptor {
                label: "Position X",
                description: Some("X coordinate of the repeat layer position (0 = left, 1 = right)."),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label: "Position Y",
                description: Some("Y coordinate of the repeat layer position (0 = top, 1 = bottom)."),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label: "Rotation",
                description: Some("Rotation of the repeat layer in degrees around the image center."),
                kind: SettingKind::FloatRange {
                    range: 0.0..=360.0,
                    logarithmic: false,
                },
                id: setting_id::ROTATION,
            },
            SettingDescriptor {
                label: "Layer Order",
                description: Some("Whether new repeat layers appear above or below existing content."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label: "Above",
                            description: Some("New repeat layers are composited on top of existing content."),
                            index: LayerOrder::Above as u32,
                        },
                        MenuItem {
                            label: "Below",
                            description: Some("New repeat layers are composited beneath existing content."),
                            index: LayerOrder::Below as u32,
                        },
                    ],
                },
                id: setting_id::LAYER_ORDER,
            },
            SettingDescriptor {
                label: "Max Layers",
                description: Some("Maximum number of layers (including the original). 0 = unlimited. Older layers are discarded first when the limit is exceeded."),
                kind: SettingKind::FloatRange {
                    range: 0.0..=999.0,
                    logarithmic: false,
                },
                id: setting_id::MAX_LAYERS,
            },
            SettingDescriptor {
                label: "Blend Mode",
                description: Some("How repeat layers are composited with each other."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label: "Normal", description: Some("Standard alpha blending."), index: BlendMode::Normal as u32 },
                        MenuItem { label: "Dissolve", description: Some("Random dithering based on alpha."), index: BlendMode::Dissolve as u32 },
                        MenuItem { label: "Darken", description: Some("Keeps the darker of the two layers."), index: BlendMode::Darken as u32 },
                        MenuItem { label: "Multiply", description: Some("Multiplies the two layers."), index: BlendMode::Multiply as u32 },
                        MenuItem { label: "Color Burn", description: Some("Darkens base to reflect blend layer."), index: BlendMode::ColorBurn as u32 },
                        MenuItem { label: "Linear Burn", description: Some("Linear darkening of base."), index: BlendMode::LinearBurn as u32 },
                        MenuItem { label: "Add", description: Some("Adds layer values together."), index: BlendMode::Add as u32 },
                        MenuItem { label: "Screen", description: Some("Inverse multiply, lightens."), index: BlendMode::Screen as u32 },
                        MenuItem { label: "Color Dodge", description: Some("Brightens base to reflect blend layer."), index: BlendMode::ColorDodge as u32 },
                        MenuItem { label: "Linear Dodge", description: Some("Linear brightening (same as Add)."), index: BlendMode::LinearDodge as u32 },
                        MenuItem { label: "Overlay", description: Some("Combines Multiply and Screen."), index: BlendMode::Overlay as u32 },
                        MenuItem { label: "Soft Light", description: Some("Subtle contrast blend."), index: BlendMode::SoftLight as u32 },
                        MenuItem { label: "Linear Light", description: Some("Linear contrast blend."), index: BlendMode::LinearLight as u32 },
                        MenuItem { label: "Hard Mix", description: Some("High-contrast threshold blend."), index: BlendMode::HardMix as u32 },
                        MenuItem { label: "Difference", description: Some("Absolute difference between layers."), index: BlendMode::Difference as u32 },
                        MenuItem { label: "Exclusion", description: Some("Lower-contrast difference."), index: BlendMode::Exclusion as u32 },
                        MenuItem { label: "Subtract", description: Some("Subtracts blend layer from base."), index: BlendMode::Subtract as u32 },
                        MenuItem { label: "Divide", description: Some("Divides base by blend layer."), index: BlendMode::Divide as u32 },
                        MenuItem { label: "Stencil Alpha", description: Some("Uses layer alpha as a stencil."), index: BlendMode::StencilAlpha as u32 },
                        MenuItem { label: "Stencil Luma", description: Some("Uses layer luminance as a stencil."), index: BlendMode::StencilLuma as u32 },
                        MenuItem { label: "Outline Alpha", description: Some("Replaces image with layer, preserving alpha."), index: BlendMode::OutlineAlpha as u32 },
                        MenuItem { label: "Outline Luma", description: Some("Replaces image with layer, using luminescence."), index: BlendMode::OutlineLuma as u32 },
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
