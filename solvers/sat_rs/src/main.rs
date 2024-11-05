use std::vec;

use sat_tree::conv_to_fomula;

use rustsat_minisat::core::MiniSat;
fn main() {
    
    /**
     * Problem: (t or t) and (f or t) and (f or f)
    */

    let v1: Vec<i16> = vec![1, 1];
    let v2: Vec<i16> = vec![0, 1];
    let v3: Vec<i16> = vec![0, 0];

    let vec_problem: Vec<Vec<i16>> = vec![v1, v2, v3];

    // tree
    let mut inst: SatInstance = SatInstance::new();
    // sat solver
    let mut solver = MiniSat::default();

    conv_to_fomula();
}

#[cfg(test)]
#[test]
fn test1() {
    use kissat_rs::Assignment;
    use kissat_rs::Solver;

    // Define three literals used in both formulae.
    let x = 1;
    let y = 2;
    let z = 3;

    // Construct a formula from clauses (i.e. an iterator over literals).
    // (~x || y) && (~y || z) && (x || ~z) && (x || y || z)
    let formula1 = vec![vec![-x, y], vec![-y, z], vec![x, -z], vec![x, y, z]];
    let satisfying_assignment = Solver::solve_formula(formula1).unwrap();

    // The formula from above is satisfied by the assignment: x -> True, y -> True, z -> True
    if let Some(assignments) = satisfying_assignment {
        assert_eq!(assignments.get(&x).unwrap(), &Some(Assignment::True));
        assert_eq!(assignments.get(&y).unwrap(), &Some(Assignment::True));
        assert_eq!(assignments.get(&z).unwrap(), &Some(Assignment::True));
    }

    // (x || y || ~z) && ~x && (x || y || z) && (x || ~y)
    let formula2 = vec![vec![x, y, -z], vec![-x], vec![x, y, z], vec![x, -y]];
    let unsat_result = Solver::solve_formula(formula2).unwrap();

    // The second formula is unsatisfiable.
    // This can for example be proved by resolution.
    assert_eq!(unsat_result, None);
}