use std::env::var;
use std::fs::{File, read_dir};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use walkdir::WalkDir;

mod test_config;
use test_config::TestConfig;

mod runcase;
use runcase::*;

fn main() -> io::Result<()> {
    // let out_dir = var("OUT_DIR").map_err(io::Error::other)?; // wrapping in a std::io::Error to match main's error type

    // Integration Tests
    // let dest = Path::new(&out_dir).join("gen_tests.rs");
    // let mut f = File::create(dest)?;
    let test_dir = "tests/integration";
    let substring = "cnf";

    // let dirs = WalkDir::new(test_dir)
    //     .into_iter()
    //     .filter_map(|e| e.is_ok())
    //     .filter(|p| p.to_str().map_or(contains(substr)));

    let matching_dirs: Vec<_> = WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_dir() && e.path().to_str().map_or(false, |s| s.contains(substring))
        })
        .collect();

    for subdir in matching_dirs {
        // let subdir = subdir?;
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
            let path = subdir.path().display();
            eprintln!("\x1b[32mRunning tests for path: {}\x1b[0m", path);
            if essence_files != [] {
                let (essence_base, extension) = &essence_files[0];
                let config: TestConfig = if let Ok(config_contents) =
                    std::fs::read_to_string(format!("{path}/config.toml"))
                {
                    toml::from_str(&config_contents).unwrap()
                } else {
                    Default::default()
                };

                if !config.skip {
                    let validate_with_conjure = config.validate_with_conjure;
                    let minion_discrete_threshold = config.minion_discrete_threshold;

                    let parsers = config.configured_parsers().map_err(|err| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, err)
                    })?;
                    let rewriters = config.configured_rewriters().map_err(|err| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, err)
                    })?;
                    let comprehension_expanders =
                        config.configured_comprehension_expanders().map_err(|err| {
                            std::io::Error::new(std::io::ErrorKind::InvalidInput, err)
                        })?;
                    let solvers = config.configured_solvers().map_err(|err| {
                        std::io::Error::new(std::io::ErrorKind::InvalidInput, err)
                    })?;
                    // Conjure output depends only on the input model, so cache it once per test case.
                    // let model_path = format!("{path}/{essence_base}.{extension}");
                    // let conjure_solutions = if accept && validate_with_conjure {
                    //     eprintln!("[integration] loading Conjure reference solutions for {model_path}");
                    //     Some(Arc::new(
                    //         get_solutions_from_conjure(&model_path, None, Default::default()).map_err(|err| {
                    //             std::io::Error::other(format!(
                    //                "failed to fetch Conjure reference solutions for {model_path}: {err}"
                    //             ))
                    //         })?,
                    //     ))
                    // } else {
                    //     if accept && !validate_with_conjure {
                    //         eprintln!("[integration] skipping Conjure validation for {model_path}");
                    //     }
                    //     None
                    // };

                    for parser in parsers {
                        for rewriter in rewriters.clone() {
                            for comprehension_expander in comprehension_expanders.clone() {
                                for solver in solvers.clone() {
                                    let case_name = runcase::run_case_name(
                                        parser,
                                        rewriter,
                                        comprehension_expander,
                                    );
                                    let run_case = RunCase {
                                        parser,
                                        rewriter,
                                        comprehension_expander,
                                        solver,
                                        case_name: case_name.as_str(),
                                    };
                                    let file = File::create(format!(
                                        "{path}/{}-{}-generated-rule-trace.txt",
                                        run_case.case_name,
                                        run_case.solver.as_str()
                                    ))?;
                                    // let subscriber = Arc::new(
                                    //     tracing_subscriber::registry().with(
                                    //         fmt::layer()
                                    //             .with_writer(file)
                                    //             .with_level(false)
                                    //             .without_time()
                                    //             .with_target(false)
                                    //             .with_filter(EnvFilter::new("rule_engine_human=trace"))
                                    //             .with_filter(FilterFn::new(|meta| {
                                    //                 meta.target() == "rule_engine_human"
                                    //             })),
                                    //     ),
                                    // )
                                    //     as Arc<dyn tracing::Subscriber + Send + Sync>;
                                    let run_label = run_case_label(
                                        &path.to_string(),
                                        &essence_base,
                                        &extension,
                                        run_case,
                                    );
                                    eprintln!("[integration] running {run_label}");
                                    // tracing::subscriber::with_default(subscriber, || {
                                    //     integration_test_inner(
                                    //         path,
                                    //         essence_base,
                                    //         extension,
                                    //         run_case,
                                    //         minion_discrete_threshold,
                                    //         conjure_solutions.clone(),
                                    //         accept,
                                    //     )
                                    // })
                                    // .map_err(|err| {
                                    //     std::io::Error::other(format!("{run_label}: {err}"))
                                    // })?;
                                }
                            }
                        }
                    }
                }

                // println!(
                //     "Int Test: {}, esscs: {:?}",
                //     subdir.path().display(),
                //     essence_files
                // );
            }
            // write_integration_test(&mut f, subdir.path().display().to_string(), essence_files)?;
        }
    }

    // Custom Tests
    // let dest_custom = Path::new(&out_dir).join("gen_tests_custom.rs");
    // let mut f = File::create(dest_custom)?;
    // let test_dir = "tests/custom";

    // for subdir in WalkDir::new(test_dir) {
    //     let subdir = subdir?;
    //     if subdir.file_type().is_dir()
    //         && read_dir(subdir.path())
    //             .unwrap_or_else(|_| std::fs::read_dir(subdir.path()).unwrap())
    //             .filter_map(Result::ok)
    //             .any(|entry| entry.file_name() == "run.sh" && entry.path().is_file())
    //     {
    //         println!("Custom Test: {}", subdir.path().display());
    //         // write_custom_test(&mut f, subdir.path().display().to_string())?;
    //     }
    // }

    // // Roundtrip Tests
    // let dest_roundtrip = Path::new(&out_dir).join("gen_tests_roundtrip.rs");
    // let mut f = File::create(dest_roundtrip)?;
    // let test_dir = "tests/roundtrip";

    // for subdir in WalkDir::new(test_dir) {
    //     let subdir = subdir?;
    //     // Checks every subdirectory
    //     if subdir.file_type().is_dir() {
    //         if std::env::var("ALLTEST").is_ok() {
    //             // Finds Essence and disabled Essence filenames
    //             let names: Vec<String> = read_dir(subdir.path())?
    //                 .filter_map(Result::ok)
    //                 .map(|entry| entry.path())
    //                 .filter(|path| {
    //                     path.extension()
    //                         .is_some_and(|ext| ext == "essence" || ext == "disabled")
    //                 })
    //                 // Ensures not to include test result files
    //                 .filter(|path| {
    //                     path.file_stem()
    //                         .and_then(|name| name.to_str())
    //                         .is_some_and(|name| {
    //                             !name.contains(".generated") && !name.contains(".expected")
    //                         })
    //                 })
    //                 // Stores the filename in the collected vector
    //                 .filter_map(|path| {
    //                     path.file_stem()
    //                         .and_then(|stem| stem.to_str())
    //                         .map(|s| s.to_owned())
    //                 })
    //                 .collect();
    //             // Finds Essence and disabled file extensions
    //             let exts: Vec<String> = read_dir(subdir.path())?
    //                 .filter_map(Result::ok)
    //                 .map(|entry| entry.path())
    //                 .filter(|path| {
    //                     path.extension()
    //                         .is_some_and(|ext| ext == "essence" || ext == "disabled")
    //                 })
    //                 // Ensures not to include test result files
    //                 .filter(|path| {
    //                     path.file_stem()
    //                         .and_then(|name| name.to_str())
    //                         .is_some_and(|name| {
    //                             !name.contains(".generated") && !name.contains(".expected")
    //                         })
    //                 })
    //                 // Stores the extension in the collected vector
    //                 .filter_map(|path| {
    //                     path.extension()
    //                         .and_then(|ext| ext.to_str())
    //                         .map(|s| s.to_owned())
    //                 })
    //                 .collect();

    //             let essence_files: Vec<(String, String)> = std::iter::zip(names, exts).collect();
    //             // There should only be one test file per directory
    //             if essence_files.len() == 1 {
    //                 println!("Roundtrip: {}", subdir.path().display());
    //                 // write_roundtrip_test(
    //                 //     &mut f,
    //                 //     subdir.path().display().to_string(),
    //                 //     essence_files[0].clone(),
    //                 // )?;
    //             }
    //         } else {
    //             // Finds Essence filenames
    //             let names: Vec<String> = read_dir(subdir.path())?
    //                 .filter_map(Result::ok)
    //                 .map(|entry| entry.path())
    //                 .filter(|path| path.extension().is_some_and(|ext| ext == "essence"))
    //                 // Ensures not to include test result files
    //                 .filter(|path| {
    //                     path.file_stem()
    //                         .and_then(|name| name.to_str())
    //                         .is_some_and(|name| {
    //                             !name.contains(".generated") && !name.contains(".expected")
    //                         })
    //                 })
    //                 // Stores the filename in the collected vector
    //                 .filter_map(|path| {
    //                     path.file_stem()
    //                         .and_then(|stem| stem.to_str())
    //                         .map(|s| s.to_owned())
    //                 })
    //                 .collect();
    //             // Finds Essence file extensions
    //             let exts: Vec<String> = read_dir(subdir.path())?
    //                 .filter_map(Result::ok)
    //                 .map(|entry| entry.path())
    //                 .filter(|path| path.extension().is_some_and(|ext| ext == "essence"))
    //                 // Ensures not to include test result files
    //                 .filter(|path| {
    //                     path.file_stem()
    //                         .and_then(|name| name.to_str())
    //                         .is_some_and(|name| {
    //                             !name.contains(".generated") && !name.contains(".expected")
    //                         })
    //                 })
    //                 // Stores the extension in the collected vector
    //                 .filter_map(|path| {
    //                     path.extension()
    //                         .and_then(|ext| ext.to_str())
    //                         .map(|s| s.to_owned())
    //                 })
    //                 .collect();

    //             let essence_files: Vec<(String, String)> = std::iter::zip(names, exts).collect();
    //             // There should only be one test file per directory
    //             if essence_files.len() == 1 {
    //                 println!("Roundtrip: {}", subdir.path().display());
    //                 // write_roundtrip_test(
    //                 //     &mut f,
    //                 //     subdir.path().display().to_string(),
    //                 //     essence_files[0].clone(),
    //                 // )?;
    //             }
    //         }
    //     }
    // }

    Ok(())
}

