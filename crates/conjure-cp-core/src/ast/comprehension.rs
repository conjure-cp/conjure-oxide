#![allow(clippy::arc_with_non_send_sync)]

use std::{collections::BTreeSet, fmt::Display};

use crate::{ast::Metadata, into_matrix_expr, matrix_expr};
use conjure_cp_core::ast::ReturnType;
use itertools::Itertools as _;
use parking_lot::RwLockReadGuard;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uniplate::{Biplate, Uniplate};

use super::{
    DeclarationPtr, Domain, DomainPtr, Expression, Model, Moo, Name, Range, SymbolTable,
    SymbolTablePtr, Typeable, ac_operators::ACOperatorKind, serde::PtrAsInner,
};

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Debug, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=Name)]
pub enum ComprehensionQualifier {
    Generator { name: Name, domain: DomainPtr },
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
                ComprehensionQualifier::Generator { name, .. } => Some(name.clone()),
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

    /// Adds a guard to the comprehension. Returns false if the guard does not only reference quantified variables.
    pub fn add_quantified_guard(&mut self, guard: Expression) -> bool {
        if self.is_quantified_guard(&guard) {
            self.qualifiers
                .push(ComprehensionQualifier::Condition(guard));
            true
        } else {
            false
        }
    }

    /// True iff expr only references quantified variables.
    pub fn is_quantified_guard(&self, expr: &Expression) -> bool {
        let quantified: BTreeSet<Name> = self.quantified_vars().into_iter().collect();
        is_quantified_guard(&quantified, expr)
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
                ComprehensionQualifier::Generator { name, domain } => {
                    format!("{name} : {domain}")
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
        let domain = declaration.domain().unwrap();
        assert!(!self.quantified_variables.contains(&name));

        self.quantified_variables.insert(name.clone());

        // insert into comprehension scope as a local quantified variable
        let quantified_decl = DeclarationPtr::new_quantified(name.clone(), domain.clone());
        self.symbols.write().insert(quantified_decl);

        self.qualifiers
            .push(ComprehensionQualifier::Generator { name, domain });

        self
    }

    /// Creates a comprehension with the given return expression.
    ///
    /// If this comprehension is inside an AC-operator, the kind of this operator should be passed
    /// in the `comprehension_kind` field.
    ///
    /// If a comprehension kind is not given, comprehension guards containing decision variables
    /// are invalid, and will cause a panic.
    pub fn with_return_value(
        self,
        mut expression: Expression,
        comprehension_kind: Option<ACOperatorKind>,
    ) -> Comprehension {
        let quantified_variables = self.quantified_variables;

        let mut qualifiers = Vec::new();
        let mut other_guards = Vec::new();

        for qualifier in self.qualifiers {
            match qualifier {
                ComprehensionQualifier::Generator { .. } => qualifiers.push(qualifier),
                ComprehensionQualifier::Condition(condition) => {
                    if is_quantified_guard(&quantified_variables, &condition) {
                        qualifiers.push(ComprehensionQualifier::Condition(condition));
                    } else {
                        other_guards.push(condition);
                    }
                }
            }
        }

        // handle guards that reference non-quantified variables
        if !other_guards.is_empty() {
            let comprehension_kind = comprehension_kind.expect(
                "if any guards reference decision variables, a comprehension kind should be given",
            );

            let guard_expr = match other_guards.as_slice() {
                [x] => x.clone(),
                xs => Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(xs.to_vec()))),
            };

            expression = match comprehension_kind {
                ACOperatorKind::And => {
                    Expression::Imply(Metadata::new(), Moo::new(guard_expr), Moo::new(expression))
                }
                ACOperatorKind::Or => Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![guard_expr, expression]),
                ),

                ACOperatorKind::Sum => {
                    panic!("guards that reference decision variables not yet implemented for sum");
                }

                ACOperatorKind::Product => {
                    panic!(
                        "guards that reference decision variables not yet implemented for product"
                    );
                }
            }
        }

        Comprehension {
            return_expression: expression,
            qualifiers,
            symbols: self.symbols,
        }
    }
}

/// True iff the guard only references quantified variables.
fn is_quantified_guard(quantified_variables: &BTreeSet<Name>, guard: &Expression) -> bool {
    guard
        .universe_bi()
        .iter()
        .all(|x| quantified_variables.contains(x))
}
