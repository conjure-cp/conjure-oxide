pub mod sat_tree;

use rustsat::types::{Clause, Literal, Var};
use rustsat::{clause, solvers, types::Lit};  
use rustsat::instances::{self, SatInstance};

use std::vec;

pub fn conv_to_clause(to_convert: Vec<i16>, instance_in_use: &mut SatInstance) -> () {
    let l1: Lit = mk_lit(to_convert[0], instance_in_use);
    let l2: Lit = mk_lit(to_convert[1], instance_in_use);

    instance_in_use.add_binary(lit1, lit2); // (!x or y) and (x or y)
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
        (conv_to_clause(value, instance_in_use));
    }
}

// [[0, 1][1, 1][0, 0]] 