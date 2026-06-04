use clap::builder::PathBufValueParser;

use crate::util::{PathBufExt, StatusExt, workspace_dir};

use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use zzzfx::i18n::{ja, ko, zh_cn};
use zzzfx::settings::{SettingDescriptor, SettingKind, Settings};
use zzzfx::TrKey;

// ── .aul2 generation (mirrors aviutl2-plugin/src/aul2.rs) ──────────

fn collect_labels<T: Settings<Key = TrKey> + Clone>(
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

fn generate_effect_section<T: Settings<Key = TrKey> + Clone>(
    ja_filter_name: &str,
    translate: &dyn Fn(TrKey) -> String,
) -> String {
    let descriptors = T::setting_descriptors();
    let mut entries = Vec::new();
    collect_labels::<T>(&descriptors, translate, &mut entries);

    let mut seen = std::collections::HashSet::new();
    entries.retain(|(k, _)| seen.insert(k.clone()));

    let mut out = format!("[{}]\n", ja_filter_name);
    for (key, val) in &entries {
        out.push_str(&format!("{}={}\n", key, val));
    }
    out.push('\n');
    out
}

fn ja_tr(key: TrKey) -> String {
    ja::translate_cstr(key).to_str().unwrap().to_string()
}

fn generate_aul2_en() -> String {
    let translate = |k: TrKey| k.en().to_string();
    generate_all_sections(&translate)
}

fn generate_aul2_zh_cn() -> String {
    let translate = |k: TrKey| zh_cn::translate_cstr(k).to_str().unwrap().to_string();
    generate_all_sections(&translate)
}

fn generate_aul2_ko() -> String {
    let translate = |k: TrKey| ko::translate_cstr(k).to_str().unwrap().to_string();
    generate_all_sections(&translate)
}

fn generate_all_sections(translate: &dyn Fn(TrKey) -> String) -> String {
    use zzzfx::*;
    let mut out = String::new();
    out.push_str(&generate_effect_section::<StrokeFullSettings>(
        &ja_tr(TrKey::EffectStrokeName), translate,
    ));
    out.push_str(&generate_effect_section::<RepeaterFullSettings>(
        &ja_tr(TrKey::EffectRepeaterName), translate,
    ));
    out.push_str(&generate_effect_section::<SpriteSheetFullSettings>(
        &ja_tr(TrKey::EffectSpritesheetName), translate,
    ));
    out.push_str(&generate_effect_section::<AssSubtitleFullSettings>(
        &ja_tr(TrKey::EffectAssSubtitleName), translate,
    ));
    out.push_str(&generate_effect_section::<AsciiArtFullSettings>(
        &ja_tr(TrKey::EffectAsciiArtName), translate,
    ));
    out.push_str(&generate_effect_section::<PixelArtFullSettings>(
        &ja_tr(TrKey::EffectPixelArtName), translate,
    ));
    out.push_str(&generate_effect_section::<LongShadowFullSettings>(
        &ja_tr(TrKey::EffectLongShadowName), translate,
    ));
    out.push_str(&generate_effect_section::<CastShadowFullSettings>(
        &ja_tr(TrKey::EffectCastShadowName), translate,
    ));
    out.push_str(&generate_effect_section::<AmbientLightFullSettings>(
        &ja_tr(TrKey::EffectAmbientLightName), translate,
    ));
    out.push_str(&generate_effect_section::<ChromaKeyFullSettings>(
        &ja_tr(TrKey::EffectChromaKeyName), translate,
    ));
    out.push_str(&generate_effect_section::<MidiDisplayFullSettings>(
        &ja_tr(TrKey::EffectMidiDisplayName), translate,
    ));
    out.push_str(&generate_effect_section::<SvgDisplayFullSettings>(
        &ja_tr(TrKey::EffectSvgDisplayName), translate,
    ));
    out.push_str(&generate_effect_section::<LaTeXDisplayFullSettings>(
        &ja_tr(TrKey::EffectLaTeXDisplayName), translate,
    ));
    out.push_str(&generate_effect_section::<QrCodeFullSettings>(
        &ja_tr(TrKey::EffectQrCodeName), translate,
    ));
    out
}

// ── Package metadata ──────────────────────────────────────────────

const PACKAGE_TOML: &str = "\
id = \"zzzfx-aviutl2-plugin\"
name = \"zzzFX\"
version = \"0.1.0\"
information = \"zzzFX multi-effect plugin for AviUtl2\"
";

// ── xtask command ─────────────────────────────────────────────────

pub fn command() -> clap::Command {
    clap::Command::new("build-aviutl2-plugin")
        .about("Builds the AviUtl2 filter plugin (.aux2), generates language files, and packages into .au2pkg.zip.")
        .arg(
            clap::Arg::new("release")
                .long("release")
                .help("Build in release mode")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("destdir")
                .long("destdir")
                .help("Output directory for the build artifacts")
                .value_parser(PathBufValueParser::new())
                .default_value(
                    workspace_dir()
                        .plus_iter(["crates", "aviutl2-plugin", "build"])
                        .as_os_str()
                        .to_owned(),
                ),
        )
}

pub fn main(args: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let release_mode = args.get_flag("release");
    let profile = if release_mode { "release" } else { "debug" };

    let output_dir = args.get_one::<PathBuf>("destdir").unwrap();

    let dll_path = build_plugin(release_mode)?;

    fs::create_dir_all(output_dir.plus_iter(["Language"]))?;

    let auf2_path = output_dir.plus("zzzFX.aux2");
    fs::copy(&dll_path, &auf2_path)?;
    println!("Copied DLL → {}", auf2_path.display());

    // 3 .aul2 files (Japanese is built-in, no file needed)
    let en_content = generate_aul2_en();
    let en_path = output_dir.plus_iter(["Language", "English.zzzfx_aviutl2_plugin.aul2"]);
    fs::write(&en_path, &en_content)?;
    println!("Written English .aul2");

    let zh_content = generate_aul2_zh_cn();
    let zh_path = output_dir.plus_iter(["Language", "简体中文.zzzfx_aviutl2_plugin.aul2"]);
    fs::write(&zh_path, &zh_content)?;
    println!("Written 简体中文 .aul2");

    let ko_content = generate_aul2_ko();
    let ko_path = output_dir.plus_iter(["Language", "한국어.zzzfx_aviutl2_plugin.aul2"]);
    fs::write(&ko_path, &ko_content)?;
    println!("Written 한국어 .aul2");

    let pkg_path = output_dir.plus("package.txt");
    fs::write(&pkg_path, PACKAGE_TOML)?;
    println!("Written package.txt");

    let zip_path = output_dir.plus("zzzFX.au2pkg.zip");
    write_au2pkg_zip(
        &zip_path,
        &auf2_path,
        &en_path,
        &zh_path,
        &ko_path,
        &pkg_path,
    )?;
    println!("Packaged zip → {}", zip_path.display());

    println!(
        "\nBuild complete ({profile}). Output: {}",
        output_dir.display()
    );

    Ok(())
}

// ── Build helpers ──────────────────────────────────────────────────

fn build_plugin(release_mode: bool) -> Result<PathBuf, Box<dyn Error>> {
    let profile = if release_mode { "release" } else { "debug" };
    println!("Building AviUtl2 filter plugin ({profile})...");

    let mut cargo_args = vec![
        "build",
        "--package=zzzfx-aviutl2-plugin",
        "--lib",
        "--no-default-features",
    ];
    if release_mode {
        cargo_args.push("--release");
    }

    Command::new("cargo")
        .args(&cargo_args)
        .status()
        .expect_success()?;

    let dll_path =
        workspace_dir()
            .to_path_buf()
            .plus_iter(["target", profile, "zzzfx_aviutl2_plugin.dll"]);

    if !dll_path.exists() {
        return Err(format!("Build artifact not found: {}", dll_path.display()).into());
    }

    Ok(dll_path)
}

fn write_au2pkg_zip(
    zip_path: &std::path::Path,
    auf2_path: &std::path::Path,
    en_aul2_path: &std::path::Path,
    zh_aul2_path: &std::path::Path,
    ko_aul2_path: &std::path::Path,
    pkg_path: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let file = fs::File::create(zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    add_file_to_zip(
        &mut zip,
        options,
        "Plugin/zzzFX.aux2",
        &fs::read(auf2_path)?,
    )?;
    add_file_to_zip(
        &mut zip,
        options,
        "Language/English.zzzfx_aviutl2_plugin.aul2",
        &fs::read(en_aul2_path)?,
    )?;
    add_file_to_zip(
        &mut zip,
        options,
        "Language/简体中文.zzzfx_aviutl2_plugin.aul2",
        &fs::read(zh_aul2_path)?,
    )?;
    add_file_to_zip(
        &mut zip,
        options,
        "Language/한국어.zzzfx_aviutl2_plugin.aul2",
        &fs::read(ko_aul2_path)?,
    )?;
    add_file_to_zip(&mut zip, options, "package.txt", &fs::read(pkg_path)?)?;

    zip.finish()?;
    Ok(())
}

fn add_file_to_zip(
    zip: &mut zip::ZipWriter<std::fs::File>,
    options: zip::write::SimpleFileOptions,
    name: &str,
    data: &[u8],
) -> Result<(), Box<dyn Error>> {
    zip.start_file(name, options)?;
    zip.write_all(data)?;
    Ok(())
}
