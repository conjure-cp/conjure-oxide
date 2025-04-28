//! Representation rule library

/// Prelude for representation rule writing.
mod prelude {
    #![allow(unused_imports)]
    pub use conjure_core::{
        ast::{
            matrix, AbstractLiteral, Atom, Declaration, Domain, Expression, Literal, Name,
            RecordEntry, SymbolTable,
        },
        bug, into_matrix,
        metadata::Metadata,
        register_representation,
        representation::{get_repr_rule, Representation},
        rule_engine::{ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult},
    };
}

mod matrix_to_atom;
mod record_to_atom;
mod tuple_to_atom;
