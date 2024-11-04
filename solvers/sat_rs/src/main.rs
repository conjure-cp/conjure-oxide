use std::vec;

use core::num;

use rustsat::types::Clause;
use rustsat::{clause, solvers, types::Lit};  
use rustsat::instances::SatInstance;


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

pub enum mode {
    // negative/positve
    zero_normal,
    // traditional
    general_cnf,
}

pub fn pretty_print_prob(var_type: mode, to_convert: Vec<Vec<i16>>) {

}

pub fn conv_to_clause(to_convert: Vec<i16>) {
    
}

pub fn conv_to_fomula() {
    
}

pub fn add_lits(vec_lits: Vec<Vec<>>) {

}