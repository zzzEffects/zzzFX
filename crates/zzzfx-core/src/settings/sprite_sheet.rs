use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use effect_settings::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum, TrKey};

// ---------------------------------------------------------------------------
// Reading direction enum (8 options, matching C++ reference)
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ReadingDirection {
    HForward = 0,
    HBackward,
    VForward,
    VBackward,
    HForwardS,
    HBackwardS,
    VForwardS,
    VBackwardS,
}
impl SettingsEnum for ReadingDirection {}

// ---------------------------------------------------------------------------
// Playback mode enum (3 options, matching C++ reference)
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum PlaybackMode {
    Normal = 0,
    NormalReverse,
    NormalReverseMerge,
}
impl SettingsEnum for PlaybackMode {}

// ---------------------------------------------------------------------------
// Scale algorithm enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ScaleAlgorithm {
    Nearest = 0,
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}
impl SettingsEnum for ScaleAlgorithm {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(FullSettings, Clone, Debug, PartialEq)]
pub struct ZzzSpriteSheet {
    pub sprite_columns: i32,
    pub sprite_rows: i32,
    pub sprite_range_start: i32,
    pub sprite_range_end: i32,
    pub frame_offset: f32,
    pub play_count: i32,
    pub speed: f32,
    pub reading_direction: ReadingDirection,
    pub playback_mode: PlaybackMode,
    pub loop_offset: f32,
    pub repeat_range_start: i32,
    pub repeat_range_end: i32,
    pub repeat_count: i32,
    pub sprites_cut_x: i32,
    pub sprites_cut_y: i32,
    pub scale: f32,
    pub scale_algorithm: ScaleAlgorithm,
    pub displacement_x: f32,
    pub displacement_y: f32,
    pub rotation: f32,
    pub displacement_pixel_based: bool,
    pub rotation_pixel_based: bool,
    pub selection_mode: bool,
    pub fit_sprite_sheet_to_output: bool,
    pub show_grid_overlay: bool,
}

