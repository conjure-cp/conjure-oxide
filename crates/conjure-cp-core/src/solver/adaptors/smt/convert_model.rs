use z3::Solver;
use z3::Symbol;
use z3::ast::*;

use super::store::Store;

use crate::Model;
use crate::ast::AbstractLiteral;
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
        Expression::Neq(_, a, b) => binop(store, a, b, ast_id, ast_id, |a, b| a.ne(b)),
        Expression::Eq(_, a, b) => binop(store, a, b, ast_id, ast_id, |a, b| a.eq(b)),

        Expression::Not(_, a) => unop(store, a, Dynamic::as_bool, |a| a.not()),

        Expression::Imply(_, a, b) => {
            binop(store, a, b, Dynamic::as_bool, Dynamic::as_bool, |a, b| {
                a.implies(b)
            })
        }
        Expression::Iff(_, a, b) => {
            binop(store, a, b, Dynamic::as_bool, Dynamic::as_bool, |a, b| {
                a.iff(b)
            })
        }

        // TODO: support AND once it's relevant, currently they are bubbled up
        Expression::Or(_, a) => {
            let exprs =
                a.as_ref()
                    .clone()
                    .unwrap_list()
                    .ok_or(SolverError::ModelFeatureNotImplemented(format!(
                        "inner expression must be a list: {expr}"
                    )))?;
            manyop(store, exprs.as_slice(), Dynamic::as_bool, |asts| {
                Bool::or(asts)
            })
        }

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

fn conv_ast<From>(
    ast: Dynamic,
    conv: impl Fn(&Dynamic) -> Option<From>,
) -> Result<From, SolverError> {
    conv(&ast).ok_or(SolverError::ModelInvalid(format!(
        "conversion failed on: {ast}"
    )))
}

fn ast_id(ast: &Dynamic) -> Option<Dynamic> {
    Some(ast.clone())
}

fn unop<FromA, Out>(
    store: &Store,
    a: &Expression,
    conv_a: impl Fn(&Dynamic) -> Option<FromA>,
    op: impl Fn(FromA) -> Out,
) -> Result<Dynamic, SolverError>
where
    Out: Into<Dynamic>,
{
    let a_ast = conv_ast(expr_to_ast(store, a)?, &conv_a)?;
    Ok((op)(a_ast).into())
}

fn binop<FromA, FromB, Out>(
    store: &Store,
    a: &Expression,
    b: &Expression,
    conv_a: impl Fn(&Dynamic) -> Option<FromA>,
    conv_b: impl Fn(&Dynamic) -> Option<FromB>,
    op: impl Fn(FromA, FromB) -> Out,
) -> Result<Dynamic, SolverError>
where
    Out: Into<Dynamic>,
{
    let a_ast = conv_ast(expr_to_ast(store, a)?, conv_a)?;
    let b_ast = conv_ast(expr_to_ast(store, b)?, conv_b)?;
    Ok((op)(a_ast, b_ast).into())
}

fn manyop<From, Out>(
    store: &Store,
    exprs: &[Expression],
    conv: impl Fn(&Dynamic) -> Option<From>,
    op: impl Fn(&[From]) -> Out,
) -> Result<Dynamic, SolverError>
where
    Out: Into<Dynamic>,
{
    // Result implements FromIter, collecting into either the full collection or an error
    let asts: Result<Vec<_>, SolverError> = exprs
        .iter()
        .map(|e| expr_to_ast(store, e).and_then(|ast| conv_ast(ast, &conv)))
        .collect();
    let asts = asts?;

    Ok((op)(asts.as_slice()).into())
}
