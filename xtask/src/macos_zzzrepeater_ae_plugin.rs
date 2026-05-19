//! Builds the zzzRepeater After Effects plugin and bundles it on macOS.

use std::error::Error;
use std::path::PathBuf;

use crate::util::{PathBufExt, workspace_dir};

pub fn command() -> clap::Command {
    clap::Command::new("macos-zzzrepeater-ae-plugin")
        .about("Builds and bundles the zzzRepeater After Effects plugin (macOS only).")
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
}

pub fn main(args: &clap::ArgMatches) -> Result<(), Box<dyn Error>> {
    let release_mode = args.get_flag("release");

    // Determine target architecture
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("The AE plugin build is only supported on macOS.");
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::OsString;
        use std::fs;
        use std::process::Command;

        let mut cargo_args = vec![
            String::from("build"),
            String::from("--package=zzz-repeater-ae-plugin"),
            String::from("--lib"),
        ];
        if release_mode {
            cargo_args.push(String::from("--release"));
        }

        Command::new("cargo")
            .args(&cargo_args)
            .status()
            .map_err(|e| format!("Failed to build: {}", e))?;

        let target_dir = workspace_dir()
            .plus_iter(["target", if release_mode { "release" } else { "debug" }]);
        let lib_name = format!(
            "{}zzz_repeater_ae_plugin.dylib",
            if cfg!(target_os = "macos") { "lib" } else { "" }
        );
        let built_lib_path = target_dir.plus(&lib_name);

        let plugin_dir = workspace_dir().plus_iter(["target", "ZZZRepeater.plugin"]);
        let plugin_contents = plugin_dir.plus_iter(["Contents"]);
        let plugin_macos = plugin_contents.plus_iter(["MacOS"]);
        let plugin_resources = plugin_contents.plus_iter(["Resources"]);

        fs::create_dir_all(&plugin_macos)?;
        fs::create_dir_all(&plugin_resources)?;

        fs::copy(&built_lib_path, plugin_macos.plus("ZZZRepeater"))?;

        let rsrc_path = target_dir.plus("zzz-repeater-ae-plugin.rsrc");
        if rsrc_path.exists() {
            fs::copy(&rsrc_path, plugin_resources.plus("ZZZRepeater.rsrc"))?;
        }

        // Create Info.plist
        let mut info_plist = plist::dictionary::Dictionary::new();
        info_plist.insert("CFBundleName".to_string(), plist::Value::from("ZZZ Repeater"));
        info_plist.insert("CFBundleIdentifier".to_string(), plist::Value::from("com.example.zzzrepeater"));
        info_plist.insert("CFBundleInfoDictionaryVersion".to_string(), plist::Value::from("6.0"));
        info_plist.insert("CFBundlePackageType".to_string(), plist::Value::from("BNDL"));
        info_plist.insert("CFBundleSignature".to_string(), plist::Value::from("????"));
        info_plist.insert("CFBundleVersion".to_string(), plist::Value::from("0.1.0"));
        info_plist.insert("CFBundleShortVersionString".to_string(), plist::Value::from("0.1.0"));
        plist::Value::Dictionary(info_plist).to_file_xml(plugin_contents.plus("Info.plist"))?;

        println!("AE plugin built at: {}", plugin_dir.display());
    }

    Ok(())
}
