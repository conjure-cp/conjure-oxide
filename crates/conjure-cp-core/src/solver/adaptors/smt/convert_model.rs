use z3::Solver;
use z3::Symbol;
use z3::ast::*;

use super::store::Store;

use crate::Model;
use crate::ast::{
    Atom, DecisionVariable, DeclarationPtr, Domain, Expression, Literal, Name, SymbolTable,
};
use crate::solver::SolverError;

/// Adds variables from the symbol table to the store to be referenced when adding constraints.
pub fn load_store(store: &mut Store, symbols: &SymbolTable) -> Result<(), SolverError> {
    for (name, decl) in symbols.clone().into_iter_local() {
        let Some(var) = decl.as_var() else {
            continue;
        };
        let ast = var_to_ast(&name, &var)?;
        store.insert(name, ast);
    }
    Ok(())
}

/// Converts top-level expressions into assertions on the model.
pub fn load_assertions(
    store: &Store,
    model: &[Expression],
    solver: &mut Solver,
) -> Result<(), SolverError> {
    for expr in model.iter() {
        let bool = expr_to_ast(store, expr)?
            .as_bool()
            .ok_or(SolverError::ModelInvalid(format!(
                "top-level expression must be boolean type: {expr}",
            )))?;
        solver.assert(bool);
    }
    Ok(())
}

fn var_to_ast(name: &Name, decl: &DecisionVariable) -> Result<Dynamic, SolverError> {
    let sym = name_to_symbol(name)?;
    match decl.domain {
        Domain::Bool => Ok(Dynamic::from_ast(&Bool::new_const(sym))),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "domain kind for '{name}' not implemented: {}",
            decl.domain
        ))),
    }
}

fn name_to_symbol(name: &Name) -> Result<Symbol, SolverError> {
    match name {
        Name::User(ustr) => Ok(Symbol::String((*ustr).into())),
        Name::Machine(num) => Ok(Symbol::Int(*num as u32)),
        _ => Err(SolverError::ModelInvalid(format!(
            "variable '{name}' is part of a representation"
        ))),
    }
}

fn expr_to_ast(store: &Store, expr: &Expression) -> Result<Dynamic, SolverError> {
    match expr {
        Expression::Atomic(_, atom) => atom_to_ast(store, atom),

        // Since equality is part of the core SMT theory, any two dynamic ASTs
        // of the same type can be compared with it.
        // We simply convert back into a Dynamic with `.into()`
        Expression::Neq(_, a, b) => binop(store, a, b, |a, b| a.ne(b).into()),
        Expression::Eq(_, a, b) => binop(store, a, b, |a, b| a.eq(b).into()),

        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "expression type not yet implemented: {expr}"
        ))),
    }
}

fn atom_to_ast(store: &Store, atom: &Atom) -> Result<Dynamic, SolverError> {
    match atom {
        Atom::Reference(decl) => store
            .get(&decl.name())
            .ok_or(SolverError::ModelInvalid(format!(
                "variable '{}' does not exist",
                decl.name()
            )))
            .cloned(),
        Atom::Literal(lit) => literal_to_ast(lit),
    }
}

fn literal_to_ast(lit: &Literal) -> Result<Dynamic, SolverError> {
    match lit {
        Literal::Bool(b) => Ok(Bool::from_bool(*b).into()),
        _ => Err(SolverError::ModelFeatureNotImplemented(format!(
            "literal type not implemented: {lit}"
        ))),
    }
}

fn binop(
    store: &Store,
    a: &Expression,
    b: &Expression,
    op: impl Fn(Dynamic, Dynamic) -> Dynamic,
) -> Result<Dynamic, SolverError> {
    let a_ast = expr_to_ast(store, a)?;
    let b_ast = expr_to_ast(store, b)?;
    Ok((op)(a_ast, b_ast))
}
