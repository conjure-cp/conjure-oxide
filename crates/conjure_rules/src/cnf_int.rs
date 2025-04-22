use conjure_core::ast::AbstractLiteral;
use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::into_matrix_expr;
use conjure_core::matrix_expr;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};

use conjure_core::ast::Atom;
use conjure_core::ast::Domain;
use conjure_core::ast::Expression;
use conjure_core::ast::Literal;
use conjure_core::ast::Name;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::ApplicationError;

//TODO: largely copied from the matrix rules, This should be possible to simplify
#[register_rule(("CNF", 2000))]
fn index_tuple_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // i assume the MkOpIndexing is the same as matrix indexing
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    // tuples are always one dimensional
    if indices.len() != 1 {
        return Err(RuleNotApplicable);
    }

    let repr = symbols
        .get_representation(name, &["tuple_to_atom"])
        .unwrap()[0]
        .clone();

    let decl = symbols.lookup(name).unwrap();

    let Some(Domain::DomainTuple(_)) = decl.domain().cloned().map(|x| x.resolve(symbols)) else {
        return Err(RuleNotApplicable);
    };

    let mut indices_as_lit: Literal = Literal::Bool(false);

    for index in indices {
        let Some(index) = index.clone().to_literal() else {
            return Err(RuleNotApplicable); // we don't support non-literal indices
        };
        indices_as_lit = index;
    }

    let indices_as_name = Name::RepresentedName(
        name.clone(),
        "tuple_to_atom".into(),
        indices_as_lit.to_string(),
    );

    let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

    Ok(Reduction::pure(subject))
}
