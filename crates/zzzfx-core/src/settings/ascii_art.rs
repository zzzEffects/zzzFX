use num_enum::{IntoPrimitive, TryFromPrimitive};

use effect_settings::{
    MenuItem, SettingDescriptor, SettingID, SettingKind, Settings, SettingsEnum, TrKey,
};

// ---------------------------------------------------------------------------
// Character set constants (ordered dense → sparse within each category)
// ---------------------------------------------------------------------------

const CHARS_SYMBOLS: &str = "@%#*+=-:. ";
const CHARS_NUMBERS: &str = "9876543210 ";
const CHARS_CHINESE: &str =
    "\u{9F8D}\u{9F98}\u{9B31}\u{9A6B}\u{9E1E}\u{9F07}\u{9F49}\u{7228}\u{91C1}\u{7641}\u{7C3F}\u{7C73}\u{7530}\u{6728}\u{91D1}\u{6C34}\u{706B}\u{571F}\u{65E5}\u{6708}\u{3002} ";
const CHARS_KATAKANA: &str =
    "\u{30E2}\u{30EF}\u{30F0}\u{30F1}\u{30F2}\u{30F3}\u{30F4}\u{30AB}\u{30AD}\u{30AF}\u{30B1}\u{30B3}\u{30B5}\u{30B7}\u{30B9}\u{30BB}\u{30BD}\u{30BF}\u{30C1}\u{30C4}\u{30C6}\u{30C8}\u{30CA}\u{30CB}\u{30CC}\u{30CD}\u{30CE}\u{30CF}\u{30D2}\u{30D5}\u{30D8}\u{30DB}\u{30DE}\u{30DF}\u{30E0}\u{30E1}\u{30E2}\u{30E4}\u{30E6}\u{30E8}\u{30E9}\u{30EA}\u{30EB}\u{30EC}\u{30ED}\u{30EF}\u{30F2}\u{30F3}\u{3002} ";
const CHARS_HIRAGANA: &str =
    "\u{3082}\u{308F}\u{3090}\u{3091}\u{3092}\u{3093}\u{3094}\u{304B}\u{304D}\u{304F}\u{3051}\u{3053}\u{3055}\u{3057}\u{3059}\u{305B}\u{305D}\u{305F}\u{3061}\u{3064}\u{3066}\u{3068}\u{306A}\u{306B}\u{306C}\u{306D}\u{306E}\u{306F}\u{3072}\u{3075}\u{3078}\u{307B}\u{307E}\u{307F}\u{3080}\u{3081}\u{3082}\u{3084}\u{3086}\u{3088}\u{3089}\u{308A}\u{308B}\u{308C}\u{308D}\u{308F}\u{3092}\u{3093}\u{3002} ";
const CHARS_KOREAN: &str =
    "\u{D55C}\u{AD6D}\u{C11C}\u{C6B8}\u{BD80}\u{C0B0}\u{B300}\u{AD6C}\u{C778}\u{CC9C}\u{AD11}\u{C8FC}\u{3002} ";

// ---------------------------------------------------------------------------
// Color mode enum
// ---------------------------------------------------------------------------

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
pub enum ColorMode {
    Grayscale = 0,
    Colored,
    GreenTerminal,
}
impl SettingsEnum for ColorMode {}

// ---------------------------------------------------------------------------
// Main settings struct
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct ZzzAsciiArt {
    pub char_set_enabled: bool,
    pub use_symbols: bool,
    pub use_numbers: bool,
    pub use_chinese: bool,
    pub use_katakana: bool,
    pub use_hiragana: bool,
    pub use_korean: bool,
    pub use_custom: bool,
    pub custom_chars: String,
    pub font_size: i32,
    pub brightness: f32,
    pub contrast: f32,
    pub invert_luma: bool,
    pub color_mode: ColorMode,
    pub background_alpha: f32,
    pub font_name: String,
}

