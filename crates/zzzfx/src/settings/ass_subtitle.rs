use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// Blend mode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum AssBlendMode {
    Normal = 0,
    Add,
    Screen,
    Multiply,
    Overlay,
}
impl SettingsEnum for AssBlendMode {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct AssSubtitle {
    pub file_path: String,
    pub file_data: String,
    pub time_offset_s: f32,
    pub scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub blend_mode: AssBlendMode,
    pub font_scale_x: f32,
    pub font_scale_y: f32,
    pub use_native_size: bool,
}

impl Default for AssSubtitle {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            file_data: String::new(),
            time_offset_s: 0.0,
            scale: 1.0,
            position_x: 0.5,
            position_y: 0.5,
            blend_mode: AssBlendMode::Normal,
            font_scale_x: 1.0,
            font_scale_y: 1.0,
            use_native_size: true,
        }
    }
}

// ---------------------------------------------------------------------------
// FullSettings struct (manual — derive macro doesn't support String)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct AssSubtitleFullSettings {
    pub file_path: String,
    pub file_data: String,
    pub time_offset_s: f32,
    pub scale: f32,
    pub position_x: f32,
    pub position_y: f32,
    pub blend_mode: AssBlendMode,
    pub font_scale_x: f32,
    pub font_scale_y: f32,
    pub use_native_size: bool,
}

impl Default for AssSubtitleFullSettings {
    fn default() -> Self { Self::from(AssSubtitle::default()) }
}
impl From<&AssSubtitle> for AssSubtitleFullSettings {
    fn from(v: &AssSubtitle) -> Self {
        Self { file_path: v.file_path.clone(), file_data: v.file_data.clone(), time_offset_s: v.time_offset_s, scale: v.scale, position_x: v.position_x, position_y: v.position_y, blend_mode: v.blend_mode, font_scale_x: v.font_scale_x, font_scale_y: v.font_scale_y, use_native_size: v.use_native_size }
    }
}
impl From<AssSubtitle> for AssSubtitleFullSettings {
    fn from(v: AssSubtitle) -> Self {
        Self { file_path: v.file_path, file_data: v.file_data, time_offset_s: v.time_offset_s, scale: v.scale, position_x: v.position_x, position_y: v.position_y, blend_mode: v.blend_mode, font_scale_x: v.font_scale_x, font_scale_y: v.font_scale_y, use_native_size: v.use_native_size }
    }
}
impl From<&AssSubtitleFullSettings> for AssSubtitle {
    fn from(v: &AssSubtitleFullSettings) -> Self {
        Self { file_path: v.file_path.clone(), file_data: v.file_data.clone(), time_offset_s: v.time_offset_s, scale: v.scale, position_x: v.position_x, position_y: v.position_y, blend_mode: v.blend_mode, font_scale_x: v.font_scale_x, font_scale_y: v.font_scale_y, use_native_size: v.use_native_size }
    }
}
impl From<AssSubtitleFullSettings> for AssSubtitle {
    fn from(v: AssSubtitleFullSettings) -> Self {
        Self { file_path: v.file_path, file_data: v.file_data, time_offset_s: v.time_offset_s, scale: v.scale, position_x: v.position_x, position_y: v.position_y, blend_mode: v.blend_mode, font_scale_x: v.font_scale_x, font_scale_y: v.font_scale_y, use_native_size: v.use_native_size }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::AssSubtitleFullSettings;
    type SID = SettingID<AssSubtitleFullSettings>;

    pub const FILE_PATH:     SID = setting_id!("file_path", file_path);
    pub const FILE_DATA:     SID = setting_id!("file_data", file_data);
    pub const TIME_OFFSET_S: SID = setting_id!("time_offset_s", time_offset_s);
    pub const SCALE:         SID = setting_id!("scale", scale);
    pub const POSITION_X:    SID = setting_id!("position_x", position_x);
    pub const POSITION_Y:    SID = setting_id!("position_y", position_y);
    pub const BLEND_MODE:    SID = setting_id!("blend_mode", blend_mode);
    pub const FONT_SCALE_X:  SID = setting_id!("font_scale_x", font_scale_x);
    pub const FONT_SCALE_Y:  SID = setting_id!("font_scale_y", font_scale_y);
    pub const USE_NATIVE_SIZE: SID = setting_id!("use_native_size", use_native_size);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for AssSubtitleFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::NativeFilePath,
                description_key: None,
                kind: SettingKind::String { secret: true, multiline: false, animates: false },
                id: setting_id::FILE_PATH,
            },
            SettingDescriptor {
                label_key: TrKey::NativeFilePath,
                description_key: None,
                kind: SettingKind::String { secret: true, multiline: false, animates: false },
                id: setting_id::FILE_DATA,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssTimeOffsetS,
                description_key: Some(TrKey::ParamAssTimeOffsetSDesc),
                kind: SettingKind::FloatRange {
                    range: -3600.0..=3600.0,
                    logarithmic: false,
                },
                id: setting_id::TIME_OFFSET_S,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssScale,
                description_key: Some(TrKey::ParamAssScaleDesc),
                kind: SettingKind::FloatRange {
                    range: 0.01..=5.0,
                    logarithmic: false,
                },
                id: setting_id::SCALE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssPositionX,
                description_key: Some(TrKey::ParamAssPositionXDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssPositionY,
                description_key: Some(TrKey::ParamAssPositionYDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssBlendMode,
                description_key: Some(TrKey::ParamAssBlendModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuNormal, description_key: Some(TrKey::MenuAssBlendNormalDesc), index: AssBlendMode::Normal as u32 },
                        MenuItem { label_key: TrKey::MenuAdd, description_key: Some(TrKey::MenuAssBlendAddDesc), index: AssBlendMode::Add as u32 },
                        MenuItem { label_key: TrKey::MenuScreen, description_key: Some(TrKey::MenuAssBlendScreenDesc), index: AssBlendMode::Screen as u32 },
                        MenuItem { label_key: TrKey::MenuMultiply, description_key: Some(TrKey::MenuAssBlendMultiplyDesc), index: AssBlendMode::Multiply as u32 },
                        MenuItem { label_key: TrKey::MenuOverlay, description_key: Some(TrKey::MenuAssBlendOverlayDesc), index: AssBlendMode::Overlay as u32 },
                    ],
                },
                id: setting_id::BLEND_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssFontScaleX,
                description_key: Some(TrKey::ParamAssFontScaleXDesc),
                kind: SettingKind::FloatRange {
                    range: 0.01..=5.0,
                    logarithmic: false,
                },
                id: setting_id::FONT_SCALE_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssFontScaleY,
                description_key: Some(TrKey::ParamAssFontScaleYDesc),
                kind: SettingKind::FloatRange {
                    range: 0.01..=5.0,
                    logarithmic: false,
                },
                id: setting_id::FONT_SCALE_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAssUseNativeSize,
                description_key: Some(TrKey::ParamAssUseNativeSizeDesc),
                kind: SettingKind::Boolean,
                id: setting_id::USE_NATIVE_SIZE,
            },
        ]
        .into_boxed_slice()
    }

}
