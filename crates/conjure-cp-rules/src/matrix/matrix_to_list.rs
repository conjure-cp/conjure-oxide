use std::collections::VecDeque;

use conjure_cp::ast::{Domain, Expression as Expr, Range, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};
use uniplate::Uniplate;

/// Converts a matrix to a list if possible.
///
/// A list is a matrix with the unbounded domain `int(1..)`. Unlike matrices in general, lists can
/// be resized; consequently, a lot more rules apply to them.
///
/// A matrix can be converted to a list if:
///
///  1. It has some contiguous domain `int(1..n)`.
///
///  2. It is a matrix literal (i.e. not a reference to a decision variable).
///
///  3. Its direct parent is a constraint, not another matrix or `AbstractLiteral`.
///
///    This prevents the conversion of rows in a 2d matrix from being turned into lists. If were
///    to happen, the rows of the matrix might become different lengths, which is invalid!
///
///  4. The matrix is stored as `Expression` type inside (i.e. not as an `Atom` inside a Minion
///     constraint)
///
/// Because of condition 4, and this rules low priority, this rule will not run post-flattening, so
/// matrices that do not need to be converted to lists in order to get them ready for Minion will
/// be left alone.
#[register_rule(("Base", 2000))]
fn matrix_to_list(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // match on the parent: do not apply this rule to things descended from abstract literal, or
    // special language constructs like bubble.
    //
    // As Minion/ flat constraints do not have expression children, they are automatically
    // excluded.

    if matches!(
        expr,
        Expr::AbstractLiteral(_, _)
            | Expr::Bubble(_, _, _)
            | Expr::Atomic(_, _)
             // not sure if this needs to be excluded, being cautious.
            | Expr::DominanceRelation(_, _)
    ) {
        return Err(RuleNotApplicable);
    }

    let mut new_children = VecDeque::new();
    let mut any_changes = false;
    for child in expr.children() {
        // already a list => no change
        if child.clone().unwrap_list().is_some() {
            new_children.push_back(child);
            continue;
        }

        // not a matrix => no change
        let Some((elems, domain)) = child.clone().unwrap_matrix_unchecked() else {
            new_children.push_back(child);
            continue;
        };

        let Domain::Int(ranges) = &domain else {
            new_children.push_back(child);
            continue;
        };

        // must be domain int(1..n)
        let [Range::Bounded(1, _)] = ranges[..] else {
            new_children.push_back(child);
            continue;
        };

        any_changes = true;
        new_children.push_back(into_matrix_expr![elems;Domain::Int(vec![Range::UnboundedR(1)])]);
    }

    let new_expr = expr.with_children(new_children);

    if any_changes {
        Ok(Reduction::pure(new_expr))
    } else {
        Err(RuleNotApplicable)
    }
}
