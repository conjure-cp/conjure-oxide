use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Uniplate};

use crate::ast::{Expression, Moo, SubModel, SymbolTablePtr, abstract_comprehension::AbstractComprehension};

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Uniplate)]
pub enum Scope {
    SubModel(SubModel), 
    AbstractComprehension(AbstractComprehension),
    ScopedExpression(SymbolTablePtr, Moo<Expression>)
}

impl Biplate<Expression> for Scope {
    fn biplate() {
        
    }
}