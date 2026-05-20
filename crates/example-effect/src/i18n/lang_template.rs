//! Translation template for Example Effect — copy to bootstrap a new language.
//!
//! Covers **only `ExTrKey`** (Example Effect keys).  For zzzFX keys see
//! `zzzfx-core/src/i18n/lang_template.rs`.
//!
//! # How to add a new language
//!
//! 1. Copy this file → `ja.rs`
//! 2. Replace every `c"..."` placeholder with translated text.
//! 3. In `mod.rs`: add `mod ja;`, extend the `Lang` enum, add branches in
//!    `tr()` and `tr_cstr()`, and extend `detect_system_lang()`.
//! 4. Run `cargo check -p example-effect` to verify.

use std::ffi::CStr;
use effect_settings::ExTrKey;

pub fn translate_cstr(key: ExTrKey) -> &'static CStr {
    match key {
        ExTrKey::ParamColorRed => c"Color Red",
        ExTrKey::ParamColorRedDesc => c"Red component of the solid color.",
        ExTrKey::ParamColorGreen => c"Color Green",
        ExTrKey::ParamColorGreenDesc => c"Green component of the solid color.",
        ExTrKey::ParamColorBlue => c"Color Blue",
        ExTrKey::ParamColorBlueDesc => c"Blue component of the solid color.",
        ExTrKey::ParamBlendAmount => c"Blend Amount",
        ExTrKey::ParamBlendAmountDesc => c"Alpha channel blending. 0% = original image, 100% = solid color.",
        ExTrKey::ParamExampleBlendMode => c"Blend Mode",
        ExTrKey::ParamExampleBlendModeDesc => c"How the solid color is blended with the image.",
        ExTrKey::MenuNormal => c"Normal",
        ExTrKey::MenuMultiply => c"Multiply",
        ExTrKey::MenuScreen => c"Screen",
        ExTrKey::MenuOverlay => c"Overlay",
        ExTrKey::MenuExampleNormalDesc => c"Linear interpolation between image and solid color.",
        ExTrKey::MenuExampleMultiplyDesc => c"Multiplies the image by the solid color.",
        ExTrKey::MenuExampleScreenDesc => c"Screens the image with the solid color (inverse multiply).",
        ExTrKey::MenuExampleOverlayDesc => c"Combines Multiply and Screen based on image brightness.",
        ExTrKey::ParamColor => c"Color",
        ExTrKey::ParamColorDesc => c"Solid color for the effect.",
        ExTrKey::ParamStandardBlendMode => c"Blend Mode",
        ExTrKey::ParamStandardBlendModeDesc => c"How the solid color is blended with the image.",
        ExTrKey::ParamGroup1 => c"Group1",
        ExTrKey::ParamGroup1Desc => c"Nested group with inner parameters.",
        ExTrKey::ParamInnerFloat => c"Inner Float",
        ExTrKey::ParamInnerFloatDesc => c"A floating-point parameter inside a group.",
        ExTrKey::ParamInnerBool => c"Inner Bool",
        ExTrKey::ParamInnerBoolDesc => c"A boolean parameter inside a group.",
        ExTrKey::ParamExampleEffectName => c"Example Effect",
        ExTrKey::ParamGroup1Enabled => c"Enabled",
        ExTrKey::ParamBrightness => c"Brightness",
        ExTrKey::ParamBrightnessDesc => c"Overall brightness multiplier.",
        ExTrKey::ParamInvertColors => c"Invert Colors",
        ExTrKey::ParamInvertColorsDesc => c"Invert all colors in the image.",
        ExTrKey::ParamTintRed => c"Tint Red",
        ExTrKey::ParamTintRedDesc => c"Red channel tint multiplier.",
        ExTrKey::ParamTintGreen => c"Tint Green",
        ExTrKey::ParamTintGreenDesc => c"Green channel tint multiplier.",
        ExTrKey::ParamTintBlue => c"Tint Blue",
        ExTrKey::ParamTintBlueDesc => c"Blue channel tint multiplier.",
        ExTrKey::ParamAdvanced => c"Advanced",
        ExTrKey::ParamAdvancedDesc => c"Additional advanced settings.",
        ExTrKey::ParamContrast => c"Contrast",
        ExTrKey::ParamContrastDesc => c"Contrast adjustment.",
        ExTrKey::ParamSaturation => c"Saturation",
        ExTrKey::ParamSaturationDesc => c"Color saturation adjustment.",
        ExTrKey::ParamColorPreset => c"Color Preset",
        ExTrKey::ParamColorPresetDesc => c"Choose a color preset.",
        ExTrKey::MenuNone => c"None",
        ExTrKey::MenuNoneDesc => c"No color preset.",
        ExTrKey::MenuWarm => c"Warm",
        ExTrKey::MenuWarmDesc => c"Warm color tone.",
        ExTrKey::MenuCool => c"Cool",
        ExTrKey::MenuCoolDesc => c"Cool color tone.",
        ExTrKey::MenuSepia => c"Sepia",
        ExTrKey::MenuSepiaDesc => c"Sepia color tone.",
    }
}
