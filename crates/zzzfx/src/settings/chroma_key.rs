use zzzfx_macros::FullSettings;

use super::{SettingDescriptor, SettingKind, Settings};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ChromaKey {
    pub key_color_r: f32,
    pub key_color_g: f32,
    pub key_color_b: f32,
    pub key_color_a: f32,
    pub threshold: f32,
    pub edge_softness: f32,
    pub spill_suppression: f32,
    pub edge_blur: f32,
    pub show_matte: bool,
    pub invert: bool,
}

impl Default for ChromaKey {
    fn default() -> Self {
        Self {
            key_color_r: 0.0,
            key_color_g: 1.0,
            key_color_b: 0.0,
            key_color_a: 1.0,
            threshold: 0.10,
            edge_softness: 0.10,
            spill_suppression: 0.5,
            edge_blur: 2.0,
            show_matte: false,
            invert: false,
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
    use super::ChromaKeyFullSettings;
    type SID = SettingID<ChromaKeyFullSettings>;

    pub const KEY_COLOR:         SID = setting_id!("key_color_r", key_color_r);
    pub const KEY_COLOR_R:       SID = setting_id!("key_color_r", key_color_r);
    pub const KEY_COLOR_G:       SID = setting_id!("key_color_g", key_color_g);
    pub const KEY_COLOR_B:       SID = setting_id!("key_color_b", key_color_b);
    pub const KEY_COLOR_A:       SID = setting_id!("key_color_a", key_color_a);
    pub const THRESHOLD:         SID = setting_id!("threshold", threshold);
    pub const EDGE_SOFTNESS:     SID = setting_id!("edge_softness", edge_softness);
    pub const SPILL_SUPPRESSION: SID = setting_id!("spill_suppression", spill_suppression);
    pub const EDGE_BLUR:        SID = setting_id!("edge_blur", edge_blur);
    pub const SHOW_MATTE:        SID = setting_id!("show_matte", show_matte);
    pub const INVERT:           SID = setting_id!("invert", invert);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ChromaKeyFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamChromaKeyKeyColor,
                description_key: Some(TrKey::ParamChromaKeyKeyColorDesc),
                kind: SettingKind::ColorRGBA {
                    r_id: setting_id::KEY_COLOR_R,
                    g_id: setting_id::KEY_COLOR_G,
                    b_id: setting_id::KEY_COLOR_B,
                    a_id: setting_id::KEY_COLOR_A,
                },
                id: setting_id::KEY_COLOR,
            },
            SettingDescriptor {
                label_key: TrKey::ParamChromaKeyThreshold,
                description_key: Some(TrKey::ParamChromaKeyThresholdDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::THRESHOLD,
            },
            SettingDescriptor {
                label_key: TrKey::ParamChromaKeyEdgeSoftness,
                description_key: Some(TrKey::ParamChromaKeyEdgeSoftnessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::EDGE_SOFTNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamChromaKeySpillSuppression,
                description_key: Some(TrKey::ParamChromaKeySpillSuppressionDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::SPILL_SUPPRESSION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamChromaKeyEdgeBlur,
                description_key: Some(TrKey::ParamChromaKeyEdgeBlurDesc),
                kind: SettingKind::FloatRange { range: 0.0..=20.0, logarithmic: false },
                id: setting_id::EDGE_BLUR,
            },
            SettingDescriptor {
                label_key: TrKey::ParamChromaKeyShowMatte,
                description_key: Some(TrKey::ParamChromaKeyShowMatteDesc),
                kind: SettingKind::Boolean,
                id: setting_id::SHOW_MATTE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamChromaKeyInvert,
                description_key: Some(TrKey::ParamChromaKeyInvertDesc),
                kind: SettingKind::Boolean,
                id: setting_id::INVERT,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
