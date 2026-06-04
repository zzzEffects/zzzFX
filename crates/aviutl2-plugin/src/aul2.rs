use std::io;
use std::path::Path;

use zzzfx::i18n::{ja, ko, zh_cn};
use zzzfx::settings::{SettingDescriptor, SettingKind, Settings};
use zzzfx::TrKey;

/// Recursively collect (Japanese key, target-language value) pairs from all descriptors.
fn collect_labels<T: Settings<Key = zzzfx::TrKey> + Clone>(
    descriptors: &[SettingDescriptor<T>],
    translate: &dyn Fn(TrKey) -> String,
    entries: &mut Vec<(String, String)>,
) {
    for desc in descriptors {
        let ja_key = ja::translate_cstr(desc.label_key)
            .to_str()
            .unwrap()
            .to_string();
        let val = translate(desc.label_key);
        entries.push((ja_key, val));

        if let SettingKind::Enumeration { options } = &desc.kind {
            for opt in options {
                let ja_key = ja::translate_cstr(opt.label_key)
                    .to_str()
                    .unwrap()
                    .to_string();
                let val = translate(opt.label_key);
                entries.push((ja_key, val));
            }
        }

        if let SettingKind::Group { children } = &desc.kind {
            collect_labels::<T>(children, translate, entries);
        }
    }
}

/// Generate aul2 section for one effect.
fn generate_effect_section<T: Settings<Key = zzzfx::TrKey> + Clone>(
    ja_filter_name: &str,
    translate: &dyn Fn(TrKey) -> String,
) -> String {
    let descriptors = T::setting_descriptors();
    let mut entries = Vec::new();
    collect_labels::<T>(&descriptors, translate, &mut entries);

    // Deduplicate by key
    let mut seen = std::collections::HashSet::new();
    entries.retain(|(k, _)| seen.insert(k.clone()));

    let mut out = format!("[{}]\n", ja_filter_name);
    for (key, val) in &entries {
        out.push_str(&format!("{}={}\n", key, val));
    }
    out.push('\n');
    out
}

// ── Japanese filter names (from TrKey, translated to Japanese) ──

