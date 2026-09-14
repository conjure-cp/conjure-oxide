//! `FunctionAsRelation`-specific lowering of `image(f, arg)`.
//!
//! The representation stores the function's graph and nothing else -- no per-domain-value lookup
//! table, which is the point of choosing it for a sparse partial function -- so `image` reads that
//! graph directly, through two auxiliaries:
//!
//! ```plain
//! image(f, arg)
//! ~>
//! { value @ defined }
//!   where, at the top level,
//!     defined <-> exists e in relation . e[1] = arg
//!     defined -> (arg, value) in relation
//! ```
//!
//! Both auxiliaries are pinned by *top-level* constraints, not by the bubble condition. That
//! distinction is the whole correctness argument: a bubble condition can be satisfied by being
//! false, so an application sitting in a context where false is acceptable -- `toInt(f(i) != i)`
//! under a `sum(...) = 0`, say -- would otherwise let the solver pick a `value` that is not in the
//! relation at all and call the constraint satisfied. Pinning at the top level leaves it nowhere to
//! go: where the function is defined at `arg`, `defined` is forced true and the membership forces
//! `value` to be the function's value there.
//!
//! `defined` covers both ways an application can be undefined -- a partial function with no entry
//! for `arg`, and an `arg` outside the function's domain -- without either needing a case of its own.

use super::super::FunctionAsRelation;
use super::super::representation::{exists_entry, tuple_field};
use conjure_cp::ast::{
    AbstractLiteral, Atom, Domain, Expression as Expr, GroundDomain, Metadata, Moo, Reference,
    SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("Base", 8400, [Image])]
fn image_function_as_relation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Image(_, function, arg) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<FunctionAsRelation>() else {
        return Err(RuleNotApplicable);
    };

    let domain = function
        .domain_of()
        .ok_or(RuleNotApplicable)?
        .resolve()
        .map_err(|_| RuleNotApplicable)?;
    let GroundDomain::Function(_, _, codomain) = domain.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();
    let value = Expr::from(Reference::new(
        symbols.gen_find_auxiliary(&codomain.clone().into()),
    ));
    let defined = Expr::from(Reference::new(symbols.gen_find_auxiliary(&Domain::bool())));

    let relation = Expr::from(Reference::new(representation.relation_decl.clone()));
    let entry = Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Tuple(vec![arg.as_ref().clone(), value.clone()]),
    );

    let defines_arg = exists_entry(&relation, |entry| {
        Expr::Eq(
            Metadata::new(),
            Moo::new(tuple_field(entry, 1)),
            Moo::new(arg.as_ref().clone()),
        )
    });
    let new_top = vec![
        Expr::Iff(
            Metadata::new(),
            Moo::new(defined.clone()),
            Moo::new(defines_arg),
        ),
        Expr::Imply(
            Metadata::new(),
            Moo::new(defined.clone()),
            Moo::new(Expr::In(
                Metadata::new(),
                Moo::new(entry),
                Moo::new(relation),
            )),
        ),
    ];

    Ok(RuleEffect::new(
        Expr::Bubble(Metadata::new(), Moo::new(value), Moo::new(defined)),
        new_top,
        symbols,
    ))
}
