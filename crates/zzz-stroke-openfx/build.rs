use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .blocklist_function("OfxGetNumberOfPlugins")
        .blocklist_function("OfxGetPlugin")
        .blocklist_type("OfxStatus")
        .blocklist_var("kOfxStat.+")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate_cstr(true)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
