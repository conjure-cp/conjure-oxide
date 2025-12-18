//! Representation rule library

/// Prelude for representation rule writing.
mod prelude {
    #![allow(unused_imports)]
    pub use conjure_cp::{
        ast::Metadata,
        ast::{
            matrix, AbstractLiteral, Atom, DeclarationPtr, Expression, Literal, Name, RecordEntry,
            SymbolTable,
        },
        bug, into_matrix, register_representation,
        representation::{get_repr_rule, Representation},
        rule_engine::{ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult},
    };
}

mod matrix_to_atom;
mod record_to_atom;
mod sat_direct_int;
mod sat_log_int;
mod tuple_to_atom;
