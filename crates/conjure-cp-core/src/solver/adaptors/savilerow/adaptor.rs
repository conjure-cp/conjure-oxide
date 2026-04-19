//! Solver adaptor for Savile Row, called as an external process.

use std::collections::HashMap;
use std::io::Write;
use std::process::Command;

use tempfile::tempdir;

use crate::Model;
use crate::ast::{Literal, Name};
use crate::settings::SolverFamily;
use crate::solver::{
    SearchComplete, SearchStatus, SolveSuccess, SolverAdaptor, SolverCallback, SolverError,
    SolverMutCallback, private,
};
use crate::stats::SolverStats;

/// Solver adaptor for Savile Row.
///
/// Savile Row is called as an external process. The model is written to a
/// temporary file, Savile Row is invoked, and its output is parsed back
/// into Conjure Oxide's solution format.
pub struct SavileRow {
    model: Option<Model>,
}

impl SavileRow {
    pub fn new() -> Self {
        SavileRow { model: None }
    }
}

impl Default for SavileRow {
    fn default() -> Self {
        Self::new()
    }
}

impl private::Sealed for SavileRow {}

impl SolverAdaptor for SavileRow {
    fn load_model(
        &mut self,
        model: Model,
        _: private::Internal,
    ) -> Result<(), SolverError> {
        self.model = Some(model);
        Ok(())
    }

    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        // Get the model we stored in load_model
        let model = self.model.as_ref().ok_or(SolverError::ModelInvalid(
            "No model loaded".into(),
        ))?;

        // Prepend the Essence' header to the model's Display output
        // Model::Display already produces variable declarations and constraints
        // in Essence' format - we just need the language header at the top
        let model_str = format!("language ESSENCE' 1.0\n\n{}", model);

        // Create a temporary directory - this gets automatically deleted
        // when it goes out of scope at the end of this function
        let tmp_dir = tempdir().map_err(|e| {
            SolverError::Runtime(format!("Failed to create temp directory: {e}"))
        })?;

        // Write the model string to a file inside the temp directory
        let input_path = tmp_dir.path().join("model.eprime");
        std::fs::write(&input_path, &model_str).map_err(|e| {
            SolverError::Runtime(format!("Failed to write model file: {e}"))
        })?;

        // Step 2: invoke Savile Row as an external process
        // -in-eprime tells Savile Row the input is an Essence' file
        // -run-solver tells Savile Row to actually run the solver and produce a solution file
        let output = Command::new("savilerow")
            .arg("-in-eprime")
            .arg(&input_path)
            .arg("-run-solver")
            .output()
            .map_err(|e| SolverError::Runtime(
                format!("Could not launch Savile Row. Is it installed and on your PATH? Error: {e}")
            ))?;

        // Check if Savile Row exited successfully
        // If it failed, return the error message it printed to stderr
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(SolverError::Runtime(
                format!("Savile Row exited with an error:\n{stderr}")
            ));
        }
        
        // Step 3: find .solution files Savile Row produced
        // Savile Row writes solutions to the same temp directory as the input file
        // They are named like model.eprime.solution or model.eprime.001.solution
        let solution_files: Vec<_> = std::fs::read_dir(tmp_dir.path())
            .map_err(|e| SolverError::Runtime(
                format!("Could not read temp directory: {e}")
            ))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension().map(|ext| ext == "solution").unwrap_or(false)
            })
            .collect();
        
        // If no solution files exist, the problem has no solutions
        if solution_files.is_empty() {
            return Ok(SolveSuccess {
                stats: SolverStats::default(),
                status: SearchStatus::Complete(SearchComplete::NoSolutions),
            });
        }

        // Step 4: parse each solution file and call the callback
        for solution_path in &solution_files {
            // Read the entire file into a string
            let content = std::fs::read_to_string(solution_path)
                .map_err(|e| SolverError::Runtime(
                    format!("Could not read solution file: {e}")
                ))?;

            // Build a map of variable name -> value for this solution
            let mut solution: HashMap<Name, Literal> = HashMap::new();

            for line in content.lines() {
                let line = line.trim();

                // Solution lines look like: letting x be 3
                // We check if the line starts with "letting "
                if let Some(rest) = line.strip_prefix("letting ") {
                    // rest is now "x be 3"
                    // split_once splits on the FIRST occurrence of " be "
                    // giving us ("x", "3")
                    if let Some((name_part, value_part)) = rest.split_once(" be ") {
                        let name = Name::user(name_part.trim());
                        let value = parse_literal(value_part.trim());

                        if let Some(literal) = value {
                            solution.insert(name, literal);
                        }
                    }
                }
            }

            // Pass this solution up to the pipeline via the callback
            // The callback returns true if we should keep looking for more solutions
            // and false if we should stop
            if !callback(solution) {
                break;
            }
        }

        // All solutions have been passed to the callback
        Ok(SolveSuccess {
            stats: SolverStats::default(),
            status: SearchStatus::Complete(SearchComplete::HasSolutions),
        })
    }

    fn solve_mut(
        &mut self,
        _callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotSupported(
            "Savile Row does not support incremental solving".into(),
        ))
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SavileRow
    }

    fn get_name(&self) -> &'static str {
        "SavileRow"
    }

    fn write_solver_input_file(
        &self,
        writer: &mut Box<dyn Write>,
    ) -> Result<(), std::io::Error> {
        // Get the model, returning an error if none is loaded
        let model = self.model.as_ref().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "No model loaded"
            )
        })?;

        // Write the Essence' header and model to the writer
        // This is the same format we pass to Savile Row in solve()
        let model_str = format!("language ESSENCE' 1.0\n\n{}", model);
        writer.write_all(model_str.as_bytes())
    }
}

/// Converts a value string from a Savile Row solution file into a Literal.
/// For example "3" becomes Literal::Int(3), "true" becomes Literal::Bool(true).
/// Returns None if the value cannot be parsed.
fn parse_literal(s: &str) -> Option<Literal> {
    // Try parsing as an integer first
    if let Ok(n) = s.parse::<i32>() {
        return Some(Literal::Int(n));
    }

    // Try parsing as a boolean
    match s {
        "true" => return Some(Literal::Bool(true)),
        "false" => return Some(Literal::Bool(false)),
        _ => {}
    }

    // Unknown format - skip this value
    None
}
