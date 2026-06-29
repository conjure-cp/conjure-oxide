#![allow(clippy::arc_with_non_send_sync)]

use std::{collections::BTreeSet, fmt::Display};

use conjure_cp_core::ast::ReturnType;
use itertools::Itertools as _;
use parking_lot::RwLockReadGuard;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uniplate::{Biplate, Uniplate};

use super::{
    DeclarationPtr, Domain, DomainPtr, Expression, Model, Name, Range, SymbolTable, SymbolTablePtr,
    Typeable,
    ac_operators::ACOperatorKind,
    categories::{Category, CategoryOf},
    serde::{AsId, PtrAsInner},
};

#[serde_as]
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=Name)]
#[biplate(to=DeclarationPtr)]
pub enum ComprehensionQualifier {
    ExpressionGenerator {
        #[serde_as(as = "AsId")]
        ptr: DeclarationPtr,
    },
    Generator {
        #[serde_as(as = "AsId")]
        ptr: DeclarationPtr,
    },
    Condition(Expression),
}

/// A comprehension.
#[serde_as]
#[derive(Clone, PartialEq, Eq, Hash, Uniplate, Serialize, Deserialize, Debug)]
#[biplate(to=Expression)]
#[biplate(to=SymbolTable)]
#[biplate(to=SymbolTablePtr)]
#[non_exhaustive]
pub struct Comprehension {
    pub return_expression: Expression,
    pub qualifiers: Vec<ComprehensionQualifier>,
    /// When this comprehension appears inside an AC operator, records which operator so
    /// expansion can apply the correct skip semantics for symbolic guards.
    #[serde(default)]
    pub skip_operator: Option<ACOperatorKind>,
    #[doc(hidden)]
    #[serde_as(as = "PtrAsInner")]
    pub symbols: SymbolTablePtr,
}

impl Comprehension {
    pub fn domain_of(&self) -> Option<DomainPtr> {
        let return_expr_domain = self.return_expression.domain_of()?;

        // return a list (matrix with index domain int(1..)) of return_expr elements
        Some(Domain::matrix(
            return_expr_domain,
            vec![Domain::int(vec![Range::UnboundedR(1)])],
        ))
    }

    pub fn return_expression(self) -> Expression {
        self.return_expression
    }

    pub fn replace_return_expression(&mut self, new_expr: Expression) {
        self.return_expression = new_expr;
    }

    pub fn symbols(&self) -> RwLockReadGuard<'_, SymbolTable> {
        self.symbols.read()
    }

    pub fn quantified_vars(&self) -> Vec<Name> {
        self.qualifiers
            .iter()
            .filter_map(|q| match q {
                ComprehensionQualifier::ExpressionGenerator { ptr } => Some(ptr.name().clone()),
                ComprehensionQualifier::Generator { ptr } => Some(ptr.name().clone()),
                ComprehensionQualifier::Condition(_) => None,
            })
            .collect()
    }

    pub fn generator_conditions(&self) -> Vec<Expression> {
        self.qualifiers
            .iter()
            .filter_map(|q| match q {
                ComprehensionQualifier::Condition(c) => Some(c.clone()),
                ComprehensionQualifier::Generator { .. } => None,
                ComprehensionQualifier::ExpressionGenerator { .. } => None,
            })
            .collect()
    }

    /// Builds a temporary model containing generator qualifiers and guards.
    pub fn to_generator_model(&self) -> Model {
        let mut model = self.empty_model_with_symbols();
        model.add_constraints(self.generator_conditions());
        model
    }

    /// Builds a temporary model containing the return expression only.
    pub fn to_return_expression_model(&self) -> Model {
        let mut model = self.empty_model_with_symbols();
        model.add_constraint(self.return_expression.clone());
        model
    }

    fn empty_model_with_symbols(&self) -> Model {
        let parent = self.symbols.read().parent().clone();
        let mut model = if let Some(parent) = parent {
            Model::new_in_parent_scope(parent)
        } else {
            Model::default()
        };
        *model.symbols_ptr_unchecked_mut() = self.symbols.clone();
        model
    }

    /// Adds a guard to the comprehension.
    ///
    /// Returns false if the guard references non-quantified decision variables.
    pub fn add_quantified_guard(&mut self, guard: Expression) -> bool {
        if self.is_quantified_guard(&guard) {
            self.qualifiers
                .push(ComprehensionQualifier::Condition(guard));
            true
        } else {
            false
        }
    }

    /// True iff expr does not reference non-quantified decision variables.
    pub fn is_quantified_guard(&self, expr: &Expression) -> bool {
        let quantified: BTreeSet<Name> = self.quantified_vars().into_iter().collect();
        is_quantified_guard(&self.symbols.read(), &quantified, expr)
    }
}

