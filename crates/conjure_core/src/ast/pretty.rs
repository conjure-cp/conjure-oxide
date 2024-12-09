//! Functions for pretty printing Conjure models.
//!
//! Most things can be pretty printed using `Display`; however some, notably collections
//! can not, for example, Vec<Expression>

use std::fmt::Display;

use itertools::Itertools;

use super::Expression;

/// Pretty prints a `Vec<Expression>` as if it were a top level constraint list in a `such that`.
///
/// Each expression is printed on a new line, and expressions are delimited by commas.
///
/// For some input expressions A,B,C:
/// ```text
/// A,
/// B,
/// C
/// ```
///
/// Each `Expression` is printed using its underlying `Display` implementation.
pub fn pretty_expressions_as_top_level(expressions: &[Expression]) -> String {
    expressions.iter().map(|x| format!("{}", x)).join(",\n")
}

/// Pretty prints a `Vec<Expression>` as if it were a conjunction.
///
/// For some input expressions A,B,C:
///
/// ```text
/// (A /\ B /\ C)
/// ```
///
/// Each `Expression` is printed using its underlying `Display` implementation.
pub fn pretty_expressions_as_conjunction(expressions: &[Expression]) -> String {
    let mut str = expressions.iter().map(|x| format!("{}", x)).join(" /\\ ");

    str.insert(0, '(');
    str.push(')');

    str
}

/// Pretty prints a `Vec<T>` in a vector like syntax.
///
/// For some input values A,B,C:
///
/// ```text
/// [A,B,C]
/// ````
///
/// Each element is printed using its underlying `Display` implementation.
pub fn pretty_vec<T: Display>(elems: &[T]) -> String {
    let mut str = elems.iter().map(|x| format!("{}", x)).join(", ");
    str.insert(0, '[');
    str.push(']');

    str
}