fn ja_stroke_name() -> String {
    ja::translate_cstr(TrKey::EffectStrokeName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_repeater_name() -> String {
    ja::translate_cstr(TrKey::EffectRepeaterName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_spritesheet_name() -> String {
    ja::translate_cstr(TrKey::EffectSpritesheetName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_ass_subtitle_name() -> String {
    ja::translate_cstr(TrKey::EffectAssSubtitleName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_ascii_art_name() -> String {
    ja::translate_cstr(TrKey::EffectAsciiArtName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_pixel_art_name() -> String {
    ja::translate_cstr(TrKey::EffectPixelArtName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_long_shadow_name() -> String {
    ja::translate_cstr(TrKey::EffectLongShadowName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_cast_shadow_name() -> String {
    ja::translate_cstr(TrKey::EffectCastShadowName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_ambient_light_name() -> String {
    ja::translate_cstr(TrKey::EffectAmbientLightName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_chroma_key_name() -> String {
    ja::translate_cstr(TrKey::EffectChromaKeyName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_midi_display_name() -> String {
    ja::translate_cstr(TrKey::EffectMidiDisplayName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_svg_display_name() -> String {
    ja::translate_cstr(TrKey::EffectSvgDisplayName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_latex_display_name() -> String {
    ja::translate_cstr(TrKey::EffectLaTeXDisplayName)
        .to_str()
        .unwrap()
        .to_string()
}
fn ja_qr_code_name() -> String {
    ja::translate_cstr(TrKey::EffectQrCodeName)
        .to_str()
        .unwrap()
        .to_string()
}

// ── English .aul2: Japanese keys → English values ──

pub fn generate_aul2_en() -> String {
    let translate = |k: TrKey| k.en().to_string();
    let mut out = String::new();
    use zzzfx::*;
    out.push_str(&generate_effect_section::<StrokeFullSettings>(
        &ja_stroke_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<RepeaterFullSettings>(
        &ja_repeater_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<SpriteSheetFullSettings>(
        &ja_spritesheet_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AssSubtitleFullSettings>(
        &ja_ass_subtitle_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AsciiArtFullSettings>(
        &ja_ascii_art_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<PixelArtFullSettings>(
        &ja_pixel_art_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<LongShadowFullSettings>(
        &ja_long_shadow_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<CastShadowFullSettings>(
        &ja_cast_shadow_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AmbientLightFullSettings>(
        &ja_ambient_light_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<ChromaKeyFullSettings>(
        &ja_chroma_key_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<MidiDisplayFullSettings>(
        &ja_midi_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<SvgDisplayFullSettings>(
        &ja_svg_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<LaTeXDisplayFullSettings>(
        &ja_latex_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<QrCodeFullSettings>(
        &ja_qr_code_name(),
        &translate,
    ));
    out
}

// ── Chinese .aul2: Japanese keys → Chinese values ──

pub fn generate_aul2_zh_cn() -> String {
    let translate = |k: TrKey| zh_cn::translate_cstr(k).to_str().unwrap().to_string();
    let mut out = String::new();
    use zzzfx::*;
    out.push_str(&generate_effect_section::<StrokeFullSettings>(
        &ja_stroke_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<RepeaterFullSettings>(
        &ja_repeater_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<SpriteSheetFullSettings>(
        &ja_spritesheet_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AssSubtitleFullSettings>(
        &ja_ass_subtitle_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AsciiArtFullSettings>(
        &ja_ascii_art_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<PixelArtFullSettings>(
        &ja_pixel_art_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<LongShadowFullSettings>(
        &ja_long_shadow_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<CastShadowFullSettings>(
        &ja_cast_shadow_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AmbientLightFullSettings>(
        &ja_ambient_light_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<ChromaKeyFullSettings>(
        &ja_chroma_key_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<MidiDisplayFullSettings>(
        &ja_midi_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<SvgDisplayFullSettings>(
        &ja_svg_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<LaTeXDisplayFullSettings>(
        &ja_latex_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<QrCodeFullSettings>(
        &ja_qr_code_name(),
        &translate,
    ));
    out
}

// ── Korean .aul2: Japanese keys → Korean values ──

pub fn generate_aul2_ko() -> String {
    let translate = |k: TrKey| ko::translate_cstr(k).to_str().unwrap().to_string();
    let mut out = String::new();
    use zzzfx::*;
    out.push_str(&generate_effect_section::<StrokeFullSettings>(
        &ja_stroke_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<RepeaterFullSettings>(
        &ja_repeater_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<SpriteSheetFullSettings>(
        &ja_spritesheet_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AssSubtitleFullSettings>(
        &ja_ass_subtitle_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AsciiArtFullSettings>(
        &ja_ascii_art_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<PixelArtFullSettings>(
        &ja_pixel_art_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<LongShadowFullSettings>(
        &ja_long_shadow_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<CastShadowFullSettings>(
        &ja_cast_shadow_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<AmbientLightFullSettings>(
        &ja_ambient_light_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<ChromaKeyFullSettings>(
        &ja_chroma_key_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<MidiDisplayFullSettings>(
        &ja_midi_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<SvgDisplayFullSettings>(
        &ja_svg_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<LaTeXDisplayFullSettings>(
        &ja_latex_display_name(),
        &translate,
    ));
    out.push_str(&generate_effect_section::<QrCodeFullSettings>(
        &ja_qr_code_name(),
        &translate,
    ));
    out
}

pub fn write_aul2_to<P: AsRef<Path>>(dir: P, lang: &str, content: &str) -> io::Result<()> {
    let filename = format!("{}.zzzfx_aviutl2_plugin.aul2", lang);
    let path = dir.as_ref().join(filename);
    std::fs::write(&path, content)?;
    Ok(())
}
