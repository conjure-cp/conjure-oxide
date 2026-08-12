use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let local_ortools_prefix = PathBuf::from(&manifest_dir).join("../../.ortools");
    let has_local_ortools = local_ortools_prefix
        .join("include/ortools/base/base_export.h")
        .exists();

    if has_local_ortools {
        let lib_path = local_ortools_prefix.join("lib");
        // Proc-macros need rpath explicitly because Cargo doesn't propagate it from rlibs (like conjure-cp-core)
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    } else if let Ok(prefix) = env::var("ORTOOLS_PREFIX") {
        let lib_path = PathBuf::from(&prefix).join("lib");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }

    // paths to search for Z3 dylib
    let candidates: Vec<PathBuf> = [
        // macOS
        "/opt/homebrew/lib",
        "/usr/local/lib",
        // linux
        "/usr/lib",
        "/usr/lib64",
        "/lib",
        "/lib64",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();

    let mut valid_paths = Vec::new();

    // check possible lib names
    for dir in candidates {
        if dir.join("libz3.dylib").exists()
            || dir.join("libz3.so").exists()
            || dir.join("libz3.a").exists()
        {
            valid_paths.push(dir);
        }
    }
    // if all else failed, maybe it is local?
    valid_paths.push(PathBuf::from("@executable_path/../lib"));

    // do some horrible linker things
    for path in &valid_paths {
        if let Some(p) = path.to_str() {
            println!("cargo:rustc-link-search=native={}", p);
        }
    }
}
