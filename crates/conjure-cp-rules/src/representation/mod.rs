//! Representation rule library

/// Prelude for representation rule writing.
mod prelude {
    #![allow(unused_imports)]
    pub use conjure_cp::{
        ast::Metadata,
        ast::{
            AbstractLiteral, Atom, DeclarationPtr, Expression, Field, Literal, Name, SymbolTable,
            matrix,
        },
        bug, into_matrix,
        representation::register_representation,
        rule_engine::{ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult},
    };
}

pub mod matrix_to_atom;
pub mod record_to_tuple;
mod sat_direct_int;
mod sat_log_int;
mod sat_order_int;
pub mod set_explicit;
pub mod set_occurrence;
pub mod tuple_packed;
pub mod tuple_to_atom;

pub use matrix_to_atom::MatrixToAtom;
pub use record_to_tuple::RecordToTuple;
pub use set_explicit::SetExplicitWithSize;
pub use set_occurrence::SetOccurrence;
pub use tuple_packed::TuplePacked;
pub use tuple_to_atom::TupleToAtom;
