use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::text_files::write_text_with_trailing_newline;

/// Gitignored directory inside each integration test case for debugging failed runs.
pub const DIAGNOSTICS_DIR: &str = "diagnostics";

#[derive(Debug, Serialize)]
pub struct FailureRecord {
    pub stage: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_label: Option<String>,
}

pub fn diagnostics_dir(test_dir: &Path) -> PathBuf {
    test_dir.join(DIAGNOSTICS_DIR)
}

pub fn conjure_artifacts_dir(test_dir: &Path) -> PathBuf {
    diagnostics_dir(test_dir).join("conjure")
}

pub fn oxide_artifacts_dir(test_dir: &Path) -> PathBuf {
    diagnostics_dir(test_dir).join("oxide")
}

/// Removes diagnostics after a passing run.
pub fn clear_diagnostics(test_dir: &Path) -> io::Result<()> {
    let dir = diagnostics_dir(test_dir);
    if dir.is_dir() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// Writes `failure.json` under `diagnostics/`.
pub fn write_failure_record(test_dir: &Path, record: &FailureRecord) -> io::Result<()> {
    fs::create_dir_all(diagnostics_dir(test_dir))?;
    let failure_json = serde_json::to_string_pretty(record).map_err(io::Error::other)?;
    write_text_with_trailing_newline(
        &diagnostics_dir(test_dir).join("failure.json"),
        &failure_json,
    )
}

pub fn copy_file_if_exists(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_file() {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to)?;
    }
    Ok(())
}

pub fn write_oxide_failure_text(test_dir: &Path, run_label: &str, message: &str) -> io::Result<()> {
    let path = oxide_artifacts_dir(test_dir).join(format!("{run_label}.txt"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_text_with_trailing_newline(&path, message)
}
