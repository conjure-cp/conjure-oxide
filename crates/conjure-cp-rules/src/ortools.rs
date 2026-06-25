use crate::utils::to_aux_var;
use conjure_cp::ast::Moo;
use conjure_cp::{
    ast::AbstractLiteral,
    ast::Metadata,
    ast::{Expression as Expr, SymbolTable},
    rule_engine::{ApplicationResult, Reduction, register_rule, register_rule_set},
    settings::SolverFamily,
};

register_rule_set!("OrToolsCpSat", ("Base"), |f: &SolverFamily| {
    matches!(f, SolverFamily::OrToolsCpSat)
});

#[register_rule("OrToolsCpSat", 4200, [Or, And])]
fn flatten_logical(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;

    if !matches!(expr, Expr::Or(_, _) | Expr::And(_, _)) {
        return Err(RuleNotApplicable);
    }

    let mut symbols = symbols.clone();
    let mut new_tops: Vec<Expr> = vec![];

    // Get the inner expression of Or/And (which is the matrix/list)
    let inner_expr = match expr {
        Expr::Or(_, inner) | Expr::And(_, inner) => inner.as_ref(),
        _ => unreachable!(),
    };

    // If it's a matrix literal, we want to flatten its elements
    let Some((es, index_domain)) = inner_expr.clone().unwrap_matrix_unchecked() else {
        return Err(RuleNotApplicable);
    };

    let mut new_es = es;
    let mut num_changed = 0;

    for e in new_es.iter_mut() {
        if let Some(aux_info) = to_aux_var(e, &symbols) {
            symbols = aux_info.symbols();
            new_tops.push(aux_info.top_level_expr());
            *e = aux_info.as_expr();
            num_changed += 1;
        }
    }

    if num_changed == 0 {
        return Err(RuleNotApplicable);
    }

    let new_matrix = Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Matrix(new_es, index_domain),
    );

    let new_expr = match expr {
        Expr::Or(meta, _) => Expr::Or(meta.clone(), Moo::new(new_matrix)),
        Expr::And(meta, _) => Expr::And(meta.clone(), Moo::new(new_matrix)),
        _ => unreachable!(),
    };

    Ok(Reduction::new(new_expr, new_tops, symbols))
}

/// Matrix a = b iff every index in the union of their indices has the same value.
#[register_rule("OrToolsCpSat", 3000, [Eq, Neq])]
fn flatten_matrix_eq_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    use conjure_cp::rule_engine::ApplicationError;
    use conjure_cp::essence_expr;
    use conjure_cp::ast::matrix;
    use conjure_cp::ast::Atom;
    use conjure_cp::ast::Expression;

    let (a, b) = match expr {
        Expr::Eq(_, a, b) | Expr::Neq(_, a, b) => (a, b),
        _ => return Err(ApplicationError::RuleNotApplicable),
    };

    let a_idx_domains = matrix::bound_index_domains_of_expr(a.as_ref()).ok_or(ApplicationError::RuleNotApplicable)?;
    let b_idx_domains = matrix::bound_index_domains_of_expr(b.as_ref()).ok_or(ApplicationError::RuleNotApplicable)?;

    // Only apply if the index domains are actually different, to avoid unnecessary expansion
    // for standard same-domain matrix equality.
    if a_idx_domains == b_idx_domains {
        return Err(ApplicationError::RuleNotApplicable);
    }

    let pairs = matrix::enumerate_index_union_indices(&a_idx_domains, &b_idx_domains)
        .map_err(|_| ApplicationError::DomainError)?
        .map(|idx_lits| {
            let idx_vec: Vec<_> = idx_lits
                .into_iter()
                .map(|lit| Atom::Literal(lit).into())
                .collect();
            (
                Expression::UnsafeIndex(Metadata::new(), a.clone(), idx_vec.clone()),
                Expression::UnsafeIndex(Metadata::new(), b.clone(), idx_vec),
            )
        });

    let new_expr = match expr {
        Expr::Eq(..) => {
            let eqs: Vec<_> = pairs.map(|(a, b)| essence_expr!("&a = &b")).collect();
            Expr::And(
                Metadata::new(),
                Moo::new(Expr::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::matrix_implied_indices(eqs),
                )),
            )
        }
        Expr::Neq(..) => {
            let neqs: Vec<_> = pairs.map(|(a, b)| essence_expr!("&a != &b")).collect();
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

    Ok(Reduction::pure(new_expr))
}
