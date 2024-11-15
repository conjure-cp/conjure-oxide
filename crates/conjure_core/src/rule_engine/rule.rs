use std::collections::HashSet;
use std::fmt::{self, Display, Formatter};
use std::hash::Hash;

use thiserror::Error;

use crate::ast::{Expression, Name, SymbolTable};
use crate::metadata::Metadata;
use crate::model::Model;

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Rule is not applicable")]
    RuleNotApplicable,

    #[error("Could not calculate the expression domain")]
    DomainError,
}

/// Represents the result of applying a rule to an expression within a model.
///
/// A `Reduction` encapsulates the changes made to a model during a rule application.
/// It includes a new expression to replace the original one, an optional top-level constraint
/// to be added to the model, and any updates to the model's symbol table.
///
/// This struct allows for representing side-effects of rule applications, ensuring that
/// all modifications, including symbol table expansions and additional constraints, are
/// accounted for and can be applied to the model consistently.
///
/// # Fields
/// - `new_expression`: The updated [`Expression`] that replaces the original one after applying the rule.
/// - `new_top`: An additional top-level [`Expression`] constraint that should be added to the model. If no top-level
///   constraint is needed, this field can be set to `Expression::Nothing`.
/// - `symbols`: A [`SymbolTable`] containing any new symbol definitions or modifications to be added to the model's
///   symbol table. If no symbols are modified, this field can be set to an empty symbol table.
///
/// # Usage
/// A `Reduction` can be created using one of the provided constructors:
/// - [`Reduction::new`]: Creates a reduction with a new expression, top-level constraint, and symbol modifications.
/// - [`Reduction::pure`]: Creates a reduction with only a new expression and no side-effects on the symbol table or constraints.
/// - [`Reduction::with_symbols`]: Creates a reduction with a new expression and symbol table modifications, but no top-level constraint.
/// - [`Reduction::with_top`]: Creates a reduction with a new expression and a top-level constraint, but no symbol table modifications.
///
/// The `apply` method allows for applying the changes represented by the `Reduction` to a [`Model`].
///
/// # Example
/// ```
/// // Need to add an example
/// ```
///
/// # See Also
/// - [`ApplicationResult`]: Represents the result of applying a rule, which may either be a `Reduction` or an `ApplicationError`.
/// - [`Model`]: The structure to which the `Reduction` changes are applied.
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
            new_top: Expression::And(Metadata::new(), Vec::new()),
            symbols: SymbolTable::new(),
        }
    }

    /// Represents a reduction that also modifies the symbol table.
    pub fn with_symbols(new_expression: Expression, symbols: SymbolTable) -> Self {
        Self {
            new_expression,
            new_top: Expression::And(Metadata::new(), Vec::new()),
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
        model.extend_sym_table(self.symbols);

        // TODO: (yb33) Remove it when we change constraints to a vector
        if let Expression::And(_, exprs) = &self.new_top {
            if exprs.is_empty() {
                model.constraints = self.new_expression.clone();
                return;
            }
        }

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
    pub rule_sets: &'a [(&'a str, u16)], // (name, priority). At runtime, we add the rule to rulesets
}

impl<'a> Rule<'a> {
    pub const fn new(
        name: &'a str,
        application: fn(&Expression, &Model) -> ApplicationResult,
        rule_sets: &'a [(&'static str, u16)],
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

impl Display for Rule<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl PartialEq for Rule<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Rule<'_> {}

impl Hash for Rule<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
