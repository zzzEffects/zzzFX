//! cargo-xtask provides a platform-independent way to run build scripts by writing them in Rust.
//! See https://github.com/matklad/cargo-xtask for more information.

use std::process;

use xtask::{build_ofx_plugin, macos_ae_plugin};

fn main() {
    let cmd = clap::Command::new("xtask")
        .subcommand_required(true)
        .subcommand(build_ofx_plugin::command())
        .subcommand(macos_ae_plugin::command());

    let matches = cmd.get_matches();

    let (task, args) = matches.subcommand().unwrap();

    match task {
        "macos-ae-plugin" => {
            macos_ae_plugin::main(args).unwrap();
        }
        "build-ofx-plugin" => {
            build_ofx_plugin::main(args).unwrap();
        }
        _ => {
            println!("Invalid xtask: {task}");
            process::exit(1);
        }
    }
}
