use num_enum::{IntoPrimitive, TryFromPrimitive};

use super::{
    MenuItem, SettingDescriptor, SettingID, SettingKind, Settings, SettingsEnum,
};
use crate::i18n::TrKey;

// ---------------------------------------------------------------------------
// Character set constants (ordered dense → sparse within each category)
// ---------------------------------------------------------------------------

const CHARS_LATIN: &str = "WMBDKHQARGZPXSONEYUFVTJCLIwmbaekdqpghsonxruvtzjcyfli. ";
const CHARS_SYMBOLS: &str = "@%#*+=-:. ";
const CHARS_NUMBERS: &str = "9876543210 ";
const CHARS_BLOCKS: &str = "\u{2588}\u{2593}\u{2592}\u{2591} ";
const CHARS_CHINESE: &str =
    "\u{7C73}\u{7530}\u{6728}\u{91D1}\u{6C34}\u{706B}\u{571F}\u{65E5}\u{6708}\u{3002} ";
const CHARS_KATAKANA: &str =
    "\u{30E2}\u{30EF}\u{30F2}\u{30F3}\u{30F4}\u{30AB}\u{30AD}\u{30AF}\u{30B1}\u{30B3}\u{30B5}\u{30B7}\u{30B9}\u{30BB}\u{30BD}\u{30BF}\u{30C1}\u{30C4}\u{30C6}\u{30C8}\u{30CA}\u{30CB}\u{30CC}\u{30CD}\u{30CE}\u{30CF}\u{30D2}\u{30D5}\u{30D8}\u{30DB}\u{30DE}\u{30DF}\u{30E0}\u{30E1}\u{30E2}\u{30E4}\u{30E6}\u{30E8}\u{30E9}\u{30EA}\u{30EB}\u{30EC}\u{30ED}\u{30EF}\u{30F2}\u{30F3}\u{3002} ";
const CHARS_HIRAGANA: &str =
    "\u{3082}\u{308F}\u{3092}\u{3093}\u{304B}\u{304D}\u{304F}\u{3051}\u{3053}\u{3055}\u{3057}\u{3059}\u{305B}\u{305D}\u{305F}\u{3061}\u{3064}\u{3066}\u{3068}\u{306A}\u{306B}\u{306C}\u{306D}\u{306E}\u{306F}\u{3072}\u{3075}\u{3078}\u{307B}\u{307E}\u{307F}\u{3080}\u{3081}\u{3082}\u{3084}\u{3086}\u{3088}\u{3089}\u{308A}\u{308B}\u{308C}\u{308D}\u{308F}\u{3092}\u{3093}\u{3002} ";
const CHARS_KOREAN: &str =
    "\u{D55C}\u{AD6D}\u{C11C}\u{C6B8}\u{BD80}\u{C0B0}\u{B300}\u{AD6C}\u{C778}\u{CC9C}\u{AD11}\u{C8FC}\u{3002} ";

