use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt::{self, Debug, Display, Formatter};
use std::hash::Hash;
use std::rc::Rc;
use std::sync::Arc;

use thiserror::Error;

use crate::Model;
use crate::ast::{CnfClause, DeclarationPtr, Expression, Metadata, Name, SymbolTable};
use crate::rule_engine::RuleData;
use crate::rule_engine::rewriter_common::{RuleResult, log_rule_application};
use tree_morph::prelude::Commands;
use tree_morph::prelude::Rule as MorphRule;

#[derive(Clone, Debug, Default)]
pub(crate) struct MorphState {
    pub symbols: SymbolTable,
    pub clauses: Vec<CnfClause>,
}

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("Rule is not applicable")]
    RuleNotApplicable,

    #[error("Could not calculate the expression domain")]
    DomainError,
}

/// Represents the result of applying a rule to an expression within a model.
///
/// A `RuleEffect` encapsulates the changes made to a model during a rule application.
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
/// A `RuleEffect` can be created using one of the provided constructors:
/// - [`RuleEffect::new`]: Creates an effect with a new expression, top-level constraint, and symbol modifications.
/// - [`RuleEffect::pure`]: Creates an effect with only a new expression and no side-effects on the symbol table or constraints.
/// - [`RuleEffect::with_symbols`]: Creates an effect with a new expression and symbol table modifications, but no top-level constraint.
/// - [`RuleEffect::with_top`]: Creates an effect with a new expression and a top-level constraint, but no symbol table modifications.
/// - [`RuleEffect::cnf`]: Creates an effect with a new expression, cnf clauses and symbol modifications, but no top-level constraints.
///
/// The `apply` method allows for applying the changes represented by the `RuleEffect` to a [`Model`].
///
/// # Example
/// ```
/// // Need to add an example
/// ```
///
/// # See Also
/// - [`ApplicationResult`]: Represents the result of applying a rule, which may either be a `RuleEffect` or an `ApplicationError`.
/// - [`Model`]: The structure to which the `RuleEffect` changes are applied.
#[non_exhaustive]
#[derive(Clone)]
pub struct RuleEffect {
    pub new_expression: Expression,
    pub new_top: Vec<Expression>,
    pub symbols: SymbolTable,
    pub new_clauses: Vec<CnfClause>,
    materialise: Option<DeferredRuleEffect>,
}

/// Deferred constructor for a concrete rule effect.
type DeferredRuleEffect = Arc<dyn Fn(&SymbolTable) -> RuleEffect + Send + Sync>;

/// The result of applying a rule to an expression.
/// Contains either a set of rule effects or an error.
pub type ApplicationResult = Result<RuleEffect, ApplicationError>;

impl Debug for RuleEffect {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuleEffect")
            .field("new_expression", &self.new_expression)
            .field("new_top", &self.new_top)
            .field("symbols", &self.symbols)
            .field("new_clauses", &self.new_clauses)
            .field("is_deferred", &self.materialise.is_some())
            .finish()
    }
}

impl RuleEffect {
    pub fn new(new_expression: Expression, new_top: Vec<Expression>, symbols: SymbolTable) -> Self {
        Self {
            new_expression,
            new_top,
            symbols,
            new_clauses: Vec::new(),
            materialise: None,
        }
    }

    /// Represents an effect with no side effects on the model.
    pub fn pure(new_expression: Expression) -> Self {
        Self {
            new_expression,
            new_top: Vec::new(),
            symbols: SymbolTable::new(),
            new_clauses: Vec::new(),
            materialise: None,
        }
    }

    /// Represents an effect that also modifies the symbol table.
    pub fn with_symbols(new_expression: Expression, symbols: SymbolTable) -> Self {
        Self {
            new_expression,
            new_top: Vec::new(),
            symbols,
            new_clauses: Vec::new(),
            materialise: None,
        }
    }

    /// Represents an effect that also adds a top-level constraint to the model.
    pub fn with_top(new_expression: Expression, new_top: Vec<Expression>) -> Self {
        Self {
            new_expression,
            new_top,
            symbols: SymbolTable::new(),
            new_clauses: Vec::new(),
            materialise: None,
        }
    }

    /// Represents an effect that also adds clauses to the model.
    pub fn cnf(
        new_expression: Expression,
        new_clauses: Vec<CnfClause>,
        symbols: SymbolTable,
    ) -> Self {
        Self {
            new_expression,
            new_top: Vec::new(),
            symbols,
            new_clauses,
            materialise: None,
        }
    }

    /// Defers constructing a concrete effect until the rewriter chooses to apply this rule.
    ///
    /// This is intended for rule effects that allocate fresh names or otherwise depend on global
    /// model state. Applicability checks can return a deferred effect without consuming those
    /// effects; the rewriter calls [`RuleEffect::materialise`] only for the selected rule.
    pub fn deferred(
        materialise: impl Fn(&SymbolTable) -> RuleEffect + Send + Sync + 'static,
    ) -> Self {
        Self {
            new_expression: Expression::Root(Metadata::new(), Vec::new()),
            new_top: Vec::new(),
            symbols: SymbolTable::new(),
            new_clauses: Vec::new(),
            materialise: Some(Arc::new(materialise)),
        }
    }

