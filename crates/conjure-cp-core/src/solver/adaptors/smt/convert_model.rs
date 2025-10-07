use z3::Solver;
use z3::Symbol;
use z3::ast::*;

use super::store::Store;

use crate::Model;
use crate::ast::Atom;
use crate::ast::Expression;
use crate::ast::{DecisionVariable, DeclarationPtr, Domain, Name, SymbolTable};
use crate::solver::SolverError;

pub fn load_store(store: &mut Store, symbols: &SymbolTable) {
    for (name, decl) in symbols.clone().into_iter_local() {
        let Some(var) = decl.as_var() else {
            continue;
        };
        let ast = var_to_ast(&name, &var);
        store.insert(name, ast);
    }
}

pub fn load_assertions(store: &Store, model: &[Expression], solver: &mut Solver) {
    for expr in model.iter() {
        let bool = expr_to_ast(store, expr)
            .as_bool()
            .expect("top-level expression must be boolean type");
        solver.assert(bool);
    }
}

// TODO: return a Result<Dynamic, SolverError>
fn var_to_ast(name: &Name, decl: &DecisionVariable) -> Dynamic {
    let sym = name_to_symbol(name);
    match decl.domain {
        Domain::Bool => Dynamic::from_ast(&Bool::new_const(sym)),
        _ => unimplemented!(),
    }
}

fn name_to_symbol(name: &Name) -> Symbol {
    match name {
        Name::User(ustr) => Symbol::String((*ustr).into()),
        Name::Machine(num) => Symbol::Int(*num as u32),
        _ => unimplemented!(),
    }
}

fn expr_to_ast(store: &Store, expr: &Expression) -> Dynamic {
    match expr {
        Expression::Atomic(_, atom) => atom_to_ast(store, atom),
        Expression::Neq(_, a, b) => {
            Dynamic::from_ast(&expr_to_ast(store, a).ne(expr_to_ast(store, b)))
        }
        _ => unimplemented!(),
    }
}

fn atom_to_ast(store: &Store, atom: &Atom) -> Dynamic {
    match atom {
        Atom::Reference(decl) => store
            .get(&decl.name())
            .expect("variable does not exist")
            .clone(),
        Atom::Literal(lit) => unimplemented!(),
    }
}
