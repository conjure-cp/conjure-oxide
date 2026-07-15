fn main() {
    println!("cargo::rustc-check-cfg=cfg(no_ortools)");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let base_path = "src/solver/adaptors/ortools-cpsat";

    let local_ortools_prefix = std::path::Path::new(&manifest_dir).join("../../.ortools");
    let has_local_ortools = local_ortools_prefix.join("include/ortools/base/base_export.h").exists();

    // Auto-detect if OR-Tools is installed on the system
    let has_ortools = has_local_ortools
        || std::path::Path::new("/usr/include/ortools/base/base_export.h").exists()
        || std::path::Path::new("/usr/local/include/ortools/base/base_export.h").exists()
        || std::env::var("ORTOOLS_PREFIX")
            .map(|p| std::path::Path::new(&p).join("include/ortools/base/base_export.h").exists())
            .unwrap_or(false);

    if !has_ortools {
        println!("cargo:warning=OR-Tools C++ library not found on the system. Compiling without OR-Tools support.");
        println!("cargo:rustc-cfg=no_ortools");
        return;
    }

    let proto_file = format!("{}/proto/cp_model.proto", base_path);

    println!("cargo:rerun-if-changed={}", proto_file);
    println!("cargo:rerun-if-changed={}/wrapper.cpp", base_path);
    println!("cargo:rerun-if-changed={}/wrapper.hpp", base_path);
    println!("cargo:rerun-if-changed={}/mod.rs", base_path);

    let mut config = prost_build::Config::new();
    config.protoc_executable(protobuf_src::protoc());
    config.compile_protos(
        &[proto_file],
        &[format!("{}/proto", base_path)],
    ).expect("failed to compile cp_model.proto");

    let mut builder = cxx_build::bridge(format!("{}/mod.rs", base_path));
    builder.file(format!("{}/wrapper.cpp", base_path))
            .include("/usr/local/include") 
            .include("/usr/include") 
            .include(&manifest_dir)
            .define("NDEBUG", None)
            .flag_if_supported("-std=c++17")
            .flag_if_supported("-fexceptions") 
            .flag_if_supported("-DABSL_LEGACY_THREAD_ANNOTATIONS");

    if has_local_ortools {
        builder.include(local_ortools_prefix.join("include"));
    }
    if let Ok(prefix) = std::env::var("ORTOOLS_PREFIX") {
        builder.include(std::path::Path::new(&prefix).join("include"));
    }

    builder.compile("ortools-wrapper");

    println!("cargo:rustc-link-search=native=/usr/local/lib");
    if has_local_ortools {
        let lib_path = local_ortools_prefix.join("lib");
        println!("cargo:rustc-link-search=native={}", lib_path.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }
    if let Ok(prefix) = std::env::var("ORTOOLS_PREFIX") {
        let lib_path = std::path::Path::new(&prefix).join("lib");
        println!("cargo:rustc-link-search=native={}", lib_path.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());
    }
    println!("cargo:rustc-link-lib=ortools");
    
    // Abseil dependencies required by inline templates in OR-Tools headers
    println!("cargo:rustc-link-lib=absl_raw_hash_set");
    println!("cargo:rustc-link-lib=absl_raw_logging_internal");
    println!("cargo:rustc-link-lib=absl_log_internal_check_op");
    println!("cargo:rustc-link-lib=absl_log_internal_message");
    
    println!("cargo:rustc-link-lib=protobuf");
}