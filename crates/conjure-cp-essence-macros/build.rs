use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let local_ortools_prefix = PathBuf::from(&manifest_dir).join("../../.ortools");
    let has_local_ortools = local_ortools_prefix.join("include/ortools/base/base_export.h").exists();

    if has_local_ortools {
        let lib_path = local_ortools_prefix.join("lib");
        // Proc-macros need rpath explicitly because Cargo doesn't propagate it from rlibs (like conjure-cp-core)
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    } else if let Ok(prefix) = env::var("ORTOOLS_PREFIX") {
        let lib_path = PathBuf::from(&prefix).join("lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }
}
