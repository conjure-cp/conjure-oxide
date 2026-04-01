use crate::guard;
use crate::representation::record_to_tuple::RecordToTuple;
use conjure_cp::ast::{Atom, Expression, Metadata, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};
use uniplate::Uniplate;

/// Indexing into a record variable
/// e.g:
/// ```plain
/// x[a]
/// ~>
/// x_RecordToTuple[1]
/// ```
/// where
/// ```plain
/// x: record { a : bool, b : int(0..9) }
/// ```
#[register_rule(("ReprGeneral", 2000))]
fn index_record_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (subject, indices, safe) = match expr {
        Expression::UnsafeIndex(_, subject, indices) => (subject, indices, false),
        Expression::SafeIndex(_, subject, indices) => (subject, indices, true),
        _ => return Err(RuleNotApplicable),
    };

    guard!(
        let Expression::Atomic(_, Atom::Reference(re)) = &**subject                &&
        let Some(repr) = re.get_repr_as::<RecordToTuple>()                         &&
        let Some(Expression::Atomic(_, Atom::Reference(idx_re))) = indices.first() &&
        let Some((field_name, _)) = idx_re.ptr.as_record_field()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let lhs = repr
        .name_to_idx_expr(&field_name)
        .expect("unexpected record index");
    let rhs = &indices[1..];

    if rhs.is_empty() {
        Ok(Reduction::pure(lhs))
    } else if safe {
        let new_expr = Expression::SafeIndex(Metadata::new(), lhs.into(), Vec::from(rhs));
        Ok(Reduction::pure(new_expr))
    } else {
        let new_expr = Expression::UnsafeIndex(Metadata::new(), lhs.into(), Vec::from(rhs));
        Ok(Reduction::pure(new_expr))
    }
}

/// Convert all references to a record variable outside of indexing expressions to a tuple
#[register_rule(("ReprGeneral", 2000))]
fn ref_record_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if let Expression::SafeIndex(..) | Expression::UnsafeIndex(..) = expr {
        return Err(RuleNotApplicable);
    };

    let mut changed = false;
    let new_children = expr
        .children()
        .into_iter()
        .map(|expr| {
            if let Expression::Atomic(_, Atom::Reference(re)) = &expr
                && let Some(repr) = re.get_repr_as::<RecordToTuple>()
            {
                changed = true;
                Reference::new(repr.tuple.clone()).into()
            } else {
                expr
            }
        })
        .collect();

    if changed {
        Ok(Reduction::pure(expr.with_children(new_children)))
    } else {
        Err(RuleNotApplicable)
    }
}
