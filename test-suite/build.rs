use std::env::var;
use std::fs;
use std::fs::{File, read_dir};
use std::io::{self, Write};
use std::path::Path;

use std::collections::HashSet;

use walkdir::WalkDir;

// Include the TestConfig and RunCase modules directly so it can be used in build.rs
// (build.rs cannot depend on the crate it's building)
#[path = "src/test_config.rs"]
mod test_config;
use test_config::RunCase;
use test_config::TestConfig;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=tests/integration");
    println!("cargo:rerun-if-changed=tests/custom");
    println!("cargo:rerun-if-changed=tests/roundtrip");
    println!("cargo:rerun-if-changed=tests/integration_test_template");
    println!("cargo:rerun-if-changed=tests/custom_test_template");
    println!("cargo:rerun-if-changed=tests/roundtrip_test_template");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=MAX_EXPECTED_TIME");

    let out_dir = var("OUT_DIR").map_err(io::Error::other)?; // wrapping in a std::io::Error to match main's error type

    // Integration Tests
    let dest = Path::new(&out_dir).join("gen_tests.rs");
    let mut f = File::create(dest)?;
    let test_dir = "tests/integration";

    for subdir in WalkDir::new(test_dir) {
        let subdir = subdir?;
        if subdir.file_type().is_dir() {
            let stems: Vec<String> = read_dir(subdir.path())?
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "essence"))
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
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "essence"))
                .filter_map(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|s| s.to_owned())
                })
                .collect();

            let essence_files: Vec<(String, String)> = std::iter::zip(stems, exts).collect();
            setup_integration_tests(&mut f, subdir.path().display().to_string(), essence_files)?;
            // write_integration_test(&mut f, subdir.path().display().to_string(), essence_files)?;
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
            if std::env::var("ALLTEST").is_ok() {
                // Finds Essence and disabled Essence filenames
                let names: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|ext| ext == "essence" || ext == "disabled")
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
                // Finds Essence and disabled file extensions
                let exts: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|ext| ext == "essence" || ext == "disabled")
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
            } else {
                // Finds Essence filenames
                let names: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().is_some_and(|ext| ext == "essence"))
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
                // Finds Essence file extensions
                let exts: Vec<String> = read_dir(subdir.path())?
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().is_some_and(|ext| ext == "essence"))
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
    }

    Ok(())
}

fn read_config_or_default(path: &str) -> TestConfig {
    let config_path = format!("{path}/config.toml");
    if let Ok(contents) = std::fs::read_to_string(&config_path) {
        toml::from_str(&contents)
            .unwrap_or_else(|err| panic!("failed to parse {config_path}: {err}"))
    } else {
        TestConfig::default()
    }
}

fn max_expected_time_limit() -> io::Result<Option<u64>> {
    match std::env::var("MAX_EXPECTED_TIME") {
        Ok(value) => value.parse::<u64>().map(Some).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid MAX_EXPECTED_TIME value '{value}': {err}"),
            )
        }),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(err) => Err(io::Error::other(err)),
    }
}

fn get_ignore_attr(cfg: &TestConfig, include_expected_time: bool) -> io::Result<String> {
    if cfg.skip {
        Ok(String::from(
            "#[ignore = \"this test has been disabled ('skip=true' in its config.toml)\"]\n",
        ))
    } else if include_expected_time
        && let (Some(expected_time), Some(limit)) = (cfg.expected_time, max_expected_time_limit()?)
    {
        if expected_time > limit {
            Ok(format!(
                "#[ignore = \"this test declares 'expected-time={expected_time}' in its config.toml, which exceeds MAX_EXPECTED_TIME={limit}\"]\n",
            ))
        } else {
            Ok(String::new())
        }
    } else {
        Ok(String::new())
    }
}

fn setup_integration_tests(
    arg_file: &mut File,
    path: String,
    essence_files: Vec<(String, String)>,
) -> io::Result<()> {
    if essence_files.len() != 1 {
        return Ok(());
    }

    let config: TestConfig =
        if let Ok(config_contents) = fs::read_to_string(format!("{path}/config.toml")) {
            toml::from_str(&config_contents).unwrap()
        } else {
            Default::default()
        };

    let parsers = config
        .configured_parsers()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let rewriters = config
        .configured_rewriters()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let comprehension_expanders = config
        .configured_comprehension_expanders()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
    let solvers = {
        let seen = config
            .configured_solvers()
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?
            .into_iter()
            .collect::<Vec<_>>();
    };

    for parser in parsers.iter().copied() {
        for rewriter in rewriters.clone() {
            for comprehension_expander in comprehension_expanders.clone() {
                for solver in solvers.clone() {
                    let _case_name = "meow";
                    let run_case = RunCase {
                        parser,
                        rewriter,
                        comprehension_expander,
                        solver,
                    };
                    write_integration_test(arg_file, &path, &essence_files, run_case)?;
                }
            }
        }
    }
    Ok(())
}

fn write_integration_test(
    file: &mut File,
    path: &String,
    essence_files: &[(String, String)],
    runcase: RunCase,
) -> io::Result<()> {
    let file_name = &essence_files[0].0;
    let ext = &essence_files[0].1;
    let cfg = read_config_or_default(path);
    let ignore = get_ignore_attr(&cfg, true)?;

    let base_name = path.replace("./", "").replace(['/', '-'], "_");
    let case_suffix = format!("{}_{}", runcase.run_case_label(), runcase.solver.as_str());
    let test_name = format!("{}_{}", base_name, case_suffix.replace('-', "_"));

    write!(
        file,
        include_str!("./tests/integration_test_template"),
        test_name = test_name,
        test_dir = path,
        essence_file = file_name,
        ext = ext,
        ignore_attr = ignore,
        runcase = runcase
    )
}

fn write_custom_test(file: &mut File, path: String) -> io::Result<()> {
    let cfg = read_config_or_default(&path);
    let ignore = get_ignore_attr(&cfg, true)?;

    write!(
        file,
        include_str!("./tests/custom_test_template"),
        test_name = path.replace("./", "").replace(['/', '-'], "_"),
        test_dir = path,
        ignore_attr = ignore
    )
}

fn write_roundtrip_test(
    file: &mut File,
    path: String,
    essence_file: (String, String),
) -> io::Result<()> {
    let cfg = read_config_or_default(&path);
    let ignore = get_ignore_attr(&cfg, false)?;

    write!(
        file,
        include_str!("./tests/roundtrip_test_template"),
        test_name = path.replace("./", "").replace(['/', '-'], "_"),
        test_dir = path,
        essence_file = essence_file.0,
        ext = essence_file.1,
        ignore_attr = ignore
    )
}
