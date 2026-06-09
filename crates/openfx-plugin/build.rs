use std::env;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown-target".to_string())
    );

    // --- bindgen: generate raw OFX C bindings ---
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

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("bindings.rs");
    bindings
        .write_to_file(&out_path)
        .expect("Couldn't write bindings!");

}
