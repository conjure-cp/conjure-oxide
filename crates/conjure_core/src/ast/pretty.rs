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
    expressions.iter().map(|x| format!("{x}")).join(",\n")
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
    let mut str = expressions.iter().map(|x| format!("{x}")).join(" /\\ ");

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
    let mut str = elems.iter().map(|x| format!("{x}")).join(", ");
    str.insert(0, '[');
    str.push(']');

    str
}

/// Pretty prints, in essence syntax, the variable declaration for the given symbol.
///
/// E.g.
///
/// ```text
/// find a: int(1..5)
/// ```
///
/// Returns None if the symbol is not in the symbol table, or if it is not a variable.
pub fn pretty_variable_declaration(symbol_table: &SymbolTable, var_name: &Name) -> Option<String> {
    let decl = symbol_table.lookup(var_name)?;
    let var = decl.as_var()?;
    let domain = &var.domain;
    Some(format!("find {var_name}: {domain}"))
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
    let decl = symbol_table.lookup(name)?;
    let letting = decl.as_value_letting()?;
    Some(format!("letting {name} be {letting}"))
}

/// Pretty prints, in essence syntax, the declaration for the given domain letting.
///
/// E.g.
///
/// ```text
/// letting A be domain bool
/// ```
///
/// Returns None if the symbol is not in the symbol table, or if it is not a domain letting.
pub fn pretty_domain_letting_declaration(
    symbol_table: &SymbolTable,
    name: &Name,
) -> Option<String> {
    let decl = symbol_table.lookup(name)?;
    let letting = decl.as_domain_letting()?;
    Some(format!("letting {name} be domain {letting}"))
}