fn read_config_or_default(path: &str) -> TestConfig {
    let config_path = format!("{}/config.toml", path);
    if let Ok(contents) = std::fs::read_to_string(&config_path) {
        toml::from_str(&contents).unwrap_or_default()
    } else {
        TestConfig::default()
    }
}

fn get_ignore_attr(cfg: &TestConfig) -> String {
    if cfg.skip {
        String::from(
            "#[ignore = \"this test has been disabled ('skip=true' in its config.toml)\"]\n",
        )
    } else {
        String::new()
    }
}

// fn write_integration_test(
//     file: &mut File,
//     path: String,
//     essence_files: Vec<(String, String)>,
// ) -> io::Result<()> {
//     // TODO: Consider supporting multiple Essence files?
//     if essence_files.len() == 1 {
//         let cfg = read_config_or_default(&path);
//         let ignore = get_ignore_attr(&cfg);
//
//         write!(
//             file,
//             include_str!("../tests/integration_test_template"),
//             // TODO: better sanitisation of paths to function names
//             test_name = path.replace("./", "").replace(['/', '-'], "_"),
//             test_dir = path,
//             essence_file = essence_files[0].0,
//             ext = essence_files[0].1,
//             ignore_attr = ignore
//         )
//     } else {
//         Ok(())
//     }
// }
//
// fn write_custom_test(file: &mut File, path: String) -> io::Result<()> {
//     write!(
//         file,
//         include_str!("../tests/custom_test_template"),
//         test_name = path.replace("./", "").replace(['/', '-'], "_"),
//         test_dir = path
//     )
// }
//
// fn write_roundtrip_test(
//     file: &mut File,
//     path: String,
//     essence_file: (String, String),
// ) -> io::Result<()> {
//     let cfg = read_config_or_default(&path);
//     let ignore = get_ignore_attr(&cfg);
//
//     write!(
//         file,
//         include_str!("../tests/roundtrip_test_template"),
//         test_name = path.replace("./", "").replace(['/', '-'], "_"),
//         test_dir = path,
//         essence_file = essence_file.0,
//         ext = essence_file.1,
//         ignore_attr = ignore
//     )
// }
