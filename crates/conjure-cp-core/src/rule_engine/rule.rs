use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use std::hash::Hash;

use thiserror::Error;

use crate::ast::{DeclarationPtr, Expression, Name, SubModel, SymbolTable};
use tree_morph::prelude::Commands;
use tree_morph::prelude::Rule as MorphRule;

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
/// - `new_top`: An additional top-level [`Vec<Expression>`] constraint that should be added to the model. If no top-level
///   constraint is needed, this field can be set to an empty vector [`Vec::new()`].
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
    pub new_top: Vec<Expression>,
    pub symbols: SymbolTable,
}

/// The result of applying a rule to an expression.
/// Contains either a set of reduction instructions or an error.
pub type ApplicationResult = Result<Reduction, ApplicationError>;

impl Reduction {
    pub fn new(new_expression: Expression, new_top: Vec<Expression>, symbols: SymbolTable) -> Self {
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
            new_top: Vec::new(),
            symbols: SymbolTable::new(),
        }
    }

    /// Represents a reduction that also modifies the symbol table.
    pub fn with_symbols(new_expression: Expression, symbols: SymbolTable) -> Self {
        Self {
            new_expression,
            new_top: Vec::new(),
            symbols,
        }
    }

    /// Represents a reduction that also adds a top-level constraint to the model.
    pub fn with_top(new_expression: Expression, new_top: Vec<Expression>) -> Self {
        Self {
            new_expression,
            new_top,
            symbols: SymbolTable::new(),
        }
    }

    /// Applies side-effects (e.g. symbol table updates)
    pub fn apply(self, model: &mut SubModel) {
        model.symbols_mut().extend(self.symbols); // Add new assignments to the symbol table
        model.add_constraints(self.new_top);
    }

    /// Gets symbols added by this reduction
    pub fn added_symbols(&self, initial_symbols: &SymbolTable) -> BTreeSet<Name> {
        let initial_symbols_set: BTreeSet<Name> = initial_symbols
            .clone()
            .into_iter_local()
            .map(|x| x.0)
            .collect();
        let new_symbols_set: BTreeSet<Name> = self
            .symbols
            .clone()
            .into_iter_local()
            .map(|x| x.0)
            .collect();

        new_symbols_set
            .difference(&initial_symbols_set)
            .cloned()
            .collect()
    }

    /// Gets symbols changed by this reduction
    ///
    /// Returns a list of tuples of (name, domain before reduction, domain after reduction)
    pub fn changed_symbols(
        &self,
        initial_symbols: &SymbolTable,
    ) -> Vec<(Name, DeclarationPtr, DeclarationPtr)> {
        let mut changes: Vec<(Name, DeclarationPtr, DeclarationPtr)> = vec![];

        for (var_name, initial_value) in initial_symbols.clone().into_iter_local() {
            let Some(new_value) = self.symbols.lookup(&var_name) else {
                continue;
            };

            if new_value != initial_value {
                changes.push((var_name.clone(), initial_value.clone(), new_value.clone()));
            }
        }
        changes
    }
}

/// The function type used in a [`Rule`].
pub type RuleFn = fn(&Expression, &SymbolTable) -> ApplicationResult;

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
    pub application: RuleFn,
    pub rule_sets: &'a [(&'a str, u16)], // (name, priority). At runtime, we add the rule to rulesets
}

impl<'a> Rule<'a> {
    pub const fn new(
        name: &'a str,
        application: RuleFn,
        rule_sets: &'a [(&'static str, u16)],
    ) -> Self {
        Self {
            name,
            application,
            rule_sets,
        }
    }

    pub fn apply(&self, expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
        (self.application)(expr, symbols)
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

impl MorphRule<Expression, SymbolTable> for Rule<'_> {
    fn apply(
        &self,
        commands: &mut Commands<Expression, SymbolTable>,
        subtree: &Expression,
        meta: &SymbolTable,
    ) -> Option<Expression> {
        let reduction = self.apply(subtree, meta).ok()?;
        commands.mut_meta(Box::new(|m: &mut SymbolTable| m.extend(reduction.symbols)));
        if !reduction.new_top.is_empty() {
            commands.transform(Box::new(|m| m.extend_root(reduction.new_top)));
        }
        Some(reduction.new_expression)
    }
}

impl MorphRule<Expression, SymbolTable> for &Rule<'_> {
    fn apply(
        &self,
        commands: &mut Commands<Expression, SymbolTable>,
        subtree: &Expression,
        meta: &SymbolTable,
    ) -> Option<Expression> {
        let reduction = Rule::apply(self, subtree, meta).ok()?;
        commands.mut_meta(Box::new(|m: &mut SymbolTable| m.extend(reduction.symbols)));
        if !reduction.new_top.is_empty() {
            commands.transform(Box::new(|m| m.extend_root(reduction.new_top)));
        }
        Some(reduction.new_expression)
    }
}