impl ZzzAsciiArt {
    pub fn resolve_charset(&self) -> String {
        let mut chars = String::new();
        if self.use_symbols {
            chars.push_str(CHARS_SYMBOLS);
        }
        if self.use_numbers {
            chars.push_str(CHARS_NUMBERS);
        }
        if self.use_chinese {
            chars.push_str(CHARS_CHINESE);
        }
        if self.use_katakana {
            chars.push_str(CHARS_KATAKANA);
        }
        if self.use_hiragana {
            chars.push_str(CHARS_HIRAGANA);
        }
        if self.use_korean {
            chars.push_str(CHARS_KOREAN);
        }
        if self.use_custom && !self.custom_chars.is_empty() {
            chars.push_str(&self.custom_chars);
        }
        if chars.is_empty() {
            chars.push_str(CHARS_SYMBOLS);
        }
        chars
    }
}

impl Default for ZzzAsciiArt {
    fn default() -> Self {
        Self {
            char_set_enabled: true,
            use_symbols: true,
            use_numbers: false,
            use_chinese: false,
            use_katakana: false,
            use_hiragana: false,
            use_korean: false,
            use_custom: false,
            custom_chars: String::new(),
            font_size: 8,
            brightness: 0.5,
            contrast: 0.5,
            invert_luma: false,
            color_mode: ColorMode::Colored,
            background_alpha: 0.0,
            font_name: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// FullSettings struct (manual — derive macro doesn't support String)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct ZzzAsciiArtFullSettings {
    pub char_set_enabled: bool,
    pub use_symbols: bool,
    pub use_numbers: bool,
    pub use_chinese: bool,
    pub use_katakana: bool,
    pub use_hiragana: bool,
    pub use_korean: bool,
    pub use_custom: bool,
    pub custom_chars: String,
    pub font_size: i32,
    pub brightness: f32,
    pub contrast: f32,
    pub invert_luma: bool,
    pub color_mode: ColorMode,
    pub background_alpha: f32,
    pub font_name: String,
}

impl Default for ZzzAsciiArtFullSettings {
    fn default() -> Self {
        Self::from(ZzzAsciiArt::default())
    }
}

impl From<&ZzzAsciiArt> for ZzzAsciiArtFullSettings {
    fn from(value: &ZzzAsciiArt) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars.clone(),
            font_size: value.font_size,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            color_mode: value.color_mode,
            background_alpha: value.background_alpha,
            font_name: value.font_name.clone(),
        }
    }
}

impl From<ZzzAsciiArt> for ZzzAsciiArtFullSettings {
    fn from(value: ZzzAsciiArt) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars,
            font_size: value.font_size,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            color_mode: value.color_mode,
            background_alpha: value.background_alpha,
            font_name: value.font_name,
        }
    }
}

impl From<&ZzzAsciiArtFullSettings> for ZzzAsciiArt {
    fn from(value: &ZzzAsciiArtFullSettings) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars.clone(),
            font_size: value.font_size,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            color_mode: value.color_mode,
            background_alpha: value.background_alpha,
            font_name: value.font_name.clone(),
        }
    }
}

