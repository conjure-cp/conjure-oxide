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
        bug, into_matrix, register_representation,
        representation::{Representation, get_repr_rule},
        rule_engine::{ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult},
    };
}

mod matrix_to_atom;
mod record_to_atom;
mod tuple_to_atom;
