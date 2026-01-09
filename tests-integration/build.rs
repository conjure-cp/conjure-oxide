use std::env::var;
use std::fs::{File, read_dir};
use std::io::{self, Write};
use std::path::Path;

use walkdir::WalkDir;

// Include the TestConfig module directly so it can be used in build.rs
// (build.rs cannot depend on the crate it's building)
#[path = "src/test_config.rs"]
mod test_config;
use test_config::TestConfig;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=tests/integration");
    println!("cargo:rerun-if-changed=tests/custom");
    println!("cargo:rerun-if-changed=tests/integration_test_template");
    println!("cargo:rerun-if-changed=tests/custom_test_template");
    println!("cargo:rerun-if-changed=tests/roundtrip_test_template");
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

                let essence_files: Vec<(String, String)> = std::iter::zip(stems, exts).collect();
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

    // Roundtrip Tests
    let dest_roundtrip = Path::new(&out_dir).join("gen_tests_roundtrip.rs");
    let mut f = File::create(dest_roundtrip)?;
    let test_dir = "tests/roundtrip";

    for subdir in WalkDir::new(test_dir) {
        let subdir = subdir?;
        // Checks every subdirectory
        if subdir.file_type().is_dir() {
            // Finds essence / eprime filenames
            let names: Vec<String> = read_dir(subdir.path())?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext == "essence" || ext == "eprime")
                })
                // Ensures not to include test result files
                .filter(|path| {
                    path.file_stem()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| {
                            !name.contains(".generated") && !name.contains(".expected")
                        })
                })
                // Stores the filename in the collected vector
                .filter_map(|path| {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .map(|s| s.to_owned())
                })
                .collect();
            // Finds essence / eprime file extensions
            let exts: Vec<String> = read_dir(subdir.path())?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext == "essence" || ext == "eprime")
                })
                // Ensures not to include test result files
                .filter(|path| {
                    path.file_stem()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| {
                            !name.contains(".generated") && !name.contains(".expected")
                        })
                })
                // Stores the extension in the collected vector
                .filter_map(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|s| s.to_owned())
                })
                .collect();

            let essence_files: Vec<(String, String)> = std::iter::zip(names, exts).collect();
            // There should only be one test file per directory
            if essence_files.len() == 1 {
                write_roundtrip_test(
                    &mut f,
                    subdir.path().display().to_string(),
                    essence_files[0].clone(),
                )?;
            }
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
        // skip tests which use smt if the feature is disabled
        let config_path = format!("{}/config.toml", path);
        let config: TestConfig = if let Ok(contents) = std::fs::read_to_string(&config_path) {
            toml::from_str(&contents).unwrap_or_default()
        } else {
            TestConfig::default()
        };

        let mut ignore_attr = "";

        if cfg!(not(feature = "smt")) && config.solve_with_smt {
            ignore_attr = "#[ignore = \"this test uses 'solve_with_smt=true', but the 'smt' feature is disabled!\"]\n"
        }

        write!(
            file,
            include_str!("./tests/integration_test_template"),
            // TODO: better sanitisation of paths to function names
            test_name = path.replace("./", "").replace(['/', '-'], "_"),
            test_dir = path,
            essence_file = essence_files[0].0,
            ext = essence_files[0].1,
            ignore_attr = ignore_attr
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

fn write_roundtrip_test(
    file: &mut File,
    path: String,
    essence_file: (String, String),
) -> io::Result<()> {
    write!(
        file,
        include_str!("./tests/roundtrip_test_template"),
        test_name = path.replace("./", "").replace(['/', '-'], "_"),
        test_dir = path,
        essence_file = essence_file.0,
        ext = essence_file.1
    )
}
