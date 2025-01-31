//! Functions for pretty printing Conjure models.
//!
//! Most things can be pretty printed using `Display`; however some, notably collections
//! can not, for example, Vec<Expression>

use std::fmt::Display;

use itertools::Itertools;

use super::{Expression, Name, SymbolTable};

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

/// Pretty prints, in essence syntax, the variable declaration for the given symbol.
///
/// E.g.
///
/// ```text
/// a: int(1..5)
/// ```
///
/// Returns None if the symbol is not in the symbol table, or if it is not a variable.
pub fn pretty_variable_declaration(symbol_table: &SymbolTable, var_name: &Name) -> Option<String> {
    let var = symbol_table.get_var(var_name)?;
    match &var.domain {
        super::Domain::BoolDomain => Some(format!("{}: bool", var_name)),
        super::Domain::IntDomain(domain) => {
            let mut domain_ranges: Vec<String> = vec![];
            for range in domain {
                domain_ranges.push(match range {
                    super::Range::Single(a) => a.to_string(),
                    super::Range::Bounded(a, b) => format!("{}..{}", a, b),
                });
            }

            if domain_ranges.is_empty() {
                Some(format!("{}: int", var_name))
            } else {
                Some(format!("{}: int({})", var_name, domain_ranges.join(",")))
            }
        }
    }
}

/// Pretty prints, in essence syntax, the declaration for the given value letting.
///
/// E.g.
///
/// ```text
/// letting A be 1+2+3
/// ```
///
/// Returns None if the symbol is not in the symbol table, or if it is not a value letting.
pub fn pretty_value_letting_declaration(symbol_table: &SymbolTable, name: &Name) -> Option<String> {
    let letting = symbol_table.get_value_letting(name)?;
    Some(format!("letting {name} be {letting}"))
}
