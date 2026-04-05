//! AFL fuzzing harness for Conjure Oxide.
//!
//! This harness reads Essence model source text from stdin (as AFL provides it),
//! then runs it through the full pipeline: parse - rewrite - solve.
//!
//! # Building
//!
//! ```bash
//! cargo install cargo-afl
//! cargo afl build -p conjure-cp-fuzz --release
//! ```
//!
//! # Running
//!
//! ```bash
//! cargo afl fuzz \
//!     -i crates/conjure-cp-fuzz/seeds \
//!     -o crates/conjure-cp-fuzz/corpus \
//!     -x crates/conjure-cp-fuzz/essence.dict \
//!     target/release/conjure-fuzz-harness
//! ```

// Force the linker to include conjure-cp-rules' rule registry.
// Without this, the distributed-slice rule registration is dropped as dead code.
#[allow(unused)]
use conjure_cp_rules as _;

use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence;
use conjure_cp::rule_engine::{resolve_rule_sets, rewrite_naive};
use conjure_cp::settings::{SolverFamily, set_current_solver_family};
use conjure_cp::solver::Solver;
use conjure_cp::solver::adaptors::Minion;

/// Attempt to parse, rewrite, and solve an Essence model from source text.
///
/// Returns `Ok(())` on any "normal" outcome (parse error, rewrite error, solve
/// success/failure). The only way this returns `Err` is on an unexpected internal
/// bug — which is exactly what we want AFL to notice.
fn run_pipeline(src: &str) -> Result<(), String> {
    let (model, _source_map) = match parse_essence(src) {
        Ok(result) => result,
        Err(_) => return Ok(()), // syntactically invalid
    };

    let target_family = SolverFamily::Minion;
    set_current_solver_family(target_family);

    let rule_sets = resolve_rule_sets(target_family, DEFAULT_RULE_SETS)
        .map_err(|e| format!("rule set resolution failed: {e}"))?;

    let rewritten = match rewrite_naive(&model, &rule_sets, false) {
        Ok(m) => m,
        Err(_) => return Ok(()), // rewrite failure
    };

    let solver = Solver::new(Minion::default());
    let solver = match solver.load_model(rewritten) {
        Ok(s) => s,
        Err(_) => return Ok(()), // model-load failure
    };

    // Run solver, collecting at most 1 solution to keep it fast.
    let _result = solver.solve(Box::new(|_| true));

    Ok(())
}

fn main() {
    afl::fuzz!(|data: &[u8]| {
        // AFL provides raw bytes — try to interpret as UTF-8.
        let src = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return, // not valid UTF-8, skip
        };

        // Run the pipeline. Panics (from bug!() / assert! / etc.) will be
        // caught by AFL as crashes.
        let _ = run_pipeline(src);
    });
}
