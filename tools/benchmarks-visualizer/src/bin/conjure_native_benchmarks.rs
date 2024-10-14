// main.rs - a simple program to run the Conjure solver on all .essence files in a directory
// @author: Pedro Gronda Garrigues

// dependencies
use rayon::prelude::*; // parallelize file processing for optimization
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// function to recursively find all .essence files in a directory
fn find_essence_files(dir: &Path) -> Vec<PathBuf> {
    let mut essence_files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "essence") {
                    essence_files.push(path);
                } else if path.is_dir() {
                    essence_files.extend(find_essence_files(&path));
                }
            }
        }
    }
    essence_files
}

fn main() -> io::Result<()> {
    println!("STATUS: Running different native Conjure solvers on all .essence files in the repository directory.");

    // define directory containing the .essence files
    let repo_dir = Path::new("./tests/exhaustive");

    // define directory where output files will be written
    let output_dir = Path::new("./data");

    // create output directory if it doesn't exist
    if !output_dir.exists() {
        fs::create_dir(output_dir)?;
    }

    // find all .essence files in the repository directory
    let essence_files = find_essence_files(&repo_dir);

    // define solvers to use
    let solvers = [
        "minion",
        "chuffed",
        "kissat",
        "lingeling",
        "glucose",
        "glucose-syrup",
    ];

    // use parallel iterator provided by Rayon for essence files
    essence_files.par_iter().for_each(|essence_file| {
        let directory = essence_file
            .parent()
            .expect("STATUS: File must be in a directory");
        let relative_path = directory
            .strip_prefix(repo_dir)
            .expect("STATUS: Directory must be inside the repository directory");
        let local_output_dir = output_dir.join(relative_path);

        // use `unwrap_or_else` for simplicity
        fs::create_dir_all(&local_output_dir).unwrap_or_else(|_| {
            panic!("STATUS: Could not create directory {:?}", &local_output_dir)
        });

        let param_files: Vec<PathBuf> = fs::read_dir(&directory)
            .unwrap_or_else(|_| panic!("STATUS: Failed to read dir {:?}", &directory))
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().is_file()
                    && entry.path().extension().map_or(false, |ext| ext == "param")
            })
            .map(|entry| entry.path())
            .collect();

        solvers.iter().for_each(|solver| {
            let output_file_name = format!(
                "{}_native_{}.json",
                essence_file
                    .file_stem()
                    .and_then(OsStr::to_str)
                    .expect("STATUS: Essence file must have a valid name"),
                solver
            );
            let output_file = local_output_dir.join(output_file_name);

            // skip the generation if JSON file already exists
            if output_file.exists() {
                println!(
                    "STATUS: Skipping {} as it already exists.",
                    output_file.display()
                );
                return;
            }

            // prepare command arguments
            let mut command = Command::new("conjure");
            command
                .arg("solve")
                .arg(&essence_file)
                .args(&param_files)
                .arg("--solver")
                .arg(solver)
                .arg("--number-of-solutions=all")
                .arg(format!(
                    "--output-directory={}/{}",
                    local_output_dir.display(),
                    solver
                ))
                .arg("--output-format=json")
                .arg("--solutions-in-one-file")
                .arg("--copy-solutions=no");

            println!("STATUS: Running command: {:?}", command);

            let output_handle = File::create(&output_file).unwrap_or_else(|_| {
                panic!("STATUS: Failed to create output file {:?}", &output_file)
            });

            command.stdout(Stdio::from(output_handle));
            let output = command.output().expect("STATUS: Failed to execute process");

            if output.status.success() {
                println!("STATUS: Command executed successfully.");
            } else {
                println!(
                    "STATUS: Command failed with exit code: {:?}",
                    output.status.code()
                );
                match String::from_utf8(output.stderr) {
                    Ok(stderr) => eprintln!("Error message: {}", stderr),
                    Err(e) => eprintln!("Error converting stderr: {}", e),
                }
            }
        });
    });

    Ok(())
}

// ALTERNATIVE w/out parallelization using Rayon
// // iterate through each .essence file
// for essence_file in essence_files {
//     let directory = essence_file.parent().expect("File must be in a directory");
//     let relative_path = directory
//         .strip_prefix(repo_dir)
//         .expect("Directory must be inside the repository directory");
//     let local_output_dir = output_dir.join(relative_path);
//
//     // only create the directory (autogen, etc) if it does not exist
//     fs::create_dir_all(&local_output_dir)?;
//
//     // find .param files in the same directory as the .essence file
//     let param_files: Vec<PathBuf> = fs::read_dir(&directory)?
//         .filter_map(|entry| entry.ok())
//         .filter(|entry| {
//             let path = entry.path();
//             path.is_file() && path.extension().map_or(false, |ext| ext == "param")
//         })
//         .map(|entry| entry.path())
//         .collect();
//
//     // iterate through each solver
//     for solver in &solvers {
//         // define the output file path
//         let output_file_name = format!(
//             "{}_{}.json",
//             essence_file
//                 .file_stem()
//                 .and_then(|name| name.to_str())
//                 .expect("Essence file must have a valid name"),
//             solver
//         );
//         let output_file = local_output_dir.join(output_file_name);
//
//         // prepare command arguments
//         let mut command = Command::new("conjure");
//         command
//             .arg("solve")
//             .arg(&essence_file)
//             .args(&param_files)
//             .arg("--solver")
//             .arg(solver)
//             .arg("--number-of-solutions=all")
//             .arg("--output-format=json")
//             .arg("--solutions-in-one-file")
//             .arg("--copy-solutions=no");
//
//         println!("Running command: {:?}", command);
//
//         // open the output file for writing
//         let output_handle = File::create(&output_file)?;
//
//         // redirect the standard output of the process to the output file
//         command.stdout(Stdio::from(output_handle));
//
//         // execute the command
//         let output = command.output()?;
//
//         // check if command execution was successful
//         if output.status.success() {
//             println!("STATUS: Command executed successfully.");
//         } else {
//             println!(
//                 "STATUS: Command failed with exit code: {:?}",
//                 output.status.code()
//             );
//             match String::from_utf8(output.stderr) {
//                 Ok(stderr) => eprintln!("Error message: {}", stderr),
//                 Err(e) => eprintln!("Error converting stderr: {}", e),
//             }
//         }
//     }
// }
