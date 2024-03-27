use std::fs;
use std::io::{self, Write};
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
    // define directory containing the .essence files
    let repo_dir = Path::new("https://github.com/conjure-cp/conjure/tree/main/tests/exhaustive");

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
        "or-tools",
        "lingeling",
        "glucose",
        "glucose-syrup",
    ];

    // iterate through each .essence file
    for essence_file in essence_files {
        // extract directory containing the .essence file
        let directory = essence_file.parent().unwrap();

        // find .param files in the same directory as the .essence file
        let param_files: Vec<PathBuf> = fs::read_dir(&directory)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let path = entry.path();
                path.is_file() && path.extension().map_or(false, |ext| ext == "param")
            })
            .map(|entry| entry.path())
            .collect();

        // iterate through each solver
        for solver in &solvers {
            // define the output file path
            let output_file = output_dir.join(format!(
                "{}_{}.json",
                essence_file.file_stem().unwrap().to_str().unwrap(),
                solver
            ));

            // prepare command arguments
            let mut command = Command::new("conjure");
            command
                .arg("solve")
                .arg(&essence_file)
                .args(&param_files)
                .arg("--solver")
                .arg(solver)
                .arg("--number-of-solutions=all")
                .arg("--output-format=json")
                .arg("--solutions-in-one-file")
                .arg("--copy-solutions=no")
                .stdout(Stdio::from(output_file));

            // execute the command
            let output = command.output()?;

            // check if command execution was successful
            if output.status.success() {
                println!("STATUS: Command executed successfully.");
            } else {
                println!("STATUS: Command failed with exit code: {}", output.status);
                if let Some(stderr) = String::from_utf8(output.stderr) {
                    eprintln!("Error message: {}", stderr);
                }
            }
        }
    }

    Ok(())
}
