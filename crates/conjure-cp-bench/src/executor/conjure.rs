use std::{
    fs::File,
    path::PathBuf,
    process::{Command, Stdio},
    time::Instant,
};
use versions::Versioning;

use crate::{FileSource, Model, ParamFile, executor::Executor};

use super::{ExecutorError, ExecutorOutput};

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConjureExecutorBuilder {
    executable: PathBuf,
    extra_flags: Vec<String>,
}

// TODO: add option to get executable from path.

impl ConjureExecutorBuilder {
    /// Creates a new ConjureExecutorBuilder.
    ///
    /// # Panics
    ///
    /// - If conjure_executable does not exist, is not executable, or is not conjure.
    pub fn new(conjure_executable: PathBuf) -> ConjureExecutorBuilder {
        assert_executable_exists_and_is_conjure(&conjure_executable);
        ConjureExecutorBuilder {
            executable: conjure_executable,
            extra_flags: vec![],
        }
    }

    // TODO: do not allow the passing of arbitrary flags, make well-typed builder methods for all
    // options.
    pub fn add_flag(mut self, flag: String) -> ConjureExecutorBuilder {
        self.extra_flags.push(flag);
        self
    }

    pub fn add_flags(mut self, flags: &[String]) -> ConjureExecutorBuilder {
        self.extra_flags.extend(flags.iter().cloned());
        self
    }

    /// Changes the Conjure executable to use.
    ///
    /// # Panics
    ///
    /// - If conjure_executable does not exist, is not executable, or is not conjure.
    pub fn conjure_executable(mut self, conjure_executable: PathBuf) -> ConjureExecutorBuilder {
        assert_executable_exists_and_is_conjure(&conjure_executable);
        self.executable = conjure_executable;
        self
    }

    pub fn build(self) -> ConjureExecutor {
        let temp_dir = std::env::temp_dir();
        ConjureExecutor {
            executable: self.executable,
            extra_flags: self.extra_flags,
            temp_dir,
        }
    }
}

const CONJURE_MIN_VERSION: &str = "2.6.0";
const CORRECT_FIRST_LINE: &str = "Conjure: The Automated Constraint Modelling Tool";

