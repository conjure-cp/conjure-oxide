use std::env;
use std::process::Command;

fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    let tree_sitter_cli = env::var("CARGO_BIN_EXE_tree-sitter").expect("tree-sitter-cli not found");

    std::process::Command::new(tree_sitter_cli)
        .arg("generate")
        .status()
        .expect("Failed to run tree-sitter generate");

    let parser_path = src_dir.join("parser.c");
    c_config.file(&parser_path);
    println!("cargo:rerun-if-changed={}", parser_path.to_str().unwrap());

    c_config.compile("tree-sitter-essence");
}
