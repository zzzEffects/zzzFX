//! Builds and bundles the zzzFX After Effects plugins for macOS.
//! Uses cargo features to build one effect per .plugin bundle.

use clap::builder::PathBufValueParser;

use crate::util::targets::{MACOS_AARCH64, MACOS_X86_64, TARGETS, Target};
use crate::util::{PathBufExt, StatusExt, workspace_dir};

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn command() -> clap::Command {
    clap::Command::new("macos-zzzfx-ae-plugin")
        .about("Builds and bundles a zzzFX After Effects plugin for macOS.")
        .arg(
            clap::Arg::new("effect")
                .long("effect")
                .help("Which effect to build: stroke, repeater, or sprite-sheet")
                .value_parser(["stroke", "repeater", "sprite-sheet"])
                .default_value("stroke"),
        )
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
                .help("The directory that the After Effects plugin bundle will be output to")
                .value_parser(PathBufValueParser::new())
                .default_value(workspace_dir().plus("build").as_os_str().to_owned()),
        )
}

fn build_plugin_for_target(
    target: &Target,
    release_mode: bool,
) -> std::io::Result<(PathBuf, PathBuf)> {
    println!("Building zzzFX AE plugin for target {}", target.target_triple);

    let mut cargo_args: Vec<_> = vec![
        String::from("build"),
        String::from("--package=zzzfx-ae-plugin"),
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

    let mut target_dir_path = workspace_dir().to_path_buf();
    target_dir_path.extend(&[
        "target",
        target.target_triple,
        if cargo_args.contains(&String::from("--release")) { "release" } else { "debug" },
    ]);

    let mut built_library_path = target_dir_path.clone();
    built_library_path.push(target.library_prefix.to_owned() + "zzzfx_ae_plugin");
    built_library_path.set_extension(target.library_extension);

    let built_rsrc_path = target_dir_path.plus("zzzfx-ae-plugin.rsrc");

    Ok((built_library_path, built_rsrc_path))
}

pub fn main(args: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let release_mode = args.get_flag("release");

    let plugin_name = "zzzFX.plugin";
    let binary_name = "zzzFX";

    let build_dir_path = args.get_one::<PathBuf>("destdir").unwrap();
    let plugin_dir_path = build_dir_path.plus(plugin_name);

    let _ = fs::remove_dir_all(&plugin_dir_path);

    let contents_dir_path = plugin_dir_path.plus("Contents");
    fs::create_dir_all(&contents_dir_path)?;

    let macos_dir_path = contents_dir_path.plus("MacOS");
    fs::create_dir_all(&macos_dir_path)?;

    let resources_dir_path = contents_dir_path.plus("Resources");
    fs::create_dir_all(&resources_dir_path)?;

    fs::write(contents_dir_path.plus("PkgInfo"), "eFKTFXTC")?;

    let mut info_plist_contents = plist::dictionary::Dictionary::new();
    info_plist_contents.insert("CFBundleIdentifier".to_string(), plist::Value::from("com.example.zzzfx"));
    info_plist_contents.insert("CFBundlePackageType".to_string(), plist::Value::from("eFKT"));
    info_plist_contents.insert("CFBundleSignature".to_string(), plist::Value::from("FXTC"));
    plist::Value::Dictionary(info_plist_contents).to_file_xml(contents_dir_path.plus("Info.plist"))?;

    let (built_library_path, built_rsrc_path) = if args.get_flag("macos-universal") {
        let x86_64_target = MACOS_X86_64;
        let aarch64_target = MACOS_AARCH64;

        let (x86_64_lib_path, x86_64_rsrc_path) = build_plugin_for_target(x86_64_target, release_mode, effect)?;
        let (aarch64_lib_path, _) = build_plugin_for_target(aarch64_target, release_mode, effect)?;

        let dst_path = std::env::temp_dir().plus(format!(
            "zzzfx-ae-{}",
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
                x86_64_lib_path.into(),
                aarch64_lib_path.into(),
            ])
            .status()
            .expect_success()?;

        (dst_path, x86_64_rsrc_path)
    } else {
        let target_triple = args.get_one::<String>("target").unwrap();
        let target = TARGETS
            .iter()
            .find(|candidate_target| candidate_target.target_triple == target_triple)
            .unwrap_or_else(|| panic!("Your target \"{}\" is not supported", target_triple));
        build_plugin_for_target(target, release_mode)?
    };

    fs::copy(built_library_path, macos_dir_path.plus(binary_name))?;
    fs::copy(built_rsrc_path, resources_dir_path.plus(format!("{binary_name}.rsrc")))?;

    Ok(())
}
