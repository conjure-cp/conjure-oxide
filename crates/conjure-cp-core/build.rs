fn main() {
    println!("cargo:rerun-if-changed=src/solver/adaptors/ortools-cpsat/proto/cp_model.proto");

    prost_build::compile_protos(
        &["src/solver/adaptors/ortools-cpsat/proto/cp_model.proto"],
        &["src/solver/adaptors/ortools-cpsat/proto"],
    )
    .expect("failed to compile cp_model.proto");
}