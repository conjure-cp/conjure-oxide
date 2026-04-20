use crate::bottom_up_adaptor::as_bottom_up;
use crate::guard;
use crate::representation::record_to_tuple::RecordToTuple;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression, Literal, Metadata, Reference, SymbolTable,
};
use conjure_cp::bug::UnwrapOrBug;
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
#[register_rule(("ReprGeneral", 5000))]
fn index_record_to_tuple(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    as_bottom_up(index_record_to_tuple_impl)(expr, symbols)
}

fn index_record_to_tuple_impl(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::RecordField(_, rec_expr, field_name) = expr        &&
        let Expression::Atomic(_, Atom::Reference(re)) = rec_expr.as_ref() &&
        let Some(repr) = re.get_repr_as::<RecordToTuple>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let new_expr = repr.name_to_idx_expr(field_name).unwrap_or_bug();
    Ok(Reduction::pure(new_expr))
}

/// Convert all record literals to tuples
#[register_rule(("ReprGeneral", 2000))]
fn record_lit_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Atomic(_, Atom::Literal(lit)) = expr              &&
        let Literal::AbstractLiteral(AbstractLiteral::Record(ents)) = lit
        else {
            return Err(RuleNotApplicable);
        }
    );

    let mut ents = ents.clone();
    ents.sort();

    let tuple = AbstractLiteral::Tuple(ents.into_iter().map(|x| x.value).collect());
    let new_expr = Expression::Atomic(Metadata::new(), Atom::Literal(tuple.into()));
    Ok(Reduction::pure(new_expr))
}

/// Convert all record expressions to tuples
#[register_rule(("ReprGeneral", 2000))]
fn record_abslit_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let Expression::AbstractLiteral(_, AbstractLiteral::Record(ents)) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut ents = ents.clone();
    ents.sort();

    let tuple = AbstractLiteral::Tuple(ents.into_iter().map(|x| x.value).collect());
    let new_expr = Expression::AbstractLiteral(Metadata::new(), tuple);
    Ok(Reduction::pure(new_expr))
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
