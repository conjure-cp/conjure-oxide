use std::{collections::HashMap, env::Vars, io::Lines};

use rustsat::{
    clause,
    instances::{BasicVarManager, Cnf, SatInstance},
    solvers::{Solve, SolverResult},
    types::{Lit, TernaryVal},
};

// use rustsat::{
//     instances::{BasicVarManager, Cnf, SatInstance},
//     solvers::SolverResult,
//     types::Lit,
// };
use rustsat_minisat::core::Minisat;

use crate::ast::Expression;

pub fn handle_lit(
    l1: &Expression,
    vars_added: &mut HashMap<String, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    match l1 {
        // not literal
        Expression::Not(_, _) => handle_not(l1, vars_added, inst),

        // simple literal
        Expression::Atomic(_, _) => handle_atom(l1.clone(), true, vars_added, inst),
        _ => panic!("Literal expected"),
    }
}

pub fn handle_not(
    expr: &Expression,
    vars_added: &mut HashMap<String, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    match expr {
        Expression::Not(_, ref_heap_a) => {
            let a = ref_heap_a.clone();
            handle_atom(*a, false, vars_added, inst)
        }
        _ => panic!("Not Expected"),
    }
}

pub fn handle_atom(
    a: Expression,
    polarity: bool,
    vars_added: &mut HashMap<String, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    // polarity false for not

    // let lit: Lit;
    match a {
        Expression::Atomic(_, atom) => match atom {
            conjure_core::ast::Atom::Literal(literal) => todo!(),
            conjure_core::ast::Atom::Reference(name) => match name {
                conjure_core::ast::Name::UserName(n) => {
                    // TODO: Temp Clone
                    let m = n.clone();
                    let lit_temp: Lit = fetch_lit(n, vars_added, inst);
                    if polarity {
                        print!("  {} ", m);
                        lit_temp
                    } else {
                        print!(" ¬{} ", m);
                        !lit_temp
                    }
                }
                conjure_core::ast::Name::MachineName(_) => todo!(),
            },
        },
        _ => panic!("atomic expected"),
    }
}

pub fn fetch_lit(
    symbol: String,
    vars_added: &mut HashMap<String, Lit>,
    inst: &mut SatInstance,
) -> Lit {
    let lit_to_add: Lit;

    if !vars_added.contains_key(&symbol) {
        vars_added.insert(symbol.to_string(), inst.new_lit());
    }

    lit_to_add = *(vars_added.get(&symbol).unwrap());
    lit_to_add
}

pub fn handle_disjn(
    disjn: &Expression,
    vars_added: &mut HashMap<String, Lit>,
    inst_in_use: &mut SatInstance,
) {
    let cl = match disjn {
        Expression::Or(_, vec) => vec,
        _ => panic!(),
    };
    let l1 = &cl[0];
    let l2 = &cl[1];

    print!("clause i.e: ");
    // handle literal:
    let lit1: Lit = handle_lit(l1, vars_added, inst_in_use);
    // also handle literal
    let lit2: Lit = handle_lit(l2, vars_added, inst_in_use);

    print!("\n");
    println!("clause being added: {}, {}", lit1, lit2);

    inst_in_use.add_binary(lit1, lit2);
}

pub fn handle_cnf(vec_cnf: &Vec<Expression>) {
    let mut vars_added: HashMap<String, Lit> = HashMap::new();
    let mut inst_in_use = SatInstance::new();

    println!("------------Or Constraints------------\n\n");

    for disjn in vec_cnf {
        handle_disjn(disjn, &mut vars_added, &mut inst_in_use);
    }

    println!("\n..finished loading..\n\n");

    println!("---------------Solution---------------\n\n");
    let mut solver: Minisat = rustsat_minisat::core::Minisat::default();

    let cnf: (Cnf, BasicVarManager) = inst_in_use.into_cnf();
    println!("CNF: {:?}", cnf.0);

    solver.add_cnf(cnf.0).unwrap();
    let res = solver.solve().unwrap();

    print!("Solution: ");
    match res {
        SolverResult::Sat => println!("SAT"),
        SolverResult::Unsat => println!("UNSAT"),
        SolverResult::Interrupted => println!("NOPE"),
    }

    // assert_eq!(res[l1.var()], TernaryVal::True);
    // assert_eq!(res[l2.var()], TernaryVal::True);
}
