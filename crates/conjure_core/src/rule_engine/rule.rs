use std::fmt::{self, Display, Formatter};
use std::hash::Hash;

use thiserror::Error;

use crate::ast::{Expression, SymbolTable};
use crate::metadata::Metadata;
use crate::model::Model;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Rule is not applicable")]
    RuleNotApplicable,

    #[error("Could not find the min/max bounds for the expression")]
    BoundError,
}

/// The result of applying a rule to an expression.
///
/// Contains an expression to replace the original, a top-level constraint to add to the top of the constraint AST, and an expansion to the model symbol table.
#[non_exhaustive]
#[derive(Clone, Debug)]
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

    // Apply side-effects (e.g. symbol table updates
    pub fn apply(self, model: &mut Model) {
        model.variables.extend(self.symbols); // Add new assignments to the symbol table
        if self.new_top.is_nothing() {
            model.constraints = self.new_expression.clone();
        } else {
            model.constraints = match self.new_expression {
                Expression::And(metadata, mut exprs) => {
                    // Avoid creating a nested conjunction
                    exprs.push(self.new_top.clone());
                    Expression::And(metadata.clone_dirty(), exprs)
                }
                _ => Expression::And(
                    Metadata::new(),
                    vec![self.new_expression.clone(), self.new_top],
                ),
            };
        }
    }
}

/**
 * A rule with a name, application function, and rule sets.
 *
 * # Fields
 * - `name` The name of the rule.
 * - `application` The function to apply the rule.
 * - `rule_sets` A list of rule set names and priorities that this rule is a part of. This is used to populate rulesets at runtime.
 */
#[derive(Clone, Debug)]
pub struct Rule<'a> {
    pub name: &'a str,
    pub application: fn(&Expression, &Model) -> ApplicationResult,
    pub rule_sets: &'a [(&'a str, u8)], // (name, priority). At runtime, we add the rule to rulesets
}

impl<'a> Rule<'a> {
    pub const fn new(
        name: &'a str,
        application: fn(&Expression, &Model) -> ApplicationResult,
        rule_sets: &'a [(&'static str, u8)],
    ) -> Self {
        Self {
            name,
            application,
            rule_sets,
        }
    }

    pub fn apply(&self, expr: &Expression, mdl: &Model) -> ApplicationResult {
        (self.application)(expr, mdl)
    }
}

impl<'a> Display for Rule<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl<'a> PartialEq for Rule<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<'a> Eq for Rule<'a> {}

impl<'a> Hash for Rule<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
