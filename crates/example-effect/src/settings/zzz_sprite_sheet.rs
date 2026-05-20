use example_effect_macros::FullSettings;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};

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
    pub frame_offset: i32,
    pub speed: f32,
    pub reading_direction: ReadingDirection,
    pub playback_mode: PlaybackMode,
    pub loop_offset: i32,
    pub repeat_range_start: i32,
    pub repeat_range_end: i32,
    pub repeat_count: i32,
    pub sprites_cut_x: i32,
    pub sprites_cut_y: i32,
    pub scale: f32,
    pub scale_algorithm: ScaleAlgorithm,
}

impl Default for ZzzSpriteSheet {
    fn default() -> Self {
        Self {
            sprite_columns: 1,
            sprite_rows: 1,
            sprite_range_start: 0,
            sprite_range_end: 0,
            frame_offset: 0,
            speed: 1.0,
            reading_direction: ReadingDirection::HForward,
            playback_mode: PlaybackMode::Normal,
            loop_offset: 0,
            repeat_range_start: 0,
            repeat_range_end: 0,
            repeat_count: 0,
            sprites_cut_x: 1,
            sprites_cut_y: 1,
            scale: 1.0,
            scale_algorithm: ScaleAlgorithm::Nearest,
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
    pub const SPEED:               SID = setting_id!("speed", speed);
    pub const READING_DIRECTION:  SID = setting_id!("reading_direction", reading_direction);
    pub const PLAYBACK_MODE:      SID = setting_id!("playback_mode", playback_mode);
    pub const LOOP_OFFSET:        SID = setting_id!("loop_offset", loop_offset);
    pub const REPEAT_RANGE_START: SID = setting_id!("repeat_range_start", repeat_range_start);
    pub const REPEAT_RANGE_END:   SID = setting_id!("repeat_range_end", repeat_range_end);
    pub const REPEAT_COUNT:       SID = setting_id!("repeat_count", repeat_count);
    pub const SPRITES_CUT_X:      SID = setting_id!("sprites_cut_x", sprites_cut_x);
    pub const SPRITES_CUT_Y:      SID = setting_id!("sprites_cut_y", sprites_cut_y);
    pub const SCALE:              SID = setting_id!("scale", scale);
    pub const SCALE_ALGORITHM:    SID = setting_id!("scale_algorithm", scale_algorithm);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for ZzzSpriteSheetFullSettings {
    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            SettingDescriptor {
                label: "Columns",
                description: Some("Number of sprite columns in the sheet. Sprite width = sheet width / columns."),
                kind: SettingKind::IntRange { range: 1..=1000 },
                id: setting_id::SPRITE_COLUMNS,
            },
            SettingDescriptor {
                label: "Rows",
                description: Some("Number of sprite rows in the sheet. Sprite height = sheet height / rows."),
                kind: SettingKind::IntRange { range: 1..=1000 },
                id: setting_id::SPRITE_ROWS,
            },
            SettingDescriptor {
                label: "Sprite Range Start",
                description: Some("Index of the first sprite in the animation."),
                kind: SettingKind::IntRange { range: 0..=9999 },
                id: setting_id::SPRITE_RANGE_START,
            },
            SettingDescriptor {
                label: "Sprite Range End",
                description: Some("Index of the last sprite in the animation."),
                kind: SettingKind::IntRange { range: 0..=9999 },
                id: setting_id::SPRITE_RANGE_END,
            },
            SettingDescriptor {
                label: "Frame Offset",
                description: Some("Output frame number for the first sprite."),
                kind: SettingKind::IntRange { range: -9999..=9999 },
                id: setting_id::FRAME_OFFSET,
            },
            SettingDescriptor {
                label: "Speed",
                description: Some("Playback speed. 0 = paused. Internally computed as speed ÷ project frame rate."),
                kind: SettingKind::FloatRange {
                    range: 0.0..=1000.0,
                    logarithmic: false,
                },
                id: setting_id::SPEED,
            },
            SettingDescriptor {
                label: "Reading Direction",
                description: Some("The reading direction of the sprites."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label: "H. Forward", description: Some("Read sprites horizontally, left to right."), index: ReadingDirection::HForward as u32 },
                        MenuItem { label: "H. Backward", description: Some("Read sprites horizontally, right to left."), index: ReadingDirection::HBackward as u32 },
                        MenuItem { label: "V. Forward", description: Some("Read sprites vertically, top to bottom."), index: ReadingDirection::VForward as u32 },
                        MenuItem { label: "V. Backward", description: Some("Read sprites vertically, bottom to top."), index: ReadingDirection::VBackward as u32 },
                        MenuItem { label: "H. Forward (S)", description: Some("Read sprites horizontally in S-shape."), index: ReadingDirection::HForwardS as u32 },
                        MenuItem { label: "H. Backward (S)", description: Some("Read sprites horizontally backward in S-shape."), index: ReadingDirection::HBackwardS as u32 },
                        MenuItem { label: "V. Forward (S)", description: Some("Read sprites vertically in S-shape."), index: ReadingDirection::VForwardS as u32 },
                        MenuItem { label: "V. Backward (S)", description: Some("Read sprites vertically backward in S-shape."), index: ReadingDirection::VBackwardS as u32 },
                    ],
                },
                id: setting_id::READING_DIRECTION,
            },
            SettingDescriptor {
                label: "Playback Mode",
                description: Some("The playback mode for the sprite animation."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label: "Normal", description: Some("Play sprites in normal order."), index: PlaybackMode::Normal as u32 },
                        MenuItem { label: "Normal & Reverse", description: Some("Play forward then backward."), index: PlaybackMode::NormalReverse as u32 },
                        MenuItem { label: "N.&R. (Merge)", description: Some("Play forward then backward, merging repeated first/last frames."), index: PlaybackMode::NormalReverseMerge as u32 },
                    ],
                },
                id: setting_id::PLAYBACK_MODE,
            },
            SettingDescriptor {
                label: "Loop Offset",
                description: Some("Frame offset for the first sprite in a single loop."),
                kind: SettingKind::IntRange { range: -9999..=9999 },
                id: setting_id::LOOP_OFFSET,
            },
            SettingDescriptor {
                label: "Repeat Range Start",
                description: Some("First sprite index in the repeat sub-range."),
                kind: SettingKind::IntRange { range: 0..=9999 },
                id: setting_id::REPEAT_RANGE_START,
            },
            SettingDescriptor {
                label: "Repeat Range End",
                description: Some("Last sprite index in the repeat sub-range."),
                kind: SettingKind::IntRange { range: 0..=9999 },
                id: setting_id::REPEAT_RANGE_END,
            },
            SettingDescriptor {
                label: "Repeat Count",
                description: Some("How many times to repeat the repeat range (0 = no repeat)."),
                kind: SettingKind::IntRange { range: 0..=9999 },
                id: setting_id::REPEAT_COUNT,
            },
            SettingDescriptor {
                label: "Sprites Cut X",
                description: Some("Number of horizontal cut blocks in the sprite sheet."),
                kind: SettingKind::IntRange { range: 1..=99 },
                id: setting_id::SPRITES_CUT_X,
            },
            SettingDescriptor {
                label: "Sprites Cut Y",
                description: Some("Number of vertical cut blocks in the sprite sheet."),
                kind: SettingKind::IntRange { range: 1..=99 },
                id: setting_id::SPRITES_CUT_Y,
            },
            SettingDescriptor {
                label: "Scale",
                description: Some("Scale factor applied to the output sprite (1.0 = original size)."),
                kind: SettingKind::FloatRange {
                    range: 0.01..=20.0,
                    logarithmic: false,
                },
                id: setting_id::SCALE,
            },
            SettingDescriptor {
                label: "Scale Algorithm",
                description: Some("Resampling algorithm used when scaling."),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label: "Nearest Neighbor", description: Some("Fastest, no interpolation."), index: ScaleAlgorithm::Nearest as u32 },
                        MenuItem { label: "Triangle", description: Some("Bilinear interpolation."), index: ScaleAlgorithm::Triangle as u32 },
                        MenuItem { label: "Catmull-Rom", description: Some("Cubic filter, sharp results."), index: ScaleAlgorithm::CatmullRom as u32 },
                        MenuItem { label: "Gaussian", description: Some("Gaussian blur filter."), index: ScaleAlgorithm::Gaussian as u32 },
                        MenuItem { label: "Lanczos3", description: Some("Highest quality, 3-lobe Lanczos."), index: ScaleAlgorithm::Lanczos3 as u32 },
                    ],
                },
                id: setting_id::SCALE_ALGORITHM,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
