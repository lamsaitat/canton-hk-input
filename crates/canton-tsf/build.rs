use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let def_path = manifest_dir.join("canton_ime.def");
    println!("cargo:rerun-if-changed={}", def_path.display());
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-cdylib-link-arg=/DEF:{}", def_path.display());
    } else {
        println!("cargo:rustc-cdylib-link-arg={}", def_path.display());
    }
}
