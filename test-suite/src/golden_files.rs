use std::collections::BTreeSet;
use std::env;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptMode {
    /// Normal test mode: compare generated files with expected files and do not rewrite fixtures.
    Disabled,
    /// Rewrite expected output fixtures, but leave expected runtime budgets untouched.
    Accept,
    /// Rewrite output fixtures and runtime budgets exactly as observed.
    AcceptWithTimes,
    /// Rewrite output fixtures, but only raise runtime budgets.
    ///
    /// Catching slowdowns is more important than automatically accepting speedups. Runtimes
    /// are non-deterministic and machine/load dependent, so a significant slowdown may be
    /// worth noticing while a one-off faster run should not lower the recorded budget.
    AcceptWithSlowerTimes,
}

impl AcceptMode {
    pub fn from_env() -> Self {
        match env::var("ACCEPT").as_deref() {
            Ok("false") => Self::Disabled,
            Ok("true") => Self::Accept,
            Ok("with-times") => Self::AcceptWithTimes,
            Ok("with-exact-times") => Self::AcceptWithTimes,
            Ok("with-slower-times") => Self::AcceptWithSlowerTimes,
            _ => Self::Disabled,
        }
    }

    pub fn accepts_outputs(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub fn records_expected_time(self) -> bool {
        matches!(self, Self::AcceptWithTimes | Self::AcceptWithSlowerTimes)
    }

    pub fn expected_time_to_record(self, current: Option<u64>, observed: u64) -> Option<u64> {
        match self {
            Self::AcceptWithTimes => Some(observed),
            Self::AcceptWithSlowerTimes if current.is_none_or(|current| observed > current) => {
                Some(observed)
            }
            _ => None,
        }
    }

    pub fn refresh_hint() -> &'static str {
        "Run with ACCEPT=true, ACCEPT=with-slower-times, or ACCEPT=with-exact-times"
    }
}
/// Returns whether a file name represents an expected golden artifact.
fn is_expected_golden_file(file_name: &str) -> bool {
    file_name.contains(".expected") || file_name.contains("-expected-")
}

/// Lists expected snapshot files in `path` that are not present in `allowed_expected_files`.
pub fn find_redundant_expected_files(
    path: &Path,
    allowed_expected_files: &BTreeSet<String>,
) -> Result<Vec<String>, std::io::Error> {
    println!(
        "running redundant golden file test. Allowed: {:#?}",
        allowed_expected_files
    );

    let mut redundant = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if !entry.path().is_file() {
            continue;
        }

        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy().to_string();

        if is_expected_golden_file(&file_name) && !allowed_expected_files.contains(&file_name) {
            println!("unexpected: {}", file_name);
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