// ---------------------------------------------------------------------------
// Color mode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ColorMode {
    Colored = 0,
    Grayscale,
    Solid,
    SolidMapGrayscale,
}
impl SettingsEnum for ColorMode {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct AsciiArt {
    pub char_set_enabled: bool,
    pub use_symbols: bool,
    pub use_latin: bool,
    pub use_numbers: bool,
    pub use_blocks: bool,
    pub use_chinese: bool,
    pub use_katakana: bool,
    pub use_hiragana: bool,
    pub use_korean: bool,
    pub use_custom: bool,
    pub custom_chars: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub font_size: f32,
    pub font_fill: bool,
    pub font_scale_x: f32,
    pub font_scale_y: f32,
    pub font_rotation: f32,
    pub color_mode: ColorMode,
    pub font_color_r: f32,
    pub font_color_g: f32,
    pub font_color_b: f32,
    pub font_color_a: f32,
    pub grid_thickness: f32,
    pub grid_color_r: f32,
    pub grid_color_g: f32,
    pub grid_color_b: f32,
    pub grid_color_a: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub invert_luma: bool,
    pub bg_color_r: f32,
    pub bg_color_g: f32,
    pub bg_color_b: f32,
    pub bg_color_a: f32,
    pub font_name: String,
}

impl AsciiArt {
    /// Build a character set string with monotonic dense→sparse density order.
    ///
    /// When multiple character sets are enabled, we interleave them by rank
    /// (round-robin: char 0 from set 0, char 0 from set 1, ..., char 1 from
    /// set 0, ...). This preserves the monotonic density property across the
    /// combined set instead of resetting density at each set boundary.
    pub fn resolve_charset(&self) -> String {
        let sets: [Option<&str>; 8] = [
            self.use_symbols.then_some(CHARS_SYMBOLS),
            self.use_latin.then_some(CHARS_LATIN),
            self.use_numbers.then_some(CHARS_NUMBERS),
            self.use_blocks.then_some(CHARS_BLOCKS),
            self.use_chinese.then_some(CHARS_CHINESE),
            self.use_katakana.then_some(CHARS_KATAKANA),
            self.use_hiragana.then_some(CHARS_HIRAGANA),
            self.use_korean.then_some(CHARS_KOREAN),
        ];
        let custom = (self.use_custom && !self.custom_chars.is_empty())
            .then(|| self.custom_chars.as_str());
        let active: Vec<&str> = sets.into_iter()
            .flatten()
            .chain(custom)
            .collect();
        if active.is_empty() {
            return CHARS_SYMBOLS.to_owned();
        }
        let max_len = active.iter().map(|s| s.chars().count()).max().unwrap_or(0);
        let char_vecs: Vec<Vec<char>> = active.iter()
            .map(|s| s.chars().collect())
            .collect();
        let mut out = String::with_capacity(max_len * active.len());
        for i in 0..max_len {
            for chars in &char_vecs {
                if let Some(&c) = chars.get(i) {
                    out.push(c);
                }
            }
        }
        out
    }
}

impl Default for AsciiArt {
    fn default() -> Self {
        Self {
            char_set_enabled: true,
            use_symbols: true,
            use_latin: false,
            use_numbers: false,
            use_blocks: false,
            use_chinese: false,
            use_katakana: false,
            use_hiragana: false,
            use_korean: false,
            use_custom: false,
            custom_chars: String::new(),
            pos_x: 0.5,
            pos_y: 0.5,
            font_size: 5.0,
            font_fill: false,
            font_scale_x: 1.0,
            font_scale_y: 1.0,
            font_rotation: 0.0,
            color_mode: ColorMode::Colored,
            font_color_r: 0.0,
            font_color_g: 1.0,
            font_color_b: 0.0,
            font_color_a: 1.0,
            grid_thickness: 0.0,
            grid_color_r: 0.0,
            grid_color_g: 0.0,
            grid_color_b: 0.0,
            grid_color_a: 0.5,
            brightness: 0.5,
            contrast: 0.5,
            invert_luma: false,
            bg_color_r: 0.0,
            bg_color_g: 0.0,
            bg_color_b: 0.0,
            bg_color_a: 0.0,
            font_name: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// FullSettings struct (manual — derive macro doesn't support String)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct AsciiArtFullSettings {
    pub char_set_enabled: bool,
    pub use_symbols: bool,
    pub use_latin: bool,
    pub use_numbers: bool,
    pub use_blocks: bool,
    pub use_chinese: bool,
    pub use_katakana: bool,
    pub use_hiragana: bool,
    pub use_korean: bool,
    pub use_custom: bool,
    pub custom_chars: String,
    pub pos_x: f32,
    pub pos_y: f32,
    pub font_size: f32,
    pub font_fill: bool,
    pub font_scale_x: f32,
    pub font_scale_y: f32,
    pub font_rotation: f32,
    pub color_mode: ColorMode,
    pub font_color_r: f32,
    pub font_color_g: f32,
    pub font_color_b: f32,
    pub font_color_a: f32,
    pub grid_thickness: f32,
    pub grid_color_r: f32,
    pub grid_color_g: f32,
    pub grid_color_b: f32,
    pub grid_color_a: f32,
    pub brightness: f32,
    pub contrast: f32,
    pub invert_luma: bool,
    pub bg_color_r: f32,
    pub bg_color_g: f32,
    pub bg_color_b: f32,
    pub bg_color_a: f32,
    pub font_name: String,
}

impl Default for AsciiArtFullSettings {
    fn default() -> Self {
        Self::from(AsciiArt::default())
    }
}

impl From<&AsciiArt> for AsciiArtFullSettings {
    fn from(value: &AsciiArt) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_latin: value.use_latin,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_blocks: value.use_blocks,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars.clone(),
            pos_x: value.pos_x,
            pos_y: value.pos_y,
            font_size: value.font_size,
            font_fill: value.font_fill,
            font_scale_x: value.font_scale_x,
            font_scale_y: value.font_scale_y,
            font_rotation: value.font_rotation,
            color_mode: value.color_mode,
            font_color_r: value.font_color_r,
            font_color_g: value.font_color_g,
            font_color_b: value.font_color_b,
            font_color_a: value.font_color_a,
            grid_thickness: value.grid_thickness,
            grid_color_r: value.grid_color_r,
            grid_color_g: value.grid_color_g,
            grid_color_b: value.grid_color_b,
            grid_color_a: value.grid_color_a,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            bg_color_r: value.bg_color_r,
            bg_color_g: value.bg_color_g,
            bg_color_b: value.bg_color_b,
            bg_color_a: value.bg_color_a,
            font_name: value.font_name.clone(),
        }
    }
}

impl From<AsciiArt> for AsciiArtFullSettings {
    fn from(value: AsciiArt) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_latin: value.use_latin,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_blocks: value.use_blocks,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars,
            pos_x: value.pos_x,
            pos_y: value.pos_y,
            font_size: value.font_size,
            font_fill: value.font_fill,
            font_scale_x: value.font_scale_x,
            font_scale_y: value.font_scale_y,
            font_rotation: value.font_rotation,
            color_mode: value.color_mode,
            font_color_r: value.font_color_r,
            font_color_g: value.font_color_g,
            font_color_b: value.font_color_b,
            font_color_a: value.font_color_a,
            grid_thickness: value.grid_thickness,
            grid_color_r: value.grid_color_r,
            grid_color_g: value.grid_color_g,
            grid_color_b: value.grid_color_b,
            grid_color_a: value.grid_color_a,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            bg_color_r: value.bg_color_r,
            bg_color_g: value.bg_color_g,
            bg_color_b: value.bg_color_b,
            bg_color_a: value.bg_color_a,
            font_name: value.font_name,
        }
    }
}

