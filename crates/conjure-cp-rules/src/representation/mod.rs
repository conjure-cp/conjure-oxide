//! Representation rule library

/// Prelude for representation rule writing.
mod prelude {
    #![allow(unused_imports)]
    pub use conjure_cp::{
        ast::Metadata,
        ast::{
            AbstractLiteral, Atom, DeclarationPtr, Expression, Literal, Name, RecordEntry,
            SymbolTable, matrix,
        },
        bug, into_matrix,
        representation::register_representation,
        rule_engine::{ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult},
    };
}

pub mod matrix_to_atom;
pub mod record_to_atom;
pub mod sat_direct_int;
pub mod set_explicit;
pub mod tuple_to_atom;
// mod sat_log_int;
// mod sat_order_int;

pub use matrix_to_atom::MatrixToAtom;
