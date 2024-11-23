use anyhow::{anyhow, Result};
use rustsat::instances::SatInstance;
use rustsat::types::{Clause, Lit, Var};
use std::collections::HashMap;

pub fn conv_to_clause(
    to_convert: &Vec<i32>,
    instance_in_use: &mut SatInstance,
    var_map: &mut HashMap<i32, Var>,
) -> Result<()> {
    let lits: Vec<Lit> = to_convert
        .iter()
        .map(|&num| mk_lit(num, instance_in_use, var_map))
        .collect::<Result<Vec<Lit>, anyhow::Error>>()?;
    let clause: Clause = lits.into_iter().collect();
    instance_in_use.add_clause(clause);
    Ok(())
}

pub fn mk_lit(
    num: i32,
    instance_in_use: &mut SatInstance,
    var_map: &mut HashMap<i32, Var>,
) -> Result<Lit, anyhow::Error> {
    if num == 0 {
        return Err(anyhow!("Variable index cannot be zero. Received: {}", num));
    }

    let var_index = num.abs();
    let var = if let Some(&v) = var_map.get(&var_index) {
        v
    } else {
        let v = instance_in_use.new_var();
        var_map.insert(var_index, v);
        v
    };
    if num > 0 {
        Ok(var.pos_lit())
    } else {
        Ok(var.neg_lit())
    }
}

pub fn conv_to_formula(vec_cnf: &Vec<Vec<i32>>, instance_in_use: &mut SatInstance) -> Result<()> {
    let mut var_map: HashMap<i32, Var> = HashMap::new();
    for clause in vec_cnf {
        conv_to_clause(clause, instance_in_use, &mut var_map)?;
    }
    Ok(())
}

// use rustsat::instances::SatInstance;
// use rustsat::types::Lit;

// pub fn conv_to_clause(to_convert: &Vec<i32>, instance_in_use: &mut SatInstance) -> () {
//     let l1: Lit = mk_lit(to_convert[0], instance_in_use);
//     let l2: Lit = mk_lit(to_convert[1], instance_in_use);

//     instance_in_use.add_binary(l1, l2); // (!x or y) and (x or y)
// }

// pub fn mk_lit(num: i32, instance_in_use: &mut SatInstance) -> Lit {
//     let var = instance_in_use.new_var();
//     let lit;

//     // decide polarity
//     if num >= 0 {
//         lit = var.pos_lit();
//     } else {
//         lit = var.neg_lit();
//     }

//     lit
// }

// pub fn conv_to_formula(vec_cnf: &Vec<Vec<i32>>, instance_in_use: &mut SatInstance) {
//     for value in vec_cnf {
//         (conv_to_clause(value, instance_in_use));
//     }
// }