// adapted from conjure-cp-cli
fn assert_executable_exists_and_is_conjure(executable: &PathBuf) {
    let mut cmd = std::process::Command::new(executable);
    let output = cmd
        .arg("--version")
        .output()
        .expect("Failed conjure executable check: expect conjure --version to succeed");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    if !stderr.is_empty() {
        panic!(
            "Failed Conjure executable check: running conjure results in error: {}",
            &stderr
        );
    }

    let first = stdout
        .lines()
        .next()
        .expect("Failed Conjure executable check: could not read stddout.");

    if first != CORRECT_FIRST_LINE {
        panic!(
            "Failed Conjure executable check: first line is incorrect when using --version. Expected {CORRECT_FIRST_LINE}, got {first}."
        );
    }

    let version_line = stdout
        .lines()
        .nth(1)
        .expect("Failed conjure executable check: could not read stdout");

    let version_and_repo = version_line.strip_prefix("Conjure v").unwrap_or_else(|| panic!("Failed Conjure executable check: could not read conjure's version from: {version_line}"));

    let (version, _) = version_and_repo.split_once(" (Repository version ").unwrap_or_else(|| panic!("Failed Conjure executable check: could not read Conjure's version from: {version_line}"));

    if Versioning::new(version) < Versioning::new(CONJURE_MIN_VERSION) {
        panic!(
            "failed Conjure executable check: conjure version is too old (< {CONJURE_MIN_VERSION}): {version}"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConjureExecutor {
    pub executable: PathBuf,
    pub extra_flags: Vec<String>,
    temp_dir: PathBuf,
}

impl Executor for ConjureExecutor {
    fn run(
        &self,
        model: &Model,
        param_file: Option<&ParamFile>,
    ) -> Result<ExecutorOutput, ExecutorError> {
        let model_file_path: String = match &model.source {
            FileSource::File(path_buf) => {
                if !path_buf.exists() {
                    let model_path_str = path_buf.to_str().unwrap();
                    panic!("Model {model_path_str} does not exist");
                }

                if !path_buf.is_absolute() {
                    let model_path_str = path_buf.to_str().unwrap();
                    panic!("Model {model_path_str} is not an absolute path.");
                }

                String::from(path_buf.to_string_lossy())
            }
            FileSource::Text(src) => {
                let path = &self.temp_dir.join(format!("{}.essence", &model.name));
                std::fs::write(path, src).unwrap();
                String::from(path.to_string_lossy())
            }
        };

        let param_file_path: String = match &param_file.map(|x| &x.source) {
            Some(FileSource::File(path_buf)) => {
                if !path_buf.exists() {
                    let param_path_str = path_buf.to_str().unwrap();
                    panic!("Param file {param_path_str} does not exist");
                }

                if !path_buf.is_absolute() {
                    let param_path_str = path_buf.to_str().unwrap();
                    panic!("Param file {param_path_str} is not an absolute path");
                }
                String::from(path_buf.to_string_lossy())
            }
            Some(FileSource::Text(src)) => {
                let path = &self.temp_dir.join(format!("{}.essence", &model.name));
                std::fs::write(path, src).unwrap();
                String::from(path.to_string_lossy())
            }
            None => String::from(""),
        };

        let mut output = ExecutorOutput::new();

        let mut command = Command::new(&self.executable);
        let command = command
            .current_dir(&self.temp_dir)
            .arg("solve")
            .arg(model_file_path)
            .arg(param_file_path)
            .args(&self.extra_flags)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut command_as_string: Vec<String> =
            vec![String::from(command.get_program().to_string_lossy())];

        command_as_string.extend(
            command
                .get_args()
                .map(|x| String::from(x.to_string_lossy())),
        );

        let command_as_string = command_as_string.join(" ");
        output.set_metadata("command", command_as_string);

        // clear conjure cache, ignoring errors that arise if conjure-output does not exist yet
        let _ = std::fs::remove_dir_all(self.temp_dir.join("conjure-output"));

        // run and time conjure
        let now = Instant::now();
        let p_handle = command.spawn().expect("conjure failed to start");
        let conjure_output = p_handle
            .wait_with_output()
            .expect("TODO: return error upwards instead of panicing");

        // TODO: on linux, call getrusage syscall on this process to see peak memory usage, cpu time, swap, etc.

        let elapsed_wall_time = now.elapsed();

        output.set_counter("wall_time_s", Some(elapsed_wall_time.as_secs_f64()));
        output.set_counter(
            "exit_status",
            conjure_output.status.code().map(|x| x as f64),
        );

        output.set_metadata("stdout", String::from_utf8(conjure_output.stdout).unwrap());
        output.set_metadata("stderr", String::from_utf8(conjure_output.stderr).unwrap());

        process_stats_json(&self.temp_dir, param_file.map(|x| &x.name), &mut output);

        Ok(output)
    }
}

macro_rules! _counter_from_json {
    ($json: expr, $out: expr, $counter_name: expr, $pointer_name: expr) => {
        $out.set_counter($counter_name, f64_from_stats_json(&$json, $pointer_name));
    };

    ($json: expr, $out: expr, $counter_name: expr) => {
        $out.set_counter(
            $counter_name,
            f64_from_stats_json(&$json, "/" + $counter_name),
        );
    };
}

macro_rules! _meta_from_json {
    ($json: expr, $out: expr, $counter_name: expr, $pointer_name: expr) => {
        $out.set_metadata($counter_name, str_from_stats_json(&$json, $pointer_name));
    };

    ($json: expr, $out: expr, $counter_name: expr) => {
        $out.set_metadata(
            $counter_name,
            str_from_stats_json(&$json, format!("/{}", $counter_name).as_str()),
        );
    };
}

// Add counters and metadata from stats.json to the output, if stats.json exists.
fn process_stats_json(
    temp_dir: &PathBuf,
    param_file_name: Option<&String>,
    out: &mut ExecutorOutput,
) {
    // called model000001.stats.json without param file,
    // model000001-{param_file_name}.stats.json with param file
    let stats_json_path = match param_file_name {
        Some(p) => temp_dir.join(format!("conjure-output/model000001-{p}.stats.json")),
        None => temp_dir.join("conjure-output/model000001.stats.json"),
    };

    if !stats_json_path.exists() {
        return;
    }
    let file = File::open(stats_json_path).unwrap();
    let json: serde_json::Value =
        serde_json::from_reader(file).expect("conjure stats.json to be valid json");

    _counter_from_json!(
        json,
        out,
        "SavileRowTotalTime",
        "/savilerowInfo/SavileRowTotalTime"
    );
    _counter_from_json!(json, out, "SolverNodes", "/savilerowInfo/SolverNodes");
    _counter_from_json!(
        json,
        out,
        "SolverSolveTime",
        "/savilerowInfo/SolverSolveTime"
    );
    _counter_from_json!(
        json,
        out,
        "SolverSetupTime",
        "/savilerowInfo/SolverSetupTime"
    );

    _meta_from_json!(json, out, "conjureVersion");
    _meta_from_json!(json, out, "savileRowVersion");
    _meta_from_json!(json, out, "solver");
    _meta_from_json!(json, out, "solverOptions");
    _meta_from_json!(json, out, "conjureTimestamp");
}

fn f64_from_stats_json(json: &serde_json::Value, pointer: &str) -> Option<f64> {
    json.pointer(pointer)
        .and_then(|x| x.as_str())
        .and_then(|x| x.parse().ok())
}

fn str_from_stats_json(json: &serde_json::Value, pointer: &str) -> String {
    json.pointer(pointer)
        .map(|x| x.to_string())
        .unwrap_or_default()
}