impl Default for ZzzSpriteSheet {
    fn default() -> Self {
        Self {
            sprite_columns: 1,
            sprite_rows: 1,
            sprite_range_start: 0,
            sprite_range_end: 0,
            frame_offset: 0.0,
            play_count: 0,
            speed: 1.0,
            reading_direction: ReadingDirection::HForward,
            playback_mode: PlaybackMode::Normal,
            loop_offset: 0.0,
            repeat_range_start: 0,
            repeat_range_end: 0,
            repeat_count: 0,
            sprites_cut_x: 1,
            sprites_cut_y: 1,
            scale: 1.0,
            scale_algorithm: ScaleAlgorithm::Nearest,
            displacement_x: 0.5,
            displacement_y: 0.5,
            rotation: 0.0,
            displacement_pixel_based: true,
            rotation_pixel_based: true,
            selection_mode: false,
            fit_sprite_sheet_to_output: true,
            show_grid_overlay: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::ZzzSpriteSheetFullSettings;
    type SID = SettingID<ZzzSpriteSheetFullSettings>;

    pub const SPRITE_COLUMNS:     SID = setting_id!("sprite_columns", sprite_columns);
    pub const SPRITE_ROWS:        SID = setting_id!("sprite_rows", sprite_rows);
    pub const SPRITE_RANGE_START: SID = setting_id!("sprite_range_start", sprite_range_start);
    pub const SPRITE_RANGE_END:   SID = setting_id!("sprite_range_end", sprite_range_end);
    pub const FRAME_OFFSET:       SID = setting_id!("frame_offset", frame_offset);
    pub const PLAY_COUNT:          SID = setting_id!("play_count", play_count);
    pub const SPEED:               SID = setting_id!("speed", speed);
    pub const READING_DIRECTION:  SID = setting_id!("reading_direction", reading_direction);
    pub const PLAYBACK_MODE:      SID = setting_id!("playback_mode", playback_mode);
    pub const LOOP_OFFSET:        SID = setting_id!("loop_offset", loop_offset);
    pub const REPEAT_RANGE_START: SID = setting_id!("repeat_range_start", repeat_range_start);
    pub const REPEAT_RANGE_END:   SID = setting_id!("repeat_range_end", repeat_range_end);
    pub const REPEAT_COUNT:       SID = setting_id!("repeat_count", repeat_count);
    pub const SPRITES_CUT_X:      SID = setting_id!("sprites_cut_x", sprites_cut_x);
    pub const SPRITES_CUT_Y:      SID = setting_id!("sprites_cut_y", sprites_cut_y);
    pub const SCALE:                       SID = setting_id!("scale", scale);
    pub const SCALE_ALGORITHM:             SID = setting_id!("scale_algorithm", scale_algorithm);
    pub const DISPLACEMENT_X:              SID = setting_id!("displacement_x", displacement_x);
    pub const DISPLACEMENT_Y:              SID = setting_id!("displacement_y", displacement_y);
    pub const ROTATION:                    SID = setting_id!("rotation", rotation);
    pub const DISPLACEMENT_PIXEL_BASED:    SID = setting_id!("displacement_pixel_based", displacement_pixel_based);
    pub const ROTATION_PIXEL_BASED:        SID = setting_id!("rotation_pixel_based", rotation_pixel_based);
    pub const SELECTION_MODE:              SID = setting_id!("selection_mode", selection_mode);
    pub const FIT_SPRITE_SHEET_TO_OUTPUT:   SID = setting_id!("fit_sprite_sheet_to_output", fit_sprite_sheet_to_output);
    pub const SHOW_GRID_OVERLAY:            SID = setting_id!("show_grid_overlay", show_grid_overlay);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ZzzSpriteSheetFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label_key: TrKey::ParamColumns,
                description_key: Some(TrKey::ParamColumnsDesc),
                kind: SettingKind::IntRange { range: 1..=1000 },
                id: setting_id::SPRITE_COLUMNS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamRows,
                description_key: Some(TrKey::ParamRowsDesc),
                kind: SettingKind::IntRange { range: 1..=1000 },
                id: setting_id::SPRITE_ROWS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpriteSelectionMode,
                description_key: Some(TrKey::ParamSpriteSelectionModeDesc),
                kind: SettingKind::Group {
                    children: vec![
                        SettingDescriptor {
                            label_key: TrKey::ParamSpriteFitToOutput,
                            description_key: Some(TrKey::ParamSpriteFitToOutputDesc),
                            kind: SettingKind::Boolean,
                            id: setting_id::FIT_SPRITE_SHEET_TO_OUTPUT,
                        },
                        SettingDescriptor {
                            label_key: TrKey::ParamSpriteShowGridOverlay,
                            description_key: Some(TrKey::ParamSpriteShowGridOverlayDesc),
                            kind: SettingKind::Boolean,
                            id: setting_id::SHOW_GRID_OVERLAY,
                        },
                    ],
                },
                id: setting_id::SELECTION_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpriteRangeStart,
                description_key: Some(TrKey::ParamSpriteRangeStartDesc),
                kind: SettingKind::IntRange { range: 0..=1000 },
                id: setting_id::SPRITE_RANGE_START,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpriteRangeEnd,
                description_key: Some(TrKey::ParamSpriteRangeEndDesc),
                kind: SettingKind::IntRange { range: 0..=1000 },
                id: setting_id::SPRITE_RANGE_END,
            },
            SettingDescriptor {
                label_key: TrKey::ParamFrameOffset,
                description_key: Some(TrKey::ParamFrameOffsetDesc),
                kind: SettingKind::FloatRange {
                    range: -100.0..=100.0,
                    logarithmic: false,
                },
                id: setting_id::FRAME_OFFSET,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpritePlayCount,
                description_key: Some(TrKey::ParamSpritePlayCountDesc),
                kind: SettingKind::IntRange { range: -100..=100 },
                id: setting_id::PLAY_COUNT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpeed,
                description_key: Some(TrKey::ParamSpeedDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1000.0,
                    logarithmic: false,
                },
                id: setting_id::SPEED,
            },
            SettingDescriptor {
                label_key: TrKey::ParamReadingDirection,
                description_key: Some(TrKey::ParamReadingDirectionDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuHForward, description_key: Some(TrKey::MenuHForwardDesc), index: ReadingDirection::HForward as u32 },
                        MenuItem { label_key: TrKey::MenuHBackward, description_key: Some(TrKey::MenuHBackwardDesc), index: ReadingDirection::HBackward as u32 },
                        MenuItem { label_key: TrKey::MenuVForward, description_key: Some(TrKey::MenuVForwardDesc), index: ReadingDirection::VForward as u32 },
                        MenuItem { label_key: TrKey::MenuVBackward, description_key: Some(TrKey::MenuVBackwardDesc), index: ReadingDirection::VBackward as u32 },
                        MenuItem { label_key: TrKey::MenuHForwardS, description_key: Some(TrKey::MenuHForwardSDesc), index: ReadingDirection::HForwardS as u32 },
                        MenuItem { label_key: TrKey::MenuHBackwardS, description_key: Some(TrKey::MenuHBackwardSDesc), index: ReadingDirection::HBackwardS as u32 },
                        MenuItem { label_key: TrKey::MenuVForwardS, description_key: Some(TrKey::MenuVForwardSDesc), index: ReadingDirection::VForwardS as u32 },
                        MenuItem { label_key: TrKey::MenuVBackwardS, description_key: Some(TrKey::MenuVBackwardSDesc), index: ReadingDirection::VBackwardS as u32 },
                    ],
                },
                id: setting_id::READING_DIRECTION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamPlaybackMode,
                description_key: Some(TrKey::ParamPlaybackModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuNormal, description_key: Some(TrKey::MenuPlaybackNormalDesc), index: PlaybackMode::Normal as u32 },
                        MenuItem { label_key: TrKey::MenuNormalReverse, description_key: Some(TrKey::MenuNormalReverseDesc), index: PlaybackMode::NormalReverse as u32 },
                        MenuItem { label_key: TrKey::MenuNormalReverseMerge, description_key: Some(TrKey::MenuNormalReverseMergeDesc), index: PlaybackMode::NormalReverseMerge as u32 },
                    ],
                },
                id: setting_id::PLAYBACK_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamLoopOffset,
                description_key: Some(TrKey::ParamLoopOffsetDesc),
                kind: SettingKind::FloatRange {
                    range: -100.0..=100.0,
                    logarithmic: false,
                },
                id: setting_id::LOOP_OFFSET,
            },
            SettingDescriptor {
                label_key: TrKey::ParamRepeatRangeStart,
                description_key: Some(TrKey::ParamRepeatRangeStartDesc),
                kind: SettingKind::IntRange { range: 0..=1000 },
                id: setting_id::REPEAT_RANGE_START,
            },
            SettingDescriptor {
                label_key: TrKey::ParamRepeatRangeEnd,
                description_key: Some(TrKey::ParamRepeatRangeEndDesc),
                kind: SettingKind::IntRange { range: 0..=1000 },
                id: setting_id::REPEAT_RANGE_END,
            },
            SettingDescriptor {
                label_key: TrKey::ParamRepeatCount,
                description_key: Some(TrKey::ParamRepeatCountDesc),
                kind: SettingKind::IntRange { range: 0..=1000 },
                id: setting_id::REPEAT_COUNT,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpritesCutX,
                description_key: Some(TrKey::ParamSpritesCutXDesc),
                kind: SettingKind::IntRange { range: 1..=99 },
                id: setting_id::SPRITES_CUT_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpritesCutY,
                description_key: Some(TrKey::ParamSpritesCutYDesc),
                kind: SettingKind::IntRange { range: 1..=99 },
                id: setting_id::SPRITES_CUT_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpriteDisplacementPixelBased,
                description_key: Some(TrKey::ParamSpriteDisplacementPixelBasedDesc),
                kind: SettingKind::Boolean,
                id: setting_id::DISPLACEMENT_PIXEL_BASED,
            },
            // Marker descriptors for native Double2D placement (skipped in OFX via is_native_grouped_name)
            SettingDescriptor {
                label_key: TrKey::ParamSpriteDisplacementX,
                description_key: Some(TrKey::ParamSpriteDisplacementXDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::DISPLACEMENT_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpriteDisplacementY,
                description_key: Some(TrKey::ParamSpriteDisplacementYDesc),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1.0,
                    logarithmic: false,
                },
                id: setting_id::DISPLACEMENT_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpriteRotationPixelBased,
                description_key: Some(TrKey::ParamSpriteRotationPixelBasedDesc),
                kind: SettingKind::Boolean,
                id: setting_id::ROTATION_PIXEL_BASED,
            },
            SettingDescriptor {
                label_key: TrKey::ParamSpriteRotation,
                description_key: Some(TrKey::ParamSpriteRotationDesc),
                kind: SettingKind::FloatRange {
                    range: -3600.0..=3600.0,
                    logarithmic: false,
                },
                id: setting_id::ROTATION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamScaleAlgorithm,
                description_key: Some(TrKey::ParamScaleAlgorithmDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuNearestNeighbor, description_key: Some(TrKey::MenuNearestNeighborDesc), index: ScaleAlgorithm::Nearest as u32 },
                        MenuItem { label_key: TrKey::MenuTriangle, description_key: Some(TrKey::MenuTriangleDesc), index: ScaleAlgorithm::Triangle as u32 },
                        MenuItem { label_key: TrKey::MenuCatmullRom, description_key: Some(TrKey::MenuCatmullRomDesc), index: ScaleAlgorithm::CatmullRom as u32 },
                        MenuItem { label_key: TrKey::MenuGaussian, description_key: Some(TrKey::MenuGaussianDesc), index: ScaleAlgorithm::Gaussian as u32 },
                        MenuItem { label_key: TrKey::MenuLanczos3, description_key: Some(TrKey::MenuLanczos3Desc), index: ScaleAlgorithm::Lanczos3 as u32 },
                    ],
                },
                id: setting_id::SCALE_ALGORITHM,
            },
            SettingDescriptor {
                label_key: TrKey::ParamScale,
                description_key: Some(TrKey::ParamScaleDesc),
                kind: SettingKind::FloatRange {
                    range: 0.01..=25.0,
                    logarithmic: false,
                },
                id: setting_id::SCALE,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
