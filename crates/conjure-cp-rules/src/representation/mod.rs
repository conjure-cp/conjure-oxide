//! Representation rule library

/// Prelude for representation rule writing.
pub(crate) mod prelude {
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

pub use crate::matrix::components::MatrixComponents;
pub use crate::record::tuple::RecordToTuple;
pub use crate::set::explicit::SetExplicit;
pub use crate::set::occurrence::SetOccurrence;
pub use crate::set::packed::SetPacked;
pub use crate::tuple::components::TupleComponents;
pub use crate::tuple::packed::TuplePacked;
