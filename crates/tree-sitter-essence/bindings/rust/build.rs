use std::path::Path;
use tree_sitter_generate::generate_parser_in_directory;

fn main() {
    println!("cargo:rerun-if-changed=grammar.js");

    let src_dir = Path::new("src");

    generate_parser_in_directory(Path::new(""), Some("grammar.js"), 13, None, None)
        .expect("Failed to generate parser");

    let mut c_config = cc::Build::new();
    c_config.std("c11").include(src_dir);

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    let parser_path = src_dir.join("parser.c");
    c_config.file(&parser_path);

    c_config.compile("tree-sitter-essence");
}
