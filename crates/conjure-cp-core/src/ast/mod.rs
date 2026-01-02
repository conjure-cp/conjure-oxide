pub mod abstract_comprehension;
pub mod ac_operators;
mod atom;
pub mod categories;
mod cnf_clause;
pub mod comprehension;
pub mod declaration;
mod domains;
pub mod eval;
mod expressions;
mod literals;
pub mod matrix;
mod metadata;
mod model;
mod name;
pub mod pretty;
pub mod records;
mod reference;
mod sat_encoding;
pub mod serde;
mod submodel;
mod symbol_table;
mod types;
mod variables;

mod moo;
mod partial_eval;

pub use atom::Atom;
pub use cnf_clause::CnfClause;
pub use declaration::{DeclarationKind, DeclarationPtr};
pub use domains::{
    Domain, DomainOpError, DomainPtr, FuncAttr, GroundDomain, HasDomain, IntVal, JectivityAttr,
    PartialityAttr, Range, RecordEntry, RecordEntryGround, SetAttr, UnresolvedDomain,
};
pub use eval::eval_constant;
pub use expressions::Expression;
pub use literals::AbstractLiteral;
pub use literals::Literal;
pub use metadata::Metadata;
pub use model::*;
pub use moo::Moo;
pub use name::Name;
pub use partial_eval::run_partial_evaluator;
pub use reference::Reference;
pub use sat_encoding::SATIntEncoding;
pub use submodel::SubModel;
pub use symbol_table::SymbolTable;
pub use types::*;
pub use variables::DecisionVariable;

/// Creates a new matrix [`AbstractLiteral`] optionally with some index domain.
///
///  - `matrix![a,b,c]`
///  - `matrix![a,b,c;my_domain]`
///
/// To create one from a (Rust) vector, use [`into_matrix!`].
#[macro_export]
macro_rules! matrix {
    // cases copied from the std vec! macro
    () => (
        $crate::into_matrix![]
    );

    (;$domain:expr) => (
        $crate::into_matrix![;$domain]
    );

    ($x:expr) => (
        $crate::into_matrix![std::vec![$x]]
    );

    ($x:expr;$domain:expr) => (
        $crate::into_matrix![std::vec![$x];$domain]
    );

    ($($x:expr),*) => (
        $crate::into_matrix![std::vec![$($x),*]]
    );

    ($($x:expr),*;$domain:expr) => (
        $crate::into_matrix![std::vec![$($x),*];$domain]
    );

    ($($x:expr,)*) => (
        $crate::into_matrix![std::vec![$($x),*]]
    );

    ($($x:expr,)*;$domain:expr) => (
        $crate::into_matrix![std::vec![$($x),*];$domain]
    )
}

/// Creates a new matrix [`AbstractLiteral`] from some [`Vec`], optionally with some index domain.
///
///  - `matrix![my_vec]`
///  - `matrix![my_vec;my_domain]`
///
/// To create one from a list of elements, use [`matrix!`].
#[macro_export]
macro_rules! into_matrix {
    () => (
        $crate::into_matrix![std::vec::Vec::new()]
    );

    (;$domain:expr) => (
        $crate::into_matrix![std::vec::Vec::new();$domain]
    );
    ($x:expr) => (
        $crate::ast::AbstractLiteral::matrix_implied_indices($x)
    );
    ($x:expr;$domain:expr) => (
        $crate::ast::AbstractLiteral::Matrix($x,$domain)
    );
}

/// Creates a new matrix as an [`Expression`], optionally with some index domain.
///
/// For usage details, see [`matrix!`].
///
/// To create a matrix expression from a [`Vec`], use [`into_matrix_expr!`].
#[macro_export]
macro_rules! matrix_expr {
    () => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![])
    );

    (;$domain:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![;$domain])
    );


    ($x:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![$x])
    );
    ($x:expr;$domain:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![;$domain])
    );

    ($($x:expr),+) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![$($x),+])
    );

    ($($x:expr),+;$domain:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![$($x),+;$domain])
    );

    ($($x:expr,)+) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![$($x),+])
    );

    ($($x:expr,)+;$domain:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::matrix![$($x),+;$domain])
    )
}

/// Creates a new matrix as an [`Expression`] from a (Rust) vector, optionally with some index
/// domain.
///
/// For usage details, see [`into_matrix!`].
///
/// To create a matrix expression from a list of elements, use [`matrix_expr!`].
#[macro_export]
macro_rules! into_matrix_expr {
    () => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::into_matrix![])
    );

    (;$domain:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::into_matrix![;$domain])
    );
    ($x:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::into_matrix![$x])
    );
    ($x:expr;$domain:expr) => (
        $crate::ast::Expression::AbstractLiteral($crate::ast::Metadata::new(),$crate::into_matrix![$x;$domain])
    );
}