impl Typeable for Comprehension {
    fn return_type(&self) -> ReturnType {
        self.return_expression.return_type()
    }
}

impl Display for Comprehension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let generators_and_guards = self
            .qualifiers
            .iter()
            .map(|qualifier| match qualifier {
                ComprehensionQualifier::Generator { ptr } => {
                    let domain = ptr.domain().expect("generator declaration has domain");
                    format!("{} : {domain}", ptr.name())
                }
                ComprehensionQualifier::ExpressionGenerator { ptr } => {
                    let name = ptr.name();
                    if let Some(expr) = ptr.as_quantified_expr() {
                        format!("{name} <- {expr}")
                    } else {
                        panic!("Oh nein! Dat is nicht gut!")
                    }
                }
                ComprehensionQualifier::Condition(expr) => format!("{expr}"),
            })
            .join(", ");

        write!(
            f,
            "[ {} | {generators_and_guards} ]",
            self.return_expression
        )
    }
}

/// A builder for a comprehension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComprehensionBuilder {
    qualifiers: Vec<ComprehensionQualifier>,
    // A single scope for generators and return expression.
    symbols: SymbolTablePtr,
    quantified_variables: BTreeSet<Name>,
}

impl ComprehensionBuilder {
    pub fn new(symbol_table_ptr: SymbolTablePtr) -> Self {
        ComprehensionBuilder {
            qualifiers: vec![],
            symbols: SymbolTablePtr::with_parent(symbol_table_ptr),
            quantified_variables: BTreeSet::new(),
        }
    }

    /// Backwards-compatible parser API: same table for generators and return expression.
    pub fn generator_symboltable(&mut self) -> SymbolTablePtr {
        self.symbols.clone()
    }

    /// Backwards-compatible parser API: same table for generators and return expression.
    pub fn return_expr_symboltable(&mut self) -> SymbolTablePtr {
        self.symbols.clone()
    }

    pub fn guard(mut self, guard: Expression) -> Self {
        self.qualifiers
            .push(ComprehensionQualifier::Condition(guard));
        self
    }

    pub fn generator(mut self, declaration: DeclarationPtr) -> Self {
        let name = declaration.name().clone();
        assert!(!self.quantified_variables.contains(&name));

        self.quantified_variables.insert(name.clone());

        // insert into comprehension scope as a local quantified variable
        let quantified_decl = DeclarationPtr::new_quantified(name, declaration.domain().unwrap());
        self.symbols.write().insert(quantified_decl.clone());

        self.qualifiers.push(ComprehensionQualifier::Generator {
            ptr: quantified_decl,
        });

        self
    }

    pub fn expression_generator(mut self, name: Name, expr: Expression) -> Self {
        assert!(!self.quantified_variables.contains(&name));

        self.quantified_variables.insert(name.clone());

        // insert into comprehension scope as a local quantified variable
        let quantified_decl = DeclarationPtr::new_quantified_expr(name, expr);
        self.symbols.write().insert(quantified_decl.clone());

        self.qualifiers
            .push(ComprehensionQualifier::ExpressionGenerator {
                ptr: quantified_decl,
            });

        self
    }

    /// Creates a comprehension with the given return expression.
    ///
    /// Guards are always stored as [`ComprehensionQualifier::Condition`] entries. When a guard
    /// references non-quantified decision variables, the enclosing AC operator applies the
    /// appropriate skip semantics during comprehension expansion.
    pub fn with_return_value(self, expression: Expression) -> Comprehension {
        Comprehension {
            return_expression: expression,
            qualifiers: self.qualifiers,
            skip_operator: None,
            symbols: self.symbols,
        }
    }
}

/// True iff the guard does not reference non-quantified decision variables.
fn is_quantified_guard(
    symbols: &SymbolTable,
    quantified_variables: &BTreeSet<Name>,
    guard: &Expression,
) -> bool {
    guard.universe_bi().iter().all(|name| {
        quantified_variables.contains(name)
            || symbols
                .lookup(name)
                .is_some_and(|decl| decl.category_of() != Category::Decision)
    })
}
