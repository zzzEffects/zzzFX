use zzzfx_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::stroke::BlendMode;
use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};
use crate::i18n::TrKey;

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
pub struct Repeater {
    pub time_offset: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub layer_order: LayerOrder,
    pub max_layers: f32,
    pub blend_mode: BlendMode,
}

impl Default for Repeater {
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
    use super::RepeaterFullSettings;
    type SID = SettingID<RepeaterFullSettings>;

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

impl Settings for RepeaterFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamTimeOffset,
                description_key: Some(TrKey::ParamTimeOffsetDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=20.0,
                    logarithmic: false,
                },
                id: setting_id::TIME_OFFSET,
            },
            SettingDescriptor {
                label_key: TrKey::ParamPositionX,
                description_key: Some(TrKey::ParamPositionXDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamPositionY,
                description_key: Some(TrKey::ParamPositionYDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamRotation,
                description_key: Some(TrKey::ParamRotationDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=360.0,
                    logarithmic: false,
                },
                id: setting_id::ROTATION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLayerOrder,
                description_key: Some(TrKey::ParamLayerOrderDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuAbove,
                            description_key: Some(TrKey::MenuAboveDesc),
                            index: LayerOrder::Above as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuBelow,
                            description_key: Some(TrKey::MenuBelowDesc),
                            index: LayerOrder::Below as u32,
                        },
                    ],
                },
                id: setting_id::LAYER_ORDER,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMaxLayers,
                description_key: Some(TrKey::ParamMaxLayersDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=999.0,
                    logarithmic: false,
                },
                id: setting_id::MAX_LAYERS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamRepeaterBlendMode,
                description_key: Some(TrKey::ParamRepeaterBlendModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuNormal, description_key: Some(TrKey::MenuNormalDesc), index: BlendMode::Normal as u32 },
                        MenuItem { label_key: TrKey::MenuDissolve, description_key: Some(TrKey::MenuDissolveDesc), index: BlendMode::Dissolve as u32 },
                        MenuItem { label_key: TrKey::MenuDarken, description_key: Some(TrKey::MenuRepeaterDarkenDesc), index: BlendMode::Darken as u32 },
                        MenuItem { label_key: TrKey::MenuMultiply, description_key: Some(TrKey::MenuRepeaterMultiplyDesc), index: BlendMode::Multiply as u32 },
                        MenuItem { label_key: TrKey::MenuColorBurn, description_key: Some(TrKey::MenuRepeaterColorBurnDesc), index: BlendMode::ColorBurn as u32 },
                        MenuItem { label_key: TrKey::MenuLinearBurn, description_key: Some(TrKey::MenuRepeaterLinearBurnDesc), index: BlendMode::LinearBurn as u32 },
                        MenuItem { label_key: TrKey::MenuAdd, description_key: Some(TrKey::MenuRepeaterAddDesc), index: BlendMode::Add as u32 },
                        MenuItem { label_key: TrKey::MenuScreen, description_key: Some(TrKey::MenuRepeaterScreenDesc), index: BlendMode::Screen as u32 },
                        MenuItem { label_key: TrKey::MenuColorDodge, description_key: Some(TrKey::MenuRepeaterColorDodgeDesc), index: BlendMode::ColorDodge as u32 },
                        MenuItem { label_key: TrKey::MenuLinearDodge, description_key: Some(TrKey::MenuRepeaterLinearDodgeDesc), index: BlendMode::LinearDodge as u32 },
                        MenuItem { label_key: TrKey::MenuOverlay, description_key: Some(TrKey::MenuRepeaterOverlayDesc), index: BlendMode::Overlay as u32 },
                        MenuItem { label_key: TrKey::MenuSoftLight, description_key: Some(TrKey::MenuRepeaterSoftLightDesc), index: BlendMode::SoftLight as u32 },
                        MenuItem { label_key: TrKey::MenuLinearLight, description_key: Some(TrKey::MenuRepeaterLinearLightDesc), index: BlendMode::LinearLight as u32 },
                        MenuItem { label_key: TrKey::MenuHardMix, description_key: Some(TrKey::MenuRepeaterHardMixDesc), index: BlendMode::HardMix as u32 },
                        MenuItem { label_key: TrKey::MenuDifference, description_key: Some(TrKey::MenuRepeaterDifferenceDesc), index: BlendMode::Difference as u32 },
                        MenuItem { label_key: TrKey::MenuExclusion, description_key: Some(TrKey::MenuRepeaterExclusionDesc), index: BlendMode::Exclusion as u32 },
                        MenuItem { label_key: TrKey::MenuSubtract, description_key: Some(TrKey::MenuRepeaterSubtractDesc), index: BlendMode::Subtract as u32 },
                        MenuItem { label_key: TrKey::MenuDivide, description_key: Some(TrKey::MenuRepeaterDivideDesc), index: BlendMode::Divide as u32 },
                        MenuItem { label_key: TrKey::MenuStencilAlpha, description_key: Some(TrKey::MenuRepeaterStencilAlphaDesc), index: BlendMode::StencilAlpha as u32 },
                        MenuItem { label_key: TrKey::MenuStencilLuma, description_key: Some(TrKey::MenuRepeaterStencilLumaDesc), index: BlendMode::StencilLuma as u32 },
                        MenuItem { label_key: TrKey::MenuOutlineAlpha, description_key: Some(TrKey::MenuRepeaterOutlineAlphaDesc), index: BlendMode::OutlineAlpha as u32 },
                        MenuItem { label_key: TrKey::MenuOutlineLuma, description_key: Some(TrKey::MenuRepeaterOutlineLumaDesc), index: BlendMode::OutlineLuma as u32 },
                    ],
                },
                id: setting_id::BLEND_MODE,
            },
        ]
        .into_boxed_slice()
    }

}
