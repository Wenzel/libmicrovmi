//! This build script will run cbindgen to generate libmicrovmi C header file

use std::env;
use std::fs;
use std::path::PathBuf;

use cbindgen::Config;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    // a little hack to write outside of OUT_DIR
    // https://github.com/mmstick/cargo-deb/blob/e43018a46b8dc922cfdf6cdde12f7ed92fcc41aa/example/build.rs
    // libmicrovmi issue: https://github.com/Wenzel/libmicrovmi/issues/177
    let out_dir = env::var("OUT_DIR").unwrap();
    let mut out_path = PathBuf::from(&out_dir)
        .ancestors() // .../target/<debug|release>/build/example-<SHA>/out
        .nth(3) // .../target/<debug|release>
        .unwrap()
        .to_owned();
    out_path.push("capi");
    // .../target/<debug|release>/capi/

    if !out_path.exists() {
        fs::create_dir(&out_path).expect("Could not create capi dir");
    }
    // .../target/<debug|release>/capi/libmicrovmi.h
    out_path.push("libmicrovmi.h");

    let config = Config::from_root_or_default(&crate_dir);

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(&out_path);

    println!("cargo:rerun-if-changed=src/capi.rs");
    // if it has been removed
    println!(
        "{}",
        format!("cargo:rerun-if-changed={}", &out_path.display())
    );
}
