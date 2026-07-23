mod concat;
mod difference;
mod equals;
mod neq;
mod subset;
mod subseteq;
mod supset;
mod supseteq;
mod union;

use conjure_cp::ast::{Expression, ReturnType, Typeable};

/// Whether an abstract set value is still hidden behind matrix indexing.
///
/// Horizontal set rules must leave these expressions alone until matrix representation lowers the
/// index to the declaration representing that element. Otherwise generic equality/subset lowering
/// creates expression-generator comprehensions over a decision-valued set, which the native
/// comprehension expander intentionally cannot enumerate.
fn is_set_valued_index(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::SafeIndex(..) | Expression::UnsafeIndex(..)
    ) && matches!(expr.return_type(), ReturnType::Set(_))
}
