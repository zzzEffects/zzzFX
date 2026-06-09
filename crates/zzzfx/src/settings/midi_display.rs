use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{MenuItem, SettingDescriptor, SettingKind, Settings, SettingsEnum};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// BPM source enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum MidiBpmSource {
    FromMidi = 0,
    UserSpecified = 1,
}
impl SettingsEnum for MidiBpmSource {}

// ---------------------------------------------------------------------------
// Orientation enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum MidiOrientation {
    Horizontal = 0,
    Vertical = 1,
}
impl SettingsEnum for MidiOrientation {}

// ---------------------------------------------------------------------------
// Note color mode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum MidiNoteColorMode {
    Solid = 0,
    Velocity = 1,
    Channel = 2,
    Track = 3,
    Pitch = 4,
}
impl SettingsEnum for MidiNoteColorMode {}

// ---------------------------------------------------------------------------
// Track filter mode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum MidiTrackFilterMode {
    AllTracks = 0,
    SpecificTrack = 1,
}
impl SettingsEnum for MidiTrackFilterMode {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct MidiDisplay {
    pub file_path: String,
    pub file_data: String,
    // Timing
    pub time_offset: f32,
    pub bpm_source: MidiBpmSource,
    pub user_bpm: f32,
    pub speed: f32,
    // Layout
    pub orientation: MidiOrientation,
    pub note_height_min: f32,
    pub key_range_min: i32,
    pub key_range_max: i32,
    pub show_keyboard: bool,
    pub keyboard_width: f32,
    // Note Appearance
    pub note_color_mode: MidiNoteColorMode,
    pub note_color_r: f32,
    pub note_color_g: f32,
    pub note_color_b: f32,
    pub note_color_a: f32,
    pub note_opacity: f32,
    pub note_border_thickness: f32,
    pub note_border_color_r: f32,
    pub note_border_color_g: f32,
    pub note_border_color_b: f32,
    pub note_border_color_a: f32,
    pub note_border_opacity: f32,
    pub note_corner_radius: f32,
    // Velocity
    pub velocity_affects_opacity: bool,
    pub velocity_affects_brightness: bool,
    pub minimum_velocity: i32,
    // Background
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
    pub background_opacity: f32,
    // Track Selection
    pub track_filter_mode: MidiTrackFilterMode,
    pub track_number: i32,
    // Playback
    pub loop_playback: bool,
    pub quantize_display: bool,
    pub show_velocity_as_height: bool,
}

impl Default for MidiDisplay {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            file_data: String::new(),
            time_offset: 0.0,
            bpm_source: MidiBpmSource::FromMidi,
            user_bpm: 120.0,
            speed: 1.0,
            orientation: MidiOrientation::Horizontal,
            note_height_min: 4.0,
            key_range_min: 0,
            key_range_max: 127,
            show_keyboard: true,
            keyboard_width: 0.08,
            note_color_mode: MidiNoteColorMode::Solid,
            note_color_r: 1.0,
            note_color_g: 1.0,
            note_color_b: 1.0,
            note_color_a: 1.0,
            note_opacity: 0.9,
            note_border_thickness: 1.0,
            note_border_color_r: 0.0,
            note_border_color_g: 0.0,
            note_border_color_b: 0.0,
            note_border_color_a: 1.0,
            note_border_opacity: 1.0,
            note_corner_radius: 2.0,
            velocity_affects_opacity: true,
            velocity_affects_brightness: false,
            minimum_velocity: 1,
            background_color_r: 0.1,
            background_color_g: 0.1,
            background_color_b: 0.1,
            background_color_a: 1.0,
            background_opacity: 1.0,
            track_filter_mode: MidiTrackFilterMode::AllTracks,
            track_number: 0,
            loop_playback: false,
            quantize_display: false,
            show_velocity_as_height: false,
        }
    }
}

