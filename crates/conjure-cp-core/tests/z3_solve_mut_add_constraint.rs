#![cfg(feature = "smt")]

use z3::{
    SatResult, Solver,
    ast::Bool,
};

#[test]
fn test_z3_callback_style_can_add_hardcoded_constraint_mid_search() {
    let solver = Solver::new();
    let x = Bool::new_const("x");

    // Keep the search unconstrained initially so both x=true/false are possible.
    solver.assert(x.eq(&x));

    let mut seen_x_values = Vec::new();
    let mut posted_constraint = false;

    loop {
        match solver.check() {
            SatResult::Sat => {}
            SatResult::Unsat => break,
            SatResult::Unknown => panic!("z3 returned unknown"),
        }

        let model = solver.get_model().expect("failed to fetch SAT model");
        let x_value = model
            .eval(&x, true)
            .and_then(|v| v.as_bool())
            .expect("failed to evaluate x from model");
        seen_x_values.push(x_value);

        // Mirror callback behavior: on first solution, post a hardcoded
        // additional constraint and continue search.
        if !posted_constraint {
            solver.assert(&x);
            posted_constraint = true;
            continue;
        }

        // Block the current single-variable model so the loop terminates.
        solver.assert(if x_value { x.not() } else { x.clone() });
    }

    assert!(posted_constraint, "expected to post a hardcoded constraint");
    assert!(
        seen_x_values.len() >= 2,
        "expected at least one solution after posting constraint, got {seen_x_values:?}"
    );
    assert!(
        seen_x_values[1..].iter().all(|value| *value),
        "x was false after posting hardcoded constraint x, seen values: {seen_x_values:?}"
    );
}
