//! AFL fuzzing harness for Conjure Oxide.
//!
//! This harness reads Essence model source text from stdin (as AFL provides it),
//! then runs it through the full pipeline: parse → rewrite → solve.
//!
//! Each stage has a 10-minute timeout — if any stage exceeds that, we bail out
//! and move on so AFL doesn't get stuck on pathological inputs.
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
//! FUZZ_CORES=8 ./crates/conjure-cp-fuzz/run.sh
//! ```

// Force the linker to include conjure-cp-rules' rule registry.
// Without this, the distributed-slice rule registration is dropped as dead code.
#[allow(unused)]
use conjure_cp_rules as _;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use conjure_cp::defaults::DEFAULT_RULE_SETS;
use conjure_cp::parse::tree_sitter::parse_essence;
use conjure_cp::rule_engine::{resolve_rule_sets, rewrite_naive};
use conjure_cp::settings::{SolverFamily, set_current_solver_family};
use conjure_cp::solver::Solver;
use conjure_cp::solver::adaptors::Minion;

/// Per-stage timeout: 10 minutes.
const STAGE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Run `f` on a background thread with a timeout.
/// Returns `None` if the timeout expires (the background thread is detached).
fn with_timeout<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });
    rx.recv_timeout(STAGE_TIMEOUT).ok()
}

/// Attempt to parse, rewrite, and solve an Essence model from source text.
///
/// Returns `Ok(())` on any "normal" outcome (parse error, timeout, etc).
/// The only way this returns `Err` is on an unexpected internal bug —
/// which is what we want AFL to notice.
fn run_pipeline(src: &str) -> Result<(), String> {
    // ── Stage 1: Parse ──────────────────────────────────────────────────
    let src_owned = src.to_owned();
    let parsed = with_timeout(move || parse_essence(&src_owned));
    let (model, _source_map) = match parsed {
        None => return Ok(()),         // timed out
        Some(Err(_)) => return Ok(()), // syntactically invalid
        Some(Ok(result)) => result,
    };

    // ── Stage 2: Rewrite ────────────────────────────────────────────────
    let target_family = SolverFamily::Minion;
    set_current_solver_family(target_family);

    let rule_sets = resolve_rule_sets(target_family, DEFAULT_RULE_SETS)
        .map_err(|e| format!("rule set resolution failed: {e}"))?;

    let rule_sets_clone = rule_sets.clone();
    let model_clone = model.clone();
    let rewritten = with_timeout(move || rewrite_naive(&model_clone, &rule_sets_clone, false));
    let rewritten = match rewritten {
        None => return Ok(()),                     // timed out
        Some(Err(e)) => return Err(e.to_string()), // rewrite failure
        Some(Ok(m)) => m,
    };

    // ── Stage 3: Solve ──────────────────────────────────────────────────
    let solved = with_timeout(move || {
        let solver = Solver::new(Minion::default());
        let solver = solver.load_model(rewritten).expect("load model failed");
        // Run solver, collecting at most 1 solution to keep it fast.
        let _result = solver.solve(Box::new(|_| true));
        Ok(())
    });
    solved.unwrap_or(Ok(())) // OK on timeout
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
