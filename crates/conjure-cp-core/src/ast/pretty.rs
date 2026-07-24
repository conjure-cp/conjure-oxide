//! Functions for pretty printing Conjure models.
//!
//! Most things can be pretty printed using `Display`; however some, notably collections
//! can not, for example, Vec<Expression>

use std::fmt::Display;

use super::{Atom, CnfClause, Expression, Name, SymbolTable};
use crate::ast::domains::HasDomain;
use itertools::Itertools;

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

/// Pretty prints a `Vec<CnfClause>` as a list of clauses as disjunctions
///
/// Each clause is printed on a new line, and expressions are delimited by commas.
///
/// For some input expressions A,B,C:
/// ```text
/// (a_0 \/ ¬a_1 ...),
/// (b_0 \/ b_1 ...),
/// (¬c_0 \/ c_1 ...)
/// ```
///
/// Each `Expression` is printed using its underlying `Display` implementation.
pub fn pretty_clauses(clauses: &[CnfClause]) -> String {
    clauses.iter().map(|clause| format!("{clause}")).join(",\n")
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

/// Pretty prints an expression with an Essence-style type annotation.
///
/// `::` is treated as an expression operator for parenthesisation purposes.
pub fn pretty_expression_type_annotation(expr: &Expression, ty: impl Display) -> String {
    pretty_expression_annotation(expr, "::", ty)
}

/// Pretty prints an expression with an Essence-style domain annotation.
///
/// `:` is treated as an expression operator for parenthesisation purposes.
pub fn pretty_expression_domain_annotation(expr: &Expression, domain: impl Display) -> String {
    pretty_expression_annotation(expr, ":", domain)
}

fn pretty_expression_annotation(
    expr: &Expression,
    operator: &str,
    annotation: impl Display,
) -> String {
    let expr = parenthesise_if_needed(expr, Precedence::ANNOTATION);
    format!("{expr} {operator} {annotation}")
}

fn parenthesise_if_needed(expr: &Expression, parent_precedence: Precedence) -> String {
    let rendered = expr.to_string();
    if expression_precedence(expr).binds_weaker_than(parent_precedence) {
        format!("({rendered})")
    } else {
        rendered
    }
}

/// Expression precedence used for minimal parenthesisation.
///
/// These levels follow the relative operator ordering in
/// `crates/tree-sitter-essence/grammar.js`, with `:` and `::` inserted as annotation operators
/// that bind tighter than binary arithmetic/comparison expressions and looser than unary/postfix
/// expressions. The numeric values are local to the pretty-printer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Precedence(i8);

impl Precedence {
    const LOWEST: Self = Self(-100);
    const IMPLICATION: Self = Self(-4);
    const OR: Self = Self(-2);
    const AND: Self = Self(-1);
    const COMPARISON: Self = Self(5);
    const ADDITIVE: Self = Self(10);
    const MULTIPLICATIVE: Self = Self(20);
    const ANNOTATION: Self = Self(25);
    const UNARY: Self = Self(30);
    const POSTFIX: Self = Self(40);
    const ATOM: Self = Self(100);

    fn binds_weaker_than(self, parent: Self) -> bool {
        self < parent
    }
}

fn expression_precedence(expr: &Expression) -> Precedence {
    match expr {
        Expression::Imply(_, _, _) | Expression::Iff(_, _, _) => Precedence::IMPLICATION,
        Expression::Or(_, _) => Precedence::OR,
        Expression::And(_, _) => Precedence::AND,
        Expression::Eq(_, _, _)
        | Expression::Neq(_, _, _)
        | Expression::Geq(_, _, _)
        | Expression::Leq(_, _, _)
        | Expression::Gt(_, _, _)
        | Expression::Lt(_, _, _)
        | Expression::In(_, _, _)
        | Expression::Supset(_, _, _)
        | Expression::SupsetEq(_, _, _)
        | Expression::Subset(_, _, _)
        | Expression::SubsetEq(_, _, _)
        | Expression::LexLt(_, _, _)
        | Expression::LexLeq(_, _, _)
        | Expression::LexGt(_, _, _)
        | Expression::LexGeq(_, _, _) => Precedence::COMPARISON,
        Expression::Sum(_, _) | Expression::Minus(_, _, _) | Expression::PairwiseSum(_, _, _) => {
            Precedence::ADDITIVE
        }
        Expression::Product(_, _)
        | Expression::UnsafeDiv(_, _, _)
        | Expression::SafeDiv(_, _, _)
        | Expression::UnsafeMod(_, _, _)
        | Expression::SafeMod(_, _, _)
        | Expression::PairwiseProduct(_, _, _) => Precedence::MULTIPLICATIVE,
        Expression::Not(_, _)
        | Expression::Neg(_, _)
        | Expression::Abs(_, _)
        | Expression::Card(_, _)
        | Expression::ToInt(_, _) => Precedence::UNARY,
        Expression::Factorial(_, _)
        | Expression::UnsafePow(_, _, _)
        | Expression::SafePow(_, _, _)
        | Expression::UnsafeIndex(_, _, _)
        | Expression::SafeIndex(_, _, _)
        | Expression::UnsafeSlice(_, _, _)
        | Expression::SafeSlice(_, _, _) => Precedence::POSTFIX,
        Expression::Union(_, _, _) | Expression::Intersect(_, _, _) => Precedence::LOWEST,
        Expression::TypeAnnotation(_, _, _) | Expression::DomainAnnotation(_, _, _) => {
            Precedence::ANNOTATION
        }
        Expression::Atomic(_, Atom::Reference(_))
        | Expression::Atomic(_, Atom::Literal(_))
        | Expression::AbstractLiteral(_, _)
        | Expression::Comprehension(_, _)
        | Expression::AbstractComprehension(_, _)
        | Expression::Metavar(_, _)
        | Expression::FromSolution(_, _) => Precedence::ATOM,
        _ => Precedence::ATOM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Atom, DeclarationPtr, Domain, Metadata, Moo, Name, Range};

    fn atom(name: &str) -> Expression {
        Expression::Atomic(
            Metadata::new(),
            Atom::new_ref(DeclarationPtr::new_find(
                Name::user(name),
                Domain::int(vec![Range::Bounded(0, 10)]),
            )),
        )
    }

    #[test]
    fn domain_annotation_parenthesises_comparison_lhs() {
        let expr = Expression::Eq(
            Metadata::new(),
            Moo::new(atom("y")),
            Moo::new(Expression::Card(Metadata::new(), Moo::new(atom("x")))),
        );

        assert_eq!(
            pretty_expression_domain_annotation(&expr, "bool"),
            "(y = |x|) : bool"
        );
    }

    #[test]
    fn domain_annotation_does_not_parenthesise_unary_or_atom_lhs() {
        let x = atom("x");
        let card = Expression::Card(Metadata::new(), Moo::new(x.clone()));

        assert_eq!(pretty_expression_domain_annotation(&x, "int"), "x : int");
        assert_eq!(
            pretty_expression_domain_annotation(&card, "int"),
            "|x| : int"
        );
    }

    #[test]
    fn type_annotation_uses_same_expression_precedence_as_domain_annotation() {
        let expr = Expression::Eq(Metadata::new(), Moo::new(atom("x")), Moo::new(atom("y")));

        assert_eq!(
            pretty_expression_type_annotation(&expr, "bool"),
            "(x = y) :: bool"
        );
    }
}

/// Pretty prints, in essence syntax, the variable declaration for the given symbol.
///
/// E.g.
///
/// ```text
/// find a: int(1..5)
/// ```
///
/// When a representation has been selected for an abstract domain, the domain is printed with
/// that representation's short name, e.g. `find x: set{occurrence} (maxSize 3) of int(1..4)`.
///
/// Returns None if the symbol is not in the symbol table, or if it is not a variable.
pub fn pretty_variable_declaration(symbol_table: &SymbolTable, var_name: &Name) -> Option<String> {
    let decl = symbol_table.lookup(var_name)?;
    let var = decl.as_find()?;
    let domain = var.domain_of();
    let domain_str = format_domain_with_selected_representation(&decl, &domain);
    let keyword = if decl.is_find_auxiliary() {
        "findAux"
    } else {
        "find"
    };
    Some(format!("{keyword} {var_name}: {domain_str}"))
}

/// Format a domain for display, overlaying a selected representation short name when present.
fn format_domain_with_selected_representation(
    decl: &crate::ast::DeclarationPtr,
    domain: &crate::ast::DomainPtr,
) -> String {
    let selected = decl.reprs().iter().find_map(|(_, state)| {
        let rule = state.rule();
        // MatrixComponents is structural layout, not an abstract-domain representation name.
        if rule.short_name() == "components" {
            None
        } else {
            Some(rule.short_name())
        }
    });

    match selected {
        Some(short_name) if domain.representation_preference() != Some(short_name) => {
            format_domain_with_representation(domain, short_name)
        }
        _ => domain.to_string(),
    }
}

/// Print `domain` as usual, but with `repr` as the top-level set representation preference.
fn format_domain_with_representation(domain: &crate::ast::DomainPtr, repr: &str) -> String {
    use crate::ast::{Domain, GroundDomain, UnresolvedDomain};

    match domain.as_ref() {
        Domain::Ground(gd) => match gd.as_ref() {
            GroundDomain::Set(attrs, inner) => {
                let size = attrs.to_string();
                if size.is_empty() {
                    format!("set{{{repr}}} of {inner}")
                } else {
                    format!("set{{{repr}}} {size} of {inner}")
                }
            }
            _ => domain.to_string(),
        },
        Domain::Unresolved(ud) => match ud.as_ref() {
            UnresolvedDomain::Set(attrs, inner) => {
                let size = attrs.to_string();
                if size.is_empty() {
                    format!("set{{{repr}}} of {inner}")
                } else {
                    format!("set{{{repr}}} {size} of {inner}")
                }
            }
            _ => domain.to_string(),
        },
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