impl From<&AsciiArtFullSettings> for AsciiArt {
    fn from(value: &AsciiArtFullSettings) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_latin: value.use_latin,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_blocks: value.use_blocks,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars.clone(),
            pos_x: value.pos_x,
            pos_y: value.pos_y,
            font_size: value.font_size,
            font_fill: value.font_fill,
            font_scale_x: value.font_scale_x,
            font_scale_y: value.font_scale_y,
            font_rotation: value.font_rotation,
            color_mode: value.color_mode,
            font_color_r: value.font_color_r,
            font_color_g: value.font_color_g,
            font_color_b: value.font_color_b,
            font_color_a: value.font_color_a,
            grid_thickness: value.grid_thickness,
            grid_color_r: value.grid_color_r,
            grid_color_g: value.grid_color_g,
            grid_color_b: value.grid_color_b,
            grid_color_a: value.grid_color_a,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            bg_color_r: value.bg_color_r,
            bg_color_g: value.bg_color_g,
            bg_color_b: value.bg_color_b,
            bg_color_a: value.bg_color_a,
            font_name: value.font_name.clone(),
        }
    }
}

impl From<AsciiArtFullSettings> for AsciiArt {
    fn from(value: AsciiArtFullSettings) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_latin: value.use_latin,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_blocks: value.use_blocks,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars,
            pos_x: value.pos_x,
            pos_y: value.pos_y,
            font_size: value.font_size,
            font_fill: value.font_fill,
            font_scale_x: value.font_scale_x,
            font_scale_y: value.font_scale_y,
            font_rotation: value.font_rotation,
            color_mode: value.color_mode,
            font_color_r: value.font_color_r,
            font_color_g: value.font_color_g,
            font_color_b: value.font_color_b,
            font_color_a: value.font_color_a,
            grid_thickness: value.grid_thickness,
            grid_color_r: value.grid_color_r,
            grid_color_g: value.grid_color_g,
            grid_color_b: value.grid_color_b,
            grid_color_a: value.grid_color_a,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            bg_color_r: value.bg_color_r,
            bg_color_g: value.bg_color_g,
            bg_color_b: value.bg_color_b,
            bg_color_a: value.bg_color_a,
            font_name: value.font_name,
        }
    }
}

