use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::string::ToString;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use conjure_cp::ast::{Literal, Name};
use conjure_cp::context::Context;

use serde_json::{Map, Value as JsonValue};

use itertools::Itertools as _;
use tempfile::tempdir;

use crate::utils::json::sort_json_object;
use crate::utils::simplified_json::{
    domains_from_model, param_model_from_assignments, params_from_simplified_json_str,
    solutions_from_simplified_json_str,
};
use conjure_cp::instantiate::instantiate_model;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;

use glob::glob;
use uniplate::Uniplate;

#[derive(Clone, Copy, Debug, Default)]
pub struct ConjureRunTimings {
    pub wall_clock_time_s: f64,
    pub translation_time_s: f64,
    pub conjure_translation_time_s: f64,
    pub savilerow_translation_time_s: f64,
    pub solve_time_s: f64,
}

#[derive(Debug)]
pub struct ConjureSolutions {
    pub solutions: Vec<BTreeMap<Name, Literal>>,
    pub timings: Option<ConjureRunTimings>,
}

pub use conjure_cp::solutions::{
    SolutionCollectionTimings, get_solutions, get_solutions_with_timings,
};

#[derive(Clone, Debug, Default)]
pub struct ConjureSolveCaptureOptions {
    /// When set, `conjure solve -o` writes models and Minion files here instead of a temp dir.
    pub artifact_dir: Option<PathBuf>,
    /// Passed to `conjure solve --savilerow-options` (e.g. `-O0`).
    pub savilerow_options: Option<String>,
}

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure(
    essence_file: &str,
    param_file: Option<&str>,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Vec<BTreeMap<Name, Literal>>, anyhow::Error> {
    Ok(get_solutions_from_conjure_with_stats(
        essence_file,
        param_file,
        context,
        0,
        ConjureSolveCaptureOptions::default(),
    )?
    .solutions)
}

#[allow(clippy::unwrap_used)]
pub fn get_solutions_from_conjure_with_stats(
    essence_file: &str,
    param_file: Option<&str>,
    context: Arc<RwLock<Context<'static>>>,
    number_of_solutions: i32,
    capture_options: ConjureSolveCaptureOptions,
) -> Result<ConjureSolutions, anyhow::Error> {
    enum ConjureOutputDir {
        Temp(tempfile::TempDir),
        Fixed(PathBuf),
    }

    impl ConjureOutputDir {
        fn path(&self) -> &std::path::Path {
            match self {
                Self::Temp(dir) => dir.path(),
                Self::Fixed(path) => path,
            }
        }
    }

    let output_dir = match &capture_options.artifact_dir {
        Some(path) => {
            // A failed run leaves its diagnostics behind. Conjure may reuse files in an existing
            // output directory, and the glob below would also read stale solution files, so each
            // invocation must start with an empty artifact directory.
            if path.exists() {
                fs::remove_dir_all(path)?;
            }
            fs::create_dir_all(path)?;
            ConjureOutputDir::Fixed(path.clone())
        }
        None => ConjureOutputDir::Temp(tempdir()?),
    };

    let mut cmd = std::process::Command::new("conjure");
    let number_of_solutions_arg = if number_of_solutions == 0 {
        "all".to_string()
    } else {
        number_of_solutions.to_string()
    };

    cmd.arg("solve")
        .arg(format!("--number-of-solutions={number_of_solutions_arg}"))
        .arg("--copy-solutions=no")
        .arg("--solutions-in-one-file")
        .arg("--output-format=json")
        .arg("-o")
        .arg(output_dir.path());

    if let Some(options) = &capture_options.savilerow_options {
        cmd.arg(format!("--savilerow-options={options}"));
    }

    cmd.arg(essence_file);

    if let Some(file) = param_file {
        cmd.arg(file);
    }

    let conjure_solve_start = Instant::now();
    let output = cmd.output()?;
    let conjure_solve_wall_time_s = conjure_solve_start.elapsed().as_secs_f64();

    if !output.status.success() {
        let stderr =
            String::from_utf8(output.stderr).unwrap_or_else(|e| e.utf8_error().to_string());
        return Err(anyhow::Error::msg(format!(
            "Error: `conjure solve` exited with code {}; stderr: {}",
            output.status, stderr
        )));
    }

    let domains = domains_for_conjure_solutions(essence_file, param_file, Arc::clone(&context))?;

    let solutions_files: Vec<_> =
        glob(&format!("{}/*.solutions.json", output_dir.path().display()))?.collect();
    if solutions_files.is_empty() {
        // Unsatisfiable / no solutions: Conjure may omit the solutions file.
        let timings = read_conjure_timings(output_dir.path(), conjure_solve_wall_time_s)?;
        return Ok(ConjureSolutions {
            solutions: Vec::new(),
            timings,
        });
    }

    let mut solutions_set = Vec::new();
    for solutions_file in solutions_files {
        let solutions_file = solutions_file?;
        let text = fs::read_to_string(&solutions_file)?;
        solutions_set.extend(solutions_from_simplified_json_str(&text, &domains)?);
    }

    let timings = read_conjure_timings(output_dir.path(), conjure_solve_wall_time_s)?;

    Ok(ConjureSolutions {
        solutions: solutions_set
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect(),
        timings,
    })
}

fn domains_for_conjure_solutions(
    essence_file: &str,
    param_file: Option<&str>,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<BTreeMap<Name, conjure_cp::ast::DomainPtr>, anyhow::Error> {
    let problem = parse_essence_file_native(essence_file, Arc::clone(&context))?;
    let unified = match param_file {
        Some(param_path) if param_path.ends_with(".json") => {
            let given_domains = domains_from_model(&problem);
            let params =
                params_from_simplified_json_str(&fs::read_to_string(param_path)?, &given_domains)?;
            let param_model =
                param_model_from_assignments(params, &given_domains, Arc::clone(&context));
            instantiate_model(problem, param_model)?
        }
        Some(param_path) => {
            let param_model = parse_essence_file_native(param_path, Arc::clone(&context))?;
            instantiate_model(problem, param_model)?
        }
        None => problem,
    };
    Ok(domains_from_model(&unified))
}

fn read_conjure_timings(
    path: &std::path::Path,
    conjure_solve_wall_time_s: f64,
) -> Result<Option<ConjureRunTimings>, anyhow::Error> {
    let stats_files: Vec<_> = glob(&format!("{}/*.stats.json", path.display()))?.collect();
    if stats_files.is_empty() {
        return Ok(None);
    }

    let mut timings = ConjureRunTimings {
        wall_clock_time_s: conjure_solve_wall_time_s,
        ..Default::default()
    };
    for stats_file in stats_files {
        let stats_file = stats_file?;
        let stats: JsonValue = serde_json::from_str(&fs::read_to_string(&stats_file)?)?;
        let savilerow_total_time = stats
            .pointer("/savilerowInfo/SavileRowTotalTime")
            .and_then(JsonValue::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or_default();
        let solve_time = stats
            .pointer("/savilerowInfo/SolverTotalTime")
            .and_then(JsonValue::as_str)
            .and_then(|value| value.parse::<f64>().ok())
            .or_else(|| {
                stats
                    .pointer("/savilerowInfo/SolverSolveTime")
                    .and_then(JsonValue::as_str)
                    .and_then(|value| value.parse::<f64>().ok())
            })
            .unwrap_or_default();

        timings.savilerow_translation_time_s += savilerow_total_time;
        timings.solve_time_s += solve_time;
    }

    timings.conjure_translation_time_s =
        (conjure_solve_wall_time_s - timings.savilerow_translation_time_s - timings.solve_time_s)
            .max(0.0);
    timings.translation_time_s =
        timings.conjure_translation_time_s + timings.savilerow_translation_time_s;

    Ok(Some(timings))
}

pub fn solutions_to_json(solutions: &[BTreeMap<Name, Literal>]) -> JsonValue {
    let json_solutions = solutions.iter().map(solution_to_json).collect();
    let ans = JsonValue::Array(json_solutions);
    sort_json_object(&ans, true)
}

/// Render solutions in the format produced by Conjure's `--solutions-in-one-file` option.
pub fn solutions_to_essence(solutions: &[BTreeMap<Name, Literal>]) -> String {
    let mut solutions = solutions.iter().collect::<Vec<_>>();
    solutions.sort_by(|lhs, rhs| solution_essence_cmp(lhs, rhs));

    let mut output = String::new();
    for (index, solution) in solutions.iter().enumerate() {
        writeln!(output, "$ Solution: {:06}", index + 1).unwrap();
        writeln!(output, "language Essence 1.3\n").unwrap();
        let mut unnamed_types = BTreeMap::new();
        for value in solution.values() {
            for literal in value.universe() {
                if let Literal::Unnamed(value) = literal {
                    unnamed_types.insert(value.ty.name.clone(), value.ty.size);
                }
            }
        }
        for (name, size) in unnamed_types {
            let members = (1..=size).map(|index| format!("{name}_{index}")).join(", ");
            writeln!(output, "letting {name} be new type enum {{{members}}}").unwrap();
        }
        for (name, value) in *solution {
            writeln!(output, "letting {name} be {value}").unwrap();
        }
        output.push_str("\n\n");
    }
    output
}

fn solution_essence_cmp(
    lhs: &BTreeMap<Name, Literal>,
    rhs: &BTreeMap<Name, Literal>,
) -> std::cmp::Ordering {
    lhs.iter()
        .zip(rhs)
        .find_map(|((lhs_name, lhs_value), (rhs_name, rhs_value))| {
            let ordering = lhs_name.cmp(rhs_name);
            (ordering != std::cmp::Ordering::Equal)
                .then_some(ordering)
                .or_else(|| {
                    let ordering = lhs_value.essence_cmp(rhs_value);
                    (ordering != std::cmp::Ordering::Equal).then_some(ordering)
                })
        })
        .unwrap_or_else(|| lhs.len().cmp(&rhs.len()))
}

fn solution_to_json(solution: &BTreeMap<Name, Literal>) -> JsonValue {
    let mut json_solution = Map::new();
    for (var_name, constant) in solution {
        let serialized_constant = serde_json::to_value(constant).unwrap();
        json_solution.insert(var_name.to_string(), serialized_constant);
    }
    sort_json_object(&JsonValue::Object(json_solution), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_conjure_multi_solution_essence_format() {
        let mut first = BTreeMap::new();
        first.insert(Name::User("b".into()), Literal::Bool(false));
        first.insert(Name::User("x".into()), Literal::Int(1));
        let mut second = BTreeMap::new();
        second.insert(Name::User("b".into()), Literal::Bool(true));
        second.insert(Name::User("x".into()), Literal::Int(2));

        assert_eq!(
            solutions_to_essence(&[first, second]),
            concat!(
                "$ Solution: 000001\n",
                "language Essence 1.3\n\n",
                "letting b be false\n",
                "letting x be 1\n\n\n",
                "$ Solution: 000002\n",
                "language Essence 1.3\n\n",
                "letting b be true\n",
                "letting x be 2\n\n\n",
            )
        );
    }
}
