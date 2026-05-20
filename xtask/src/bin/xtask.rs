//! cargo-xtask provides a platform-independent way to run build scripts by writing them in Rust.
//! See https://github.com/matklad/cargo-xtask for more information.

use std::process;

use xtask::{build_ofx_plugin, build_zzzrepeater_ofx_plugin, build_zzzsprite_sheet_ofx_plugin, build_zzzstroke_ofx_plugin, macos_ae_plugin, macos_zzzrepeater_ae_plugin, macos_zzzstroke_ae_plugin};

fn main() {
    let cmd = clap::Command::new("xtask")
        .subcommand_required(true)
        .subcommand(build_ofx_plugin::command())
        .subcommand(build_zzzrepeater_ofx_plugin::command())
        .subcommand(build_zzzstroke_ofx_plugin::command())
        .subcommand(build_zzzsprite_sheet_ofx_plugin::command())
        .subcommand(macos_ae_plugin::command())
        .subcommand(macos_zzzrepeater_ae_plugin::command())
        .subcommand(macos_zzzstroke_ae_plugin::command());

    let matches = cmd.get_matches();

    let (task, args) = matches.subcommand().unwrap();

    match task {
        "macos-ae-plugin" => {
            macos_ae_plugin::main(args).unwrap();
        }
        "macos-zzzrepeater-ae-plugin" => {
            macos_zzzrepeater_ae_plugin::main(args).unwrap();
        }
        "macos-zzzstroke-ae-plugin" => {
            macos_zzzstroke_ae_plugin::main(args).unwrap();
        }
        "build-ofx-plugin" => {
            build_ofx_plugin::main(args).unwrap();
        }
        "build-zzzrepeater-ofx-plugin" => {
            build_zzzrepeater_ofx_plugin::main(args).unwrap();
        }
        "build-zzzstroke-ofx-plugin" => {
            build_zzzstroke_ofx_plugin::main(args).unwrap();
        }
        "build-zzzsprite-sheet-ofx-plugin" => {
            build_zzzsprite_sheet_ofx_plugin::main(args).unwrap();
        }
        _ => {
            println!("Invalid xtask: {task}");
            process::exit(1);
        }
    }
}
