use std::env;
use std::error::Error;
use std::fs::read_dir;

use conjure_cp::context::Context;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use std::sync::Arc;
use std::sync::RwLock;

fn list_features_test() -> Result<(), Box<dyn Error>> {
    let mut failing = 0;
    let mut total = 0;
    let list_dir = "tests/essence_list";

    for file in read_dir(list_dir)? {
        let file = file?;
        let path = file.path();
        if path.is_file() && (path.extension().is_some_and(|ext| ext == "essence")) {
            let context: Arc<RwLock<Context<'static>>> = Default::default();
            let parsed = parse_essence_file(&path.display().to_string(), context.clone());
            total += 1;
            if parsed.is_err() {
                failing += 1;
                println!(
                    "Failed to parse file {}: {}",
                    file.file_name().display().to_string(),
                    parsed.unwrap_err().to_string()
                );
            }
        }
    }
    println!("Success = {} / {}", total - failing, total);
    assert_eq!(failing, 0);
    Ok(())
}

include!(concat!(env!("OUT_DIR"), "/gen_tests_list_features.rs"));