// ---------------------------------------------------------------------------
// FullSettings struct (manual — derive macro doesn't support String)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct MidiDisplayFullSettings {
    pub file_path: String,
    pub file_data: String,
    pub time_offset: f32,
    pub bpm_source: MidiBpmSource,
    pub user_bpm: f32,
    pub speed: f32,
    pub orientation: MidiOrientation,
    pub note_height_min: f32,
    pub key_range_min: i32,
    pub key_range_max: i32,
    pub show_keyboard: bool,
    pub keyboard_width: f32,
    pub note_color_mode: MidiNoteColorMode,
    pub note_color_r: f32,
    pub note_color_g: f32,
    pub note_color_b: f32,
    pub note_color_a: f32,
    pub note_opacity: f32,
    pub note_border_thickness: f32,
    pub note_border_color_r: f32,
    pub note_border_color_g: f32,
    pub note_border_color_b: f32,
    pub note_border_color_a: f32,
    pub note_border_opacity: f32,
    pub note_corner_radius: f32,
    pub velocity_affects_opacity: bool,
    pub velocity_affects_brightness: bool,
    pub minimum_velocity: i32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub background_color_a: f32,
    pub background_opacity: f32,
    pub track_filter_mode: MidiTrackFilterMode,
    pub track_number: i32,
    pub loop_playback: bool,
    pub quantize_display: bool,
    pub show_velocity_as_height: bool,
}

impl Default for MidiDisplayFullSettings { fn default() -> Self { Self::from(MidiDisplay::default()) } }
impl From<&MidiDisplay> for MidiDisplayFullSettings {
    fn from(v: &MidiDisplay) -> Self { Self { file_path: v.file_path.clone(), file_data: v.file_data.clone(), time_offset: v.time_offset, bpm_source: v.bpm_source, user_bpm: v.user_bpm, speed: v.speed, orientation: v.orientation, note_height_min: v.note_height_min, key_range_min: v.key_range_min, key_range_max: v.key_range_max, show_keyboard: v.show_keyboard, keyboard_width: v.keyboard_width, note_color_mode: v.note_color_mode, note_color_r: v.note_color_r, note_color_g: v.note_color_g, note_color_b: v.note_color_b, note_color_a: v.note_color_a, note_opacity: v.note_opacity, note_border_thickness: v.note_border_thickness, note_border_color_r: v.note_border_color_r, note_border_color_g: v.note_border_color_g, note_border_color_b: v.note_border_color_b, note_border_color_a: v.note_border_color_a, note_border_opacity: v.note_border_opacity, note_corner_radius: v.note_corner_radius, velocity_affects_opacity: v.velocity_affects_opacity, velocity_affects_brightness: v.velocity_affects_brightness, minimum_velocity: v.minimum_velocity, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a, background_opacity: v.background_opacity, track_filter_mode: v.track_filter_mode, track_number: v.track_number, loop_playback: v.loop_playback, quantize_display: v.quantize_display, show_velocity_as_height: v.show_velocity_as_height } }
}
impl From<MidiDisplay> for MidiDisplayFullSettings {
    fn from(v: MidiDisplay) -> Self { Self { file_path: v.file_path, file_data: v.file_data, time_offset: v.time_offset, bpm_source: v.bpm_source, user_bpm: v.user_bpm, speed: v.speed, orientation: v.orientation, note_height_min: v.note_height_min, key_range_min: v.key_range_min, key_range_max: v.key_range_max, show_keyboard: v.show_keyboard, keyboard_width: v.keyboard_width, note_color_mode: v.note_color_mode, note_color_r: v.note_color_r, note_color_g: v.note_color_g, note_color_b: v.note_color_b, note_color_a: v.note_color_a, note_opacity: v.note_opacity, note_border_thickness: v.note_border_thickness, note_border_color_r: v.note_border_color_r, note_border_color_g: v.note_border_color_g, note_border_color_b: v.note_border_color_b, note_border_color_a: v.note_border_color_a, note_border_opacity: v.note_border_opacity, note_corner_radius: v.note_corner_radius, velocity_affects_opacity: v.velocity_affects_opacity, velocity_affects_brightness: v.velocity_affects_brightness, minimum_velocity: v.minimum_velocity, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a, background_opacity: v.background_opacity, track_filter_mode: v.track_filter_mode, track_number: v.track_number, loop_playback: v.loop_playback, quantize_display: v.quantize_display, show_velocity_as_height: v.show_velocity_as_height } }
}
impl From<&MidiDisplayFullSettings> for MidiDisplay {
    fn from(v: &MidiDisplayFullSettings) -> Self { Self { file_path: v.file_path.clone(), file_data: v.file_data.clone(), time_offset: v.time_offset, bpm_source: v.bpm_source, user_bpm: v.user_bpm, speed: v.speed, orientation: v.orientation, note_height_min: v.note_height_min, key_range_min: v.key_range_min, key_range_max: v.key_range_max, show_keyboard: v.show_keyboard, keyboard_width: v.keyboard_width, note_color_mode: v.note_color_mode, note_color_r: v.note_color_r, note_color_g: v.note_color_g, note_color_b: v.note_color_b, note_color_a: v.note_color_a, note_opacity: v.note_opacity, note_border_thickness: v.note_border_thickness, note_border_color_r: v.note_border_color_r, note_border_color_g: v.note_border_color_g, note_border_color_b: v.note_border_color_b, note_border_color_a: v.note_border_color_a, note_border_opacity: v.note_border_opacity, note_corner_radius: v.note_corner_radius, velocity_affects_opacity: v.velocity_affects_opacity, velocity_affects_brightness: v.velocity_affects_brightness, minimum_velocity: v.minimum_velocity, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a, background_opacity: v.background_opacity, track_filter_mode: v.track_filter_mode, track_number: v.track_number, loop_playback: v.loop_playback, quantize_display: v.quantize_display, show_velocity_as_height: v.show_velocity_as_height } }
}
impl From<MidiDisplayFullSettings> for MidiDisplay {
    fn from(v: MidiDisplayFullSettings) -> Self { Self { file_path: v.file_path, file_data: v.file_data, time_offset: v.time_offset, bpm_source: v.bpm_source, user_bpm: v.user_bpm, speed: v.speed, orientation: v.orientation, note_height_min: v.note_height_min, key_range_min: v.key_range_min, key_range_max: v.key_range_max, show_keyboard: v.show_keyboard, keyboard_width: v.keyboard_width, note_color_mode: v.note_color_mode, note_color_r: v.note_color_r, note_color_g: v.note_color_g, note_color_b: v.note_color_b, note_color_a: v.note_color_a, note_opacity: v.note_opacity, note_border_thickness: v.note_border_thickness, note_border_color_r: v.note_border_color_r, note_border_color_g: v.note_border_color_g, note_border_color_b: v.note_border_color_b, note_border_color_a: v.note_border_color_a, note_border_opacity: v.note_border_opacity, note_corner_radius: v.note_corner_radius, velocity_affects_opacity: v.velocity_affects_opacity, velocity_affects_brightness: v.velocity_affects_brightness, minimum_velocity: v.minimum_velocity, background_color_r: v.background_color_r, background_color_g: v.background_color_g, background_color_b: v.background_color_b, background_color_a: v.background_color_a, background_opacity: v.background_opacity, track_filter_mode: v.track_filter_mode, track_number: v.track_number, loop_playback: v.loop_playback, quantize_display: v.quantize_display, show_velocity_as_height: v.show_velocity_as_height } }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::MidiDisplayFullSettings;
    type SID = SettingID<MidiDisplayFullSettings>;

