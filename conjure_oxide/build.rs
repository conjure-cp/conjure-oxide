use std::fs::read_dir;
use std::io::Write;
use std::{env::var, fs::File, path::Path};
use walkdir::WalkDir;

fn main() {
    let out_dir = var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("gen_tests.rs");
    let mut f = File::create(&dest).unwrap();

    let test_dir = "tests/integration";

    for subdir in WalkDir::new(test_dir) {
        let subdir = subdir.unwrap();
        if subdir.file_type().is_dir() {
            let essence_file_count = read_dir(subdir.path())
                .expect("Failed to read directory")
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .map_or(false, |ext| ext == "essence")
                })
                .count();
            if essence_file_count == 1 {
                write_test(&mut f, subdir.path().display().to_string());
            }
        }
    }
}

fn write_test(file: &mut File, path: String) {
    write!(
        file,
        include_str!("./tests/gen_test_template"),
        // TODO: better sanitisation of paths to function names
        name = path.replace("./", "").replace("/", "_").replace("-", "_"),
        path = path
    )
    .unwrap();
}
