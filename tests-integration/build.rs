use std::env::var;
use std::fs::{File, read_dir};
use std::io::{self, Write};
use std::path::Path;

use walkdir::WalkDir;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=tests/integration");
    println!("cargo:rerun-if-changed=tests/custom");
    println!("cargo:rerun-if-changed=tests/integration_test_template");
    println!("cargo:rerun-if-changed=tests/custom_test_template");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = var("OUT_DIR").map_err(io::Error::other)?; // wrapping in a std::io::Error to match main's error type

    // Integration Tests
    let dest = Path::new(&out_dir).join("gen_tests.rs");
    let mut f = File::create(dest)?;
    let test_dir = "tests/integration";

    for subdir in WalkDir::new(test_dir) {
        let subdir = subdir?;
        if subdir.file_type().is_dir() {
            if std::env::var("ALLTEST").is_ok() {
                let stems: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry.path().extension().is_some_and(|ext| {
                            ext == "essence" || ext == "eprime" || ext == "disabled"
                        })
                    })
                    .filter_map(|entry| {
                        entry
                            .path()
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .map(|s| s.to_owned())
                    })
                    .collect();

                let exts: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry.path().extension().is_some_and(|ext| {
                            ext == "essence" || ext == "eprime" || ext == "disabled"
                        })
                    })
                    .filter_map(|entry| {
                        entry
                            .path()
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|s| s.to_owned())
                    })
                    .collect();

                let essence_files = std::iter::zip(stems, exts).collect();

                write_integration_test(&mut f, subdir.path().display().to_string(), essence_files)?;
            } else {
                let stems: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry
                            .path()
                            .extension()
                            .is_some_and(|ext| ext == "essence" || ext == "eprime")
                    })
                    .filter_map(|entry| {
                        entry
                            .path()
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .map(|s| s.to_owned())
                    })
                    .collect();

                let exts: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry
                            .path()
                            .extension()
                            .is_some_and(|ext| ext == "essence" || ext == "eprime")
                    })
                    .filter_map(|entry| {
                        entry
                            .path()
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|s| s.to_owned())
                    })
                    .collect();

                let essence_files = std::iter::zip(stems, exts).collect();

                write_integration_test(&mut f, subdir.path().display().to_string(), essence_files)?;
            }
        }
    }

    // Custom Tests
    let dest_custom = Path::new(&out_dir).join("gen_tests_custom.rs");
    let mut f = File::create(dest_custom)?;
    let test_dir = "tests/custom";

    for subdir in WalkDir::new(test_dir) {
        let subdir = subdir?;
        if subdir.file_type().is_dir()
            && read_dir(subdir.path())
                .unwrap_or_else(|_| std::fs::read_dir(subdir.path()).unwrap())
                .filter_map(Result::ok)
                .any(|entry| entry.file_name() == "run.sh" && entry.path().is_file())
        {
            write_custom_test(&mut f, subdir.path().display().to_string())?;
        }
    }

    Ok(())
}

fn write_integration_test(
    file: &mut File,
    path: String,
    essence_files: Vec<(String, String)>,
) -> io::Result<()> {
    // TODO: Consider supporting multiple Essence files?
    if essence_files.len() == 1 {
        write!(
            file,
            include_str!("./tests/integration_test_template"),
            // TODO: better sanitisation of paths to function names
            test_name = path.replace("./", "").replace(['/', '-'], "_"),
            test_dir = path,
            essence_file = essence_files[0].0,
            ext = essence_files[0].1
        )
    } else {
        Ok(())
    }
}

fn write_custom_test(file: &mut File, path: String) -> io::Result<()> {
    write!(
        file,
        include_str!("./tests/custom_test_template"),
        test_name = path.replace("./", "").replace(['/', '-'], "_"),
        test_dir = path
    )
}
