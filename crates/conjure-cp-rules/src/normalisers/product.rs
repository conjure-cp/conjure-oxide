//! Normalising rules for `Product`

use conjure_cp::rule_engine::register_rule;

use conjure_cp::{
    ast::Metadata,
    ast::{Atom, Expression as Expr, Literal as Lit, Moo, SymbolTable, categories::CategoryOf},
    bug, into_matrix_expr,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
};

/// Reorders a product expression.
///
///  All literal coefficients in the product are folded together, and placed at the start of the
///  product.
///
/// Factors are first sorted by category. Then within each category, references are placed before
/// compound expressions.
///
/// # Justification
///
/// + Having a canonical ordering here is helpful in identifying weighted sums: 2x + 3y + 4d + ....
///
/// + Having constant, quantified, given references appear before decision variable references
///   means that we do not have to reorder the product again once those references get evaluated to
///   literals later on in the rewriting process.
///
#[register_rule(("Base", 8800))]
fn reorder_product(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Product(meta, factors) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let (factors, index_domain) = Moo::unwrap_or_clone(factors)
        .unwrap_matrix_unchecked()
        .ok_or(RuleNotApplicable)?;
    let factors_copy = factors.clone();

    // Order variables by category.
    //
    // This ensures that references that will eventually become constants are in front of decision
    // variables, preventing them from needing to be moved again once they do become constant.
    let mut constant_exprs: Vec<Expr> = vec![];
    let mut bottom_exprs: Vec<Expr> = vec![];
    let mut parameter_exprs: Vec<Expr> = vec![];
    let mut quantified_exprs: Vec<Expr> = vec![];
    let mut decision_exprs: Vec<Expr> = vec![];

    for expr in factors {
        match expr.category_of() {
            conjure_cp::ast::categories::Category::Bottom => bottom_exprs.push(expr),
            conjure_cp::ast::categories::Category::Constant => constant_exprs.push(expr),
            conjure_cp::ast::categories::Category::Parameter => parameter_exprs.push(expr),
            conjure_cp::ast::categories::Category::Quantified => quantified_exprs.push(expr),
            conjure_cp::ast::categories::Category::Decision => decision_exprs.push(expr),
        }
    }

    let mut coefficient = 1;

    let (i, constant_exprs) = order_by_complexity(constant_exprs);
    coefficient *= i;

    let (i, parameter_exprs) = order_by_complexity(parameter_exprs);
    coefficient *= i;

    let (i, quantified_exprs) = order_by_complexity(quantified_exprs);
    coefficient *= i;

    let (i, decision_exprs) = order_by_complexity(decision_exprs);
    coefficient *= i;

    let (i, bottom_exprs) = order_by_complexity(bottom_exprs);
    coefficient *= i;

    let mut factors = if coefficient != 1 {
        vec![Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Lit::Int(coefficient)),
        )]
    } else {
        vec![]
    };

    factors.extend(constant_exprs);
    factors.extend(bottom_exprs);
    factors.extend(parameter_exprs);
    factors.extend(quantified_exprs);
    factors.extend(decision_exprs);

    // check if we have actually done anything
    if factors_copy != factors {
        Ok(Reduction::pure(Expr::Product(
            meta,
            Moo::new(into_matrix_expr!(factors;index_domain)),
        )))
    } else {
        Err(RuleNotApplicable)
    }
}

// orders factors by "complexity":
//
// This returns an integer coefficient, created by folding all literals in `factors` into one
// value, as well a list of expressions ordered like so:
//
// 1. references
// 2. other expressions
fn order_by_complexity(factors: Vec<Expr>) -> (i32, Vec<Expr>) {
    // literal coefficient
    let mut literal: i32 = 1;
    let mut variables: Vec<Expr> = vec![];
    let mut compound_exprs: Vec<Expr> = vec![];

    for expr in factors {
        match expr {
            Expr::Atomic(_, Atom::Literal(lit)) => {
                let Lit::Int(i) = lit else {
                    bug!("Literals in a product operation should be integer, but got {lit}")
                };
                literal *= i;
            }

            Expr::Atomic(_, Atom::Reference(_)) => {
                variables.push(expr);
            }

            // -1 * literal
            Expr::Neg(_, expr2) if matches!(*expr2, Expr::Atomic(_, Atom::Literal(_))) => {
                let Expr::Atomic(_, Atom::Literal(lit)) = &*expr2 else {
                    unreachable!()
                };

                let Lit::Int(i) = lit else {
                    bug!("Literals in a product operation should be integer, but got {lit}")
                };

                literal *= -i;
            }

            // -1 * x
            Expr::Neg(_, expr2) if matches!(&*expr2, Expr::Atomic(_, Atom::Reference(_))) => {
                literal *= -1;
                variables.push(Moo::unwrap_or_clone(expr2));
            }

            // -1 * <expression>
            Expr::Neg(_, expr2) => {
                literal *= -1;
                compound_exprs.push(Moo::unwrap_or_clone(expr2));
            }
            _ => {
                compound_exprs.push(expr);
            }
        }
    }
    variables.extend(compound_exprs);

    (literal, variables)
}

/// Removes products with a single argument.
///
/// ```text
/// product([a]) ~> a
/// ```
///
#[register_rule(("Base", 8800))]
fn remove_unit_vector_products(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Product(_, mat) => {
            let list = (**mat).clone().unwrap_list().ok_or(RuleNotApplicable)?;
            if list.len() == 1 {
                return Ok(Reduction::pure(list[0].clone()));
            }
            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
