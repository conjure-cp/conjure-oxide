use std::collections::BTreeSet;
use std::path::Path;

use crate::AcceptMode;

/// Returns whether a file name represents an expected golden artifact.
fn is_expected_golden_file(file_name: &str) -> bool {
    file_name.contains(".expected") || file_name.contains("-expected-")
}

/// Lists expected snapshot files in `path` that are not present in `allowed_expected_files`.
pub fn find_redundant_expected_files(
    path: &Path,
    allowed_expected_files: &BTreeSet<String>,
) -> Result<Vec<String>, std::io::Error> {
    let mut redundant = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if !entry.path().is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy().to_string();

        if is_expected_golden_file(&file_name) && !allowed_expected_files.contains(&file_name) {
            redundant.push(file_name);
        }
    }

    redundant.sort();
    Ok(redundant)
}

/// Builds a standardised error describing redundant golden files for a test directory.
pub fn redundant_golden_files_error(
    path: &Path,
    redundant_files: Vec<String>,
    context: Option<&str>,
) -> std::io::Error {
    let file_list = redundant_files
        .into_iter()
        .map(|file| format!("  - {file}"))
        .collect::<Vec<_>>()
        .join("\n");

    let context = context.map_or(String::new(), |context| format!(" {context}"));

    std::io::Error::other(format!(
        "Redundant golden files detected in {}{context}:\n{file_list}\n{} to refresh snapshots.",
        path.display(),
        AcceptMode::refresh_hint()
    ))
}

/// Fails when `path` contains any expected snapshot file not listed in `allowed_expected_files`.
pub fn assert_no_redundant_expected_files(
    path: &Path,
    allowed_expected_files: &BTreeSet<String>,
    context: Option<&str>,
) -> Result<(), std::io::Error> {
    let redundant = find_redundant_expected_files(path, allowed_expected_files)?;
    if redundant.is_empty() {
        return Ok(());
    }

    Err(redundant_golden_files_error(path, redundant, context))
}
