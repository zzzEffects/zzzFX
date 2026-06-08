use super::{SettingDescriptor, SettingKind, Settings};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct SvgDisplay {
    pub file_path: String,
    pub file_data: String,
    pub scale: f32,
    pub fit_to_output: bool,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub preserve_aspect_ratio: bool,
    pub dpi: f32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
}

impl Default for SvgDisplay {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            file_data: String::new(),
            scale: 1.0,
            fit_to_output: true,
            position_x: 0.5,
            position_y: 0.5,
            rotation: 0.0,
            opacity: 1.0,
            preserve_aspect_ratio: true,
            dpi: 96.0,
            background_color_r: 0.0,
            background_color_g: 0.0,
            background_color_b: 0.0,
            background_color_a: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// FullSettings struct (manual — derive macro doesn't support String)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct SvgDisplayFullSettings {
    pub file_path: String,
    pub file_data: String,
    pub scale: f32,
    pub fit_to_output: bool,
    pub position_x: f32,
    pub position_y: f32,
    pub rotation: f32,
    pub opacity: f32,
    pub preserve_aspect_ratio: bool,
    pub dpi: f32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
}

impl Default for SvgDisplayFullSettings { fn default() -> Self { Self::from(SvgDisplay::default()) } }
impl From<&SvgDisplay> for SvgDisplayFullSettings {
    fn from(v: &SvgDisplay) -> Self { Self { file_path: v.file_path.clone(), file_data: v.file_data.clone(), scale: v.scale, fit_to_output: v.fit_to_output, position_x: v.position_x, position_y: v.position_y, rotation: v.rotation, opacity: v.opacity, preserve_aspect_ratio: v.preserve_aspect_ratio, dpi: v.dpi, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a } }
}
impl From<SvgDisplay> for SvgDisplayFullSettings {
    fn from(v: SvgDisplay) -> Self { Self { file_path: v.file_path, file_data: v.file_data, scale: v.scale, fit_to_output: v.fit_to_output, position_x: v.position_x, position_y: v.position_y, rotation: v.rotation, opacity: v.opacity, preserve_aspect_ratio: v.preserve_aspect_ratio, dpi: v.dpi, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a } }
}
impl From<&SvgDisplayFullSettings> for SvgDisplay {
    fn from(v: &SvgDisplayFullSettings) -> Self { Self { file_path: v.file_path.clone(), file_data: v.file_data.clone(), scale: v.scale, fit_to_output: v.fit_to_output, position_x: v.position_x, position_y: v.position_y, rotation: v.rotation, opacity: v.opacity, preserve_aspect_ratio: v.preserve_aspect_ratio, dpi: v.dpi, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a } }
}
impl From<SvgDisplayFullSettings> for SvgDisplay {
    fn from(v: SvgDisplayFullSettings) -> Self { Self { file_path: v.file_path, file_data: v.file_data, scale: v.scale, fit_to_output: v.fit_to_output, position_x: v.position_x, position_y: v.position_y, rotation: v.rotation, opacity: v.opacity, preserve_aspect_ratio: v.preserve_aspect_ratio, dpi: v.dpi, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a } }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::setting_id;
    use crate::settings::SettingID;
    use super::SvgDisplayFullSettings;
    type SID = SettingID<SvgDisplayFullSettings>;

    pub const FILE_PATH:             SID = setting_id!("file_path", file_path);
    pub const FILE_DATA:             SID = setting_id!("file_data", file_data);
    pub const SCALE:                 SID = setting_id!("scale", scale);
    pub const FIT_TO_OUTPUT:         SID = setting_id!("fit_to_output", fit_to_output);
    pub const POSITION_X:            SID = setting_id!("position_x", position_x);
    pub const POSITION_Y:            SID = setting_id!("position_y", position_y);
    pub const ROTATION:              SID = setting_id!("rotation", rotation);
    pub const OPACITY:               SID = setting_id!("opacity", opacity);
    pub const PRESERVE_ASPECT_RATIO: SID = setting_id!("preserve_aspect_ratio", preserve_aspect_ratio);
    pub const DPI:                   SID = setting_id!("dpi", dpi);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for SvgDisplayFullSettings {
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
                label_key: TrKey::ParamSvgScale,
                description_key: Some(TrKey::ParamSvgScaleDesc),
                kind: SettingKind::FloatRange { range: 0.01..=10.0, logarithmic: false },
                id: setting_id::SCALE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgFitToOutput,
                description_key: Some(TrKey::ParamSvgFitToOutputDesc),
                kind: SettingKind::Boolean,
                id: setting_id::FIT_TO_OUTPUT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgPositionX,
                description_key: Some(TrKey::ParamSvgPositionXDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgPositionY,
                description_key: Some(TrKey::ParamSvgPositionYDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::POSITION_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgRotation,
                description_key: Some(TrKey::ParamSvgRotationDesc),
                kind: SettingKind::FloatRange { range: 0.0..=360.0, logarithmic: false },
                id: setting_id::ROTATION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgOpacity,
                description_key: Some(TrKey::ParamSvgOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgPreserveAspectRatio,
                description_key: Some(TrKey::ParamSvgPreserveAspectRatioDesc),
                kind: SettingKind::Boolean,
                id: setting_id::PRESERVE_ASPECT_RATIO,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSvgDpi,
                description_key: Some(TrKey::ParamSvgDpiDesc),
                kind: SettingKind::FloatRange { range: 72.0..=600.0, logarithmic: false },
                id: setting_id::DPI,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