    pub const FILE_PATH:              SID = setting_id!("file_path", file_path);
    pub const FILE_DATA:              SID = setting_id!("file_data", file_data);
    // Timing
    pub const TIME_OFFSET:            SID = setting_id!("time_offset", time_offset);
    pub const BPM_SOURCE:             SID = setting_id!("bpm_source", bpm_source);
    pub const USER_BPM:               SID = setting_id!("user_bpm", user_bpm);
    pub const SPEED:                  SID = setting_id!("speed", speed);
    // Layout
    pub const ORIENTATION:            SID = setting_id!("orientation", orientation);
    pub const NOTE_HEIGHT_MIN:        SID = setting_id!("note_height_min", note_height_min);
    pub const KEY_RANGE_MIN:          SID = setting_id!("key_range_min", key_range_min);
    pub const KEY_RANGE_MAX:          SID = setting_id!("key_range_max", key_range_max);
    pub const SHOW_KEYBOARD:          SID = setting_id!("show_keyboard", show_keyboard);
    pub const KEYBOARD_WIDTH:         SID = setting_id!("keyboard_width", keyboard_width);
    // Note Appearance
    pub const NOTE_COLOR_MODE:        SID = setting_id!("note_color_mode", note_color_mode);
    pub const NOTE_COLOR:             SID = setting_id!("note_color_r", note_color_r);
    pub const NOTE_COLOR_R:           SID = setting_id!("note_color_r", note_color_r);
    pub const NOTE_COLOR_G:           SID = setting_id!("note_color_g", note_color_g);
    pub const NOTE_COLOR_B:           SID = setting_id!("note_color_b", note_color_b);
    pub const NOTE_COLOR_A:           SID = setting_id!("note_color_a", note_color_a);
    pub const NOTE_OPACITY:           SID = setting_id!("note_opacity", note_opacity);
    pub const NOTE_BORDER_THICKNESS:  SID = setting_id!("note_border_thickness", note_border_thickness);
    pub const NOTE_BORDER_COLOR:      SID = setting_id!("note_border_color_r", note_border_color_r);
    pub const NOTE_BORDER_COLOR_R:    SID = setting_id!("note_border_color_r", note_border_color_r);
    pub const NOTE_BORDER_COLOR_G:    SID = setting_id!("note_border_color_g", note_border_color_g);
    pub const NOTE_BORDER_COLOR_B:    SID = setting_id!("note_border_color_b", note_border_color_b);
    pub const NOTE_BORDER_COLOR_A:    SID = setting_id!("note_border_color_a", note_border_color_a);
    pub const NOTE_BORDER_OPACITY:    SID = setting_id!("note_border_opacity", note_border_opacity);
    pub const NOTE_CORNER_RADIUS:     SID = setting_id!("note_corner_radius", note_corner_radius);
    // Velocity
    pub const VELOCITY_AFFECTS_OPACITY:    SID = setting_id!("velocity_affects_opacity", velocity_affects_opacity);
    pub const VELOCITY_AFFECTS_BRIGHTNESS: SID = setting_id!("velocity_affects_brightness", velocity_affects_brightness);
    pub const MINIMUM_VELOCITY:             SID = setting_id!("minimum_velocity", minimum_velocity);
    // Background
    pub const BACKGROUND_COLOR:    SID = setting_id!("background_color_r", background_color_r);
    pub const BACKGROUND_COLOR_R:  SID = setting_id!("background_color_r", background_color_r);
    pub const BACKGROUND_COLOR_G:  SID = setting_id!("background_color_g", background_color_g);
    pub const BACKGROUND_COLOR_B:  SID = setting_id!("background_color_b", background_color_b);
    pub const BACKGROUND_COLOR_A:  SID = setting_id!("background_color_a", background_color_a);
    pub const BACKGROUND_OPACITY:  SID = setting_id!("background_opacity", background_opacity);
    // Track Selection
    pub const TRACK_FILTER_MODE:   SID = setting_id!("track_filter_mode", track_filter_mode);
    pub const TRACK_NUMBER:        SID = setting_id!("track_number", track_number);
    // Playback
    pub const LOOP_PLAYBACK:              SID = setting_id!("loop_playback", loop_playback);
    pub const QUANTIZE_DISPLAY:           SID = setting_id!("quantize_display", quantize_display);
    pub const SHOW_VELOCITY_AS_HEIGHT:    SID = setting_id!("show_velocity_as_height", show_velocity_as_height);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

impl Settings for MidiDisplayFullSettings {
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
            // ── Timing ──────────────────────────────────────────
            SettingDescriptor {
                label_key: TrKey::ParamMidiTimeOffsetS,
                description_key: Some(TrKey::ParamMidiTimeOffsetSDesc),
                kind: SettingKind::FloatRange { range: -3600.0..=3600.0, logarithmic: false },
                id: setting_id::TIME_OFFSET,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiBpmSource,
                description_key: Some(TrKey::ParamMidiBpmSourceDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuMidiFromMidi, description_key: Some(TrKey::MenuMidiFromMidiDesc), index: MidiBpmSource::FromMidi as u32 },
                        MenuItem { label_key: TrKey::MenuMidiUserSpecified, description_key: Some(TrKey::MenuMidiUserSpecifiedDesc), index: MidiBpmSource::UserSpecified as u32 },
                    ],
                },
                id: setting_id::BPM_SOURCE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiUserBpm,
                description_key: Some(TrKey::ParamMidiUserBpmDesc),
                kind: SettingKind::FloatRange { range: 1.0..=999.0, logarithmic: false },
                id: setting_id::USER_BPM,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiSpeed,
                description_key: Some(TrKey::ParamMidiSpeedDesc),
                kind: SettingKind::FloatRange { range: 0.01..=10.0, logarithmic: false },
                id: setting_id::SPEED,
            },
            // ── Layout ──────────────────────────────────────────
            SettingDescriptor {
                label_key: TrKey::ParamMidiOrientation,
                description_key: Some(TrKey::ParamMidiOrientationDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuMidiHorizontal, description_key: Some(TrKey::MenuMidiHorizontalDesc), index: MidiOrientation::Horizontal as u32 },
                        MenuItem { label_key: TrKey::MenuMidiVertical, description_key: Some(TrKey::MenuMidiVerticalDesc), index: MidiOrientation::Vertical as u32 },
                    ],
                },
                id: setting_id::ORIENTATION,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteHeightMin,
                description_key: Some(TrKey::ParamMidiNoteHeightMinDesc),
                kind: SettingKind::FloatRange { range: 1.0..=200.0, logarithmic: false },
                id: setting_id::NOTE_HEIGHT_MIN,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiKeyRangeMin,
                description_key: Some(TrKey::ParamMidiKeyRangeMinDesc),
                kind: SettingKind::IntRange { range: 0..=127 },
                id: setting_id::KEY_RANGE_MIN,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiKeyRangeMax,
                description_key: Some(TrKey::ParamMidiKeyRangeMaxDesc),
                kind: SettingKind::IntRange { range: 0..=127 },
                id: setting_id::KEY_RANGE_MAX,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiShowKeyboard,
                description_key: Some(TrKey::ParamMidiShowKeyboardDesc),
                kind: SettingKind::Boolean,
                id: setting_id::SHOW_KEYBOARD,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiKeyboardWidth,
                description_key: Some(TrKey::ParamMidiKeyboardWidthDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::KEYBOARD_WIDTH,
            },
            // ── Note Appearance ─────────────────────────────────
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteColorMode,
                description_key: Some(TrKey::ParamMidiNoteColorModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuMidiSolid, description_key: Some(TrKey::MenuMidiSolidDesc), index: MidiNoteColorMode::Solid as u32 },
                        MenuItem { label_key: TrKey::MenuMidiVelocity, description_key: Some(TrKey::MenuMidiVelocityDesc), index: MidiNoteColorMode::Velocity as u32 },
                        MenuItem { label_key: TrKey::MenuMidiChannel, description_key: Some(TrKey::MenuMidiChannelDesc), index: MidiNoteColorMode::Channel as u32 },
                        MenuItem { label_key: TrKey::MenuMidiTrack, description_key: Some(TrKey::MenuMidiTrackDesc), index: MidiNoteColorMode::Track as u32 },
                        MenuItem { label_key: TrKey::MenuMidiPitch, description_key: Some(TrKey::MenuMidiPitchDesc), index: MidiNoteColorMode::Pitch as u32 },
                    ],
                },
                id: setting_id::NOTE_COLOR_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteColor,
                description_key: Some(TrKey::ParamMidiNoteColorDesc),
                kind: SettingKind::ColorRGBA {
                    r_id: setting_id::NOTE_COLOR_R,
                    g_id: setting_id::NOTE_COLOR_G,
                    b_id: setting_id::NOTE_COLOR_B,
                    a_id: setting_id::NOTE_COLOR_A,
                },
                id: setting_id::NOTE_COLOR,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteOpacity,
                description_key: Some(TrKey::ParamMidiNoteOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::NOTE_OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteBorderThickness,
                description_key: Some(TrKey::ParamMidiNoteBorderThicknessDesc),
                kind: SettingKind::FloatRange { range: 0.0..=10.0, logarithmic: false },
                id: setting_id::NOTE_BORDER_THICKNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteBorderColor,
                description_key: Some(TrKey::ParamMidiNoteBorderColorDesc),
                kind: SettingKind::ColorRGBA {
                    r_id: setting_id::NOTE_BORDER_COLOR_R,
                    g_id: setting_id::NOTE_BORDER_COLOR_G,
                    b_id: setting_id::NOTE_BORDER_COLOR_B,
                    a_id: setting_id::NOTE_BORDER_COLOR_A,
                },
                id: setting_id::NOTE_BORDER_COLOR,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteBorderOpacity,
                description_key: Some(TrKey::ParamMidiNoteBorderOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::NOTE_BORDER_OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiNoteCornerRadius,
                description_key: Some(TrKey::ParamMidiNoteCornerRadiusDesc),
                kind: SettingKind::FloatRange { range: 0.0..=20.0, logarithmic: false },
                id: setting_id::NOTE_CORNER_RADIUS,
            },
            // ── Velocity ────────────────────────────────────────
            SettingDescriptor {
                label_key: TrKey::ParamMidiVelocityAffectsOpacity,
                description_key: Some(TrKey::ParamMidiVelocityAffectsOpacityDesc),
                kind: SettingKind::Boolean,
                id: setting_id::VELOCITY_AFFECTS_OPACITY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiVelocityAffectsBrightness,
                description_key: Some(TrKey::ParamMidiVelocityAffectsBrightnessDesc),
                kind: SettingKind::Boolean,
                id: setting_id::VELOCITY_AFFECTS_BRIGHTNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiMinimumVelocity,
                description_key: Some(TrKey::ParamMidiMinimumVelocityDesc),
                kind: SettingKind::IntRange { range: 1..=127 },
                id: setting_id::MINIMUM_VELOCITY,
            },
            // ── Background ───────────────────────────────────────
            SettingDescriptor {
                label_key: TrKey::ParamMidiBackgroundColor,
                description_key: Some(TrKey::ParamMidiBackgroundColorDesc),
                kind: SettingKind::ColorRGBA {
                    r_id: setting_id::BACKGROUND_COLOR_R,
                    g_id: setting_id::BACKGROUND_COLOR_G,
                    b_id: setting_id::BACKGROUND_COLOR_B,
                    a_id: setting_id::BACKGROUND_COLOR_A,
                },
                id: setting_id::BACKGROUND_COLOR,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiBackgroundOpacity,
                description_key: Some(TrKey::ParamMidiBackgroundOpacityDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::BACKGROUND_OPACITY,
            },
            // ── Track Selection ──────────────────────────────────
            SettingDescriptor {
                label_key: TrKey::ParamMidiTrackFilterMode,
                description_key: Some(TrKey::ParamMidiTrackFilterModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem { label_key: TrKey::MenuMidiAllTracks, description_key: Some(TrKey::MenuMidiAllTracksDesc), index: MidiTrackFilterMode::AllTracks as u32 },
                        MenuItem { label_key: TrKey::MenuMidiSpecificTrack, description_key: Some(TrKey::MenuMidiSpecificTrackDesc), index: MidiTrackFilterMode::SpecificTrack as u32 },
                    ],
                },
                id: setting_id::TRACK_FILTER_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiTrackNumber,
                description_key: Some(TrKey::ParamMidiTrackNumberDesc),
                kind: SettingKind::IntRange { range: 0..=255 },
                id: setting_id::TRACK_NUMBER,
            },
            // ── Playback ─────────────────────────────────────────
            SettingDescriptor {
                label_key: TrKey::ParamMidiLoop,
                description_key: Some(TrKey::ParamMidiLoopDesc),
                kind: SettingKind::Boolean,
                id: setting_id::LOOP_PLAYBACK,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiQuantizeDisplay,
                description_key: Some(TrKey::ParamMidiQuantizeDisplayDesc),
                kind: SettingKind::Boolean,
                id: setting_id::QUANTIZE_DISPLAY,
            },
            SettingDescriptor {
                label_key: TrKey::ParamMidiShowVelocityAsHeight,
                description_key: Some(TrKey::ParamMidiShowVelocityAsHeightDesc),
                kind: SettingKind::Boolean,
                id: setting_id::SHOW_VELOCITY_AS_HEIGHT,
            },
        ]
        .into_boxed_slice()
    }

}
