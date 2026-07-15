use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::{DomainError, RuleNotApplicable},
    ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{ast::matrix, essence_expr};

/// Matrix a = b iff every index in the union of their indices has the same value.
/// E.g. a: matrix indexed by [int(1..2)] of int(1..2), b: matrix indexed by [int(2..3)] of int(1..2)
/// a = b ~> a[1] = b[1] /\ a[2] = b[2] /\ a[3] = b[3]
///
/// Must run before `index_matrix_to_atom` ("Base", 5000), otherwise matrix equality can be
/// rewritten into `int(1..)` indexed literals, losing finite index bounds for this rule.
#[register_rule("Base", 3000, [Eq, Neq])]
fn flatten_matrix_eq_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (a, b) = match expr {
        Expr::Eq(_, a, b) | Expr::Neq(_, a, b) => (a, b),
        _ => return Err(RuleNotApplicable),
    };

    let a_idx_domains = matrix::bound_index_domains_of_expr(a.as_ref()).ok_or(RuleNotApplicable)?;
    let b_idx_domains = matrix::bound_index_domains_of_expr(b.as_ref()).ok_or(RuleNotApplicable)?;

    let pairs = matrix::enumerate_index_union_indices(&a_idx_domains, &b_idx_domains)
        .map_err(|_| DomainError)?
        .map(|idx_lits| {
            let idx_vec: Vec<_> = idx_lits
                .into_iter()
                .map(|lit| Atom::Literal(lit).into())
                .collect();
            (
                Expr::UnsafeIndex(Metadata::new(), a.clone(), idx_vec.clone()),
                Expr::UnsafeIndex(Metadata::new(), b.clone(), idx_vec),
            )
        });

    let new_expr = match expr {
        Expr::Eq(..) => {
            let eqs: Vec<_> = pairs.map(|(a, b)| essence_expr!(&a = &b)).collect();
            Expr::And(
                Metadata::new(),
                Moo::new(Expr::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::matrix_implied_indices(eqs),
                )),
            )
        }
        Expr::Neq(..) => {
            let neqs: Vec<_> = pairs.map(|(a, b)| essence_expr!(&a != &b)).collect();
            Expr::Or(
                Metadata::new(),
                Moo::new(Expr::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::matrix_implied_indices(neqs),
                )),
            )
        }
        _ => unreachable!(),
    };

    Ok(RuleEffect::pure(new_expr))
}