// ---------------------------------------------------------------------------
// Setting IDs
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub mod setting_id {
    use crate::{setting_id, settings::SettingID};
    use super::AsciiArtFullSettings;
    type SID = SettingID<AsciiArtFullSettings>;

    pub const CHAR_SET_ENABLED: SID = setting_id!("char_set_enabled", char_set_enabled);
    pub const USE_LATIN:        SID = setting_id!("use_latin", use_latin);
    pub const USE_SYMBOLS:      SID = setting_id!("use_symbols", use_symbols);
    pub const USE_NUMBERS:      SID = setting_id!("use_numbers", use_numbers);
    pub const USE_BLOCKS:       SID = setting_id!("use_blocks", use_blocks);
    pub const USE_CHINESE:      SID = setting_id!("use_chinese", use_chinese);
    pub const USE_KATAKANA:     SID = setting_id!("use_katakana", use_katakana);
    pub const USE_HIRAGANA:     SID = setting_id!("use_hiragana", use_hiragana);
    pub const USE_KOREAN:       SID = setting_id!("use_korean", use_korean);
    pub const USE_CUSTOM:       SID = setting_id!("use_custom", use_custom);
    pub const POS_X:            SID = setting_id!("pos_x", pos_x);
    pub const POS_Y:            SID = setting_id!("pos_y", pos_y);
    pub const FONT_SIZE:        SID = setting_id!("font_size", font_size);
    pub const FONT_FILL:        SID = setting_id!("font_fill", font_fill);
    pub const FONT_SCALE_X:     SID = setting_id!("font_scale_x", font_scale_x);
    pub const FONT_SCALE_Y:     SID = setting_id!("font_scale_y", font_scale_y);
    pub const FONT_ROTATION:    SID = setting_id!("font_rotation", font_rotation);
    pub const COLOR_MODE:       SID = setting_id!("color_mode", color_mode);
    pub const FONT_COLOR:       SID = setting_id!("font_color_r", font_color_r);
    pub const FONT_COLOR_R:     SID = setting_id!("font_color_r", font_color_r);
    pub const FONT_COLOR_G:     SID = setting_id!("font_color_g", font_color_g);
    pub const FONT_COLOR_B:     SID = setting_id!("font_color_b", font_color_b);
    pub const FONT_COLOR_A:     SID = setting_id!("font_color_a", font_color_a);
    pub const GRID_THICKNESS:   SID = setting_id!("grid_thickness", grid_thickness);
    pub const GRID_COLOR:       SID = setting_id!("grid_color_r", grid_color_r);
    pub const GRID_COLOR_R:     SID = setting_id!("grid_color_r", grid_color_r);
    pub const GRID_COLOR_G:     SID = setting_id!("grid_color_g", grid_color_g);
    pub const GRID_COLOR_B:     SID = setting_id!("grid_color_b", grid_color_b);
    pub const GRID_COLOR_A:     SID = setting_id!("grid_color_a", grid_color_a);
    pub const BRIGHTNESS:       SID = setting_id!("brightness", brightness);
    pub const CONTRAST:         SID = setting_id!("contrast", contrast);
    pub const INVERT_LUMA:      SID = setting_id!("invert_luma", invert_luma);
    pub const BG_COLOR:   SID = setting_id!("bg_color_r", bg_color_r);
    pub const BG_COLOR_R: SID = setting_id!("bg_color_r", bg_color_r);
    pub const BG_COLOR_G: SID = setting_id!("bg_color_g", bg_color_g);
    pub const BG_COLOR_B: SID = setting_id!("bg_color_b", bg_color_b);
    pub const BG_COLOR_A: SID = setting_id!("bg_color_a", bg_color_a);
    pub const CUSTOM_CHARS: SID = setting_id!("custom_chars", custom_chars);
    pub const FONT_NAME:    SID = setting_id!("font_name", font_name);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

fn desc_bool(
    label_key: TrKey,
    desc_key: TrKey,
    id: SettingID<AsciiArtFullSettings>,
) -> SettingDescriptor<AsciiArtFullSettings> {
    SettingDescriptor {
        label_key,
        description_key: Some(desc_key),
        kind: SettingKind::Boolean,
        id,
    }
}

impl Settings for AsciiArtFullSettings {
    type Key = TrKey;

    fn setting_descriptors() -> Box<[SettingDescriptor<Self>]> {
        vec![
            // Character Set group (first)
            SettingDescriptor {
                label_key: TrKey::ParamAsciiCharSetGroup,
                description_key: Some(TrKey::ParamAsciiCharSetGroupDesc),
                kind: SettingKind::Group {
                    children: vec![
                        desc_bool(TrKey::ParamAsciiUseSymbols, TrKey::ParamAsciiUseSymbolsDesc, setting_id::USE_SYMBOLS),
                        desc_bool(TrKey::ParamAsciiUseLatin, TrKey::ParamAsciiUseLatinDesc, setting_id::USE_LATIN),
                        desc_bool(TrKey::ParamAsciiUseNumbers, TrKey::ParamAsciiUseNumbersDesc, setting_id::USE_NUMBERS),
                        desc_bool(TrKey::ParamAsciiUseBlocks, TrKey::ParamAsciiUseBlocksDesc, setting_id::USE_BLOCKS),
                        desc_bool(TrKey::ParamAsciiUseChinese, TrKey::ParamAsciiUseChineseDesc, setting_id::USE_CHINESE),
                        desc_bool(TrKey::ParamAsciiUseKatakana, TrKey::ParamAsciiUseKatakanaDesc, setting_id::USE_KATAKANA),
                        desc_bool(TrKey::ParamAsciiUseHiragana, TrKey::ParamAsciiUseHiraganaDesc, setting_id::USE_HIRAGANA),
                        desc_bool(TrKey::ParamAsciiUseKorean, TrKey::ParamAsciiUseKoreanDesc, setting_id::USE_KOREAN),
                        desc_bool(TrKey::ParamAsciiUseCustom, TrKey::ParamAsciiUseCustomDesc, setting_id::USE_CUSTOM),
                        SettingDescriptor {
                            label_key: TrKey::NativeAsciiCustomChars,
                            description_key: Some(TrKey::NativeAsciiCustomCharsHint),
                            kind: SettingKind::String { secret: true, multiline: false, animates: false },
                            id: setting_id::CUSTOM_CHARS,
                        },
                    ],
                },
                id: setting_id::CHAR_SET_ENABLED,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiPositionX,
                description_key: Some(TrKey::ParamAsciiPositionXDesc),
                kind: SettingKind::FloatRange { range: 0.0..=1.0, logarithmic: false },
                id: setting_id::POS_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiPositionY,
                description_key: Some(TrKey::ParamAsciiPositionYDesc),
                kind: SettingKind::FloatRange { range: 0.0..=1.0, logarithmic: false },
                id: setting_id::POS_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiFontSize,
                description_key: Some(TrKey::ParamAsciiFontSizeDesc),
                kind: SettingKind::FloatRange { range: 0.0..=100.0, logarithmic: false },
                id: setting_id::FONT_SIZE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiFontFill,
                description_key: Some(TrKey::ParamAsciiFontFillDesc),
                kind: SettingKind::Boolean,
                id: setting_id::FONT_FILL,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiFontScaleX,
                description_key: Some(TrKey::ParamAsciiFontScaleXDesc),
                kind: SettingKind::FloatRange { range: 0.1..=10.0, logarithmic: false },
                id: setting_id::FONT_SCALE_X,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiFontScaleY,
                description_key: Some(TrKey::ParamAsciiFontScaleYDesc),
                kind: SettingKind::FloatRange { range: 0.1..=10.0, logarithmic: false },
                id: setting_id::FONT_SCALE_Y,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiFontRotation,
                description_key: Some(TrKey::ParamAsciiFontRotationDesc),
                kind: SettingKind::FloatRange { range: -360.0..=360.0, logarithmic: false },
                id: setting_id::FONT_ROTATION,
            },
            SettingDescriptor {
                label_key: TrKey::NativeAsciiFontChoice,
                description_key: None,
                kind: SettingKind::String { secret: true, multiline: false, animates: false },
                id: setting_id::FONT_NAME,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiColorMode,
                description_key: Some(TrKey::ParamAsciiColorModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuAsciiColored,
                            description_key: Some(TrKey::MenuAsciiColoredDesc),
                            index: ColorMode::Colored as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuAsciiGrayscale,
                            description_key: Some(TrKey::MenuAsciiGrayscaleDesc),
                            index: ColorMode::Grayscale as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuAsciiSolid,
                            description_key: Some(TrKey::MenuAsciiSolidDesc),
                            index: ColorMode::Solid as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuAsciiSolidMapGrayscale,
                            description_key: Some(TrKey::MenuAsciiSolidMapGrayscaleDesc),
                            index: ColorMode::SolidMapGrayscale as u32,
                        },
                    ],
                },
                id: setting_id::COLOR_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiFontColor,
                description_key: Some(TrKey::ParamAsciiFontColorDesc),
                kind: SettingKind::ColorRGBA {
                    r_id: setting_id::FONT_COLOR_R,
                    g_id: setting_id::FONT_COLOR_G,
                    b_id: setting_id::FONT_COLOR_B,
                    a_id: setting_id::FONT_COLOR_A,
                },
                id: setting_id::FONT_COLOR,
            },
            SettingDescriptor {
                label_key: TrKey::ParamGridThickness,
                description_key: Some(TrKey::ParamGridThicknessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::GRID_THICKNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiGridColor,
                description_key: Some(TrKey::ParamAsciiGridColorDesc),
                kind: SettingKind::ColorRGBA {
                    r_id: setting_id::GRID_COLOR_R,
                    g_id: setting_id::GRID_COLOR_G,
                    b_id: setting_id::GRID_COLOR_B,
                    a_id: setting_id::GRID_COLOR_A,
                },
                id: setting_id::GRID_COLOR,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiBrightness,
                description_key: Some(TrKey::ParamAsciiBrightnessDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::BRIGHTNESS,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiContrast,
                description_key: Some(TrKey::ParamAsciiContrastDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::CONTRAST,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiInvertLuma,
                description_key: Some(TrKey::ParamAsciiInvertLumaDesc),
                kind: SettingKind::Boolean,
                id: setting_id::INVERT_LUMA,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiBgColor,
                description_key: Some(TrKey::ParamAsciiBgColorDesc),
                kind: SettingKind::ColorRGBA {
                    r_id: setting_id::BG_COLOR_R,
                    g_id: setting_id::BG_COLOR_G,
                    b_id: setting_id::BG_COLOR_B,
                    a_id: setting_id::BG_COLOR_A,
                },
                id: setting_id::BG_COLOR,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
