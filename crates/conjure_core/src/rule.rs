use std::fmt::{self, Display, Formatter};

use thiserror::Error;

use crate::ast::{Expression, SymbolTable};

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Rule is not applicable")]
    RuleNotApplicable,
}

/// The result of applying a rule to an expression.
///
/// Contains an expression to replace the original, a top-level constraint to add to the top of the constraint AST, and an expansion to the model symbol table.
pub struct Reduction {
    pub new_expression: Expression,
    pub new_top: Expression,
    pub symbols: SymbolTable,
}

/// The result of applying a rule to an expression.
/// Contains either a set of reduction instructions or an error.
pub type ApplicationResult = Result<Reduction, ApplicationError>;

impl Reduction {
    pub fn new(new_expression: Expression, new_top: Expression, symbols: SymbolTable) -> Self {
        Self {
            new_expression,
            new_top,
            symbols,
        }
    }

    /// Represents a reduction with no side effects on the model.
    pub fn pure(new_expression: Expression) -> Self {
        Self {
            new_expression,
            new_top: Expression::Nothing,
            symbols: SymbolTable::new(),
        }
    }

    /// Represents a reduction that also modifies the symbol table.
    pub fn with_symbols(new_expression: Expression, symbols: SymbolTable) -> Self {
        Self {
            new_expression,
            new_top: Expression::Nothing,
            symbols,
        }
    }

    /// Represents a reduction that also adds a top-level constraint to the model.
    pub fn with_top(new_expression: Expression, new_top: Expression) -> Self {
        Self {
            new_expression,
            new_top,
            symbols: SymbolTable::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Rule<'a> {
    pub name: &'a str,
    pub application: fn(&Expression) -> ApplicationResult,
}

impl<'a> Rule<'a> {
    pub fn new(name: &'a str, application: fn(&Expression) -> ApplicationResult) -> Self {
        Self { name, application }
    }

    pub fn apply(&self, expr: &Expression) -> ApplicationResult {
        (self.application)(expr)
    }
}

impl<'a> Display for Rule<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}
