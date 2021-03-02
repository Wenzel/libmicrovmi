//! This build script will run cbindgen to generate libmicrovmi C header file

use std::env;
use std::path::Path;

use cbindgen::Config;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let out_path = Path::new(&crate_dir).join("c_examples/libmicrovmi.h");

    let config = Config::from_root_or_default(&crate_dir);

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_path);

    println!("cargo:rerun-if-changed=src/capi.rs");
    // if it has been removed
    println!("cargo:rerun-if-changed=c_examples/libmicrovmi.h");
}
