use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let local_ortools_prefix = PathBuf::from(&manifest_dir).join("../../.ortools");

    if local_ortools_prefix.exists() {
        let lib_path = local_ortools_prefix.join("lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    } else if let Ok(prefix) = env::var("ORTOOLS_PREFIX") {
        let lib_path = PathBuf::from(&prefix).join("lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }
}
