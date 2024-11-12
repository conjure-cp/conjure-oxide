use rustsat::instances::SatInstance;
use rustsat::types::Lit;

pub fn conv_to_clause(to_convert: &Vec<i32>, instance_in_use: &mut SatInstance) -> () {
    let l1: Lit = mk_lit(to_convert[0], instance_in_use);
    let l2: Lit = mk_lit(to_convert[1], instance_in_use);

    instance_in_use.add_binary(l1, l2); // (!x or y) and (x or y)
}

pub fn mk_lit(num: i32, instance_in_use: &mut SatInstance) -> Lit {
    let var = instance_in_use.new_var();
    let lit;

    // decide polarity
    if num >= 0 {
        lit = var.pos_lit();
    } else {
        lit = var.neg_lit();
    }

    lit
}

pub fn conv_to_formula(vec_cnf: &Vec<Vec<i32>>, instance_in_use: &mut SatInstance) {
    for value in vec_cnf {
        (conv_to_clause(value, instance_in_use));
    }
}
