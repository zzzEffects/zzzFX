//! Builds the zzzFX OpenFX plugin (bundles zzzStroke, zzzRepeater, zzzSpriteSheet) and bundles it.

use clap::builder::PathBufValueParser;

use crate::util::targets::{MACOS_AARCH64, MACOS_X86_64, TARGETS, Target};
use crate::util::{PathBufExt, StatusExt, workspace_dir};

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn command() -> clap::Command {
    clap::Command::new("build-ofx-plugin")
        .about("Builds and bundles the zzzFX OpenFX plugin (Stroke + Repeater + SpriteSheet).")
        .arg(
            clap::Arg::new("release")
                .long("release")
                .help("Build the plugin in release mode")
                .conflicts_with("debug")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("debug")
                .long("debug")
                .help("Build the plugin in debug mode")
                .conflicts_with("release")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("target")
                .long("target")
                .help("Set the target triple to compile for")
                .default_value(current_platform::CURRENT_PLATFORM),
        )
        .arg(
            clap::Arg::new("macos-universal")
                .long("macos-universal")
                .help("Build a macOS universal library (x86_64 and aarch64)")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("target"),
        )
        .arg(
            clap::Arg::new("destdir")
                .long("destdir")
                .help("The directory that the OpenFX plugin bundle will be output to")
                .value_parser(PathBufValueParser::new())
                .default_value(
                    workspace_dir()
                        .plus_iter(["crates", "openfx-plugin", "build"])
                        .as_os_str()
                        .to_owned(),
                ),
        )
}

fn get_info_plist() -> plist::Value {
    let cargo_toml_path = workspace_dir().plus_iter(["crates", "openfx-plugin", "Cargo.toml"]);
    let manifest = cargo_toml::Manifest::from_path(cargo_toml_path).unwrap();
    let version = manifest.package().version();

    let mut info_plist_contents = plist::dictionary::Dictionary::new();
    info_plist_contents.insert("CFBundleInfoDictionaryVersion".to_string(), plist::Value::from("6.0"));
    info_plist_contents.insert("CFBundleDevelopmentRegion".to_string(), plist::Value::from("en"));
    info_plist_contents.insert("CFBundlePackageType".to_string(), plist::Value::from("BNDL"));
    info_plist_contents.insert("CFBundleIdentifier".to_string(), plist::Value::from("com.example.zzzfx"));
    info_plist_contents.insert("CFBundleVersion".to_string(), plist::Value::from(version));
    info_plist_contents.insert("CFBundleShortVersionString".to_string(), plist::Value::from(version));
    info_plist_contents.insert("NSHumanReadableCopyright".to_string(), plist::Value::from("zzzFX Plugin"));
    info_plist_contents.insert("CFBundleSignature".to_string(), plist::Value::from("????"));
    plist::Value::Dictionary(info_plist_contents)
}

fn build_plugin_for_target(target: &Target, release_mode: bool) -> std::io::Result<PathBuf> {
    println!("Building zzzFX OFX plugin for target {}", target.target_triple);

    let mut cargo_args: Vec<_> = vec![
        String::from("build"),
        String::from("--package=zzzfx-openfx-plugin"),
        String::from("--lib"),
        String::from("--target"),
        target.target_triple.to_string(),
    ];
    if release_mode {
        cargo_args.push(String::from("--release"));
    }
    Command::new("cargo")
        .args(&cargo_args)
        .status()
        .expect_success()?;

    let target_dir_path = workspace_dir().to_path_buf().plus_iter([
        "target",
        target.target_triple,
        if cargo_args.contains(&String::from("--release")) { "release" } else { "debug" },
    ]);

    let mut built_library_path = target_dir_path.plus(target.library_prefix.to_owned() + "zzzfx_openfx_plugin");
    built_library_path.set_extension(target.library_extension);

    Ok(built_library_path)
}

pub fn main(args: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let release_mode = args.get_flag("release");

    let (built_library_path, ofx_architecture) = if args.get_flag("macos-universal") {
        let x86_64_target = MACOS_X86_64;
        let aarch64_target = MACOS_AARCH64;
        let x86_64_path = build_plugin_for_target(x86_64_target, release_mode)?;
        let aarch64_path = build_plugin_for_target(aarch64_target, release_mode)?;

        let dst_path = std::env::temp_dir().plus(format!(
            "zzzfx-ofx-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));

        Command::new("lipo")
            .args(&[
                OsString::from("-create"),
                OsString::from("-output"),
                dst_path.clone().into(),
                x86_64_path.into(),
                aarch64_path.into(),
            ])
            .status()
            .expect_success()?;

        assert_eq!(x86_64_target.ofx_architecture, aarch64_target.ofx_architecture);
        (dst_path, x86_64_target.ofx_architecture)
    } else {
        let target_triple = args.get_one::<String>("target").unwrap();
        let target = TARGETS
            .iter()
            .find(|candidate_target| candidate_target.target_triple == target_triple)
            .unwrap_or_else(|| {
                eprintln!("Error: target \"{target_triple}\" is not supported. Available targets:");
                for t in TARGETS {
                    eprintln!("  {}", t.target_triple);
                }
                std::process::exit(1);
            });
        (build_plugin_for_target(target, release_mode)?, target.ofx_architecture)
    };

    let output_dir = args.get_one::<PathBuf>("destdir").unwrap();

    let plugin_bundle_path = output_dir.plus_iter(["zzzFX.ofx.bundle", "Contents"]);
    let plugin_bin_path = plugin_bundle_path.plus_iter([ofx_architecture, "zzzFX.ofx"]);
    let plugin_resources_path = plugin_bundle_path.plus_iter(["Resources"]);

    fs::create_dir_all(plugin_bin_path.parent().unwrap())?;
    fs::create_dir_all(&plugin_resources_path)?;
    fs::copy(built_library_path, plugin_bin_path)?;
    if ofx_architecture == "MacOS" {
        get_info_plist().to_file_xml(plugin_bundle_path.plus("Info.plist"))?;
    }

    Ok(())
}
