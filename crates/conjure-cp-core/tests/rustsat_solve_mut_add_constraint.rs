use rustsat::instances::{BasicVarManager, Cnf, SatInstance};
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Assignment, Clause, TernaryVal};
use rustsat_minisat::core::Minisat;

fn blocking_clause_from_assignment(sol: &Assignment) -> Clause {
    let mut blocking = Clause::new();
    for lit in sol.clone().iter().map(|lit| !lit) {
        blocking.add(lit);
    }
    blocking
}

#[test]
fn test_rustsat_callback_style_can_add_hardcoded_clause_mid_search() {
    let mut inst = SatInstance::new();
    let x = inst.new_lit();
    let _y = inst.new_lit();

    let (cnf, _): (Cnf, BasicVarManager) = inst.into_cnf();

    let mut solver = Minisat::default();
    solver.add_cnf(cnf).expect("failed to add initial CNF");

    let mut seen_x_values = Vec::new();
    let mut posted_constraint = false;

    loop {
        match solver.solve().expect("solver failed") {
            SolverResult::Sat => {}
            SolverResult::Unsat => break,
            SolverResult::Interrupted => panic!("solver interrupted"),
        }

        let sol = solver
            .full_solution()
            .expect("failed to fetch SAT solution");
        let x_value = sol.var_value(x.var());
        seen_x_values.push(x_value);

        // Mirror callback behavior: on first solution, post a hardcoded clause mid-search.
        if !posted_constraint {
            let mut hardcoded_clause = Clause::new();
            hardcoded_clause.add(x);
            solver
                .add_clause(hardcoded_clause)
                .expect("failed to add hardcoded clause");
            posted_constraint = true;
            continue;
        }

        solver
            .add_clause(blocking_clause_from_assignment(&sol))
            .expect("failed to add blocking clause");
    }

    assert!(posted_constraint, "expected to post a hardcoded clause");
    assert!(
        seen_x_values.len() >= 2,
        "expected at least one solution after posting clause, got {seen_x_values:?}"
    );
    assert!(
        seen_x_values[1..]
            .iter()
            .all(|value| *value != TernaryVal::False),
        "x was false after posting clause x, seen values: {seen_x_values:?}"
    );
}
