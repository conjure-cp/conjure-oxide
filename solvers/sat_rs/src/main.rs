use std::vec;

use core::num;

use rustsat::types::{Clause, Literal, Var};
use rustsat::{clause, solvers, types::Lit};  
use rustsat::instances::{self, SatInstance};


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

    let l1: rustsat::types::Lit = inst.new_lit();
    let cl1: Clause = Clause::new();
}

pub fn conv_to_clause(to_convert: Vec<i16>, instance_in_use: &mut SatInstance) -> () {
    let l1: Lit = mk_lit(to_convert[0], instance_in_use);
    let l2: Lit = mk_lit(to_convert[1], instance_in_use);

    instance_in_use.add_binary(lit1, lit2);
}

pub fn mk_lit(num: i16, instance_in_use: &mut SatInstance) -> Lit {

    let var = instance_in_use.new_var();

    let polarity: bool;
    if num >= 0 {
        polarity = true;
    } else {
        polarity = false;
    }

    let lit = Literal::new(polarity);
    lit
}

pub fn conv_to_fomula(vec_cnf: &Vec<Vec<i16>>, instance_in_use: &mut SatInstance) {
    
    for value in &vec_cnf {
        instance_in_use.add_clause(conv_to_clause(value, instance_in_use));
    }
}
