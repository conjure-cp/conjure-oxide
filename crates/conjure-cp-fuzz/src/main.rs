//! AFL fuzzing harness for Conjure Oxide.
//!
//! This harness reads Essence model source text from stdin (as AFL provides it),
//! then runs it through both our pipeline and `conjure solve`, comparing the
//! solutions. The only "crash" AFL sees is a solution mismatch — everything
//! else (parse errors, rewrite failures, solver failures, panics) is silently
//! swallowed so the corpus converges on models that produce wrong answers.
//!
//! # Building
//!
//! ```bash
//! cargo install cargo-afl
//! cargo afl build -p conjure-cp-fuzz --profile profiling
//! ```
//!
//! # Running
//!
//! ```bash
//! FUZZ_CORES=8 ./crates/conjure-cp-fuzz/run.sh
//! ```

// Force the linker to include conjure-cp-rules' rule registry.
#[allow(unused)]
use conjure_cp_rules as _;

use anyhow::anyhow;
use conjure_cp::ast::{Literal, Name};
use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence_file;
use conjure_cp::rule_engine::{resolve_rule_sets, rewrite_naive};
use conjure_cp::settings::{
    QuantifiedExpander, Rewriter, SolverFamily, set_comprehension_expander, set_current_rewriter,
    set_current_solver_family,
};
use conjure_cp::solver::Solver;
use conjure_cp::solver::adaptors::Minion;
use std::collections::BTreeMap;
use std::fs::File;
use std::panic::{self, AssertUnwindSafe};
use std::process;
use tempfile::tempdir;

use conjure_cp_cli::utils::conjure::{
    get_solutions, get_solutions_from_conjure, solutions_to_json,
};
use conjure_cp_cli::utils::testing::normalize_solutions_for_comparison;

/// Write Essence to a temp file
pub fn write_model(src: &str) -> Result<(String, tempfile::TempDir), anyhow::Error> {
    use std::io::Write;

    let tmp_dir = tempdir()?;
    let model_path = tmp_dir.path().join("model.essence");
    File::create(&model_path)?.write_all(src.as_bytes())?;
    let path_str = model_path
        .to_str()
        .ok_or(anyhow!("invalid UTF-8"))?
        .to_string();
    Ok((path_str, tmp_dir)) // caller keeps tmp_dir alive
}

/// Run our pipeline (parse - rewrite - solve) and return all solutions.
/// Returns `None` if any stage fails.
fn oxide_solutions(pth: &str) -> Option<Vec<BTreeMap<Name, Literal>>> {
    let model = parse_essence_file(pth, Default::default()).ok()?;

    let target_family = SolverFamily::Minion;
    set_current_solver_family(target_family);
    set_current_rewriter(Rewriter::Naive);
    set_comprehension_expander(QuantifiedExpander::ViaSolverAc);

    let rule_sets = resolve_rule_sets(target_family, DEFAULT_RULE_SETS).ok()?;
    let rewritten = rewrite_naive(&model, &rule_sets, false).ok()?;

    let solver = Solver::new(Minion::default());
    get_solutions(solver, rewritten, 0, &None)
        .ok()
        .map(|r| r.solutions)
}

/// Run `conjure solve` on the given source text and return all solutions.
///
/// Returns `None` if conjure fails or the model is invalid.
fn conjure_solutions(pth: &str) -> Option<Vec<BTreeMap<Name, Literal>>> {
    get_solutions_from_conjure(pth, None, Default::default()).ok()
}

/// The core fuzz target.
///
/// Returns normally in all cases except a solution mismatch, where it calls
/// `process::abort()` so AFL registers the input as a crash.
fn run_pipeline(src: &str) {
    let Some((essence_file, tmp_guard)) = write_model(src).ok() else {
        return;
    };

    // Run both pipelines, catching panics
    let oxide = panic::catch_unwind(AssertUnwindSafe(|| oxide_solutions(&essence_file)));
    let conjure = panic::catch_unwind(AssertUnwindSafe(|| conjure_solutions(&essence_file)));

    // Unwrap panic results — if either panicked, treat as "no solutions"
    let oxide = oxide.ok().flatten();
    let conjure = conjure.ok().flatten();

    // Both must have produced solutions for us to compare
    let (oxide, conjure) = match (oxide, conjure) {
        (Some(o), Some(c)) => (o, c),
        _ => return, // Neither could solve, skip
    };

    // Normalize and compare
    let oxide_normalized = normalize_solutions_for_comparison(&oxide);
    let conjure_normalized = normalize_solutions_for_comparison(&conjure);

    let oxide_json = solutions_to_json(&oxide_normalized);
    let conjure_json = solutions_to_json(&conjure_normalized);

    if oxide_json != conjure_json {
        // clean up before we abort
        drop(tmp_guard);
        // Case we want to catch - conjure and oxide disagreeing
        process::abort();
    }
}

fn main() {
    afl::fuzz!(|data: &[u8]| {
        let src = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return,
        };
        run_pipeline(src);
    });
}