impl From<ZzzAsciiArtFullSettings> for ZzzAsciiArt {
    fn from(value: ZzzAsciiArtFullSettings) -> Self {
        Self {
            char_set_enabled: value.char_set_enabled,
            use_symbols: value.use_symbols,
            use_numbers: value.use_numbers,
            use_chinese: value.use_chinese,
            use_katakana: value.use_katakana,
            use_hiragana: value.use_hiragana,
            use_korean: value.use_korean,
            use_custom: value.use_custom,
            custom_chars: value.custom_chars,
            font_size: value.font_size,
            brightness: value.brightness,
            contrast: value.contrast,
            invert_luma: value.invert_luma,
            color_mode: value.color_mode,
            background_alpha: value.background_alpha,
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
    use super::ZzzAsciiArtFullSettings;
    type SID = SettingID<ZzzAsciiArtFullSettings>;

    pub const CHAR_SET_ENABLED: SID = setting_id!("char_set_enabled", char_set_enabled);
    pub const USE_SYMBOLS:      SID = setting_id!("use_symbols", use_symbols);
    pub const USE_NUMBERS:      SID = setting_id!("use_numbers", use_numbers);
    pub const USE_CHINESE:      SID = setting_id!("use_chinese", use_chinese);
    pub const USE_KATAKANA:     SID = setting_id!("use_katakana", use_katakana);
    pub const USE_HIRAGANA:     SID = setting_id!("use_hiragana", use_hiragana);
    pub const USE_KOREAN:       SID = setting_id!("use_korean", use_korean);
    pub const USE_CUSTOM:       SID = setting_id!("use_custom", use_custom);
    pub const FONT_SIZE:        SID = setting_id!("font_size", font_size);
    pub const BRIGHTNESS:       SID = setting_id!("brightness", brightness);
    pub const CONTRAST:         SID = setting_id!("contrast", contrast);
    pub const INVERT_LUMA:      SID = setting_id!("invert_luma", invert_luma);
    pub const COLOR_MODE:       SID = setting_id!("color_mode", color_mode);
    pub const BACKGROUND_ALPHA: SID = setting_id!("background_alpha", background_alpha);
}

// ---------------------------------------------------------------------------
// Settings trait impl
// ---------------------------------------------------------------------------

fn desc_bool(
    label_key: TrKey,
    desc_key: TrKey,
    id: SettingID<ZzzAsciiArtFullSettings>,
) -> SettingDescriptor<ZzzAsciiArtFullSettings> {
    SettingDescriptor {
        label_key,
        description_key: Some(desc_key),
        kind: SettingKind::Boolean,
        id,
    }
}

impl Settings for ZzzAsciiArtFullSettings {
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
                        desc_bool(TrKey::ParamAsciiUseNumbers, TrKey::ParamAsciiUseNumbersDesc, setting_id::USE_NUMBERS),
                        desc_bool(TrKey::ParamAsciiUseChinese, TrKey::ParamAsciiUseChineseDesc, setting_id::USE_CHINESE),
                        desc_bool(TrKey::ParamAsciiUseKatakana, TrKey::ParamAsciiUseKatakanaDesc, setting_id::USE_KATAKANA),
                        desc_bool(TrKey::ParamAsciiUseHiragana, TrKey::ParamAsciiUseHiraganaDesc, setting_id::USE_HIRAGANA),
                        desc_bool(TrKey::ParamAsciiUseKorean, TrKey::ParamAsciiUseKoreanDesc, setting_id::USE_KOREAN),
                        desc_bool(TrKey::ParamAsciiUseCustom, TrKey::ParamAsciiUseCustomDesc, setting_id::USE_CUSTOM),
                    ],
                },
                id: setting_id::CHAR_SET_ENABLED,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiFontSize,
                description_key: Some(TrKey::ParamAsciiFontSizeDesc),
                kind: SettingKind::IntRange { range: 4..=64 },
                id: setting_id::FONT_SIZE,
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
                label_key: TrKey::ParamAsciiColorMode,
                description_key: Some(TrKey::ParamAsciiColorModeDesc),
                kind: SettingKind::Enumeration {
                    options: vec![
                        MenuItem {
                            label_key: TrKey::MenuAsciiGrayscale,
                            description_key: Some(TrKey::MenuAsciiGrayscaleDesc),
                            index: ColorMode::Grayscale as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuAsciiColored,
                            description_key: Some(TrKey::MenuAsciiColoredDesc),
                            index: ColorMode::Colored as u32,
                        },
                        MenuItem {
                            label_key: TrKey::MenuAsciiGreenTerminal,
                            description_key: Some(TrKey::MenuAsciiGreenTerminalDesc),
                            index: ColorMode::GreenTerminal as u32,
                        },
                    ],
                },
                id: setting_id::COLOR_MODE,
            },
            SettingDescriptor {
                label_key: TrKey::ParamAsciiBackgroundAlpha,
                description_key: Some(TrKey::ParamAsciiBackgroundAlphaDesc),
                kind: SettingKind::Percentage { logarithmic: false },
                id: setting_id::BACKGROUND_ALPHA,
            },
        ]
        .into_boxed_slice()
    }

    fn legacy_value() -> Self {
        Default::default()
    }
}
