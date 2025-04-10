//! Representation rule library

/// Prelude for representation rule writing.
mod prelude {
    #![allow(unused_imports)]
    pub use crate::ast::{
        AbstractLiteral, Atom, Declaration, Domain, Expression, Literal, Name, SymbolTable,
    };
    pub use crate::bug;
    pub use crate::metadata::Metadata;
    pub use crate::register_represention;
    pub use crate::representation::Representation;
    pub use crate::rule_engine::{
        ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult,
    };
}

mod matrix_to_atom;
mod tuple_to_atom;
