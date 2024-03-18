use std::env::var;
use std::fs::{File, read_dir};
use std::io::{self, Write};
use std::path::Path;

use walkdir::WalkDir;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=tests/integration");
    println!("cargo:rerun-if-changed=tests/gen_test_template");
    println!("cargo:rerun-if-changed=build.rs");
    let out_dir = var("OUT_DIR").map_err(|e| io::Error::new(io::ErrorKind::Other, e))?; // wrapping in a std::io::Error to match main's error type
    let dest = Path::new(&out_dir).join("gen_tests.rs");
    let mut f = File::create(dest)?;

    let test_dir = "tests/integration";

    for subdir in WalkDir::new(test_dir) {
        let subdir = subdir?;
        if subdir.file_type().is_dir() {
            let essence_files: Vec<String> = read_dir(subdir.path())?
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .map_or(false, |ext| ext == "essence")
                })
                .filter_map(|entry| {
                    entry
                        .path()
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(|s| s.to_owned())
                })
                .collect();

            write_test(&mut f, subdir.path().display().to_string(), essence_files)?;
        }
    }

    Ok(())
}

fn write_test(file: &mut File, path: String, essence_files: Vec<String>) -> io::Result<()> {
    // TODO: Consider supporting multiple Essence files?
    if essence_files.len() == 1 {
        write!(
            file,
            include_str!("./tests/gen_test_template"),
            // TODO: better sanitisation of paths to function names
            test_name = path.replace("./", "").replace(['/', '-'], "_"),
            test_dir = path,
            essence_file = essence_files[0]
        )
    } else {
        Ok(())
    }
}
