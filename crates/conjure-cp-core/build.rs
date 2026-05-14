fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR");
    let base_path = "src/solver/adaptors/ortools-cpsat";
    let proto_file = format!("{}/proto/cp_model.proto", base_path);

    println!("cargo:rerun-if-changed={}", proto_file);
    println!("cargo:rerun-if-changed={}/wrapper.cpp", base_path);
    println!("cargo:rerun-if-changed={}/wrapper.hpp", base_path);
    println!("cargo:rerun-if-changed={}/mod.rs", base_path);

    prost_build::compile_protos(
        &[proto_file],
        &[format!("{}/proto", base_path)],
    ).expect("failed to compile cp_model.proto");

    cxx_build::bridge(format!("{}/mod.rs", base_path))
            .file(format!("{}/wrapper.cpp", base_path))
            .include("/usr/include") 
            .include(&manifest_dir)
            .flag_if_supported("-std=c++17")
            .flag_if_supported("-fexceptions") 
            .flag_if_supported("-DABSL_LEGACY_THREAD_ANNOTATIONS")
            .compile("ortools-wrapper");

    println!("cargo:rustc-link-lib=ortools");
    println!("cargo:rustc-link-lib=protobuf");
}