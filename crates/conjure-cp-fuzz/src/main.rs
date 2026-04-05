//! AFL fuzzing harness for Conjure Oxide.
//!
//! This harness reads Essence model source text from stdin (as AFL provides it),
//! then runs it through the full pipeline: parse → rewrite → solve.
//!
//! Per-run timeouts are enforced by AFL itself (see run.sh).
//!
//! # Building
//!
//! ```bash
//! cargo install cargo-afl
//! cargo afl build -p conjure-cp-fuzz
//! ```
//!
//! # Running
//!
//! ```bash
//! FUZZ_CORES=8 ./crates/conjure-cp-fuzz/run.sh
//! ```

// Force the linker to include conjure-cp-rules' rule registry.
// Without this, the distributed-slice rule registration is dropped as dead code.
#[allow(unused)]
use conjure_cp_rules as _;

use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence;
use conjure_cp::rule_engine::{resolve_rule_sets, rewrite_naive};
use conjure_cp::settings::{
    QuantifiedExpander, Rewriter, SolverFamily, set_comprehension_expander, set_current_rewriter,
    set_current_solver_family,
};
use conjure_cp::solver::Solver;
use conjure_cp::solver::adaptors::Minion;

/// Attempt to parse, rewrite, and solve an Essence model from source text.
///
/// Parse errors are silently ignored (syntactically invalid input isn't interesting).
/// All other failures (rewrite errors, solver load errors, etc.) are left to
/// panic — AFL will record these as crashes worth investigating.
fn run_pipeline(src: &str) {
    // ── Stage 1: Parse ──────────────────────────────────────────────────
    let (model, _source_map) = match parse_essence(src) {
        Ok(result) => result,
        Err(_) => return, // syntactically invalid — not interesting
    };

    // ── Stage 2: Rewrite ────────────────────────────────────────────────
    // Set thread-local globals that the rewriter/rules expect to be present.
    // These mirror the CLI defaults in conjure-cp-cli.
    let target_family = SolverFamily::Minion;
    set_current_solver_family(target_family);
    set_current_rewriter(Rewriter::Naive);
    set_comprehension_expander(QuantifiedExpander::ViaSolverAc);

    let rule_sets = resolve_rule_sets(target_family, DEFAULT_RULE_SETS)
        .unwrap_or_else(|e| panic!("rule resolution failed: {e}"));

    let rewritten =
        rewrite_naive(&model, &rule_sets, false).unwrap_or_else(|e| panic!("rewrite failed: {e}"));

    // ── Stage 3: Solve ──────────────────────────────────────────────────
    let solver = Solver::new(Minion::default());
    let solver = solver
        .load_model(rewritten)
        .unwrap_or_else(|e| panic!("model load failed: {e}"));

    // Run solver, collecting at most 1 solution to keep it fast.
    let _result = solver.solve(Box::new(|_| true));
}

fn main() {
    afl::fuzz!(|data: &[u8]| {
        // AFL provides raw bytes — try to interpret as UTF-8.
        let src = match std::str::from_utf8(data) {
            Ok(s) => s,
            Err(_) => return, // not valid UTF-8, skip
        };

        // Run the pipeline. Panics (from bug!() / assert! / expect! / etc.)
        // will be caught by AFL as crashes.
        run_pipeline(src);
    });
}