    /// Returns the concrete effect for the current symbol table.
    pub fn materialise(&self, symbols: &SymbolTable) -> Self {
        let Some(materialise) = &self.materialise else {
            return self.clone();
        };

        materialise(symbols).materialise(symbols)
    }

    /// Applies side-effects (e.g. symbol table updates)
    pub fn apply(self, model: &mut Model) {
        debug_assert!(
            self.materialise.is_none(),
            "deferred rule effects must be materialised before being applied"
        );
        model.symbols_mut().extend(self.symbols); // Add new assignments to the symbol table
        model.add_constraints(self.new_top.clone());
        model.add_clauses(self.new_clauses);
    }

    /// Gets symbols added by this effect.
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

    /// Gets symbols changed by this effect.
    ///
    /// Returns a list of tuples of (name, domain before effect, domain after effect).
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
    /// Discriminant ids of Expression variants this rule applies to, or None for universal rules.
    pub applicable_to: Option<&'static [usize]>,
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
            applicable_to: None,
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

impl MorphRule<Expression, MorphState> for Rule<'_> {
    fn apply(
        &self,
        commands: &mut Commands<Expression, MorphState>,
        subtree: &Expression,
        meta: &MorphState,
    ) -> Option<Expression> {
        let effect = self
            .apply(subtree, &meta.symbols)
            .ok()?
            .materialise(&meta.symbols);
        let new_expression = effect.new_expression;
        let new_top = effect.new_top;
        let added_symbols = effect.symbols;
        let added_clauses = effect.new_clauses;
        commands.mut_meta(Box::new(move |m: &mut MorphState| {
            m.symbols.extend(added_symbols);
            m.clauses.extend(added_clauses);
        }));
        if !new_top.is_empty() {
            commands.transform(Box::new(move |m| m.extend_root(new_top)));
        }
        Some(new_expression)
    }

    fn name(&self) -> &str {
        self.name
    }

    fn applicable_to(&self) -> Option<Vec<usize>> {
        self.applicable_to.map(|s| s.to_vec())
    }
}

impl MorphRule<Expression, MorphState> for &Rule<'_> {
    fn apply(
        &self,
        commands: &mut Commands<Expression, MorphState>,
        subtree: &Expression,
        meta: &MorphState,
    ) -> Option<Expression> {
        let effect = Rule::apply(self, subtree, &meta.symbols)
            .ok()?
            .materialise(&meta.symbols);
        let new_expression = effect.new_expression;
        let new_top = effect.new_top;
        let added_symbols = effect.symbols;
        let added_clauses = effect.new_clauses;
        commands.mut_meta(Box::new(move |m: &mut MorphState| {
            m.symbols.extend(added_symbols);
            m.clauses.extend(added_clauses);
        }));
        if !new_top.is_empty() {
            commands.transform(Box::new(move |m| m.extend_root(new_top)));
        }
        Some(new_expression)
    }

    fn name(&self) -> &str {
        self.name
    }
}

impl MorphRule<Expression, Rc<RefCell<MorphState>>> for Rule<'_> {
    fn apply(
        &self,
        commands: &mut Commands<Expression, Rc<RefCell<MorphState>>>,
        subtree: &Expression,
        meta: &Rc<RefCell<MorphState>>,
    ) -> Option<Expression> {
        let state = meta.borrow();
        let effect = self
            .apply(subtree, &state.symbols)
            .ok()?
            .materialise(&state.symbols);
        let new_expression = effect.new_expression;
        let new_top = effect.new_top;
        let added_symbols = effect.symbols;
        let added_clauses = effect.new_clauses;
        commands.mut_meta(Box::new(move |m| {
            let mut state = m.borrow_mut();
            state.symbols.extend(added_symbols);
            state.clauses.extend(added_clauses);
        }));

        if !new_top.is_empty() {
            commands.transform(Box::new(move |m| m.extend_root(new_top)));
        }

        Some(new_expression)
    }

    fn name(&self) -> &str {
        self.name
    }
}

impl MorphRule<Expression, MorphState> for RuleData<'_> {
    fn apply(
        &self,
        commands: &mut Commands<Expression, MorphState>,
        subtree: &Expression,
        meta: &MorphState,
    ) -> Option<Expression> {
        let effect = self
            .rule
            .apply(subtree, &meta.symbols)
            .ok()?
            .materialise(&meta.symbols);
        let result = RuleResult {
            rule_data: self.clone(),
            effect: effect.clone(),
        };

        log_rule_application(&result, subtree, &meta.symbols, None);

        let new_expression = effect.new_expression;
        let new_top = effect.new_top;
        let added_symbols = effect.symbols;
        let added_clauses = effect.new_clauses;
        commands.mut_meta(Box::new(move |m: &mut MorphState| {
            m.symbols.extend(added_symbols);
            m.clauses.extend(added_clauses);
        }));

        if !new_top.is_empty() {
            commands.transform(Box::new(move |m| m.extend_root(new_top)));
        }
        Some(new_expression)
    }

    fn name(&self) -> &str {
        self.rule.name
    }

    fn applicable_to(&self) -> Option<Vec<usize>> {
        self.rule.applicable_to.map(|s| s.to_vec())
    }
}
